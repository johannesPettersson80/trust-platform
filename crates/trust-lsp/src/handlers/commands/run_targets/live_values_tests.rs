//! The runtime status of Live Values (specification 25 §4.1 to §4.4) and the simulation facts of a
//! runtime's `status` (specification 35 A2.4 and B9.2).

use std::cell::RefCell;

use serde_json::{json, Value};

use super::{answer, live_values, rows, target_facts, LiveArgs, Runtimes};

/// A runtime that answers as it is told, recording who was asked.
#[derive(Default)]
struct Told {
    status: Option<Value>,
    ads: Option<Value>,
    reachable: bool,
    asked: RefCell<Vec<String>>,
}

impl Runtimes for Told {
    fn status(&self, endpoint: &str, token: Option<&str>) -> Option<Value> {
        self.asked
            .borrow_mut()
            .push(format!("status {endpoint} {}", token.unwrap_or("-")));
        self.status.clone()
    }

    fn ads_status(&self, endpoint: &str, _token: Option<&str>) -> Option<Value> {
        self.asked.borrow_mut().push(format!("ads {endpoint}"));
        self.ads.clone()
    }

    fn reachable(&self, endpoint: &str) -> bool {
        self.asked.borrow_mut().push(format!("probe {endpoint}"));
        self.reachable
    }
}

fn status_of(arguments: Value, runtimes: &Told) -> (Value, Option<Value>) {
    let args: LiveArgs = serde_json::from_value(arguments).expect("the arguments");
    live_values(&args, runtimes)
}

/// The stopped Simulator asks nothing, may do everything and pins a Force at once. Catches a probe
/// of the configured remote endpoint while the user simulates.
#[test]
fn the_stopped_simulator() {
    let told = Told::default();
    let (status, ads) = status_of(
        json!({ "runtimeMode": "simulate", "endpoint": "tcp://10.0.0.2:9901" }),
        &told,
    );
    assert!(told.asked.borrow().is_empty());
    assert_eq!(status["runtimeState"], "stopped");
    assert_eq!(status["running"], false);
    assert_eq!(status["simulated"], false);
    assert_eq!(status["endpointConfigured"], true);
    assert_eq!(status["access"]["allowForce"], true);
    assert_eq!(status["access"]["role"], "admin");
    assert_eq!(status["forceRequiresArm"], false);
    assert_eq!(ads, None);
}

/// A running Simulator's session is asked at its own control endpoint with its own token, and its
/// `status` decides access, simulation and cycle (specification 35 A2.4); outputs and memory are
/// forced but never written (specification 25 §4.2). Catches the configured endpoint asked in place
/// of the session's, a simulated flag taken from the editor's mode, and a Write offered on an
/// output.
#[test]
fn a_running_simulator_is_asked_at_its_session_endpoint() {
    let told = Told {
        status: Some(json!({
            "simulated": true,
            "simulation_enabled": true,
            "cycle_interval_ms": 10,
            "access": { "role": "engineer", "io": { "write": true, "force": false, "release": true } },
        })),
        ..Told::default()
    };
    let (status, _) = status_of(
        json!({
            "runtimeMode": "simulate",
            "endpoint": "tcp://10.0.0.2:9901",
            "authToken": "configured",
            "session": {
                "request": "launch",
                "controlEndpoint": "unix:///tmp/trust-debug-1.sock",
                "controlAuthToken": "session-token",
                "targetLabel": " Simulator ",
            },
        }),
        &told,
    );
    assert_eq!(
        told.asked.borrow().as_slice(),
        ["status unix:///tmp/trust-debug-1.sock session-token"]
    );
    assert_eq!(status["runtimeState"], "running");
    assert_eq!(status["endpoint"], "unix:///tmp/trust-debug-1.sock");
    assert_eq!(status["targetLabel"], "Simulator");
    assert_eq!(status["simulated"], true);
    assert_eq!(status["cycleIntervalMs"], 10.0);
    assert_eq!(
        status["access"],
        json!({ "role": "engineer", "allowWrite": true, "allowForce": false, "allowRelease": true, "reason": null })
    );
    assert_eq!(status["forceRequiresArm"], false);
    let rows = &status["rows"];
    assert_eq!(rows["inputs"]["live"]["write"], true);
    assert_eq!(rows["inputs"]["live"]["force"], false);
    assert_eq!(
        rows["inputs"]["live"]["forceReason"],
        "Force is not available for this value."
    );
    assert_eq!(rows["outputs"]["live"]["write"], false);
    assert_eq!(
        rows["outputs"]["live"]["writeReason"],
        "Program-driven — use Force to override"
    );
    assert_eq!(rows["memory"]["forced"]["release"], true);
    assert_eq!(rows["ads"]["live"]["write"], true);
}

