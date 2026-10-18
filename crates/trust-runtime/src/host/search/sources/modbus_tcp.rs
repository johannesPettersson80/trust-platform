//! Modbus TCP (specification 11, "comm.discover" and "Sources"). A probe asks FC43/14 device
//! identification and, when the scope names one, a safe FC03 read: an answer proves Modbus, and a
//! TCP connection without one only the port. `discover` probes a known address or the hosts of a
//! subnet for `comm.discover`, or one target of a search; `search` probes a search's targets many
//! at a time.

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::{json, Value};

use super::super::model::{Identity, ModbusIdentity, ProtocolFind, SourceId};
use super::super::runner::SourceSink;
use super::find::{address_key, find, text, FindSink};
use super::probe::{connect, DiscoveryProbeOutcome};
use super::report::{millis, CandidateSink, Deadline, DiscoverCandidate};
use super::scope::{ipv4_targets_for_cidr, socket_target, timeout, DiscoverScope};

const MODBUS_PORT: u16 = 502;
/// How many Modbus TCP hosts a search probes at the same time.
const MODBUS_PARALLEL_PROBES: usize = 32;

/// One host that a search's Modbus TCP source probes at one unit ID, and how long the probe waits.
#[derive(Debug, Clone)]
pub(crate) struct ModbusTarget {
    pub(crate) host: String,
    pub(crate) unit_id: u8,
    pub(crate) timeout: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ModbusDiscoveryProbe {
    pub unit_id: u8,
    pub safe_read: Option<ModbusSafeReadProbe>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ModbusSafeReadProbe {
    pub address: u16,
    pub quantity: u16,
}

/// Probes the known address of `scope`, or the hosts of its subnet (without one, the `/24` of the
/// first private IPv4 interface), one after another until `deadline`.
pub(crate) fn discover(
    scope: &DiscoverScope,
    deadline: Deadline,
    sink: &mut dyn CandidateSink,
) -> Result<(), String> {
    if deadline.passed() {
        return Ok(());
    }
    let timeout = timeout(scope);
    let targets = if let Some(host) = scope.host.as_deref().filter(|host| !host.trim().is_empty()) {
        vec![modbus_socket_target(host.trim())?]
    } else {
        let cidr = scope
            .cidr
            .clone()
            .or_else(default_ipv4_cidr)
            .ok_or_else(|| {
                "Modbus discovery needs --cidr or a private IPv4 interface to derive a /24 scan"
                    .to_string()
            })?;
        ipv4_targets_for_cidr(cidr.as_str())?
            .into_iter()
            .map(|ip| SocketAddr::new(IpAddr::V4(ip), MODBUS_PORT))
            .collect()
    };

    if targets.len() > 256 {
        return Err(
            "Modbus discovery is capped at 256 hosts; use a /24 or tighter CIDR".to_string(),
        );
    }
    if targets.len() > 32 {
        sink.warn(format!(
            "Scanning {} hosts with Modbus protocol probes; this may take a moment.",
            targets.len()
        ));
    }

    let probe = modbus_discovery_probe(scope)?;
    probe_targets_in_parallel(&targets, probe, timeout, deadline, sink);
    Ok(())
}

/// Probes every target, many at the same time, and reports the results in target order, so that a
/// whole subnet is scanned within the deadline rather than one host after another.
fn probe_targets_in_parallel(
    targets: &[SocketAddr],
    probe: ModbusDiscoveryProbe,
    timeout: Duration,
    deadline: Deadline,
    sink: &mut dyn CandidateSink,
) {
    let results: Mutex<Vec<Option<ModbusProbeOutcome>>> =
        Mutex::new((0..targets.len()).map(|_| None).collect());
    let next = AtomicUsize::new(0);
    std::thread::scope(|threads| {
        let workers = MODBUS_PARALLEL_PROBES.min(targets.len()).max(1) - 1;
        for _ in 0..workers {
            let spawned = std::thread::Builder::new()
                .name("discover-modbus".to_string())
                .spawn_scoped(threads, || {
                    probe_next_target(targets, probe, timeout, deadline, &next, &results);
                });
            if spawned.is_err() {
                break;
            }
        }
        probe_next_target(targets, probe, timeout, deadline, &next, &results);
    });
    let Ok(results) = results.into_inner() else {
        return;
    };
    for (target, outcome) in targets.iter().zip(results) {
        let Some(probed) = outcome else {
            continue;
        };
        report_modbus_candidate(*target, &probe, probed, sink);
    }
}

/// Probes the next unclaimed target until none is left or the deadline passes.
fn probe_next_target(
    targets: &[SocketAddr],
    probe: ModbusDiscoveryProbe,
    timeout: Duration,
    deadline: Deadline,
    next: &AtomicUsize,
    results: &Mutex<Vec<Option<ModbusProbeOutcome>>>,
) {
    while !deadline.passed() {
        let index = next.fetch_add(1, Ordering::Relaxed);
        let Some(&target) = targets.get(index) else {
            return;
        };
        let outcome = probe_modbus_tcp(target, deadline.bound(timeout), probe);
        if let Ok(mut results) = results.lock() {
            results[index] = outcome;
        }
    }
}

/// Reports one answered target, with what the FC43/14 answer read about the device.
fn report_modbus_candidate(
    target: SocketAddr,
    probe: &ModbusDiscoveryProbe,
    probed: ModbusProbeOutcome,
    sink: &mut dyn CandidateSink,
) {
    let address = target.to_string();
    let mut params = json!({
        "address": address,
        "unit_id": probe.unit_id,
        "probe_source": probed.outcome.source,
        "probe_detail": probed.outcome.detail,
    });
    if let Some(safe_read) = probe.safe_read {
        params["probe_read_address"] = json!(safe_read.address);
        params["probe_read_quantity"] = json!(safe_read.quantity);
    }
    if let Some(identification) = probed.identification {
        if let Some(vendor_name) = identification.vendor_name {
            params["vendor_name"] = json!(vendor_name);
        }
        if let Some(product_code) = identification.product_code {
            params["product_code"] = json!(product_code);
        }
        if let Some(revision) = identification.revision {
            params["revision"] = json!(revision);
        }
    }
    sink.found(DiscoverCandidate {
        id: format!("modbus:{address}"),
        label: address.clone(),
        source: probed.outcome.source,
        confidence: probed.outcome.confidence,
        params,
        warnings: probed.outcome.warnings,
    });
}

fn modbus_discovery_probe(scope: &DiscoverScope) -> Result<ModbusDiscoveryProbe, String> {
    let safe_read = match (scope.probe_read_address, scope.probe_read_quantity) {
        (Some(address), quantity) => {
            let quantity = quantity.unwrap_or(1);
            if quantity == 0 || quantity > 125 {
                return Err(
                    "Modbus discovery scope.probe_read_quantity must be between 1 and 125"
                        .to_string(),
                );
            }
            Some(ModbusSafeReadProbe { address, quantity })
        }
        (None, Some(_)) => {
            return Err(
                "Modbus discovery scope.probe_read_address is required when scope.probe_read_quantity is set"
                    .to_string(),
            );
        }
        (None, None) => None,
    };
    Ok(ModbusDiscoveryProbe {
        unit_id: scope.unit_id.unwrap_or(1),
        safe_read,
    })
}

fn modbus_socket_target(host: &str) -> Result<SocketAddr, String> {
    socket_target(host, MODBUS_PORT, "Modbus")
}

fn default_ipv4_cidr() -> Option<String> {
    let candidates = crate::ads::onboarding::runtime_address_candidates_from_interfaces().ok()?;
    let ip = candidates.into_iter().find_map(|candidate| {
        let ip = candidate.ip.parse::<Ipv4Addr>().ok()?;
        if ip.is_loopback() || ip.is_link_local() || !ip.is_private() {
            return None;
        }
        Some(ip)
    })?;
    let [a, b, c, _] = ip.octets();
    Some(format!("{a}.{b}.{c}.0/24"))
}

/// The answer of one Modbus TCP probe: what it proves, and what the FC43/14 answer read about the
/// device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ModbusProbeOutcome {
    pub(super) outcome: DiscoveryProbeOutcome,
    pub(super) identification: Option<ModbusBasicObjects>,
}

/// The basic objects of an FC43/14 Read Device Identification answer.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct ModbusBasicObjects {
    pub(super) vendor_name: Option<String>,
    pub(super) product_code: Option<String>,
    pub(super) revision: Option<String>,
}

