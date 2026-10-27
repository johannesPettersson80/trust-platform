//! The devices and systems at the other end of the connections, the links and the mesh bus
//! (specification 25 §5.7 and §5.13), ported from `network-canvas.test.ts` with its texts.

use serde_json::{json, Value};

use super::peers::{
    connection, device, endpoint_of, external_of, link_of, link_with, pairs, peer_view, plain,
    topology_of,
};
use crate::topology::tests::fixtures::fleet_topology;
use crate::view::devices::link_arrow;
use crate::view::health;
use crate::view::types::{ArrowAt, DeviceKind, MeshBus, MeshDrop, Severity, Tone};

/// An endpoint of `Line runtime` with the role `role`.
fn endpoint_with_role(id: &str, protocol: &str, role: &str) -> Value {
    let mut endpoint = endpoint_of(id, protocol, protocol, "pending", "");
    endpoint["role"] = json!(role);
    endpoint
}

/// The one mesh bus of a view of `endpoints`.
fn mesh_of(endpoints: Vec<Value>) -> MeshBus {
    let view = peer_view(
        topology_of(endpoints, Vec::new(), Vec::new()),
        None,
        plain(),
    );
    assert_eq!(view.mesh.len(), 1, "one mesh bus");
    view.mesh[0].clone()
}

/// The drop of the item `item` of `bus`.
fn drop_of<'a>(bus: &'a MeshBus, item: &crate::view::types::ItemId) -> &'a MeshDrop {
    bus.drops
        .iter()
        .find(|member| &member.item == item)
        .unwrap_or_else(|| panic!("a drop of {}", item.as_str()))
}

#[test]
fn link_is_solid_only_for_connected_degraded_error() {
    for (status, proven) in [
        ("connected", true),
        ("degraded", true),
        ("error", true),
        ("pending", false),
        ("configured_policy", false),
        ("not_configured", false),
        ("disabled", false),
        ("unknown", false),
        ("", false),
    ] {
        assert_eq!(health::link_proven(status), proven, "{status:?}");
    }
    let statuses = [
        "pending",
        "connected",
        "degraded",
        "configured_policy",
        "error",
    ];
    let links = statuses
        .iter()
        .map(|status| {
            link_of(
                &format!("link:{status}"),
                "endpoint:runtime-a:ads",
                "external:ads",
                "ads",
                "client",
                status,
            )
        })
        .collect();
    let topology = topology_of(
        vec![endpoint_of(
            "endpoint:runtime-a:ads",
            "ads",
            "ADS",
            "connected",
            "",
        )],
        links,
        vec![external_of("external:ads", "ads server", "ads")],
    );
    let view = peer_view(topology, None, plain());
    for (word, proven, tone) in [
        ("Pending", false, Tone::Neutral),
        ("Connected", true, Tone::Ok),
        ("Degraded", true, Tone::Warning),
        ("Configured", false, Tone::Neutral),
        ("Error", true, Tone::Error),
    ] {
        let link = link_with(&view, word);
        assert_eq!((link.proven, link.tone), (proven, tone), "{word}");
    }
}

#[test]
fn unknown_future_link_status_stays_dashed() {
    assert!(!health::link_proven("future_status"));
    let topology = topology_of(
        vec![endpoint_of(
            "endpoint:runtime-a:ads",
            "ads",
            "ADS",
            "connected",
            "",
        )],
        vec![link_of(
            "link:future-status",
            "endpoint:runtime-a:ads",
            "external:ads",
            "ads",
            "client",
            "future_status",
        )],
        vec![external_of("external:ads", "ads server", "ads")],
    );
    let view = peer_view(topology, None, plain());
    let link = link_with(&view, "Future Status");
    assert!(
        !link.proven,
        "an unrecognized status fails closed as unproven"
    );
    assert_eq!(link.tone, Tone::Neutral, "and is never green");
    let bus = mesh_of(vec![endpoint_of(
        "endpoint:mesh-future",
        "mesh",
        "Mesh (future status)",
        "future_status",
        "",
    )]);
    assert!(
        bus.draft,
        "a mesh peer of an unknown status keeps the bus a draft"
    );
    assert!(!bus.drops[0].proven);
}

