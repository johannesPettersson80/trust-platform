//! The connections and their inspectors (specification 25 §5.4, §5.5, §5.7, §5.8 and §5.13),
//! ported from `network-canvas.test.ts` and `ads-multiport-live-values.test.ts` with their texts.

use serde_json::{json, Value};

use super::fixtures::host;
use super::peers::{
    connection, connector_answer, endpoint_of, headed, pairs, peer, peer_view, plain, topology_of,
    value, view_of, PEER,
};
use crate::request::RunMode;
use crate::texts::connections::{protocol_name, signals};
use crate::topology::tests::fixtures::fleet_topology;
use crate::view::types::Tone;

/// An endpoint of a server that `Line runtime` serves.
fn server(id: &str, protocol: &str, name: &str, params: Value, live: Option<Value>) -> Value {
    let mut server = endpoint_of(id, protocol, name, "connected", "Serving.");
    server["kind"] = json!("service");
    server["role"] = json!("server");
    server["params"] = params;
    if let Some(live) = live {
        server["live"] = live;
    }
    server
}

#[test]
fn connector_rows_use_product_words() {
    let view = peer_view(fleet_topology(), Some(connector_answer()), plain());
    let modbus = connection(&view, "Modbus meter");
    let mqtt = connection(&view, "MQTT broker");
    assert_eq!(
        pairs(headed(&modbus.inspector.sections, "Connector")),
        [
            ("Connection", "Ready"),
            ("Health", "OK"),
            ("Verification", "Confirmed"),
            ("Signals", "4 good"),
        ]
    );
    let words = [
        ("Connection", "Needs attention"),
        ("Health", "Degraded"),
        ("Verification", "Port reachable only"),
        ("Signals", "1 good, 2 need attention"),
    ];
    assert_eq!(pairs(headed(&mqtt.inspector.sections, "Connector")), words);
    for (label, word) in words {
        assert!(
            mqtt.hover
                .rows
                .iter()
                .any(|row| row.label == label && row.value == word),
            "the hover card shows {label} {word}"
        );
    }
    for row in &headed(&mqtt.inspector.sections, "Connector").rows {
        assert!(
            !row.value.contains('_') && !["stale", "degraded", "ok"].contains(&row.value.as_str()),
            "{} shows the raw token {}",
            row.label,
            row.value
        );
    }
}

#[test]
fn signals_combine_degraded_and_unavailable() {
    assert_eq!(signals(1, 2), "1 good, 2 need attention");
    assert_eq!(signals(4, 0), "4 good");
    let view = peer_view(fleet_topology(), Some(connector_answer()), plain());
    assert_eq!(
        value(
            &connection(&view, "MQTT broker").inspector.sections,
            "Signals"
        ),
        Some("1 good, 2 need attention"),
        "one degraded and one unavailable point are two that need attention"
    );
}

#[test]
fn secret_values_never_appear_in_any_row() {
    let mut mqtt = endpoint_of(
        "endpoint:runtime-a:mqtt",
        "mqtt",
        "Plant broker",
        "connected",
        "Connected.",
    );
    mqtt["params"] = json!({
        "broker": "10.0.0.7:1883", "username": "plc", "password": "hunter2-hidden", "tls": true,
    });
    let mut mesh = endpoint_of(
        "endpoint:runtime-a:mesh",
        "mesh",
        "Mesh / Zenoh",
        "connected",
        "Two peers.",
    );
    mesh["params"] = json!({
        "role": "peer", "auth_token": "tok-hidden-123",
        "subscribe": { "plant/level": "Level", "password": "nested-hidden" },
    });
    let mut opcua = endpoint_of(
        "endpoint:runtime-a:opcua_client",
        "opcua_client",
        "OPC UA client",
        "connected",
        "Reading.",
    );
    opcua["params"] = json!({ "connections": [{
        "name": "line-a", "endpoint_url": "opc.tcp://10.0.0.5:4840", "password": "pw-hidden",
        "points": [{ "var": "Level", "node_id": "ns=2;s=Level", "password": "point-hidden" }],
    }] });
    let view = peer_view(
        topology_of(vec![mqtt, mesh, opcua], Vec::new(), Vec::new()),
        None,
        plain(),
    );
    let shown = serde_json::to_string(&view).expect("a view serializes to JSON");
    for hidden in [
        "hunter2-hidden",
        "tok-hidden-123",
        "nested-hidden",
        "pw-hidden",
        "point-hidden",
    ] {
        assert!(!shown.contains(hidden), "the view shows {hidden}");
    }
    let broker = &connection(&view, "Plant broker").inspector.sections;
    assert_eq!(value(broker, "Password"), Some("••• (set)"));
    assert_eq!(value(broker, "Username"), Some("plc"));
    assert_eq!(value(broker, "TLS"), Some("On"), "a switch reads On or Off");
    assert_eq!(
        value(
            &connection(&view, "Mesh / Zenoh").inspector.sections,
            "Mesh token"
        ),
        Some("••• (set)")
    );
    assert_eq!(
        value(
            &connection(&view, "OPC UA client").inspector.sections,
            "Connections"
        ),
        Some("line-a · opc.tcp://10.0.0.5:4840\n- Level ← ns=2;s=Level"),
        "the points stay, their secrets do not"
    );
}

