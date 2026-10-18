//! MQTT (specification 11, "comm.discover" and "Sources"). A probe sends a clean-session CONNECT
//! and reads the CONNACK: an accepted session proves MQTT, a refused one is likely MQTT, and the
//! TLS port or a broker without an answer only proves the port. Brokers are probed at known
//! addresses and at the ones that announce `_mqtt._tcp.local.` over mDNS: `discover` probes one
//! for `comm.discover` or for a search, and `search` probes each known address of a search and
//! then listens to the announcements until the search ends.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::connectors::DiscoveryConfidence;

use super::super::model::{Identity, MqttIdentity, ProtocolFind, SourceId};
use super::super::runner::SourceSink;
use super::find::{address_key, confidence, find, text, FindSink};
use super::mdns_services::{self, AnnouncedProtocol};
use super::probe::{connect, DiscoveryProbeOutcome};
use super::report::{millis, sanitize_id, CandidateSink, Deadline, DiscoverCandidate};
use super::scope::{authority_host_and_port, resolve_authority_all, timeout, DiscoverScope};
use super::SourceContext;

const MQTT_PORT: u16 = 1883;
const MQTTS_PORT: u16 = 8883;
/// A search's probe of a known broker waits this long for an answer.
const MQTT_PROBE: Duration = Duration::from_millis(1_000);

/// Probes the broker at the known address of `scope`: its port, or 1883 and then 8883.
pub(crate) fn discover(
    scope: &DiscoverScope,
    deadline: Deadline,
    sink: &mut dyn CandidateSink,
) -> Result<(), String> {
    if deadline.passed() {
        return Ok(());
    }
    let mut warnings = Vec::new();
    let targets = mqtt_targets(scope, &mut warnings);
    for warning in warnings {
        sink.warn(warning);
    }
    let Some(targets) = targets? else {
        return Ok(());
    };
    let timeout = timeout(scope);
    let mut accepted = false;
    for target in targets {
        if deadline.passed() {
            break;
        }
        if let Some(outcome) = probe_mqtt(target, deadline.bound(timeout)) {
            sink.found(mqtt_candidate(target, outcome));
            accepted = true;
        }
    }
    if !accepted {
        sink.warn("No MQTT broker accepted a TCP connection at the requested target.".to_string());
    }
    Ok(())
}

/// The candidate of the broker at `target` whose probe answered `outcome`.
pub(super) fn mqtt_candidate(
    target: SocketAddr,
    outcome: DiscoveryProbeOutcome,
) -> DiscoverCandidate {
    let broker = target.to_string();
    DiscoverCandidate {
        id: format!("mqtt:{}", sanitize_id(broker.as_str())),
        label: format!("MQTT broker {broker}"),
        source: outcome.source,
        confidence: outcome.confidence,
        params: json!({
            "broker": broker,
            "tls": target.port() == MQTTS_PORT,
            "allow_insecure_remote": target.port() != MQTTS_PORT && !target.ip().is_loopback(),
            "auth_required": outcome.auth_required,
            "probe_detail": outcome.detail,
        }),
        warnings: outcome.warnings,
    }
}

fn mqtt_targets(
    scope: &DiscoverScope,
    warnings: &mut Vec<String>,
) -> Result<Option<Vec<SocketAddr>>, String> {
    let Some(host) = scope
        .host
        .as_deref()
        .map(str::trim)
        .filter(|host| !host.is_empty())
    else {
        warnings.push(
            "MQTT discovery needs a broker host; broad network scans are not run for MQTT."
                .to_string(),
        );
        return Ok(None);
    };
    let (host, explicit_port) = authority_host_and_port(host, "MQTT")?;
    let ports: Vec<u16> =
        explicit_port.map_or_else(|| vec![MQTT_PORT, MQTTS_PORT], |port| vec![port]);
    // Every distinct resolved address is probed, at 1883 and then 8883 when the target names no
    // port (specification 32, §10).
    let resolved = resolve_authority_all(host.as_str(), ports[0], "MQTT")?;
    if resolved.is_empty() {
        return Err(format!("resolve MQTT target '{host}': no address found"));
    }
    let mut addrs = Vec::new();
    for address in resolved {
        for &port in &ports {
            let target = SocketAddr::new(address.ip(), port);
            if !addrs.contains(&target) {
                addrs.push(target);
            }
        }
    }
    Ok(Some(addrs))
}

