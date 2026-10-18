//! The actions of a runtime (specification 25 §5.1, §5.11, §5.12 and §6.1, decision O4), ported
//! from `runtime-controls-contract.test.ts` where the TypeScript asserted them.

use serde_json::{json, Value};
use trust_runtime_client::{AuthFailure, TargetProbe};

use super::fixtures::{
    action, answered, attached, computer_headed, configured_project, gathered, host, keeps_control,
    managed, no_project, probed, project, report, request, row, runtime, runtime_named, status,
    with_simulator, NOW_MS, SIMULATOR,
};
use crate::gather::ManagedState;
use crate::request::{HostFacts, RunMode};
use crate::targets::TargetOrigin;
use crate::view::project as project_view;
use crate::view::types::{
    Action, DevicesView, Emphasis, HostEffect, ItemId, Op, Pane, Runtime, Tone, UiOp,
};

const CELL: &str = "tcp://192.168.10.31:5680";

fn view(targets: Vec<crate::gather::TargetReport>, host: HostFacts) -> DevicesView {
    project_view(
        &gathered(no_project(), targets),
        &request(host),
        None,
        NOW_MS,
    )
}

/// The view of the runtime `cell` at `CELL` on another computer, which answers `status` with
/// `status`.
fn remote(status: Value, host: HostFacts) -> DevicesView {
    let mut cell = runtime("cell", "cell", "connected", Vec::new());
    cell["control_endpoint"] = json!(CELL);
    view(
        vec![answered(
            TargetOrigin::Configured,
            CELL,
            None,
            status,
            report("host:cell", "cell", &["192.168.10.31"], vec![cell]),
        )],
        host,
    )
}

/// Whether an action is one of the program's Start and Stop.
fn program(action: &Action) -> bool {
    rust_action(action).is_some_and(|name| name.starts_with("runtime.program_"))
}

fn ids(actions: &[Action]) -> Vec<&str> {
    actions.iter().map(|action| action.id.as_str()).collect()
}

fn rust_action(action: &Action) -> Option<&str> {
    match &action.op {
        Op::Rust { action, .. } => Some(action),
        Op::Host { .. } | Op::Ui { .. } | Op::Search { .. } => None,
    }
}

fn effect(action: &Action) -> Option<&HostEffect> {
    match &action.op {
        Op::Host { effect } => Some(effect),
        Op::Rust { .. } | Op::Ui { .. } | Op::Search { .. } => None,
    }
}

/// The first primary action: the lifecycle action.
fn lifecycle(runtime: &Runtime) -> &Action {
    &runtime.inspector.primary[0]
}

#[test]
fn lifecycle_actions_per_origin() {
    simulator_starts_and_stops();
    managed_runtime_starts_and_stops_through_the_fleet();
    other_runtime_connects_and_disconnects();
}

/// The `Set as run target`, `Settings`, optional `Logs` and `Focus` of every runtime.
fn assert_secondary_actions(runtime: &Runtime, logs: bool, run_target: &str) {
    let secondary = ids(&runtime.inspector.secondary);
    assert_eq!(
        &secondary[..2],
        ["set_run_target", "settings"],
        "{}",
        runtime.name
    );
    assert_eq!(secondary.last(), Some(&"focus"), "{}", runtime.name);
    assert_eq!(
        secondary.contains(&"logs"),
        logs,
        "{}: Logs only with a log backend",
        runtime.name
    );
    assert_eq!(
        effect(action(&runtime.inspector.secondary, "set_run_target")),
        Some(&HostEffect::SetRunTarget {
            id: run_target.to_owned()
        }),
        "{}",
        runtime.name
    );
}