pub(super) fn probe_modbus_tcp(
    target: SocketAddr,
    timeout: Duration,
    probe: ModbusDiscoveryProbe,
) -> Option<ModbusProbeOutcome> {
    let mut stream = connect(target, timeout)?;
    match send_modbus_request(&mut stream, probe.unit_id, &[0x2b, 0x0e, 0x01, 0x00], 1) {
        Ok(ModbusProbeResponse::Normal(pdu)) => {
            return Some(ModbusProbeOutcome {
                outcome: DiscoveryProbeOutcome::confirmed(
                    "modbus_device_id",
                    "Modbus FC43/14 device identification responded.",
                ),
                identification: decode_fc43_basic_objects(&pdu),
            });
        }
        Ok(ModbusProbeResponse::Exception(code)) => {
            return Some(ModbusProbeOutcome {
                outcome: DiscoveryProbeOutcome::confirmed(
                    "modbus_device_id_exception",
                    format!("Modbus FC43/14 returned exception code {code}."),
                ),
                identification: None,
            });
        }
        Err(_) => {}
    }

    if let Some(safe_read) = probe.safe_read {
        let mut stream = match connect(target, timeout) {
            Some(stream) => stream,
            None => {
                return Some(ModbusProbeOutcome {
                    outcome: DiscoveryProbeOutcome::port_reachable(
                        "tcp_connect",
                        "TCP connection succeeded, but the safe Modbus read probe could not reconnect.",
                    ),
                    identification: None,
                });
            }
        };
        let pdu = [
            0x03,
            (safe_read.address >> 8) as u8,
            safe_read.address as u8,
            (safe_read.quantity >> 8) as u8,
            safe_read.quantity as u8,
        ];
        return Some(ModbusProbeOutcome {
            outcome: match send_modbus_request(&mut stream, probe.unit_id, &pdu, 2) {
                Ok(ModbusProbeResponse::Normal(_)) => DiscoveryProbeOutcome::confirmed(
                    "modbus_safe_read",
                    "Configured Modbus FC03 safe read responded.",
                ),
                Ok(ModbusProbeResponse::Exception(code)) => DiscoveryProbeOutcome::confirmed(
                    "modbus_safe_read_exception",
                    format!("Configured Modbus FC03 safe read returned exception code {code}."),
                ),
                Err(_) => DiscoveryProbeOutcome::port_reachable(
                    "tcp_connect",
                    "TCP connection succeeded, but Modbus protocol probes did not complete.",
                )
                .with_warning(
                    "Only TCP reachability was observed; no Modbus response was received.",
                ),
            },
            identification: None,
        });
    }

    Some(ModbusProbeOutcome {
        outcome: DiscoveryProbeOutcome::port_reachable(
            "tcp_connect",
            "TCP connection succeeded, but Modbus FC43/14 did not respond.",
        )
        .with_warning(
            "Only TCP reachability was observed; configure a safe read probe to prove Modbus on devices without FC43/14.",
        ),
        identification: None,
    })
}

