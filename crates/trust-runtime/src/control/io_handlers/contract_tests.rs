//! The host's own `io.*` wiring: the snapshot slot's lock and the conversion of the hosted
//! snapshot into the portable one. The operations' parameters, refusals, error texts and JSON are
//! the shared `io_requests` contract tests.

use std::sync::{Arc, Mutex};

use serde_json::json;
use smol_str::SmolStr;

use super::*;
use crate::io::{IoAddress, IoSnapshot, IoSnapshotEntry, IoSnapshotValue};
use crate::value::Value;

const SOURCE: &str = r#"
PROGRAM Main
END_PROGRAM
"#;

fn state() -> ControlState {
    crate::control::tests::hmi_test_state(SOURCE)
}

fn poison<T>(mutex: &Arc<Mutex<T>>) {
    let mutex = Arc::clone(mutex);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _guard = mutex.lock().expect("lock before poison");
        panic!("intentional contract-test poison");
    }));
    assert!(result.is_err(), "poison setup must panic");
}

#[test]
fn io_list_fails_closed_when_snapshot_lock_is_unavailable() {
    let state = state();
    poison(&state.io_snapshot);

    let response = handle_io_list(3, &state);
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("I/O snapshot unavailable"));
}

#[test]
fn io_read_fails_closed_when_snapshot_lock_is_unavailable() {
    let state = state();
    poison(&state.io_snapshot);

    let response = handle_io_read(4, &state);
    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("I/O snapshot unavailable"));
}

#[test]
fn io_list_answers_the_hosted_snapshot_through_the_portable_one() {
    let state = state();
    let snapshot = IoSnapshot {
        scan: Some(5),
        forced: Vec::new(),
        inputs: Vec::new(),
        outputs: vec![IoSnapshotEntry {
            name: Some(SmolStr::new("Lamp")),
            address: IoAddress::parse("%QX0.0").expect("address"),
            value_type: Some(trust_hir::TypeId::BOOL),
            value_type_name: None,
            value: IoSnapshotValue::Value(Value::Bool(true)),
            source: None,
        }],
        memory: Vec::new(),
    };
    *state.io_snapshot.lock().expect("snapshot lock") = Some(snapshot);

    let response = handle_io_list(6, &state);
    assert!(response.ok, "request failed: {:?}", response.error);
    assert_eq!(
        response.result,
        Some(json!({
            "scan": 5,
            "inputs": [],
            "outputs": [{
                "name": "Lamp",
                "address": "%QX0.0",
                "value": "Bool(true)",
                "forced": false,
                "valueType": "BOOL",
            }],
            "memory": [],
        }))
    );
}