#[test]
fn server_summaries_say_where_and_what() {
    let opcua = server(
        "endpoint:runtime-a:opcua",
        "opcua",
        "OPC UA server",
        json!({
            "listen": "127.0.0.1:4840", "endpoint_path": "/trust",
            "expose": ["global.TankLevel", "global.PumpRunning"],
        }),
        None,
    );
    let ads = server(
        "endpoint:runtime-a:ads_server",
        "ads_server",
        "ADS server",
        json!({ "listen": "127.0.0.1:48898", "ams_net_id": "127.0.0.1.1.1", "ads_port": 851 }),
        Some(json!({ "value": { "connected_clients": 2 }, "last_seen_ms": 12 })),
    );
    let view = peer_view(
        topology_of(vec![opcua, ads], Vec::new(), Vec::new()),
        None,
        plain(),
    );
    let opcua = &connection(&view, "OPC UA server").inspector.sections;
    assert_eq!(
        value(opcua, "Server endpoint"),
        Some("opc.tcp://127.0.0.1:4840/trust")
    );
    assert_eq!(
        value(opcua, "Exposed globals"),
        Some("2 globals: global.TankLevel, global.PumpRunning")
    );
    let ads = &connection(&view, "ADS server").inspector.sections;
    assert_eq!(
        value(ads, "Server endpoint"),
        Some("127.0.0.1:48898 · AMS Net ID 127.0.0.1.1.1 · ADS port 851")
    );
    assert_eq!(value(ads, "Connected clients"), Some("2 clients connected"));
    assert_eq!(
        value(ads, "Verification"),
        Some("Self-test available; no external client verified")
    );
    assert_eq!(value(ads, "Writes enabled"), Some("Off"));
    assert_eq!(value(ads, "Enabled"), Some("On"));
    assert!(
        !ads[0].rows.iter().any(|row| row.label.starts_with("Max ")),
        "the ADS server's summary keeps its allowlist: no transport limits"
    );
}

#[test]
fn ads_server_client_count_and_verification_survive() {
    let live = |proof: &str| {
        json!({
            "value": {
                "connected_clients": 3, "proof_status": proof, "external_client_verified": true,
                "external_client_kind": "loopback-ads-client",
                "external_client_name": "trust-runtime-doctor",
            },
            "last_seen_ms": 99,
        })
    };
    let params = json!({
        "listen": "127.0.0.1:48898", "ams_net_id": "127.0.0.1.1.1", "ads_port": 851,
        "expose": ["global.TankLevel"],
        "clients_summary": ["127.0.0.1.1.100 (from 127.0.0.1)", "5.23.91.12.1.1 (from 127.0.0.0/8)"],
    });
    let view_with = |live: Value| {
        let ads = server(
            "endpoint:runtime-a:ads_server",
            "ads_server",
            "ADS server",
            params.clone(),
            Some(live),
        );
        peer_view(
            topology_of(vec![ads], Vec::new(), Vec::new()),
            None,
            plain(),
        )
    };
    let verified = view_with(live("external_client_verified"));
    let rows = &connection(&verified, "ADS server").inspector.sections;
    assert_eq!(
        value(rows, "Connected clients"),
        Some("3 clients connected")
    );
    assert_eq!(
        value(rows, "Verification"),
        Some("Verified by loopback-ads-client trust-runtime-doctor")
    );
    assert_eq!(
        value(rows, "Allowed clients"),
        Some("127.0.0.1.1.100 (from 127.0.0.1); 5.23.91.12.1.1 (from 127.0.0.0/8)"),
        "the runtime's humanized summary, never raw client pins"
    );
    assert_eq!(
        value(rows, "Exposed globals"),
        Some("1 global: global.TankLevel")
    );

    let production = view_with(live("production_ready"));
    assert_eq!(
        value(
            &connection(&production, "ADS server").inspector.sections,
            "Verification"
        ),
        Some("Production verified by loopback-ads-client trust-runtime-doctor")
    );
    let text = view_with(json!({ "value": "{\"connected_clients\": 1}" }));
    assert_eq!(
        value(
            &connection(&text, "ADS server").inspector.sections,
            "Connected clients"
        ),
        Some("1 client connected"),
        "evidence sent as JSON text is read too"
    );
}