/// The basic objects (vendor name, product code and revision) of an FC43/14 Read Device
/// Identification answer; `None` when the answer is not one or names none of them.
fn decode_fc43_basic_objects(pdu: &[u8]) -> Option<ModbusBasicObjects> {
    if pdu.len() < 7 || pdu[0] != 0x2b || pdu[1] != 0x0e {
        return None;
    }
    let mut objects = ModbusBasicObjects::default();
    let mut cursor = 7;
    for _ in 0..usize::from(pdu[6]) {
        let object_id = *pdu.get(cursor)?;
        let length = usize::from(*pdu.get(cursor + 1)?);
        let value = pdu.get(cursor + 2..cursor + 2 + length)?;
        cursor += 2 + length;
        let text = String::from_utf8_lossy(value).into_owned();
        match object_id {
            0x00 => objects.vendor_name = Some(text),
            0x01 => objects.product_code = Some(text),
            0x02 => objects.revision = Some(text),
            _ => {}
        }
    }
    if objects.vendor_name.is_none() && objects.product_code.is_none() && objects.revision.is_none()
    {
        return None;
    }
    Some(objects)
}

enum ModbusProbeResponse {
    Normal(Vec<u8>),
    Exception(u8),
}

fn send_modbus_request(
    stream: &mut TcpStream,
    unit_id: u8,
    pdu: &[u8],
    transaction_id: u16,
) -> Result<ModbusProbeResponse, std::io::Error> {
    if pdu.is_empty() || pdu.len() > 253 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Modbus PDU length must be in 1..=253 bytes",
        ));
    }
    let mut header = [0u8; 6];
    header[0..2].copy_from_slice(&transaction_id.to_be_bytes());
    header[2..4].copy_from_slice(&0u16.to_be_bytes());
    header[4..6].copy_from_slice(&((pdu.len() + 1) as u16).to_be_bytes());
    stream.write_all(&header)?;
    stream.write_all(&[unit_id])?;
    stream.write_all(pdu)?;
    stream.flush()?;

    let mut response_header = [0u8; 6];
    stream.read_exact(&mut response_header)?;
    if u16::from_be_bytes([response_header[0], response_header[1]]) != transaction_id
        || u16::from_be_bytes([response_header[2], response_header[3]]) != 0
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid Modbus MBAP header",
        ));
    }
    let response_len = u16::from_be_bytes([response_header[4], response_header[5]]) as usize;
    if !(2..=260).contains(&response_len) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid Modbus response length",
        ));
    }
    let mut response = vec![0u8; response_len];
    stream.read_exact(&mut response)?;
    if response.first().copied() != Some(unit_id) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Modbus response unit id does not match request",
        ));
    }
    let response_pdu = response.get(1..).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "missing Modbus PDU")
    })?;
    let Some(function) = response_pdu.first().copied() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "empty Modbus PDU",
        ));
    };
    if function == pdu[0] {
        return Ok(ModbusProbeResponse::Normal(response_pdu.to_vec()));
    }
    if function == (pdu[0] | 0x80) {
        let code = response_pdu.get(1).copied().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "missing Modbus exception code",
            )
        })?;
        return Ok(ModbusProbeResponse::Exception(code));
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "unexpected Modbus function code",
    ))
}

