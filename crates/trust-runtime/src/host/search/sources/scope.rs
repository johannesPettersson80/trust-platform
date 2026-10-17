//! What one protocol search probes (specification 11, "comm.discover"): the `scope` of a
//! `comm.discover` request, how long one probe waits, and the sockets that a known address or an
//! IPv4 subnet names. A search hands its known addresses and subnets to the same rules.

use std::net::{Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::time::Duration;

use serde::Deserialize;

const DEFAULT_TIMEOUT_MS: u64 = 150;
const MAX_TIMEOUT_MS: u64 = 2_000;

/// What one protocol search probes; an unknown member is a request error.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DiscoverScope {
    pub(super) cidr: Option<String>,
    pub(super) host: Option<String>,
    pub(super) adapter: Option<String>,
    pub(super) timeout_ms: Option<u64>,
    pub(super) unit_id: Option<u8>,
    pub(super) probe_read_address: Option<u16>,
    pub(super) probe_read_quantity: Option<u16>,
}

/// How long one probe of `scope` waits: `timeout_ms`, from 1 to 2,000 ms, and 150 ms without it.
pub(super) fn timeout(scope: &DiscoverScope) -> Duration {
    Duration::from_millis(
        scope
            .timeout_ms
            .unwrap_or(DEFAULT_TIMEOUT_MS)
            .clamp(1, MAX_TIMEOUT_MS),
    )
}

/// The hosts of an IPv4 subnet of `/24` or tighter, in ascending order; a subnet up to `/30` loses
/// its network and broadcast address.
pub(crate) fn ipv4_targets_for_cidr(cidr: &str) -> Result<Vec<Ipv4Addr>, String> {
    let (base, prefix) = parse_ipv4_cidr(cidr)?;
    if prefix < 24 {
        return Err("Discovery CIDR must be /24 or tighter for safety.".to_string());
    }
    let host_count = 1u64 << (32 - u64::from(prefix));
    if host_count > 256 {
        return Err("Discovery CIDR is too large; use a /24 or tighter range.".to_string());
    }
    let base_raw = u32::from(base);
    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - u32::from(prefix))
    };
    let network = base_raw & mask;
    let broadcast = network | !mask;
    let mut addrs = Vec::new();
    for raw in network..=broadcast {
        if prefix <= 30 && (raw == network || raw == broadcast) {
            continue;
        }
        addrs.push(Ipv4Addr::from(raw));
    }
    Ok(addrs)
}

pub(super) fn parse_ipv4_cidr(cidr: &str) -> Result<(Ipv4Addr, u8), String> {
    let cidr = cidr.trim();
    let (ip, prefix) = cidr
        .split_once('/')
        .ok_or_else(|| format!("invalid CIDR '{cidr}', expected a.b.c.d/24"))?;
    let ip = ip
        .trim()
        .parse::<Ipv4Addr>()
        .map_err(|error| format!("invalid CIDR address '{ip}': {error}"))?;
    let prefix = prefix
        .trim()
        .parse::<u8>()
        .map_err(|error| format!("invalid CIDR prefix '{prefix}': {error}"))?;
    if prefix > 32 {
        return Err(format!("invalid CIDR prefix '{prefix}', expected 0..32"));
    }
    Ok((ip, prefix))
}

/// The socket of a known address `host[:port]`, with `default_port` when it names none.
pub(super) fn socket_target(
    host: &str,
    default_port: u16,
    label: &str,
) -> Result<SocketAddr, String> {
    let (host, port) = authority_host_and_port(host, label)?;
    let port = port.unwrap_or(default_port);
    resolve_authority(host.as_str(), port, label)
}

/// The host and the port of a known address: a host name or IP address, an IPv6 address in
/// brackets, and an optional port from 1 to 65535; nothing else.
pub(super) fn authority_host_and_port(
    authority: &str,
    label: &str,
) -> Result<(String, Option<u16>), String> {
    if authority.is_empty()
        || authority.chars().any(char::is_whitespace)
        || authority
            .chars()
            .any(|ch| matches!(ch, '/' | '?' | '#' | '@'))
        || authority.contains("://")
    {
        return Err(format!("invalid {label} authority '{authority}'"));
    }
    if let Some(bracketed) = authority.strip_prefix('[') {
        let (host, suffix) = bracketed
            .split_once(']')
            .ok_or_else(|| format!("invalid {label} IPv6 authority '{authority}'"))?;
        host.parse::<std::net::Ipv6Addr>()
            .map_err(|_| format!("invalid {label} IPv6 authority '{authority}'"))?;
        let port = if suffix.is_empty() {
            None
        } else {
            Some(parse_authority_port(
                suffix
                    .strip_prefix(':')
                    .ok_or_else(|| format!("invalid {label} authority '{authority}'"))?,
                label,
            )?)
        };
        return Ok((format!("[{host}]"), port));
    }
    if authority.matches(':').count() > 1 {
        return Err(format!(
            "invalid unbracketed {label} IPv6 authority '{authority}'"
        ));
    }
    match authority.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() => {
            Ok((host.to_string(), Some(parse_authority_port(port, label)?)))
        }
        Some(_) => Err(format!("invalid {label} authority '{authority}'")),
        None => Ok((authority.to_string(), None)),
    }
}

fn parse_authority_port(port: &str, label: &str) -> Result<u16, String> {
    port.parse::<u16>()
        .ok()
        .filter(|port| *port != 0)
        .ok_or_else(|| format!("invalid {label} port '{port}', expected 1..65535"))
}

/// The first socket address of `host:port`.
pub(super) fn resolve_authority(host: &str, port: u16, label: &str) -> Result<SocketAddr, String> {
    let authority = format!("{host}:{port}");
    if let Ok(address) = authority.parse::<SocketAddr>() {
        return Ok(address);
    }
    (host, port)
        .to_socket_addrs()
        .map_err(|error| format!("resolve {label} target '{authority}': {error}"))?
        .next()
        .ok_or_else(|| format!("resolve {label} target '{authority}': no address found"))
}

/// Every distinct socket address of `host:port`, in resolution order.
pub(super) fn resolve_authority_all(
    host: &str,
    port: u16,
    label: &str,
) -> Result<Vec<SocketAddr>, String> {
    let authority = format!("{host}:{port}");
    if let Ok(address) = authority.parse::<SocketAddr>() {
        return Ok(vec![address]);
    }
    let mut addresses = Vec::new();
    for address in (host, port)
        .to_socket_addrs()
        .map_err(|error| format!("resolve {label} target '{authority}': {error}"))?
    {
        if !addresses.contains(&address) {
            addresses.push(address);
        }
    }
    if addresses.is_empty() {
        return Err(format!(
            "resolve {label} target '{authority}': no address found"
        ));
    }
    Ok(addresses)
}

#[cfg(test)]
#[path = "scope_tests.rs"]
mod tests;