/// A row's operations follow its state (specification 25 §4.2): a forced row is released, never
/// written or forced again; a read-only ADS tag is neither written nor forced; each operation not
/// allowed says why, the row's state first, then its section, then the session's access. Catches a
/// Write offered on a forced value, a Force offered on a read-only tag, and a disabled button
/// without its reason.
#[test]
fn the_operations_of_a_row_by_its_state() {
    let everything = rows(
        &json!({ "allowWrite": true, "allowForce": true, "allowRelease": true, "reason": null }),
    );
    assert_eq!(
        everything["inputs"]["live"],
        json!({ "write": true, "force": true, "release": false,
                "writeReason": null, "forceReason": null, "releaseReason": null })
    );
    assert_eq!(
        everything["inputs"]["forced"],
        json!({ "write": false, "force": false, "release": true,
                "writeReason": "Release force before writing this value.",
                "forceReason": "Force is not available for this value.", "releaseReason": null })
    );
    let read_only_tag =
        "Read-only ADS tag. Enable writes when adding the tag, then restart the runtime.";
    assert_eq!(
        everything["ads"]["readOnly"],
        json!({ "write": false, "force": false, "release": false,
                "writeReason": read_only_tag, "forceReason": read_only_tag, "releaseReason": null })
    );
    assert_eq!(
        everything["outputs"]["readOnly"]["writeReason"],
        "Program-driven — use Force to override"
    );
    assert_eq!(
        everything["outputs"]["readOnly"]["forceReason"],
        read_only_tag
    );
    let connect_first = "Connect with an engineer token to write or force.";
    let nothing = rows(
        &json!({ "allowWrite": false, "allowForce": false, "allowRelease": false, "reason": connect_first }),
    );
    assert_eq!(nothing["inputs"]["live"]["writeReason"], connect_first);
    assert_eq!(nothing["inputs"]["live"]["forceReason"], connect_first);
    assert_eq!(
        nothing["memory"]["live"]["writeReason"],
        "Program-driven — use Force to override"
    );
    assert_eq!(nothing["inputs"]["forced"]["release"], false);
    assert_eq!(nothing["inputs"]["forced"]["releaseReason"], connect_first);
    let unexplained =
        rows(&json!({ "allowWrite": false, "allowForce": false, "allowRelease": false }));
    assert_eq!(
        unexplained["ads"]["live"]["writeReason"],
        "Write is not available for this value."
    );
}

/// An attached session is `connected`, a Force on it must be armed first (specification 25 §4.2),
/// and a runtime that reports no access may do nothing until an engineer's token is used. Catches
/// an attached runtime that pins a Force with one click, and a write offered without the runtime's
/// consent.
#[test]
fn an_attached_session() {
    let told = Told::default();
    let (status, _) = status_of(
        json!({
            "runtimeMode": "online",
            "endpoint": "",
            "session": { "request": "attach", "endpoint": "tcp://10.0.0.2:9901", "authToken": "t" },
        }),
        &told,
    );
    assert_eq!(status["runtimeState"], "connected");
    assert_eq!(status["endpoint"], "tcp://10.0.0.2:9901");
    assert_eq!(status["endpointConfigured"], false);
    assert_eq!(status["forceRequiresArm"], true);
    assert_eq!(status["access"]["allowWrite"], false);
    assert_eq!(
        status["access"]["reason"],
        "Write/force permissions are unknown — reconnect with an engineer token."
    );
}

/// An online target without a session is probed; when it answers, its `status` gives its access
/// and its `ads.status` comes back, and it stays not connected; when it does not, nothing more is
/// asked and Live Values says to connect with an engineer's token. With its endpoint switched off
/// it is not probed, and the status says so (specification 25 §5.1). Catches requests sent to a
/// runtime that does not answer, a reachable runtime without a session shown as running, and a
/// runtime no probe asked reported as if its probe had failed.
#[test]
fn an_online_target_without_a_session() {
    let answering = Told {
        status: Some(json!({ "access": { "role": "viewer", "io": {} } })),
        ads: Some(json!({ "routes": [] })),
        reachable: true,
        ..Told::default()
    };
    let arguments =
        json!({ "runtimeMode": "online", "endpoint": " tcp://10.0.0.2:9901 ", "authToken": "t" });
    let (status, ads) = status_of(arguments.clone(), &answering);
    assert_eq!(status["endpointProbed"], true);
    assert_eq!(status["endpointReachable"], true);
    assert_eq!(status["runtimeState"], "stopped");
    assert_eq!(status["running"], false);
    assert_eq!(status["access"]["role"], "viewer");
    assert_eq!(status["simulated"], false);
    assert_eq!(status["forceRequiresArm"], true);
    assert_eq!(ads, Some(json!({ "routes": [] })));
    let silent = Told::default();
    let (status, ads) = status_of(arguments, &silent);
    assert_eq!(
        silent.asked.borrow().as_slice(),
        ["probe tcp://10.0.0.2:9901"]
    );
    assert_eq!(status["endpointProbed"], true);
    assert_eq!(status["endpointReachable"], false);
    assert_eq!(
        status["access"]["reason"],
        "Connect with an engineer token to write or force."
    );
    assert_eq!(ads, None);
    let disabled = Told::default();
    let (status, _) = status_of(
        json!({ "runtimeMode": "online", "endpoint": "tcp://10.0.0.2:9901", "endpointEnabled": false }),
        &disabled,
    );
    assert!(disabled.asked.borrow().is_empty());
    assert_eq!(status["endpointProbed"], false);
}

