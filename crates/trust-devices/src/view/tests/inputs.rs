//! What the view reads from a gather (`crate::inputs`): the simulator's start failures, and the
//! targets that did not answer in time (specification 25 §5.1, §5.6 and §6.5).

use serde_json::json;
use trust_runtime_client::TargetProbe;

use super::fixtures::{
    action, configured_project, gathered, host, managed, no_project, probed, project, request, row,
    runtime_named, unanswered, with_failed_start, NOW_MS,
};
use crate::gather::{ManagedState, Reach};
use crate::request::{DebugFailure, RunMode};
use crate::targets::TargetOrigin;
use crate::view::project as project_view;
use crate::view::types::{HostEffect, Op, Tone};

#[test]
fn stopped_local_socket_is_a_neutral_stopped_simulator() {
    let socket = "Runtime not reachable: unix:///tmp/trust-runtime-line-123.sock";
    for (kind, message) in [
        ("stale_runtime", socket),
        ("stale_runtime", "The local simulator is stopped."),
    ] {
        let view = project_view(
            &gathered(no_project(), Vec::new()),
            &request(with_failed_start(
                host(RunMode::Simulate, ""),
                kind,
                message,
            )),
            None,
            NOW_MS,
        );
        let simulator = runtime_named(&view, "Simulator");
        assert_eq!(
            simulator.state.word, "Stopped",
            "{message}: neutral, not red"
        );
        assert_eq!(simulator.state.tone, Tone::Neutral);
        assert!(
            simulator.inspector.primary[0].enabled,
            "Start stays available"
        );
        let json = serde_json::to_string(&view).expect("a view serializes");
        assert!(
            !json.contains("unix:///tmp/trust-runtime-line"),
            "no socket path leaks"
        );
    }

    // A real start failure is an error on the simulator, with its message and Start as the retry.
    let failed = project_view(
        &gathered(no_project(), Vec::new()),
        &request(with_failed_start(
            host(RunMode::Simulate, ""),
            "port_conflict",
            "The runtime port is already in use.",
        )),
        None,
        NOW_MS,
    );
    let simulator = runtime_named(&failed, "Simulator");
    assert_eq!(simulator.state.word, "Error");
    assert_eq!(simulator.state.tone, Tone::Error);
    assert_eq!(
        row(simulator, "State"),
        Some("Error · The runtime port is already in use.")
    );
    let retry = &simulator.inspector.primary[0];
    assert!(retry.enabled);
    assert_eq!(
        retry.op,
        Op::Host {
            effect: HostEffect::StartSimulator
        }
    );
    assert!(!crate::inputs::is_neutral_stop(&DebugFailure {
        kind: "port_conflict".to_owned(),
        message: socket.to_owned(),
    }));
}

#[test]
fn target_without_an_answer_in_time_is_never_unreachable() {
    // A configured runtime that has not answered yet is not drawn as unreachable this round.
    let late = project_view(
        &gathered(
            no_project(),
            vec![unanswered(TargetOrigin::Configured, "10.0.0.9:5510")],
        ),
        &request(host(RunMode::Simulate, "")),
        None,
        NOW_MS,
    );
    let names: Vec<&str> = late
        .computers
        .iter()
        .flat_map(|computer| &computer.runtimes)
        .map(|runtime| runtime.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["Simulator"],
        "no node says it is unreachable or stopped"
    );

    // A managed runtime that has not answered is `Status unavailable`, never stopped.
    let mut slow = managed("cell1", "tcp://127.0.0.1:9902", ManagedState::Running, None);
    slow.reach = Reach::Unanswered;
    let view = project_view(
        &gathered(no_project(), vec![slow]),
        &request(host(RunMode::Simulate, "")),
        None,
        NOW_MS,
    );
    let cell1 = runtime_named(&view, "cell1");
    assert_eq!(cell1.state.word, "Status unavailable");
    assert!(!cell1.inspector.primary[0].enabled);

    // The project's runtime in online mode whose endpoint has not answered: its status is
    // unavailable this round, and Connect waits.
    let view = project_view(
        &gathered(
            project(configured_project()),
            vec![unanswered(
                TargetOrigin::Project,
                "tcp://192.168.10.20:5680",
            )],
        ),
        &request(host(RunMode::Online, "tcp://192.168.10.20:5680")),
        None,
        NOW_MS,
    );
    let resource = runtime_named(&view, "RESOURCE");
    assert_eq!(resource.state.word, "Status unavailable");
    let connect = action(&resource.inspector.primary, "connect");
    assert!(!connect.enabled);
    assert_eq!(
        connect.reason.as_deref(),
        Some("Status unavailable — the runtime did not answer in time; it is asked again.")
    );

    // An answer from the project's endpoint is that runtime's own, even when the configuration
    // names another endpoint: a runtime that keeps control offers its program's Start.
    let view = project_view(
        &gathered(
            project(configured_project()),
            vec![probed(
                TargetOrigin::Project,
                "tcp://192.168.10.20:5680",
                None,
                TargetProbe::Reachable(json!({
                    "state": "stopped", "program_control": "keeps_control",
                    "access": { "role": "admin" },
                })),
            )],
        ),
        &request(host(RunMode::Online, "tcp://192.168.10.20:5680")),
        None,
        NOW_MS,
    );
    let resource = runtime_named(&view, "RESOURCE");
    assert!(action(&resource.inspector.primary, "program_start").enabled);
}
