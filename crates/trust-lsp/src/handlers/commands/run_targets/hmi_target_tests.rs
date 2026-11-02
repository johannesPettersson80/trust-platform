//! The run target of the HMI Builder's Live mode and Deploy (specification 39 §11 item 6 and §16.2
//! item 8), asked through `trust-lsp.runTargets` as the extension asks it.

use serde_json::{json, Value};

use super::super::{execute, RUN_TARGETS_COMMAND};

fn snapshot(
    mode: &str,
    state: &str,
    endpoint: &str,
    reachable: bool,
    label: Option<&str>,
) -> Value {
    json!({
        "runtimeMode": mode,
        "runtimeState": state,
        "endpoint": endpoint,
        "endpointProbed": true,
        "endpointReachable": reachable,
        "starting": false,
        "targetLabel": label,
    })
}

fn target_of(snapshot: Value) -> Value {
    let answer = execute(
        RUN_TARGETS_COMMAND,
        vec![json!({ "action": "hmiTarget", "snapshot": snapshot })],
    );
    assert_eq!(answer["ok"], true, "{answer}");
    answer["target"].clone()
}

fn target(mode: &str, state: &str, endpoint: &str, reachable: bool, label: Option<&str>) -> Value {
    target_of(snapshot(mode, state, endpoint, reachable, label))
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
/// answered; a blank name is no name, and the runtime is named as the status bar names it, by its
/// endpoint without the scheme. Catches the session's name dropped, an attached session that needs
/// a probe, and a blank label shown.
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
        json!({ "endpoint": "tcp://10.0.0.9:9901", "label": "10.0.0.9:9901", "simulator": false })
    );
}

/// The configured online runtime is the target while it answers, without a debug session, named as
/// the status bar names it. Catches a target that needs a session, and the scheme shown in the name
/// (HMI Live said `tcp://127.0.0.1:<port>` where the status bar said `127.0.0.1:<port>`).
#[test]
fn an_online_runtime_that_answers_is_the_target() {
    assert_eq!(
        target("online", "stopped", "tcp://192.168.1.20:9000", true, None),
        json!({ "endpoint": "tcp://192.168.1.20:9000", "label": "192.168.1.20:9000", "simulator": false })
    );
}

/// A runtime that answered Connect but keeps debugging disabled is the target by the name Connect
/// was given, as the status bar names it; the workspace project's own runtime by that name while
/// it answers, whatever Connect called it. Catches HMI Live naming a runtime without a session by
/// its endpoint where Connect and the status bar say `Cpu`, and the project's runtime renamed.
#[test]
fn a_runtime_without_a_session_has_the_status_bar_name() {
    let endpoint = "tcp://127.0.0.1:18093";
    let mut disabled = snapshot("online", "stopped", endpoint, true, None);
    disabled["failure"] = json!({ "kind": "debug_disabled", "endpoint": endpoint, "label": "Cpu" });
    assert_eq!(
        target_of(disabled),
        json!({ "endpoint": endpoint, "label": "Cpu", "simulator": false })
    );

    let project = "unix:///home/user/tank/.trust-runtime/control.sock";
    let mut own = snapshot("online", "stopped", project, true, None);
    own["projectEndpoint"] = json!(project);
    own["projectReachable"] = json!(true);
    own["failure"] = json!({ "kind": "debug_disabled", "endpoint": project, "label": "Cpu" });
    assert_eq!(
        target_of(own),
        json!({ "endpoint": project, "label": "This project's runtime", "simulator": false })
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
