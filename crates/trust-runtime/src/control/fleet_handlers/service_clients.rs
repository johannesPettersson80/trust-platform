//! The fleet topology's service clients (specification 11 §6.8.1a, "Service clients";
//! specification 48 RU-64): each service that holds a service token, as a client of its runtime,
//! with the age of its last accepted heartbeat by the runtime's own clock.

use std::time::{Duration, Instant};

use serde_json::json;

use super::endpoint_id;
use crate::control::ControlState;
use crate::fleet_topology::FleetEndpoint;
use crate::security::pairing::ServiceBeat;

/// The period of a heartbeat that named none.
const ASSUMED_PERIOD: Duration = Duration::from_secs(1);

/// The service clients of the runtime `runtime_id`: one endpoint for each service whose service
/// token its pairing store holds, in the order of the services' names; none without a store.
pub(super) fn service_client_endpoints(
    runtime_id: &str,
    state: &ControlState,
) -> Vec<FleetEndpoint> {
    let Some(store) = state.pairing.as_ref() else {
        return Vec::new();
    };
    let now = Instant::now();
    store
        .service_clients()
        .into_iter()
        .map(|(name, beat)| service_client(runtime_id, &name, beat.as_ref(), now))
        .collect()
}

/// The endpoint of the service `name` of the runtime `runtime_id` at `now`, with its last
/// heartbeat `beat`: `configured_policy` without one, `connected` while it is younger than twice
/// its period (1 s for a heartbeat that named none), `degraded` after that.
pub(super) fn service_client(
    runtime_id: &str,
    name: &str,
    beat: Option<&ServiceBeat>,
    now: Instant,
) -> FleetEndpoint {
    let (health, detail, live) = match beat {
        None => (
            "configured_policy",
            "no heartbeat since the runtime started".to_string(),
            None,
        ),
        Some(beat) => {
            let age = now.saturating_duration_since(beat.at);
            let period = beat.period_ms.map(Duration::from_millis);
            let health = if age < period.unwrap_or(ASSUMED_PERIOD) * 2 {
                "connected"
            } else {
                "degraded"
            };
            let mut detail = format!("last heartbeat {} ago", seconds(age));
            let mut live = json!({ "age_ms": u64::try_from(age.as_millis()).unwrap_or(u64::MAX) });
            if let (Some(period), Some(period_ms)) = (period, beat.period_ms) {
                detail.push_str(&format!(", every {}", seconds(period)));
                live["period_ms"] = json!(period_ms);
            }
            (health, detail, Some(live))
        }
    };
    FleetEndpoint {
        id: endpoint_id(runtime_id, &format!("service:{name}")),
        kind: "client".to_string(),
        protocol: "service".to_string(),
        name: name.to_string(),
        address: None,
        role: Some("client".to_string()),
        health: health.to_string(),
        detail,
        live,
        params: Some(json!({ "service": name })),
        children: Vec::new(),
        owned: false,
        supports_test: false,
        source: Some("self".to_string()),
    }
}

/// `duration` in seconds with one decimal: `0.4 s`.
fn seconds(duration: Duration) -> String {
    format!("{:.1} s", duration.as_secs_f64())
}

#[cfg(test)]
#[path = "service_clients_tests.rs"]
mod tests;