/// The simulator: the extension owns its process.
fn simulator_starts_and_stops() {
    let stopped = view(Vec::new(), host(RunMode::Simulate, ""));
    let simulator = runtime_named(&stopped, "Simulator");
    let start = lifecycle(simulator);
    assert_eq!(
        (start.id.as_str(), start.label.as_str()),
        ("start", "Start")
    );
    assert!(start.enabled);
    assert_eq!(
        start.emphasis,
        Emphasis::Primary,
        "only an enabled Start is filled"
    );
    assert_eq!(effect(start), Some(&HostEffect::StartSimulator));
    assert_secondary_actions(simulator, true, "simulator");

    let running = view(
        Vec::new(),
        with_simulator(host(RunMode::Simulate, ""), false),
    );
    let stop = lifecycle(runtime_named(&running, "Simulator"));
    assert_eq!((stop.id.as_str(), stop.label.as_str()), ("stop", "Stop"));
    assert_eq!(stop.emphasis, Emphasis::Secondary, "Stop is outlined");
    assert_eq!(effect(stop), Some(&HostEffect::StopSession));

    let starting = view(
        Vec::new(),
        with_simulator(host(RunMode::Simulate, ""), true),
    );
    let waiting = lifecycle(runtime_named(&starting, "Simulator"));
    assert_eq!(waiting.label, "Starting…");
    assert!(
        !waiting.enabled && waiting.reason.is_some(),
        "transitional: disabled, with its reason"
    );

    // The project's configured runtime is the simulator's while the project runs in it.
    let configured = project_view(
        &gathered(project(configured_project()), Vec::new()),
        &request(host(RunMode::Simulate, "")),
        None,
        NOW_MS,
    );
    let simulator = runtime_named(&configured, "Simulator");
    assert_eq!(
        effect(lifecycle(simulator)),
        Some(&HostEffect::StartSimulator)
    );
    assert!(
        simulator.run_target,
        "the simulator is the run target when nothing else is"
    );
}

/// A managed runtime: Start and Stop through the fleet lifecycle, never Connect.
fn managed_runtime_starts_and_stops_through_the_fleet() {
    let view = view(
        vec![
            managed("cell1", "tcp://127.0.0.1:9902", ManagedState::Stopped, None),
            managed("cell2", "tcp://127.0.0.1:9903", ManagedState::Running, None),
            managed(
                "cell3",
                "tcp://127.0.0.1:9904",
                ManagedState::Starting,
                None,
            ),
            managed(
                "cell4",
                "tcp://127.0.0.1:9905",
                ManagedState::Stopping,
                None,
            ),
        ],
        host(RunMode::Simulate, ""),
    );
    for (name, id, label, enabled, operation) in [
        ("cell1", "start", "Start", true, "runtime.managed_start"),
        ("cell2", "stop", "Stop", true, "runtime.managed_stop"),
        (
            "cell3",
            "start",
            "Updating…",
            false,
            "runtime.managed_start",
        ),
        ("cell4", "stop", "Updating…", false, "runtime.managed_stop"),
    ] {
        let runtime = runtime_named(&view, name);
        let button = lifecycle(runtime);
        assert_eq!(
            (button.id.as_str(), button.label.as_str(), button.enabled),
            (id, label, enabled),
            "{name}"
        );
        assert_eq!(rust_action(button), Some(operation), "{name}");
        assert_eq!(button.enabled, button.reason.is_none(), "{name}");
        let primary = ids(&runtime.inspector.primary);
        assert!(
            !primary.contains(&"connect") && !primary.contains(&"disconnect"),
            "{name}"
        );
        assert_secondary_actions(runtime, true, name);
    }
}

