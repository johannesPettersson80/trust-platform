//! What one TCP probe proves (specification 11, "Confidence"): a protocol answer is `confirmed`, a
//! protocol-shaped refusal `likely`, and a TCP connection without one only `port_reachable`. The
//! Modbus TCP and MQTT probes start with the same TCP connect.

use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

pub(super) const CONFIRMED: &str = "confirmed";
pub(super) const LIKELY: &str = "likely";
pub(super) const PORT_REACHABLE: &str = "port_reachable";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DiscoveryProbeOutcome {
    pub source: &'static str,
    pub confidence: &'static str,
    pub detail: String,
    pub warnings: Vec<String>,
    pub auth_required: bool,
}

impl DiscoveryProbeOutcome {
    pub(super) fn confirmed(source: &'static str, detail: impl Into<String>) -> Self {
        Self {
            source,
            confidence: CONFIRMED,
            detail: detail.into(),
            warnings: Vec::new(),
            auth_required: false,
        }
    }

    pub(super) fn likely(source: &'static str, detail: impl Into<String>) -> Self {
        Self {
            source,
            confidence: LIKELY,
            detail: detail.into(),
            warnings: Vec::new(),
            auth_required: false,
        }
    }

    pub(super) fn port_reachable(source: &'static str, detail: impl Into<String>) -> Self {
        Self {
            source,
            confidence: PORT_REACHABLE,
            detail: detail.into(),
            warnings: Vec::new(),
            auth_required: false,
        }
    }

    pub(super) fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }

    pub(super) fn with_auth_required(mut self) -> Self {
        self.auth_required = true;
        self
    }
}

pub(super) fn connect(target: SocketAddr, timeout: Duration) -> Option<TcpStream> {
    let stream = TcpStream::connect_timeout(&target, timeout).ok()?;
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));
    let _ = stream.set_nodelay(true);
    Some(stream)
}

#[cfg(test)]
#[path = "probe_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "probe_trace_cases.rs"]
mod trace_cases;
