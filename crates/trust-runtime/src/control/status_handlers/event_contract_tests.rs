use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use super::*;
use crate::debug::RuntimeEvent;
use crate::value::Duration;

const SOURCE: &str = r#"
PROGRAM Main
END_PROGRAM
"#;

fn state() -> ControlState {
    crate::control::tests::hmi_test_state(SOURCE)
}

fn time(ms: i64) -> Duration {
    Duration::from_millis(ms)
}

fn cycle(cycle: u64) -> RuntimeEvent {
    RuntimeEvent::CycleStart {
        cycle,
        time: time(cycle as i64),
    }
}

fn fault(error: &str, timestamp_ms: i64) -> RuntimeEvent {
    RuntimeEvent::Fault {
        error: error.to_string(),
        time: time(timestamp_ms),
    }
}

fn result(response: ControlResponse) -> Value {
    assert!(response.ok, "request failed: {:?}", response.error);
    response.result.expect("response result")
}

fn assert_invalid(response: ControlResponse) {
    assert!(!response.ok, "invalid parameters must reject");
    assert!(
        response
            .error
            .as_deref()
            .is_some_and(|error| error.contains("invalid params")),
        "missing invalid-params diagnostic: {:?}",
        response.error
    );
}

fn assert_unavailable(response: ControlResponse) {
    assert!(!response.ok, "unavailable event store must fail");
    assert!(
        response
            .error
            .as_deref()
            .is_some_and(|error| error.contains("unavailable")),
        "missing unavailable diagnostic: {:?}",
        response.error
    );
}

fn poison<T>(mutex: &Arc<Mutex<T>>) {
    let mutex = Arc::clone(mutex);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _guard = mutex.lock().expect("lock before poison");
        panic!("intentional contract-test poison");
    }));
    assert!(result.is_err(), "poison setup must panic");
}

fn event_cycles(value: &Value, field: &str) -> Vec<u64> {
    value[field]
        .as_array()
        .expect("event array")
        .iter()
        .filter_map(|entry| entry.get("cycle").and_then(Value::as_u64))
        .collect()
}

fn fault_errors(value: &Value) -> Vec<&str> {
    value["faults"]
        .as_array()
        .expect("fault array")
        .iter()
        .map(|entry| entry["error"].as_str().expect("fault error"))
        .collect()
}

#[test]
fn events_tail_defaults_to_fifty_newest_events() {
    let state = state();
    state
        .events
        .lock()
        .expect("events")
        .extend((1..=55).map(cycle));

    let value = result(handle_events_tail(1, None, &state));
    let cycles = event_cycles(&value, "events");
    assert_eq!(cycles.len(), 50);
    assert_eq!(cycles.first(), Some(&55));
    assert_eq!(cycles.last(), Some(&6));
}

#[test]
fn events_tail_applies_explicit_limit_newest_first() {
    let state = state();
    state
        .events
        .lock()
        .expect("events")
        .extend((1..=5).map(cycle));

    let value = result(handle_events_tail(2, Some(json!({"limit": 3})), &state));
    assert_eq!(event_cycles(&value, "events"), [5, 4, 3]);
}

#[test]
fn events_tail_accepts_minimum_limit() {
    let state = state();
    state
        .events
        .lock()
        .expect("events")
        .extend((1..=3).map(cycle));

    let value = result(handle_events_tail(3, Some(json!({"limit": 1})), &state));
    assert_eq!(event_cycles(&value, "events"), [3]);
}

#[test]
fn events_tail_accepts_maximum_limit() {
    let state = state();
    state
        .events
        .lock()
        .expect("events")
        .extend((1..=1_001).map(cycle));

    let value = result(handle_events_tail(4, Some(json!({"limit": 1_000})), &state));
    let cycles = event_cycles(&value, "events");
    assert_eq!(cycles.len(), 1_000);
    assert_eq!(cycles.first(), Some(&1_001));
    assert_eq!(cycles.last(), Some(&2));
}

#[test]
fn events_tail_empty_store_returns_empty_array() {
    assert_eq!(
        result(handle_events_tail(5, None, &state())),
        json!({"events": []})
    );
}

macro_rules! events_invalid_case {
    ($name:ident, $params:expr) => {
        #[test]
        fn $name() {
            assert_invalid(handle_events_tail(100, Some($params), &state()));
        }
    };
}

events_invalid_case!(events_tail_rejects_null_params, Value::Null);
events_invalid_case!(events_tail_rejects_array_params, json!([]));
events_invalid_case!(events_tail_rejects_string_params, json!("limit=2"));
events_invalid_case!(events_tail_rejects_boolean_params, json!(true));
events_invalid_case!(events_tail_rejects_unknown_key, json!({"count": 2}));
events_invalid_case!(
    events_tail_rejects_limit_with_unknown_sibling,
    json!({"limit": 2, "extra": false})
);
events_invalid_case!(events_tail_rejects_null_limit, json!({"limit": null}));
events_invalid_case!(events_tail_rejects_boolean_limit, json!({"limit": true}));
events_invalid_case!(events_tail_rejects_string_limit, json!({"limit": "2"}));
events_invalid_case!(events_tail_rejects_fractional_limit, json!({"limit": 2.5}));
events_invalid_case!(events_tail_rejects_negative_limit, json!({"limit": -1}));
events_invalid_case!(events_tail_rejects_zero_limit, json!({"limit": 0}));
events_invalid_case!(
    events_tail_rejects_limit_above_maximum,
    json!({"limit": 1_001})
);