#[test]
fn mesh_draft_unless_every_peer_proven() {
    let mixed = mesh_of(vec![
        endpoint_of("endpoint:mesh-draft", "mesh", "Mesh (draft)", "pending", ""),
        endpoint_of("endpoint:mesh-live", "mesh", "Mesh (live)", "connected", ""),
        endpoint_of(
            "endpoint:mesh-future",
            "mesh",
            "Mesh (future status)",
            "future_status",
            "",
        ),
    ]);
    assert!(mixed.draft, "a mixed draft/live fabric stays a draft");
    assert_eq!(mixed.drops.len(), 3);
    for member in &mixed.drops {
        assert!(!member.proven, "every drop of a draft fabric is dashed");
        assert_eq!(member.tone, Tone::Muted, "a draft fabric is never green");
    }

    let proven_endpoints = vec![
        endpoint_of(
            "endpoint:mesh-degraded",
            "mesh",
            "Mesh (degraded)",
            "degraded",
            "Mesh peer latency is elevated.",
        ),
        endpoint_of(
            "endpoint:mesh-error",
            "mesh",
            "Mesh (error)",
            "error",
            "Mesh peer authentication failed.",
        ),
    ];
    let view = peer_view(
        topology_of(proven_endpoints, Vec::new(), Vec::new()),
        None,
        plain(),
    );
    let bus = &view.mesh[0];
    assert!(!bus.draft, "an all-proven degraded/error mesh is solid");
    let degraded = drop_of(bus, &connection(&view, "Mesh (degraded)").id);
    assert_eq!(
        (
            degraded.proven,
            degraded.tone,
            degraded.hover_text.as_deref()
        ),
        (
            true,
            Tone::Warning,
            Some("Degraded — Mesh peer latency is elevated.")
        )
    );
    let error = drop_of(bus, &connection(&view, "Mesh (error)").id);
    assert_eq!(
        (error.proven, error.tone, error.hover_text.as_deref()),
        (
            true,
            Tone::Error,
            Some("Error — Mesh peer authentication failed.")
        )
    );

    let view = peer_view(fleet_topology(), None, plain());
    let bus = &view.mesh[0];
    assert!(!bus.draft);
    let peer = drop_of(bus, &device(&view, "tcp/192.168.77.11:7447").id);
    assert_eq!(
        (peer.proven, peer.tone, peer.hover_text.as_deref()),
        (
            true,
            Tone::Warning,
            Some("Degraded — tcp/192.168.77.11:7447")
        ),
        "the configured peer keeps its status and detail on the shared fabric"
    );
}

#[test]
fn single_peer_mesh_has_no_label() {
    let single = mesh_of(vec![endpoint_of(
        "endpoint:mesh-draft",
        "mesh",
        "Mesh (draft)",
        "pending",
        "",
    )]);
    assert_eq!(
        single.label, None,
        "a one-peer fabric has no redundant label"
    );
    assert_eq!(single.drops.len(), 1);
    let several = mesh_of(vec![
        endpoint_of("endpoint:mesh-a", "mesh", "Mesh A", "connected", ""),
        endpoint_of("endpoint:mesh-b", "mesh", "Mesh B", "connected", ""),
    ]);
    assert_eq!(several.label.as_deref(), Some("Mesh fabric"));
}

#[test]
fn external_counterparts_use_product_names() {
    let topology = topology_of(
        vec![
            endpoint_with_role("endpoint:modbus", "modbus_tcp", "client"),
            endpoint_with_role("endpoint:opcua-client", "opcua_client", "client"),
            endpoint_with_role("endpoint:opcua-server", "opcua", "server"),
            endpoint_with_role("endpoint:ads-client", "ads", "client"),
        ],
        vec![
            link_of(
                "link:modbus",
                "endpoint:modbus",
                "external:modbus",
                "modbus_tcp",
                "client",
                "configured_policy",
            ),
            link_of(
                "link:opcua-client",
                "endpoint:opcua-client",
                "external:opcua:server",
                "opcua_client",
                "client",
                "configured_policy",
            ),
            link_of(
                "link:opcua-server",
                "endpoint:opcua-server",
                "external:opcua:client",
                "opcua",
                "server",
                "connected",
            ),
            link_of(
                "link:ads-client",
                "endpoint:ads-client",
                "external:ads:server",
                "ads",
                "client",
                "configured_policy",
            ),
        ],
        vec![
            external_of("external:modbus", "modbus_tcp 127.0.0.1:502", "modbus_tcp"),
            external_of(
                "external:opcua:server",
                "OPC UA server line-a",
                "opcua_client",
            ),
            external_of("external:opcua:client", "External client", "opcua"),
            external_of("external:ads:server", "ads server", "ads"),
        ],
    );
    let view = peer_view(topology, None, plain());
    for (name, presents) in [
        ("Modbus TCP 127.0.0.1:502", "Modbus TCP server"),
        ("OPC UA server line-a", "OPC UA server"),
        ("External client", "OPC UA client"),
        ("ADS server", "ADS server"),
    ] {
        assert_eq!(device(&view, name).presents, presents, "{name}");
    }
    assert!(
        !view
            .devices
            .iter()
            .any(|device| ["OPC UA client server", "ADS client server"]
                .contains(&device.presents.as_str())),
        "a client link names the remote server, not the client"
    );
}