/// A runtime on another computer: Connect, and Disconnect while attached. It never gets the
/// simulator's or a managed runtime's Start and Stop, and has no Logs without a log backend.
fn other_runtime_connects_and_disconnects() {
    let connect = remote(status("running", "admin"), host(RunMode::Online, ""));
    let cell = runtime_named(&connect, "cell");
    let session = lifecycle(cell);
    assert_eq!(
        (session.id.as_str(), session.label.as_str()),
        ("connect", "Connect")
    );
    assert!(session.enabled);
    assert_eq!(session.emphasis, Emphasis::Primary);
    assert_eq!(rust_action(session), Some("runtime.connect"));
    let disconnect = remote(
        status("running", "admin"),
        attached(host(RunMode::Online, ""), CELL),
    );
    let session = lifecycle(runtime_named(&disconnect, "cell"));
    assert_eq!(
        (session.id.as_str(), session.label.as_str()),
        ("disconnect", "Disconnect")
    );
    assert_eq!(rust_action(session), Some("runtime.disconnect"));

    for offered in cell
        .inspector
        .primary
        .iter()
        .chain(&cell.inspector.secondary)
    {
        assert!(
            !matches!(
                effect(offered),
                Some(HostEffect::StartSimulator | HostEffect::StopSession)
            ),
            "{}",
            offered.id
        );
        assert!(
            !rust_action(offered).is_some_and(|name| name.starts_with("runtime.managed_")),
            "{}",
            offered.id
        );
    }
    assert_secondary_actions(cell, false, CELL);
}

#[test]
fn connect_disabled_without_endpoint_has_a_reason() {
    // A runtime that another runtime's discovery reports, without a control endpoint.
    let mut topology = report(
        "host:cell",
        "cell",
        &["192.168.10.31"],
        vec![runtime("cell", "cell", "connected", Vec::new())],
    );
    topology["hosts"]
        .as_array_mut()
        .expect("hosts")
        .push(json!({
            "host_id": "host:seen", "hostname": "seen", "arch": "", "os": "", "ips": ["192.168.10.40"],
            "containers": [], "source": "discovery",
            "runtimes": [{
                "runtime_id": "runtime:seen", "name": "seen", "mode": "unknown", "cycle_ms": 0,
                "health": "unknown", "detail": "Seen by discovery.", "endpoints": [], "source": "discovery",
            }],
        }));
    let discovered = view(
        vec![answered(
            TargetOrigin::Configured,
            CELL,
            None,
            status("running", "admin"),
            topology,
        )],
        host(RunMode::Simulate, ""),
    );
    let connect = lifecycle(runtime_named(&discovered, "seen"));
    assert_eq!(connect.label, "Connect", "Connect stays visible");
    assert!(!connect.enabled);
    assert_eq!(
        connect.reason.as_deref(),
        Some(
            "This runtime has no control endpoint. Add its address with Connect existing runtime."
        )
    );

    // A configured runtime that does not answer keeps Connect, disabled with its recovery.
    let unreachable = view(
        vec![probed(
            TargetOrigin::Configured,
            "10.0.0.9:5510",
            Some("cell1"),
            TargetProbe::Unreachable,
        )],
        host(RunMode::Simulate, ""),
    );
    let connect = lifecycle(runtime_named(&unreachable, "cell1"));
    assert!(!connect.enabled);
    assert_eq!(
        connect.reason.as_deref(),
        Some("Not reachable — check that the runtime is running and that its address is right.")
    );

    // The project's runtime in online mode without a control endpoint.
    let online = project_view(
        &gathered(
            project(configured_project()),
            vec![probed(
                TargetOrigin::Project,
                "",
                None,
                TargetProbe::MissingEndpoint,
            )],
        ),
        &request(host(RunMode::Online, "")),
        None,
        NOW_MS,
    );
    let connect = lifecycle(runtime_named(&online, "RESOURCE"));
    assert!(!connect.enabled);
    assert!(connect
        .reason
        .as_deref()
        .is_some_and(|reason| reason.contains("no control endpoint")));
}

#[test]
fn managed_status_unavailable_keeps_start_disabled() {
    let view = view(
        vec![managed(
            "cell1",
            "tcp://127.0.0.1:9902",
            ManagedState::Unavailable,
            None,
        )],
        host(RunMode::Simulate, ""),
    );
    let cell1 = runtime_named(&view, "cell1");
    assert_eq!(
        cell1.state.word, "Status unavailable",
        "never shown as stopped"
    );
    let start = lifecycle(cell1);
    assert_eq!(
        (start.id.as_str(), start.label.as_str()),
        ("start", "Start"),
        "Start stays visible"
    );
    assert!(!start.enabled);
    assert_eq!(
        start.reason.as_deref(),
        Some("Status unavailable — refresh before starting.")
    );
    assert_eq!(
        start.emphasis,
        Emphasis::Quiet,
        "a disabled action is neutral"
    );
}

