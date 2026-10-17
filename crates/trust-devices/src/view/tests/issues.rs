//! The issues list, the summary strip and the banner (specification 25 §5.10 and §5.11), ported
//! from `network-canvas.test.ts` with its texts. The simulator's failed start appears once, not
//! twice as before.

use serde_json::json;
use trust_runtime_client::ControlError;

use super::fixtures::{
    answered, host, report, runtime, runtime_named, status, with_failed_start, with_simulator,
};
use super::peers::{peer, plain, view_of, PEER};
use crate::request::RunMode;
use crate::targets::TargetOrigin;
use crate::topology::tests::fixtures::fleet_topology;
use crate::view::types::{Action, HostEffect, Op, Pane, Severity, Tone, UiOp};

/// The start failure of the TypeScript test.
const PORT_IN_USE: &str = "The runtime port is already in use.";

fn ids(actions: &[Action]) -> Vec<&str> {
    actions.iter().map(|action| action.id.as_str()).collect()
}

#[test]
fn issues_list_every_fault_once_grouped_by_item() {
    let mut topology = fleet_topology();
    let modbus = &mut topology["hosts"][0]["runtimes"][0]["endpoints"][0];
    modbus["health"] = json!("error");
    modbus["detail"] = json!("Unit 1 does not answer.");
    let view = view_of(
        vec![peer(PEER, "running", topology, None)],
        with_failed_start(host(RunMode::Simulate, ""), "port_conflict", PORT_IN_USE),
        plain(),
    );
    let listed: Vec<_> = view
        .issues
        .groups
        .iter()
        .flat_map(|group| {
            group.issues.iter().map(move |issue| {
                (
                    group.item_name.as_str(),
                    issue.text.as_str(),
                    issue.severity,
                )
            })
        })
        .collect();
    assert_eq!(
        listed,
        [
            (
                "Simulator",
                "Simulator: The runtime port is already in use.",
                Severity::Error
            ),
            (
                "Modbus meter",
                "Modbus meter: Unit 1 does not answer.",
                Severity::Error
            ),
            (
                "MQTT broker",
                "MQTT broker: Broker connection refused.",
                Severity::Warning
            ),
            (
                "Mesh / Zenoh",
                "Mesh / Zenoh: One configured peer.",
                Severity::Warning
            ),
        ],
        "every fault once, grouped by item in the order of the canvas"
    );
    assert_eq!(
        runtime_named(&view, "Line runtime").state.word,
        "Error",
        "the runtime shows its failing connection, which raises the issue"
    );
    for group in &view.issues.groups {
        let item = group.item.clone().expect("each issue belongs to its item");
        for issue in &group.issues {
            assert_eq!(
                issue.show.as_ref().map(|show| &show.op),
                Some(&Op::Ui {
                    ui: UiOp::Focus { item: item.clone() }
                }),
                "{}: Show zooms to its item",
                issue.text
            );
        }
    }
    let retry = view.issues.groups[0].issues[0]
        .recovery
        .as_ref()
        .expect("the failed start's recovery");
    assert_eq!(
        (retry.label.as_str(), &retry.op),
        (
            "Retry",
            &Op::Host {
                effect: HostEffect::StartSimulator
            }
        )
    );
    let button = &view.issues.button;
    assert_eq!(
        (
            button.text.as_str(),
            button.tone,
            button.faults,
            button.warnings
        ),
        ("4 issues", Tone::Error, 2, 2)
    );
}

#[test]
fn issue_severity_has_its_word_and_tone() {
    // A runtime whose Modbus meter does not answer and whose MQTT broker connection is degraded,
    // with the simulator's failed start: a fault and a warning, each with the word and tone of
    // its severity (specification 25 §5.11).
    let mut topology = fleet_topology();
    let modbus = &mut topology["hosts"][0]["runtimes"][0]["endpoints"][0];
    modbus["health"] = json!("error");
    modbus["detail"] = json!("Unit 1 does not answer.");
    let view = view_of(
        vec![peer(PEER, "running", topology, None)],
        with_failed_start(host(RunMode::Simulate, ""), "port_conflict", PORT_IN_USE),
        plain(),
    );
    let listed: Vec<(&str, &str, Tone)> = view
        .issues
        .groups
        .iter()
        .flat_map(|group| &group.issues)
        .map(|issue| {
            (
                issue.text.as_str(),
                issue.severity_word.as_str(),
                issue.severity_tone,
            )
        })
        .collect();
    assert_eq!(
        listed,
        [
            (
                "Simulator: The runtime port is already in use.",
                "Fault",
                Tone::Error
            ),
            (
                "Modbus meter: Unit 1 does not answer.",
                "Fault",
                Tone::Error
            ),
            (
                "MQTT broker: Broker connection refused.",
                "Warning",
                Tone::Warning
            ),
            (
                "Mesh / Zenoh: One configured peer.",
                "Warning",
                Tone::Warning
            ),
        ],
        "each issue carries the word and tone of its own severity"
    );
}