pub(super) fn probe_mqtt(target: SocketAddr, timeout: Duration) -> Option<DiscoveryProbeOutcome> {
    let mut stream = connect(target, timeout)?;
    if target.port() == MQTTS_PORT {
        return Some(probe_mqtts_tls(target, stream));
    }

    let client_id = format!("trust-discovery-{}", std::process::id());
    if write_mqtt_connect(&mut stream, client_id.as_bytes()).is_err() {
        return Some(DiscoveryProbeOutcome::port_reachable(
            "tcp_connect",
            "TCP connection succeeded, but MQTT CONNECT could not be written.",
        ));
    }
    match read_mqtt_connack(&mut stream) {
        Ok(0) => {
            let _ = stream.write_all(&[0xe0, 0x00]);
            Some(DiscoveryProbeOutcome::confirmed(
                "mqtt_connack",
                "MQTT CONNECT was accepted with clean_session=true.",
            ))
        }
        Ok(code @ (4 | 5)) => {
            let _ = stream.write_all(&[0xe0, 0x00]);
            Some(
                DiscoveryProbeOutcome::likely(
                    "mqtt_connack_auth_rejected",
                    format!("MQTT CONNACK rejected anonymous discovery with code {code}."),
                )
                .with_auth_required()
                .with_warning(
                    "MQTT broker requires credentials; discovery identified MQTT-shaped evidence without establishing a session.",
                ),
            )
        }
        Ok(code) => {
            let _ = stream.write_all(&[0xe0, 0x00]);
            Some(DiscoveryProbeOutcome::likely(
                "mqtt_connack_rejected",
                format!("MQTT CONNACK returned code {code}."),
            ))
        }
        Err(_) => Some(
            DiscoveryProbeOutcome::port_reachable(
                "tcp_connect",
                "TCP connection succeeded, but MQTT CONNACK was not received.",
            )
            .with_warning("Only TCP reachability was observed; no MQTT CONNACK was received."),
        ),
    }
}

/// An MQTTS port: a TLS handshake proves reachability, never an MQTT broker, because no MQTT
/// exchange follows (specification 32, §10).
fn probe_mqtts_tls(target: SocketAddr, stream: TcpStream) -> DiscoveryProbeOutcome {
    let connector = native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build();
    let Ok(connector) = connector else {
        return DiscoveryProbeOutcome::port_reachable(
            "tcp_connect",
            "MQTTS TCP port is reachable; the TLS client could not be built.",
        )
        .with_warning(
            "Only TCP reachability was observed for MQTTS; the TLS handshake could not start.",
        );
    };
    match connector.connect(target.ip().to_string().as_str(), stream) {
        Ok(_) => DiscoveryProbeOutcome::port_reachable(
            "tls_handshake",
            "MQTTS TLS handshake completed; no MQTT exchange followed.",
        )
        .with_warning(
            "The MQTTS TLS handshake succeeded, but the broker is not identified until an MQTT exchange succeeds.",
        ),
        Err(_) => DiscoveryProbeOutcome::port_reachable(
            "tcp_connect",
            "MQTTS TCP port is reachable; the TLS handshake failed.",
        )
        .with_warning("Only TCP reachability was observed for MQTTS; the TLS handshake failed."),
    }
}

fn write_mqtt_connect(stream: &mut TcpStream, client_id: &[u8]) -> Result<(), std::io::Error> {
    if client_id.len() > usize::from(u16::MAX) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "MQTT client id exceeds the u16 wire length",
        ));
    }
    let mut body = Vec::new();
    body.extend_from_slice(&[0x00, 0x04, b'M', b'Q', b'T', b'T']);
    body.push(0x04);
    body.push(0x02);
    body.extend_from_slice(&0u16.to_be_bytes());
    body.extend_from_slice(&(client_id.len() as u16).to_be_bytes());
    body.extend_from_slice(client_id);

    let mut packet = vec![0x10];
    encode_remaining_length(body.len(), &mut packet);
    if packet.len() == 1 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "MQTT packet exceeds maximum remaining length",
        ));
    }
    packet.extend_from_slice(&body);
    stream.write_all(&packet)?;
    stream.flush()
}

