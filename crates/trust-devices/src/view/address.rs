//! Addresses as the panel shows them: a control endpoint as host and port, never a local socket's
//! path (specification 25 §5.1 and §5.6); the address that names another computer; and the
//! subnet that groups computers into networks (decision O29).

use std::net::{IpAddr, Ipv4Addr};

use crate::topology::Host;

/// The host and port of a control endpoint, without its `tcp://` scheme. A local socket has no
/// address to show, and its path is never shown.
pub(crate) fn host_and_port(endpoint: &str) -> Option<String> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() || endpoint.starts_with("unix://") {
        return None;
    }
    let address = endpoint
        .split_once("://")
        .map_or(endpoint, |(_, rest)| rest)
        .trim_end_matches('/');
    (!address.is_empty()).then(|| address.to_owned())
}

/// Whether the endpoint is a local control socket.
pub(crate) fn is_local_socket(endpoint: &str) -> bool {
    endpoint.trim().starts_with("unix://")
}

/// The host of a control endpoint: `localhost` for a local socket.
pub(crate) fn endpoint_host(endpoint: &str) -> Option<String> {
    if is_local_socket(endpoint) {
        return Some("localhost".to_owned());
    }
    let address = host_and_port(endpoint)?;
    if let Some(bracketed) = address.strip_prefix('[') {
        return bracketed
            .split_once(']')
            .map(|(host, _)| host.to_owned())
            .filter(|host| !host.is_empty());
    }
    if address.parse::<IpAddr>().is_ok() {
        return Some(address);
    }
    let host = address
        .rsplit_once(':')
        .map_or(address.as_str(), |(host, _)| host);
    (!host.is_empty()).then(|| host.to_owned())
}

/// Whether an address names this computer only: `localhost`, a loopback address or the
/// unspecified address.
pub(crate) fn is_loopback(address: &str) -> bool {
    let address = address.trim().trim_start_matches('[').trim_end_matches(']');
    address.eq_ignore_ascii_case("localhost")
        || address
            .parse::<IpAddr>()
            .is_ok_and(|ip| ip.is_loopback() || ip.is_unspecified())
}

/// The first address of a computer that is not a loopback address: one it reports, else the host
/// of one of its runtimes' control endpoints.
pub(crate) fn remote_address(host: &Host) -> Option<String> {
    host.ips
        .iter()
        .map(|ip| ip.trim().to_owned())
        .find(|ip| !ip.is_empty() && !is_loopback(ip))
        .or_else(|| {
            host.all_runtimes()
                .filter_map(|runtime| runtime.control_endpoint.as_deref())
                .filter_map(endpoint_host)
                .find(|address| !is_loopback(address))
        })
}

/// The IPv4 subnet of a computer, from its first IPv4 address that is not a loopback address,
/// with the prefix its runtime reports or else `/24` (decision O29). No runtime reports its
/// prefix yet, so the prefix is `/24`.
pub(crate) fn subnet(host: &Host) -> Option<String> {
    let reported = host.ips.iter().map(|ip| ip.trim().to_owned());
    let endpoints = host
        .all_runtimes()
        .filter_map(|runtime| runtime.control_endpoint.as_deref())
        .filter_map(endpoint_host);
    reported
        .chain(endpoints)
        .filter_map(|address| address.parse::<Ipv4Addr>().ok())
        .find(|ip| !ip.is_loopback() && !ip.is_unspecified())
        .map(|ip| {
            let [a, b, c, _] = ip.octets();
            format!("{a}.{b}.{c}.0/24")
        })
}