#[test]
fn summary_counts_running_faults_and_warnings() {
    let packer = report(
        "host:packer",
        "packer",
        &["192.168.77.20"],
        vec![runtime("packer", "Packer", "stopped", Vec::new())],
    );
    let targets = || {
        vec![
            peer(PEER, "running", fleet_topology(), None),
            peer("tcp://192.168.77.20:5680", "stopped", packer.clone(), None),
        ]
    };
    let view = view_of(targets(), host(RunMode::Simulate, ""), plain());
    let summary = &view.summary;
    assert_eq!(
        (
            summary.computers,
            summary.runtimes,
            summary.connections,
            summary.running,
            summary.faults,
            summary.warnings
        ),
        (3, 3, 3, 1, 0, 2)
    );
    assert_eq!(
        summary.text,
        "3 computers · 3 runtimes · 3 connections · 1 running · 0 faults · 2 warnings"
    );
    assert_eq!(
        summary.open_issues.op,
        Op::Ui {
            ui: UiOp::Open { pane: Pane::Issues }
        },
        "the summary strip opens the issues list"
    );
    let simulating = view_of(
        targets(),
        with_simulator(host(RunMode::Simulate, ""), false),
        plain(),
    );
    assert_eq!(
        simulating.summary.text,
        "3 computers · 3 runtimes · 3 connections · 2 running · 0 faults · 2 warnings",
        "a running runtime counts while one of its connections is degraded"
    );
}

#[test]
fn banner_keeps_the_failed_start_and_the_unread_reports() {
    let failed = view_of(
        Vec::new(),
        with_failed_start(host(RunMode::Simulate, ""), "port_conflict", PORT_IN_USE),
        plain(),
    );
    let banner = failed.banner.as_ref().expect("a banner");
    assert_eq!(
        (banner.tone, banner.text.as_str(), ids(&banner.actions)),
        (Tone::Error, PORT_IN_USE, vec!["retry", "open_logs"])
    );

    let stopped = view_of(Vec::new(), host(RunMode::Simulate, ""), plain());
    let banner = stopped.banner.as_ref().expect("a banner");
    assert_eq!(
        (banner.tone, banner.text.as_str(), ids(&banner.actions)),
        (
            Tone::Neutral,
            "Simulator is stopped.",
            vec!["start_simulator"]
        )
    );
    let running = view_of(
        Vec::new(),
        with_simulator(host(RunMode::Simulate, ""), false),
        plain(),
    );
    let banner = running.banner.as_ref().expect("a banner");
    assert_eq!(
        (banner.text.as_str(), &banner.actions[0].op),
        (
            "Simulator running.",
            &Op::Host {
                effect: HostEffect::StopSession
            }
        )
    );

    let mut unread = answered(
        TargetOrigin::Configured,
        "tcp://192.168.77.12:5680",
        None,
        status("running", "admin"),
        json!({}),
    );
    unread.live.topology = Some(Err(ControlError::Transport("connection reset".to_owned())));
    let view = view_of(vec![unread], host(RunMode::Simulate, ""), plain());
    let text = "tcp://192.168.77.12:5680 topology: connection reset";
    let banner = view.banner.as_ref().expect("a banner");
    assert_eq!(
        (banner.tone, banner.text.clone()),
        (Tone::Error, format!("Peer topology degraded: {text}"))
    );
    let general = &view.issues.groups[0];
    assert_eq!(
        (
            general.item.as_ref(),
            general.item_name.as_str(),
            general.issues[0].text.as_str()
        ),
        (None, "General", text),
        "a report of no item is listed first, under General"
    );
}
