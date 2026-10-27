//! The status of a configured runtime target: whether a runtime answers on its control endpoint,
//! and whether it accepts the token (specification 25, 5.1).

use std::time::Duration;

use serde_json::Value;

use crate::{AuthFailure, ControlClient, ControlEndpoint, RESPONSE_LINE_LIMIT};

/// How long a probe waits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeTimeouts {
    /// The longest wait for the connection.
    pub connect: Duration,
    /// The longest wait for each read and write of the `status` request.
    pub status: Duration,
}

impl Default for ProbeTimeouts {
    /// 400 ms for the connection and 750 ms for `status`, the tools' budgets.
    fn default() -> Self {
        Self {
            connect: Duration::from_millis(400),
            status: Duration::from_millis(750),
        }
    }
}

/// What a probe found at a configured target.
#[derive(Debug, Clone, PartialEq)]
pub enum TargetProbe {
    /// No endpoint is configured, or the configured endpoint is disabled. Nothing was contacted.
    MissingEndpoint,
    /// The endpoint is not a control endpoint, nothing accepts the connection, or no valid answer
    /// to `status` arrived in time.
    Unreachable,
    /// A runtime answered and refused the token.
    AuthFailed(AuthFailure),
    /// A runtime answered `status`; this is its result.
    Reachable(Value),
}

impl TargetProbe {
    /// Whether a runtime answered: it served `status`, or it refused the token.
    #[must_use]
    pub fn answered(&self) -> bool {
        matches!(self, Self::Reachable(_) | Self::AuthFailed(_))
    }
}

/// Probes the target configured with `endpoint`: connects, sends `status` with `token`, and
/// names the outcome. A disabled or blank endpoint is never contacted, and a blank token is not
/// sent. A refusal is a credential failure only by its code; every other failure is
/// [`TargetProbe::Unreachable`].
#[must_use]
pub fn probe(
    endpoint: &str,
    enabled: bool,
    token: Option<&str>,
    timeouts: ProbeTimeouts,
) -> TargetProbe {
    let endpoint = endpoint.trim();
    if !enabled || endpoint.is_empty() {
        return TargetProbe::MissingEndpoint;
    }
    let Some(endpoint) = ControlEndpoint::parse(endpoint) else {
        return TargetProbe::Unreachable;
    };
    let status = ControlClient::connect(&endpoint, token, timeouts.connect)
        .map(|client| client.with_response_limit(RESPONSE_LINE_LIMIT))
        .and_then(|mut client| {
            client.set_timeout(timeouts.status)?;
            client.request("status", None)
        });
    match status {
        Ok(status) => TargetProbe::Reachable(status),
        Err(error) => error
            .auth_failure()
            .map_or(TargetProbe::Unreachable, TargetProbe::AuthFailed),
    }
}

#[cfg(test)]
mod tests;
