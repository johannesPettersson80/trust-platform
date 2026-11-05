//! What Live Values says when it has no session to read (specification 25 §4.4 and §5.1).

use serde_json::{json, Value};

use super::answer;

const ENDPOINT: &str = "tcp://127.0.0.1:5000";

fn unavailable(snapshot: Value) -> Value {
    answer(json!({ "action": "unavailable", "snapshot": snapshot })).expect("an answer")
}

/// The lifecycle's failure of a runtime that answered Connect at `endpoint` but keeps debugging
/// disabled.
fn debug_disabled_at(endpoint: &str) -> Value {
    json!({ "kind": "debug_disabled", "endpoint": endpoint, "label": "Line 1" })
}

/// Catches: a selected online runtime without a session told to Start, which it cannot do from
/// here, instead of to Connect.
#[test]
fn an_online_runtime_without_a_session_is_told_to_connect() {
    let answer = unavailable(json!({
        "runtimeMode": "online", "runtimeState": "stopped", "endpoint": ENDPOINT,
    }));
    assert_eq!(
        answer,
        json!({ "ok": true, "message": "Connect to the selected runtime to see live values.",
                "debugDisabled": false })
    );
}

/// Catches: a runtime that keeps debugging disabled told to Connect again, which changes nothing,
/// instead of why it has no session: §5.1 names `debug_enabled` under `[runtime.control]` in its
/// `runtime.toml`.
#[test]
fn a_runtime_that_keeps_debugging_disabled_says_why() {
    let answer = unavailable(json!({
        "runtimeMode": "online", "runtimeState": "stopped", "endpoint": ENDPOINT,
        "failure": debug_disabled_at(ENDPOINT),
    }));
    assert_eq!(answer["debugDisabled"], true);
    let message = answer["message"].as_str().expect("a message");
    for named in ["debug_enabled", "[runtime.control]", "runtime.toml"] {
        assert!(message.contains(named), "{message} names {named}");
    }
}

/// Catches: the debugging-disabled reason of another runtime applied to the selected one.
#[test]
fn debugging_disabled_at_another_endpoint_does_not_apply() {
    let answer = unavailable(json!({
        "runtimeMode": "online", "runtimeState": "stopped", "endpoint": ENDPOINT,
        "failure": debug_disabled_at("tcp://127.0.0.1:5001"),
    }));
    assert_eq!(
        answer["message"],
        "Connect to the selected runtime to see live values."
    );
    assert_eq!(answer["debugDisabled"], false);
}

/// Catches: the Simulator, or a target the lifecycle names connected, told to Connect, and the
/// debugging-disabled state kept for a target that is no online runtime waiting for its Connect,
/// such as a simulator whose status names the endpoint of an earlier Connect's report.
#[test]
fn every_other_target_is_told_to_start() {
    let start = json!({ "ok": true, "message": "Start the runtime to see live values.",
                        "debugDisabled": false });
    for snapshot in [
        json!({ "runtimeMode": "simulate", "runtimeState": "stopped", "endpoint": "" }),
        json!({ "runtimeMode": "simulate", "runtimeState": "stopped", "endpoint": ENDPOINT,
                "failure": debug_disabled_at(ENDPOINT) }),
        json!({ "runtimeMode": "online", "runtimeState": "connected", "endpoint": ENDPOINT,
                "failure": debug_disabled_at(ENDPOINT) }),
        // No session became the active one, and no status was read.
        json!({}),
    ] {
        assert_eq!(unavailable(snapshot.clone()), start, "{snapshot}");
    }
}