/// Modbus TCP in a search: every target, many at the same time, each probe until its timeout or
/// the end of the search.
pub(super) fn search(targets: &[ModbusTarget], sink: &SourceSink) {
    let next = AtomicUsize::new(0);
    let probe_targets = || {
        while !sink.ended() {
            let Some(target) = targets.get(next.fetch_add(1, Ordering::Relaxed)) else {
                return;
            };
            let scope = DiscoverScope {
                host: Some(target.host.clone()),
                timeout_ms: Some(millis(target.timeout)),
                unit_id: Some(target.unit_id),
                ..DiscoverScope::default()
            };
            let mut found = FindSink::new(sink, Some(target.host.as_str()), modbus_find);
            if let Err(error) = discover(&scope, found.deadline(), &mut found) {
                sink.warn(format!("{}: {error}", target.host));
            }
        }
    };
    std::thread::scope(|threads| {
        for _ in 1..MODBUS_PARALLEL_PROBES.min(targets.len()) {
            let spawned = std::thread::Builder::new()
                .name("search-modbus".to_string())
                .spawn_scoped(threads, probe_targets);
            if spawned.is_err() {
                break;
            }
        }
        probe_targets();
    });
}

fn modbus_find(candidate: DiscoverCandidate) -> ProtocolFind {
    let address = text(&candidate.params, "address").unwrap_or_default();
    let unit_id = candidate
        .params
        .get("unit_id")
        .and_then(Value::as_u64)
        .and_then(|unit_id| u8::try_from(unit_id).ok())
        .unwrap_or(1);
    let identity = Identity::Modbus(ModbusIdentity {
        unit_id,
        vendor_name: text(&candidate.params, "vendor_name"),
        product_code: text(&candidate.params, "product_code"),
        revision: text(&candidate.params, "revision"),
    });
    let keys = address_key(&address).into_iter().collect();
    find(SourceId::ModbusTcp, candidate, identity).keyed(format!("{address}/{unit_id}"), keys)
}

#[cfg(test)]
#[path = "modbus_tcp_tests.rs"]
mod tests;