#[test]
fn inspector_health_has_the_cards_word_and_tone() {
    // A runtime that runs: its card says `Running`, and its inspector's health is the same word
    // in the same tone (specification 25 §5.11).
    let running = remote(status("running", "admin"), host(RunMode::Online, ""));
    let cell = runtime_named(&running, "cell");
    assert_eq!(cell.state.word, "Running");
    let health = cell
        .inspector
        .health
        .as_ref()
        .expect("a runtime has a health");
    assert_eq!((health.word.as_str(), health.tone), ("Running", Tone::Ok));

    // A runtime whose program faulted: `Faulted` in the error tone.
    let faulted = remote(status("faulted", "admin"), host(RunMode::Online, ""));
    let cell = runtime_named(&faulted, "cell");
    assert_eq!(cell.state.word, "Faulted");
    let health = cell
        .inspector
        .health
        .as_ref()
        .expect("a runtime has a health");
    assert_eq!(
        (health.word.as_str(), health.tone),
        ("Faulted", Tone::Error)
    );

    // A computer that does not answer: `Unreachable`, the word of its card's pill, as a warning.
    let unreachable = view(
        vec![probed(
            TargetOrigin::Configured,
            "10.0.0.9:5510",
            Some("cell1"),
            TargetProbe::Unreachable,
        )],
        host(RunMode::Simulate, ""),
    );
    let computer = computer_headed(&unreachable, "cell1");
    assert_eq!(computer.reachability.word, "Unreachable");
    let health = computer
        .inspector
        .health
        .as_ref()
        .expect("a computer has a health");
    assert_eq!(
        (health.word.as_str(), health.tone),
        ("Unreachable", Tone::Warning)
    );
}

#[test]
fn auth_failure_makes_credential_recovery_primary() {
    for (failure, detail) in [
        (
            AuthFailure::Missing,
            "No auth token provided — this runtime requires one.",
        ),
        (
            AuthFailure::Rejected,
            "Auth token rejected — check it and try again.",
        ),
    ] {
        let view = view(
            vec![probed(
                TargetOrigin::Configured,
                CELL,
                Some("cell"),
                TargetProbe::AuthFailed(failure),
            )],
            host(RunMode::Simulate, ""),
        );
        let cell = runtime_named(&view, "cell");
        assert_eq!(cell.state.word, "Authentication failed");
        assert!(
            row(cell, "State").is_some_and(|state| state.ends_with(detail)),
            "{failure:?}: the recovery text of the failure"
        );
        let primary = &cell.inspector.primary;
        assert_eq!(
            primary[0].id, "set_auth_token",
            "credential recovery comes first"
        );
        assert_eq!(primary[0].emphasis, Emphasis::Primary);
        assert_eq!(
            effect(&primary[0]),
            Some(&HostEffect::PromptToken {
                endpoint: CELL.to_owned()
            })
        );
        let retry = action(primary, "connect");
        assert!(retry.enabled, "Connect stays as the retry");
        assert_eq!(retry.emphasis, Emphasis::Secondary);
        let all = ids(primary);
        assert!(
            !all.contains(&"start") && !all.contains(&"stop"),
            "no process ownership is invented"
        );
        assert!(ids(&cell.inspector.secondary).contains(&"set_run_target"));
    }
}

