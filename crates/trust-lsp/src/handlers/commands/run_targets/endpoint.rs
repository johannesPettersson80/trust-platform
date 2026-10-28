//! What a control endpoint's text says before anything is contacted: the channel a credential
//! would travel on, and the short names the extension shows for a runtime.

use std::net::IpAddr;

use trust_runtime_client::ControlEndpoint;

/// The channel a token sent to an endpoint travels on (specification 11, the credential channel):
/// `trusted_same_host` for a Unix socket and a TCP endpoint on a loopback address,
/// `untrusted_remote_plain_tcp` for any other TCP endpoint, and `unavailable` for a blank or
/// unreadable endpoint (a TCP endpoint needs a host and a port from 1 to 65535) and for a serial
/// port, over which the extension forwards no credential.
pub(super) fn credential_channel(endpoint: &str) -> &'static str {
    match ControlEndpoint::parse(endpoint.trim()) {
        Some(ControlEndpoint::Tcp(address)) => match tcp_host(&address) {
            Some(host) if is_loopback_host(host) => "trusted_same_host",
            Some(_) => "untrusted_remote_plain_tcp",
            None => "unavailable",
        },
        #[cfg(unix)]
        Some(ControlEndpoint::Unix(_)) => "trusted_same_host",
        Some(ControlEndpoint::Serial(_)) | None => "unavailable",
    }
}

/// The host of `host:port`, without the brackets of an IPv6 address; `None` without a host or a
/// port from 1 to 65535.
fn tcp_host(address: &str) -> Option<&str> {
    let (host, port) = match address.strip_prefix('[') {
        Some(rest) => {
            let (host, after) = rest.split_once(']')?;
            (host, after.strip_prefix(':')?)
        }
        None => address.rsplit_once(':')?,
    };
    let port = port.split('/').next().unwrap_or_default();
    (!host.is_empty() && port.parse::<u16>().is_ok_and(|port| port > 0)).then_some(host)
}

/// Whether `host` names this computer: `localhost` or a loopback address.
fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// The name of a runtime that only its endpoint names: the endpoint without its scheme, its port
/// kept so that two runtimes on one host stay apart (specification 25 §5.1), `runtime` for a blank
/// endpoint. A Unix socket keeps its path.
pub(super) fn remote_label(endpoint: &str) -> String {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return "runtime".to_owned();
    }
    let without_scheme = strip_scheme(trimmed);
    let first = without_scheme.split('/').next().unwrap_or_default();
    if !first.is_empty() {
        first.to_owned()
    } else if !without_scheme.is_empty() {
        without_scheme.to_owned()
    } else {
        "runtime".to_owned()
    }
}

/// `text` without a leading `<letters>://`.
fn strip_scheme(text: &str) -> &str {
    match text.split_once("://") {
        Some((scheme, rest))
            if !scheme.is_empty() && scheme.chars().all(|c| c.is_ascii_alphabetic()) =>
        {
            rest
        }
        _ => text,
    }
}

/// The endpoint as a sentence names it: `host:port` of a TCP endpoint, `the local control socket`
/// for a Unix socket, `the configured endpoint` for a blank one, else the endpoint's text.
pub(super) fn sentence_label(endpoint: &str) -> String {
    let text = endpoint.trim();
    if text.is_empty() {
        return "the configured endpoint".to_owned();
    }
    if let Some(rest) = text.strip_prefix("tcp://") {
        let host = rest.split('/').next().unwrap_or_default();
        return if host.is_empty() {
            "the configured endpoint".to_owned()
        } else {
            host.to_owned()
        };
    }
    if text.starts_with("unix://") {
        return "the local control socket".to_owned();
    }
    text.to_owned()
}

#[cfg(test)]
#[path = "endpoint_tests.rs"]
mod tests;
