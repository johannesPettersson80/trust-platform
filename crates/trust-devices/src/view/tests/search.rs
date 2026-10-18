//! The search and the protocol filter (specification 25 §5.5 and §5.11), ported from
//! `network-canvas.test.ts` with its numbers and texts.

use serde_json::json;

use super::fixtures::{computer_headed, runtime_named};
use super::peers::{
    connection, connections, device, endpoint_of, filtered, link_with, peer_view, plain, searched,
    topology_of,
};
use crate::topology::tests::fixtures::fleet_topology;
use crate::view::types::{MeshBus, Severity, Tone};

#[test]
fn search_dims_without_hiding_problems_from_the_rollup() {
    let view = peer_view(fleet_topology(), None, searched("modbus"));
    let runtime = runtime_named(&view, "Line runtime");
    assert_eq!(
        (runtime.state.word.as_str(), runtime.state.tone),
        ("Degraded", Tone::Warning),
        "the rollup still holds the dimmed MQTT connection"
    );
    assert!(!runtime.dimmed, "the runtime holds a match");
    let computer = computer_headed(&view, "Computer 192.168.77.10");
    assert_eq!(computer.reachability.word, "Reachable");
    assert!(!computer.dimmed);
    assert!(!connection(&view, "Modbus meter").dimmed);
    let mqtt = connection(&view, "MQTT broker");
    assert!(mqtt.dimmed, "a connection that does not match is dimmed");
    assert_eq!(mqtt.dot, Tone::Warning, "its raw health stays");
    assert!(device(&view, "MQTT broker").dimmed, "so is its counterpart");
    assert!(link_with(&view, "Configured").dimmed, "and its wire");
    let warnings: Vec<_> = view
        .issues
        .groups
        .iter()
        .flat_map(|group| &group.issues)
        .filter(|issue| issue.severity == Severity::Warning)
        .map(|issue| issue.text.as_str())
        .collect();
    assert_eq!(
        warnings,
        [
            "MQTT broker: Broker connection refused.",
            "Mesh / Zenoh: One configured peer."
        ],
        "the search hides no warning"
    );
    assert_eq!(
        (view.search.query.as_str(), view.search.matches),
        ("modbus", 3),
        "the connection, its runtime and its computer match"
    );
}

#[test]
fn search_covers_signals_and_issues() {
    let mut opcua = endpoint_of(
        "endpoint:runtime-a:opcua_client",
        "opcua_client",
        "OPC UA client",
        "connected",
        "Reading.",
    );
    opcua["params"] = json!({ "connections": [{
        "name": "line-a", "endpoint_url": "opc.tcp://10.0.0.5:4840",
        "points": [{ "var": "TankLevel", "node_id": "ns=2;s=Tank", "type": "REAL", "access": "read" }],
    }] });
    let modbus = endpoint_of(
        "endpoint:runtime-a:modbus_tcp",
        "modbus_tcp",
        "Modbus meter",
        "connected",
        "Driver is healthy.",
    );
    let topology = topology_of(vec![opcua, modbus], Vec::new(), Vec::new());
    let signal = peer_view(topology, None, searched("tanklevel"));
    assert!(
        !connection(&signal, "OPC UA client").dimmed,
        "a mapped signal matches its connection, ignoring case"
    );
    assert!(connection(&signal, "Modbus meter").dimmed);

    let invalid = json!({ "connectors": [{
        "connector_id": "io:mqtt", "protocol": "mqtt", "kind": "process_image", "state": "ready",
        "health": "certainly_healthy", "confidence": "confirmed",
    }] });
    let issue = peer_view(
        fleet_topology(),
        Some(invalid),
        searched("certainly_healthy"),
    );
    assert!(
        !connection(&issue, "MQTT broker").dimmed,
        "the text of an issue matches its item"
    );
    assert!(connection(&issue, "Modbus meter").dimmed);

    let link = peer_view(fleet_topology(), None, searched("io.toml"));
    assert!(
        !link_with(&link, "Configured").dimmed,
        "a link's detail matches it"
    );
    assert!(
        !connection(&link, "MQTT broker").dimmed,
        "a matching link lights the connection it leaves"
    );
    assert!(
        !device(&link, "MQTT broker").dimmed,
        "and the system it reaches"
    );
    assert!(connection(&link, "Modbus meter").dimmed);

    let empty = peer_view(fleet_topology(), None, searched("  "));
    assert_eq!(empty.search.matches, 0);
    assert!(
        connections(&empty)
            .iter()
            .all(|connection| !connection.dimmed)
            && empty.devices.iter().all(|device| !device.dimmed)
            && empty.links.iter().all(|link| !link.dimmed),
        "an empty search dims nothing"
    );
}

#[test]
fn protocol_filter_reports_hidden_problems() {
    let view = peer_view(fleet_topology(), None, filtered(&["mqtt"]));
    assert!(connection(&view, "MQTT broker").hidden);
    assert!(!connection(&view, "Modbus meter").hidden);
    assert!(
        link_with(&view, "Configured").hidden,
        "the link of a hidden connection is hidden"
    );
    assert!(
        device(&view, "MQTT broker").hidden,
        "a system that only hidden links reach is hidden"
    );
    assert!(!device(&view, "tcp/192.168.77.11:7447").hidden);
    let filter = &view.filter;
    assert_eq!(
        (
            filter.hidden_connections,
            filter.hidden_attention,
            filter.hidden_errors,
            filter.hidden_warnings
        ),
        (1, 1, 0, 1)
    );
    let lines: Vec<_> = filter
        .lines
        .iter()
        .map(|line| (line.text.as_str(), line.tone))
        .collect();
    assert_eq!(
        lines,
        [
            ("1 connection hidden.", Tone::Neutral),
            ("1 hidden item needs attention.", Tone::Warning),
            ("0 errors · 1 warning", Tone::Warning),
        ]
    );
    let protocols: Vec<_> = filter
        .protocols
        .iter()
        .map(|protocol| {
            (
                protocol.id.as_str(),
                protocol.label.as_str(),
                protocol.count,
                protocol.hidden,
            )
        })
        .collect();
    assert_eq!(
        protocols,
        [
            ("mesh", "Mesh", 1, false),
            ("modbus_tcp", "Modbus TCP", 1, false),
            ("mqtt", "MQTT", 1, true),
        ]
    );
    assert_eq!(
        view.summary.text,
        "2 computers · 2 runtimes · 2 connections · 1 running · 0 faults · 2 warnings",
        "the summary counts the connections that the filter shows, and every issue"
    );

    let shown = peer_view(fleet_topology(), None, plain());
    let lines: Vec<_> = shown
        .filter
        .lines
        .iter()
        .map(|line| line.text.as_str())
        .collect();
    assert_eq!(lines, ["All protocols are visible."]);

    let mesh = peer_view(fleet_topology(), None, filtered(&["mesh"]));
    assert_eq!(
        mesh.mesh,
        Vec::<MeshBus>::new(),
        "a bus without a shown connection is hidden"
    );
    assert!(
        device(&mesh, "tcp/192.168.77.11:7447").hidden,
        "and so is the peer that only it reaches"
    );
}