#[test]
fn keeps_control_runtime_offers_program_start_and_stop() {
    // A runtime that reports `keeps_control` and answers with `stopped` keeps serving the control
    // connection: its program's Start is offered and sent as `restart` with the mode `cold`, the
    // mode a new process starts with.
    let stopped = remote(
        keeps_control(status("stopped", "admin")),
        host(RunMode::Online, ""),
    );
    let cell = runtime_named(&stopped, "cell");
    assert_eq!(cell.state.word, "Stopped");
    assert_eq!(
        lifecycle(cell).id,
        "connect",
        "the session stays Connect and Disconnect"
    );
    let start = action(&cell.inspector.primary, "program_start");
    assert_eq!(start.label, "Start");
    assert!(start.enabled);
    assert_eq!(start.emphasis, Emphasis::Primary);
    assert_eq!(
        start.op,
        Op::Rust {
            action: "runtime.program_start".to_owned(),
            payload: json!({
                "runtime": cell.id.as_str(), "endpoint": CELL, "params": { "mode": "cold" },
            }),
        }
    );
    assert!(!ids(&cell.inspector.primary).contains(&"program_stop"));

    // While its program runs, the program's Stop is offered and sent as `shutdown`.
    let running = remote(
        keeps_control(status("running", "admin")),
        host(RunMode::Online, ""),
    );
    let cell = runtime_named(&running, "cell");
    let stop = action(&cell.inspector.primary, "program_stop");
    assert_eq!(stop.label, "Stop");
    assert!(stop.enabled);
    assert_eq!(stop.emphasis, Emphasis::Secondary);
    assert_eq!(rust_action(stop), Some("runtime.program_stop"));
    assert!(!ids(&cell.inspector.primary).contains(&"program_start"));

    // A runtime that does not answer has no program actions: its existing launch path stays.
    let gone = view(
        vec![probed(
            TargetOrigin::Configured,
            CELL,
            Some("cell"),
            TargetProbe::Unreachable,
        )],
        host(RunMode::Online, ""),
    );
    let primary = ids(&runtime_named(&gone, "cell").inspector.primary);
    assert!(!primary.contains(&"program_start") && !primary.contains(&"program_stop"));
}

#[test]
fn program_stop_disabled_below_admin_with_reason() {
    let running = remote(
        keeps_control(status("running", "engineer")),
        host(RunMode::Online, ""),
    );
    let stop = action(
        &runtime_named(&running, "cell").inspector.primary,
        "program_stop",
    );
    assert!(!stop.enabled, "shutdown needs Admin (specification 11)");
    assert_eq!(
        stop.reason.as_deref(),
        Some("Stopping the program needs the Admin role; this connection has the Engineer role.")
    );
    assert_eq!(stop.emphasis, Emphasis::Quiet);

    let stopped = remote(
        keeps_control(status("stopped", "viewer")),
        host(RunMode::Online, ""),
    );
    let start = action(
        &runtime_named(&stopped, "cell").inspector.primary,
        "program_start",
    );
    assert!(!start.enabled, "restart needs Operator (specification 11)");
    assert_eq!(
        start.reason.as_deref(),
        Some("Starting the program needs the Operator role; this connection has the Viewer role.")
    );
    let operator = remote(
        keeps_control(status("stopped", "operator")),
        host(RunMode::Online, ""),
    );
    assert!(
        action(
            &runtime_named(&operator, "cell").inspector.primary,
            "program_start"
        )
        .enabled
    );
}