#[test]
fn link_arrow_follows_direction_and_lock_follows_secure() {
    assert_eq!(link_arrow("outbound"), ArrowAt::To);
    assert_eq!(link_arrow(" Inbound "), ArrowAt::From);
    for direction in ["publish_subscribe", "observed", "policy", ""] {
        assert_eq!(link_arrow(direction), ArrowAt::None, "{direction:?}");
    }
    let mut topology = fleet_topology();
    topology["links"]
        .as_array_mut()
        .expect("links")
        .push(json!({
            "id": "link:modbus", "from": "endpoint:runtime-a:modbus_tcp",
            "to": "external:modbus:10.0.0.9", "protocol": "modbus_tcp", "role": "client",
            "direction": "outbound", "same_host": false, "status": "connected", "secure": true,
            "detail": "10.0.0.9:502",
        }));
    topology["external"]
        .as_array_mut()
        .expect("external systems")
        .push(json!({
            "id": "external:modbus:10.0.0.9", "kind": "device", "name": "10.0.0.9:502",
            "via_protocol": ["modbus_tcp"], "direction": "outbound",
        }));
    let view = peer_view(topology, None, plain());
    let modbus = link_with(&view, "Connected");
    assert_eq!(
        (
            modbus.arrow,
            modbus.secure,
            modbus.protocol_label.as_str(),
            modbus.hover_text.as_deref()
        ),
        (
            ArrowAt::To,
            true,
            "Modbus TCP",
            Some("Connected — 10.0.0.9:502")
        )
    );
    let mqtt = link_with(&view, "Configured");
    assert_eq!(
        (mqtt.arrow, mqtt.secure),
        (ArrowAt::None, false),
        "publish/subscribe traffic has no one direction"
    );
}

#[test]
fn subdevices_are_device_cards_of_their_bus() {
    let mut bus = endpoint_of(
        "endpoint:runtime-a:ethercat",
        "ethercat",
        "EtherCAT bus",
        "connected",
        "Driver is healthy.",
    );
    bus["role"] = json!("master");
    bus["children"] = json!([
        { "id": "slave:1", "kind": "field_slave", "name": "EK1100", "slot": 1, "model": "EK1100",
          "health": "connected", "detail": "Operational." },
        { "id": "slave:2", "kind": "field_slave", "name": "Servo drive", "slot": 2,
          "model": "A6-EC400", "channels": 8, "health": "degraded", "detail": "Following error." },
    ]);
    let view = peer_view(
        topology_of(vec![bus], Vec::new(), Vec::new()),
        None,
        plain(),
    );
    let bus = connection(&view, "EtherCAT bus");
    assert_eq!(bus.role_word, "MASTER");
    let cards: Vec<_> = view
        .devices
        .iter()
        .filter(|device| device.kind == DeviceKind::SubDevice)
        .collect();
    assert_eq!(
        cards.iter().map(|card| &card.id).collect::<Vec<_>>(),
        bus.subdevices.iter().collect::<Vec<_>>(),
        "the bus names its devices, in their order"
    );
    for card in &cards {
        assert_eq!(card.owner.as_ref(), Some(&bus.id), "{}", card.name);
    }
    let terminal = device(&view, "EK1100");
    assert_eq!(
        (
            terminal.presents.as_str(),
            terminal.status.as_ref().map(|status| status.word.as_str())
        ),
        ("EK1100", Some("Connected"))
    );
    let drive = device(&view, "Servo drive");
    assert_eq!(drive.presents, "A6-EC400");
    assert_eq!(
        drive
            .status
            .as_ref()
            .map(|status| (status.word.as_str(), status.tone)),
        Some(("Degraded", Tone::Warning))
    );
    assert_eq!(
        pairs(&drive.inspector.sections[0]),
        [
            ("Model", "A6-EC400"),
            ("Slot", "2"),
            ("Channels", "8"),
            ("State", "Degraded"),
            ("Detail", "Following error."),
            ("Bus", "EtherCAT bus"),
        ]
    );
    let issues: Vec<_> = view
        .issues
        .groups
        .iter()
        .filter(|group| group.item.as_ref() == Some(&drive.id))
        .flat_map(|group| &group.issues)
        .map(|issue| (issue.text.as_str(), issue.severity))
        .collect();
    assert_eq!(
        issues,
        [("Servo drive: Following error.", Severity::Warning)],
        "the device's warning is listed once, under it"
    );
}