#[test]
fn events_tail_fails_when_event_store_is_unavailable() {
    let state = state();
    poison(&state.events);
    assert_unavailable(handle_events_tail(6, None, &state));
}

#[test]
fn faults_filter_before_applying_limit() {
    let state = state();
    state.events.lock().expect("events").extend([
        fault("old fault", 1),
        cycle(2),
        cycle(3),
        fault("new fault", 4),
        cycle(5),
    ]);

    let value = result(handle_faults(7, Some(json!({"limit": 2})), &state));
    assert_eq!(fault_errors(&value), ["new fault", "old fault"]);
}

#[test]
fn faults_default_to_fifty_newest_faults_not_fifty_events() {
    let state = state();
    let mut events = VecDeque::new();
    for index in 1..=55 {
        events.push_back(fault(&format!("fault-{index}"), index));
        events.push_back(cycle(index as u64));
    }
    *state.events.lock().expect("events") = events;

    let value = result(handle_faults(8, None, &state));
    let errors = fault_errors(&value);
    assert_eq!(errors.len(), 50);
    assert_eq!(errors.first(), Some(&"fault-55"));
    assert_eq!(errors.last(), Some(&"fault-6"));
}

#[test]
fn faults_return_newest_fault_first() {
    let state = state();
    state.events.lock().expect("events").extend([
        fault("first", 1),
        fault("second", 2),
        fault("third", 3),
    ]);

    let value = result(handle_faults(9, Some(json!({"limit": 3})), &state));
    assert_eq!(fault_errors(&value), ["third", "second", "first"]);
}

#[test]
fn faults_exclude_safe_state_failure_events() {
    let state = state();
    state.events.lock().expect("events").extend([
        fault("root fault", 1),
        RuntimeEvent::SafeStateFailed {
            root: "root fault".to_string(),
            error: "output write failed".to_string(),
            time: time(2),
        },
    ]);

    let value = result(handle_faults(10, None, &state));
    assert_eq!(fault_errors(&value), ["root fault"]);
}

#[test]
fn events_tail_preserves_safe_state_failure_as_distinct_event() {
    let state = state();
    state
        .events
        .lock()
        .expect("events")
        .push_back(RuntimeEvent::SafeStateFailed {
            root: "division by zero".to_string(),
            error: "safe output rejected".to_string(),
            time: time(2),
        });

    let value = result(handle_events_tail(11, None, &state));
    assert_eq!(
        value.pointer("/events/0"),
        Some(&json!({
            "type": "safe_state_failed",
            "root": "division by zero",
            "error": "safe output rejected",
            "time_ns": 2_000_000,
        }))
    );
}

#[test]
fn faults_empty_store_returns_empty_array() {
    assert_eq!(
        result(handle_faults(12, None, &state())),
        json!({"faults": []})
    );
}

macro_rules! faults_invalid_case {
    ($name:ident, $params:expr) => {
        #[test]
        fn $name() {
            assert_invalid(handle_faults(200, Some($params), &state()));
        }
    };
}

faults_invalid_case!(faults_reject_null_params, Value::Null);
faults_invalid_case!(faults_reject_non_object_params, json!([]));
faults_invalid_case!(faults_reject_unknown_key, json!({"count": 2}));
faults_invalid_case!(faults_reject_null_limit, json!({"limit": null}));
faults_invalid_case!(faults_reject_boolean_limit, json!({"limit": true}));
faults_invalid_case!(faults_reject_string_limit, json!({"limit": "2"}));
faults_invalid_case!(faults_reject_fractional_limit, json!({"limit": 2.5}));
faults_invalid_case!(faults_reject_negative_limit, json!({"limit": -1}));
faults_invalid_case!(faults_reject_zero_limit, json!({"limit": 0}));
faults_invalid_case!(faults_reject_limit_above_maximum, json!({"limit": 1_001}));

#[test]
fn faults_accept_maximum_limit() {
    let state = state();
    state
        .events
        .lock()
        .expect("events")
        .extend((1..=1_001).map(|index| fault(&format!("fault-{index}"), index)));

    let value = result(handle_faults(13, Some(json!({"limit": 1_000})), &state));
    assert_eq!(value["faults"].as_array().expect("faults").len(), 1_000);
}

#[test]
fn faults_fail_when_event_store_is_unavailable() {
    let state = state();
    poison(&state.events);
    assert_unavailable(handle_faults(14, None, &state));
}