/// The project's own runtime is probed at the endpoint trust-runtime reports for it whatever the
/// target, so the Target picker offers it while it runs outside VS Code (specification 25 §5.1);
/// while the online target is that runtime, the target's probe stands for it; without an endpoint
/// nothing is probed. Catches a picker that never offers the project's runtime while the user
/// simulates, a second probe of the same endpoint in one refresh, and a probe of no endpoint.
#[test]
fn the_project_runtime_is_probed() {
    let project = "unix:///home/user/tank/.trust-runtime/control.sock";
    let told = Told {
        reachable: true,
        ..Told::default()
    };
    let (status, _) = status_of(
        json!({ "runtimeMode": "simulate", "endpoint": "tcp://10.0.0.2:9901", "projectEndpoint": project }),
        &told,
    );
    assert_eq!(*told.asked.borrow(), [format!("probe {project}")]);
    assert_eq!(status["projectEndpoint"], project);
    assert_eq!(status["projectReachable"], true);
    assert_eq!(status["endpointProbed"], false);
    assert_eq!(status["endpointReachable"], false);

    let told = Told::default();
    let (status, _) = status_of(
        json!({ "runtimeMode": "simulate", "projectEndpoint": project }),
        &told,
    );
    assert_eq!(status["projectReachable"], false);

    let told = Told {
        reachable: true,
        ..Told::default()
    };
    let (status, _) = status_of(
        json!({ "runtimeMode": "online", "endpoint": project, "projectEndpoint": format!(" {project} ") }),
        &told,
    );
    let probes = told
        .asked
        .borrow()
        .iter()
        .filter(|asked| asked.starts_with("probe"))
        .count();
    assert_eq!(probes, 1);
    assert_eq!(status["projectEndpoint"], project);
    assert_eq!(status["projectReachable"], true);
    assert_eq!(status["endpointReachable"], true);

    let told = Told {
        reachable: true,
        ..Told::default()
    };
    let (status, _) = status_of(
        json!({ "runtimeMode": "simulate", "projectEndpoint": " " }),
        &told,
    );
    assert!(told.asked.borrow().is_empty());
    assert_eq!(status["projectEndpoint"], "");
    assert_eq!(status["projectReachable"], false);
}

/// The simulation facts as the runtime states them, with the stricter default where it states
/// none (specification 35 A2.4 and B9.2): `simulated` only when stated, the time scale only while
/// simulation is enabled, no speed words at 1×. Catches a target made simulated by its simulation
/// flags alone, and a time scale read while simulation is off.
#[test]
fn the_simulation_facts_of_a_status() {
    assert!(target_facts(&json!({ "simulated": true })).simulated);
    assert!(
        !target_facts(
            &json!({ "simulated": false, "simulation_enabled": true, "simulation_only_io": true })
        )
        .simulated
    );
    assert!(
        !target_facts(&json!({ "simulation_enabled": true, "simulation_only_io": true })).simulated
    );
    assert!(!target_facts(&Value::Null).simulated);
    let facts = target_facts(&json!({
        "simulated": true,
        "simulation_enabled": true,
        "simulation_time_scale": 10,
        "simulation_achieved_time_scale": 9.5,
        "simulation_speed_text": "×9.5 of ×10",
        "simulation_speed_behind": true,
        "cycle_interval_ms": 20,
    }));
    assert_eq!(facts.time_scale, 10.0);
    assert_eq!(facts.achieved, Some(9.5));
    assert_eq!(facts.speed_text, "×9.5 of ×10");
    assert!(facts.speed_behind);
    assert_eq!(facts.cycle_interval_ms, Some(20.0));
    let off =
        target_facts(&json!({ "simulation_time_scale": 10, "simulation_achieved_time_scale": 9 }));
    assert_eq!(off.time_scale, 1.0);
    assert_eq!(off.achieved, None);
    let unusable = target_facts(
        &json!({ "simulation_enabled": true, "simulation_time_scale": -2, "cycle_interval_ms": 0 }),
    );
    assert_eq!(unusable.time_scale, 1.0);
    assert_eq!(unusable.cycle_interval_ms, None);
}

/// The speed read of a runtime that does not answer is `null`. Catches a speed shown for a
/// runtime that cannot be read.
#[test]
fn the_speed_of_a_runtime_that_does_not_answer() {
    let endpoint = super::super::fake_runtime::closed_endpoint();
    let unread = answer("speed", json!({ "endpoint": endpoint })).expect("an answer");
    assert_eq!(unread, json!({ "ok": true, "read": null }));
    let runtime = super::super::fake_runtime::FakeRuntime::answering(json!({
        "simulated": true,
        "simulation_enabled": true,
        "simulation_time_scale": 5,
        "state": "paused",
    }));
    let read = answer("speed", json!({ "endpoint": runtime.endpoint })).expect("an answer");
    assert_eq!(
        read["read"],
        json!({
            "simulated": true,
            "configured": 5.0,
            "achieved": null,
            "speedText": "",
            "speedBehind": false,
            "paused": true,
        })
    );
}