#[test]
fn process_runtime_on_another_computer_is_never_stopped_from_the_panel() {
    // `shutdown` ends the process of trust-runtime on another computer, and nothing in the panel
    // could start it again: in every state, whether it reports `process` or nothing, it offers
    // only its session (decision O4).
    for state in ["running", "stopped", "paused", "faulted"] {
        let mut process = status(state, "admin");
        process["program_control"] = json!("process");
        for answer in [status(state, "admin"), process] {
            let view = remote(answer, host(RunMode::Online, ""));
            let cell = runtime_named(&view, "cell");
            assert_eq!(
                ids(&cell.inspector.primary),
                ["connect", "live_values", "debug", "deploy"],
                "{state}"
            );
            assert!(
                !cell
                    .inspector
                    .primary
                    .iter()
                    .chain(&cell.inspector.secondary)
                    .chain(&cell.context_menu)
                    .any(program),
                "{state}: no program action"
            );
        }
    }

    // A runtime the panel launches keeps its launcher, whatever its status reports: the
    // simulator its debug session, a managed runtime its fleet lifecycle.
    let mut cell1 = runtime("cell1", "cell1", "connected", Vec::new());
    cell1["control_endpoint"] = json!("tcp://127.0.0.1:9902");
    let managed_view = view(
        vec![managed(
            "cell1",
            "tcp://127.0.0.1:9902",
            ManagedState::Running,
            Some((
                keeps_control(status("running", "admin")),
                report("host:line-pc", "line-pc", &["127.0.0.1"], vec![cell1]),
            )),
        )],
        host(RunMode::Simulate, ""),
    );
    let managed_runtime = runtime_named(&managed_view, "cell1");
    assert_eq!(
        rust_action(lifecycle(managed_runtime)),
        Some("runtime.managed_stop")
    );
    assert!(!managed_runtime.inspector.primary.iter().any(program));

    let mut simulator = runtime("RESOURCE", "RESOURCE", "simulate", Vec::new());
    simulator["control_endpoint"] = json!(SIMULATOR);
    let simulator_view = view(
        vec![answered(
            TargetOrigin::Simulator,
            SIMULATOR,
            None,
            keeps_control(status("running", "admin")),
            report("host:line-pc", "line-pc", &["127.0.0.1"], vec![simulator]),
        )],
        with_simulator(host(RunMode::Simulate, ""), false),
    );
    let simulator = runtime_named(&simulator_view, "Simulator");
    assert_eq!(effect(lifecycle(simulator)), Some(&HostEffect::StopSession));
    assert!(!simulator.inspector.primary.iter().any(program));
}

#[test]
fn deploy_disabled_with_the_section_6_1_text() {
    let view = view(
        vec![
            managed("cell1", "tcp://127.0.0.1:9902", ManagedState::Running, None),
            probed(
                TargetOrigin::Configured,
                CELL,
                Some("cell"),
                TargetProbe::Unreachable,
            ),
        ],
        with_simulator(host(RunMode::Simulate, ""), false),
    );
    let runtimes: Vec<&Runtime> = view
        .computers
        .iter()
        .flat_map(|computer| &computer.runtimes)
        .collect();
    assert_eq!(
        runtimes.len(),
        3,
        "the simulator, a managed runtime and a remote one"
    );
    for runtime in runtimes {
        let deploy = action(&runtime.inspector.primary, "deploy");
        assert_eq!(deploy.label, "Deploy", "{}", runtime.name);
        assert!(!deploy.enabled, "{}", runtime.name);
        assert_eq!(
            deploy.reason.as_deref(),
            Some("Deploy is not available for this target yet."),
            "{}",
            runtime.name
        );
        let primary = ids(&runtime.inspector.primary);
        let tail: Vec<&str> = primary.iter().rev().take(3).rev().copied().collect();
        assert_eq!(tail, ["live_values", "debug", "deploy"], "{}", runtime.name);
    }
}

#[test]
fn context_menu_has_its_four_entries() {
    let view = remote(status("running", "admin"), host(RunMode::Online, ""));
    let cell = runtime_named(&view, "cell");
    let labels: Vec<&str> = cell
        .context_menu
        .iter()
        .map(|action| action.label.as_str())
        .collect();
    assert_eq!(
        labels,
        [
            "Add device or connection…",
            "Set as run target",
            "Settings",
            "Focus"
        ]
    );
    assert_eq!(
        cell.context_menu[0].op,
        Op::Ui {
            ui: UiOp::Open {
                pane: Pane::Add {
                    runtime: Some(cell.id.clone()),
                    host: None,
                },
            },
        },
        "it opens + Add for the runtime, without choosing a protocol"
    );
    assert_eq!(
        cell.context_menu[3].op,
        Op::Ui {
            ui: UiOp::Focus {
                item: ItemId(cell.id.as_str().to_owned()),
            },
        }
    );
    assert!(cell.context_menu.iter().all(|action| action.enabled));
}
