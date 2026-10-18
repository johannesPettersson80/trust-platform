//! How a search keeps what its protocol sources report (specification 11, "Devices" and
//! "Confidence"): each candidate becomes a result of the device it names, in the closed
//! confidence vocabulary, and each warning names the target it is about.

use std::net::{IpAddr, SocketAddr};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::connectors::DiscoveryConfidence;

use super::super::model::{DeviceKey, Identity, ProtocolFind, SourceId};
use super::super::runner::SourceSink;
use super::report::{millis, CandidateSink, Deadline, DiscoverCandidate};

/// The sink of one protocol search inside a source of a search: candidates become results with
/// `to_find`, and warnings name `target`.
pub(super) struct FindSink<'a, F> {
    sink: &'a SourceSink,
    target: Option<&'a str>,
    to_find: F,
}

impl<'a, F> FindSink<'a, F>
where
    F: Fn(DiscoverCandidate) -> ProtocolFind,
{
    pub(super) fn new(sink: &'a SourceSink, target: Option<&'a str>, to_find: F) -> Self {
        Self {
            sink,
            target,
            to_find,
        }
    }

    /// The deadline of the protocol search: the end of the search, as it stands now.
    pub(super) fn deadline(&self) -> Deadline {
        Deadline::at(Instant::now() + self.sink.remaining())
    }
}

impl<F> CandidateSink for FindSink<'_, F>
where
    F: Fn(DiscoverCandidate) -> ProtocolFind,
{
    fn found(&mut self, candidate: DiscoverCandidate) {
        self.sink.found((self.to_find)(candidate));
    }

    fn warn(&mut self, warning: String) {
        self.sink.warn(match self.target {
            Some(target) => format!("{target}: {warning}"),
            None => warning,
        });
    }
}

/// The confidence of a `comm.discover` candidate. Its word `observed` names an ADS identify reply,
/// an OPC UA GetEndpoints answer, an EtherCAT read, a GPIO listing or an mDNS answer, each of
/// which is `confirmed` (specification 11, "Confidence"); any word outside the vocabulary is no
/// evidence.
pub(super) fn confidence(word: &str) -> DiscoveryConfidence {
    match word {
        "confirmed" | "observed" => DiscoveryConfidence::Confirmed,
        "likely" => DiscoveryConfidence::Likely,
        "port_reachable" => DiscoveryConfidence::PortReachable,
        _ => DiscoveryConfidence::Unavailable,
    }
}

/// The host of a known address without its port: `10.0.0.9:502` is `10.0.0.9`, `[fe80::1]:502`
/// is `fe80::1`, and a bare IPv6 address stays whole.
pub(super) fn host_part(authority: &str) -> &str {
    if let Some(bracketed) = authority.strip_prefix('[') {
        return bracketed.split(']').next().unwrap_or(bracketed);
    }
    match authority.split_once(':') {
        Some((host, port)) if !port.contains(':') => host,
        _ => authority,
    }
}

/// The device address of a host, an IP address or a socket address: the IP address, or the host
/// name in lower case.
pub(super) fn address_key(authority: &str) -> Option<DeviceKey> {
    let authority = authority.trim();
    if let Ok(socket) = authority.parse::<SocketAddr>() {
        return Some(DeviceKey::Address(socket.ip().to_string()));
    }
    let host = host_part(authority);
    if host.is_empty() {
        return None;
    }
    let address = host
        .parse::<IpAddr>()
        .map_or_else(|_| host.to_ascii_lowercase(), |ip| ip.to_string());
    Some(DeviceKey::Address(address))
}

/// The result of `protocol` that `candidate` names, seen now.
pub(super) fn find(
    protocol: SourceId,
    candidate: DiscoverCandidate,
    identity: Identity,
) -> ProtocolFind {
    ProtocolFind::new(
        protocol,
        candidate.source,
        confidence(candidate.confidence),
        identity,
        candidate.params,
        now_ms(),
    )
    .with_warnings(candidate.warnings)
}

/// A non-empty text parameter.
pub(super) fn text(params: &Value, key: &str) -> Option<String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// A whole-number parameter, zero when it is absent or out of range.
pub(super) fn number<T: TryFrom<u64> + Default>(params: &Value, key: &str) -> T {
    params
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| T::try_from(value).ok())
        .unwrap_or_default()
}

pub(super) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, millis)
}