fn read_mqtt_connack(stream: &mut TcpStream) -> Result<u8, std::io::Error> {
    let mut fixed = [0u8; 1];
    stream.read_exact(&mut fixed)?;
    if fixed[0] != 0x20 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "expected MQTT CONNACK",
        ));
    }
    let remaining = read_remaining_length(stream)?;
    if remaining != 2 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "MQTT CONNACK remaining length must be exactly two",
        ));
    }
    let mut body = vec![0u8; remaining];
    stream.read_exact(&mut body)?;
    let flags = body[0];
    let code = body[1];
    if flags & !0x01 != 0 || code > 5 || (code != 0 && flags & 0x01 != 0) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid MQTT CONNACK flags or return code",
        ));
    }
    Ok(code)
}

fn encode_remaining_length(mut len: usize, packet: &mut Vec<u8>) {
    if len > 268_435_455 {
        return;
    }
    loop {
        let mut byte = (len % 128) as u8;
        len /= 128;
        if len > 0 {
            byte |= 0x80;
        }
        packet.push(byte);
        if len == 0 {
            break;
        }
    }
}

fn read_remaining_length(stream: &mut TcpStream) -> Result<usize, std::io::Error> {
    let mut multiplier = 1usize;
    let mut value = 0usize;
    for byte_index in 0..4 {
        let mut encoded = [0u8; 1];
        stream.read_exact(&mut encoded)?;
        value += usize::from(encoded[0] & 127) * multiplier;
        if encoded[0] & 128 == 0 {
            if byte_index > 0 && encoded[0] == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "non-minimal MQTT remaining length",
                ));
            }
            return Ok(value);
        }
        multiplier *= 128;
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "MQTT remaining length too long",
    ))
}

/// MQTT in a search: each known address, and then the brokers that announce `_mqtt._tcp.local.`
/// until the search ends (specification 11, "Sources").
pub(super) fn search(context: &SourceContext, sink: &SourceSink) {
    for host in &context.hosts {
        if sink.ended() {
            return;
        }
        let scope = DiscoverScope {
            host: Some(host.clone()),
            timeout_ms: Some(millis(MQTT_PROBE)),
            ..DiscoverScope::default()
        };
        let mut found = FindSink::new(sink, Some(host.as_str()), mqtt_find);
        if let Err(error) = discover(&scope, found.deadline(), &mut found) {
            sink.warn(format!("{host}: {error}"));
        }
    }
    mdns_services::search(AnnouncedProtocol::Mqtt, sink);
}

/// How long one announced broker's probe may wait: the search's own probe of a broker, cut to the
/// deadline of the search.
pub(super) fn mqtt_probe_timeout(sink: &SourceSink) -> Duration {
    Deadline::at(Instant::now() + sink.remaining()).bound(MQTT_PROBE)
}

pub(super) fn mqtt_find(candidate: DiscoverCandidate) -> ProtocolFind {
    let broker = text(&candidate.params, "broker").unwrap_or_default();
    let tls = candidate
        .params
        .get("tls")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let auth_required = candidate
        .params
        .get("auth_required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    // A rejected login proves that one is needed; an accepted session proves that none is.
    let login_required = if auth_required {
        Some(true)
    } else if confidence(candidate.confidence) == DiscoveryConfidence::Confirmed {
        Some(false)
    } else {
        None
    };
    let keys = address_key(&broker).into_iter().collect();
    let identity = Identity::Mqtt(MqttIdentity {
        tls,
        login_required,
    });
    find(SourceId::Mqtt, candidate, identity).keyed(broker, keys)
}

#[cfg(test)]
#[path = "mqtt_tests.rs"]
mod tests;
