//! The run target of the HMI Builder's Live mode and Deploy (specification 39 §11 item 6 and §16.2
//! item 8), asked through `trust-lsp.runTargets` as the extension asks it.

use serde_json::{json, Value};

use super::super::{execute, RUN_TARGETS_COMMAND};

fn target(mode: &str, state: &str, endpoint: &str, reachable: bool, label: Option<&str>) -> Value {
    let answer = execute(
        RUN_TARGETS_COMMAND,
        vec![json!({
            "action": "hmiTarget",
            "snapshot": {
                "runtimeMode": mode,
                "runtimeState": state,
                "endpoint": endpoint,
                "endpointProbed": true,
                "endpointReachable": reachable,
                "starting": false,
            },
            "targetLabel": label,
        })],
    );
    assert_eq!(answer["ok"], true, "{answer}");
    answer["target"].clone()
}

/// The Simulator the extension launched is the target, named `Simulator`. Catches a launched
/// session left out, as the status bar's `connected` rule leaves it out, and a Simulator named by
/// its endpoint.
#[test]
fn a_launched_simulator_is_the_target() {
    assert_eq!(
        target("simulate", "running", "tcp://127.0.0.1:9001", false, None),
        json!({ "endpoint": "tcp://127.0.0.1:9001", "label": "Simulator", "simulator": true })
    );
}

/// A runtime the extension attached to is the target by its session's name, also while no probe
/// answered; a blank name is no name. Catches the session's name dropped, an attached session
/// that needs a probe, and a blank label shown.
#[test]
fn an_attached_runtime_is_the_target_by_its_session_name() {
    assert_eq!(
        target(
            "online",
            "connected",
            "tcp://10.0.0.9:9901",
            false,
            Some("Cpu")
        ),
        json!({ "endpoint": "tcp://10.0.0.9:9901", "label": "Cpu", "simulator": false })
    );
    assert_eq!(
        target(
            "online",
            "connected",
            "tcp://10.0.0.9:9901",
            false,
            Some("  ")
        ),
        json!({ "endpoint": "tcp://10.0.0.9:9901", "label": "tcp://10.0.0.9:9901", "simulator": false })
    );
}

/// The configured online runtime is the target while it answers, without a debug session, by its
/// endpoint. Catches a target that needs a session.
#[test]
fn an_online_runtime_that_answers_is_the_target() {
    assert_eq!(
        target("online", "stopped", "tcp://192.168.1.20:9000", true, None),
        json!({ "endpoint": "tcp://192.168.1.20:9000", "label": "tcp://192.168.1.20:9000", "simulator": false })
    );
}

/// Nothing that neither runs nor answers is a target: an online runtime that does not answer, a
/// stopped Simulator even with a reachable flag, a session without a control endpoint, and a
/// snapshot without a session state. Catches online mode alone taken as a target, reachability
/// counted outside online mode, a target without an endpoint, and every state but `stopped` read as
/// a session.
#[test]
fn nothing_that_neither_runs_nor_answers_is_a_target() {
    assert_eq!(
        target("online", "stopped", "tcp://192.168.1.20:9000", false, None),
        Value::Null
    );
    assert_eq!(
        target("simulate", "stopped", "tcp://127.0.0.1:9001", true, None),
        Value::Null
    );
    assert_eq!(target("simulate", "running", "", false, None), Value::Null);
    assert_eq!(
        target("online", "connected", "   ", true, Some("Cpu")),
        Value::Null
    );
    assert_eq!(
        target("online", "", "tcp://192.168.1.20:9000", false, None),
        Value::Null
    );
}