#[test]
fn local_io_titles_are_simulated_and_loopback() {
    assert_eq!(protocol_name("simulated"), "Simulated I/O");
    assert_eq!(protocol_name("loopback"), "Loopback I/O");
    assert_eq!(protocol_name("ads"), "ADS client");
    assert_eq!(protocol_name("ads_server"), "ADS server");
    let topology = topology_of(
        vec![
            endpoint_of(
                "endpoint:runtime-a:simulated",
                "simulated",
                "simulated",
                "connected",
                "Driver is healthy.",
            ),
            endpoint_of(
                "endpoint:runtime-a:loopback",
                "loopback",
                "lb",
                "connected",
                "Driver is healthy.",
            ),
        ],
        Vec::new(),
        Vec::new(),
    );
    let view = peer_view(topology, None, plain());
    for (name, protocol) in [("Simulated I/O", "simulated"), ("Loopback I/O", "loopback")] {
        let local = connection(&view, name);
        assert_eq!(local.protocol, protocol);
        assert_eq!(local.role_word, "I/O", "{name}: the band says I/O");
        assert!(!local.wire_port, "{name}: no wire leaves local I/O");
    }
}

#[test]
fn ads_client_view_lists_devices_ports_and_status() {
    let topology = |detail: &str| {
        let mut ads = endpoint_of(
            "endpoint:runtime-a:ads",
            "ads",
            "ADS client",
            "configured_policy",
            detail,
        );
        ads["kind"] = json!("peer");
        ads["role"] = json!("client");
        ads["supports_test"] = json!(true);
        ads["params"] = json!({ "connections": [
            { "name": "internal_port_301", "host": "192.168.77.11",
              "target_net_id": "100.67.6.217.1.1", "ams_port": 301,
              "points": [{ "symbol": "Task.Input" }] },
            { "name": "internal_port_851", "host": "192.168.77.11",
              "target_net_id": "100.67.6.217.1.1", "ams_port": 851,
              "points": [{ "symbol": "GVL.Read" }, { "symbol": "GVL.Write" }] },
        ] });
        topology_of(vec![ads], Vec::new(), Vec::new())
    };
    let stopped = view_of(
        vec![peer(
            PEER,
            "stopped",
            topology("Configured · runtime is not running."),
            None,
        )],
        host(RunMode::Simulate, ""),
        plain(),
    );
    let client = connection(&stopped, "ADS client");
    assert_eq!(client.inspector.kind, "ADS connection");
    assert_eq!(
        pairs(&client.inspector.sections[0]),
        [
            ("State", "Runtime stopped — start it to read tags."),
            ("Address", "192.168.77.11"),
            ("AMS Net ID", "100.67.6.217.1.1"),
            ("ADS port 301", "1 tag"),
            ("ADS port 851", "2 tags"),
        ]
    );
    assert_eq!(
        client.inspector.sections[0].rows[0].tone,
        Some(Tone::Neutral)
    );
    assert_eq!(
        pairs(headed(&client.inspector.sections, "Advanced settings")),
        [
            ("Enabled", "On"),
            ("Config file", "ads.toml"),
            ("Update interval", "20 ms"),
        ]
    );
    let actions: Vec<(&str, &str, bool)> = client
        .inspector
        .primary
        .iter()
        .map(|action| (action.id.as_str(), action.label.as_str(), action.enabled))
        .collect();
    assert_eq!(
        actions,
        [
            ("edit", "Edit advanced settings", true),
            ("test", "Test", true),
            ("diagnose", "Diagnose", true),
        ]
    );

    let running = peer_view(
        topology("Configured in project files; restart the runtime to apply this change."),
        None,
        plain(),
    );
    assert_eq!(
        value(
            &connection(&running, "ADS client").inspector.sections,
            "State"
        ),
        Some("Runtime running — ADS is configured."),
        "the ADS inspector must not call a running runtime stopped"
    );
}
