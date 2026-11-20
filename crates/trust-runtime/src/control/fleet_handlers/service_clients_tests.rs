//! The fleet topology's service clients (specification 11 §6.8.1a, "Service clients"), worked out
//! from the specification: each service as a client of its runtime, its health by the age of its
//! last heartbeat and its period, its detail and its live payload.

use std::time::{Duration, Instant};

use serde_json::json;

use super::*;
use crate::security::pairing::ServiceBeat;

/// The runtime the services are clients of.
const RUNTIME: &str = "runtime-a";

/// A heartbeat accepted at `at`, naming `period_ms`.
fn beat(at: Instant, period_ms: Option<u64>) -> ServiceBeat {
    ServiceBeat { at, period_ms }
}

#[test]
fn a_service_never_heard_from_is_configured_not_connected() {
    // §6.8.1a: before the runtime accepts a heartbeat from the service, its health is
    // configured_policy, never connected; its detail says so and it has no live payload. It is a
    // client of its runtime under the ID endpoint:<runtime>:service:<name>. Catches a service shown
    // connected merely because it holds a token.
    let endpoint = service_client(RUNTIME, "recipe", None, Instant::now());
    assert_eq!(endpoint.id, "endpoint:runtime-a:service:recipe");
    assert_eq!(
        (
            endpoint.kind.as_str(),
            endpoint.protocol.as_str(),
            endpoint.role.as_deref()
        ),
        ("client", "service", Some("client"))
    );
    assert_eq!(endpoint.name, "recipe");
    assert_eq!(endpoint.health, "configured_policy");
    assert_eq!(endpoint.detail, "no heartbeat since the runtime started");
    assert_eq!(endpoint.live, None);
    assert_eq!(endpoint.params, Some(json!({ "service": "recipe" })));
    assert!(!endpoint.owned);
}

#[test]
fn a_service_is_connected_while_its_last_heartbeat_is_younger_than_twice_its_period() {
    // §6.8.1a: every 500 ms, a last heartbeat 400 ms old is connected, and 999 ms old still; at
    // 1,000 ms, twice the period, it is degraded. The age is the runtime's: `age_ms` 400 and the
    // detail `last heartbeat 0.4 s ago, every 0.5 s`. Catches a service shown connected long after
    // it stopped, and one shown degraded between two of its heartbeats.
    let at = Instant::now();
    let beat = beat(at, Some(500));
    let young = service_client(
        RUNTIME,
        "recipe",
        Some(&beat),
        at + Duration::from_millis(400),
    );
    assert_eq!(young.health, "connected");
    assert_eq!(young.detail, "last heartbeat 0.4 s ago, every 0.5 s");
    assert_eq!(young.live, Some(json!({ "age_ms": 400, "period_ms": 500 })));
    let still = service_client(
        RUNTIME,
        "recipe",
        Some(&beat),
        at + Duration::from_millis(999),
    );
    assert_eq!(still.health, "connected");
    let old = service_client(
        RUNTIME,
        "recipe",
        Some(&beat),
        at + Duration::from_millis(1_000),
    );
    assert_eq!(old.health, "degraded");
    assert_eq!(old.detail, "last heartbeat 1.0 s ago, every 0.5 s");
}

#[test]
fn a_heartbeat_that_named_no_period_counts_as_one_of_a_second() {
    // §6.8.1a: without `period_ms` the period is 1 s: connected at 1.9 s, degraded at 2 s; the
    // detail and the live payload name no period. Catches a service without a period that is never
    // degraded, or always.
    let at = Instant::now();
    let beat = beat(at, None);
    let connected = service_client(
        RUNTIME,
        "recipe",
        Some(&beat),
        at + Duration::from_millis(1_900),
    );
    assert_eq!(connected.health, "connected");
    assert_eq!(connected.detail, "last heartbeat 1.9 s ago");
    assert_eq!(connected.live, Some(json!({ "age_ms": 1_900 })));
    let degraded = service_client(
        RUNTIME,
        "recipe",
        Some(&beat),
        at + Duration::from_millis(2_000),
    );
    assert_eq!(degraded.health, "degraded");
}
