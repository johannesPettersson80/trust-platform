//! Native tests of the project mark on the Discover cards (specification 25 §2, the project mark;
//! specification 11, "Compare with the project"): `In the project`, `Not in the project`, `Differs from the
//! project` with its rows, `On canvas` for a `TwinCAT` device, and no mark where the comparison does not
//! cover or cannot judge a device. Each expected value is a literal worked out from the specification.

use std::collections::BTreeSet;

use serde_json::{json, Map, Value};
use trust_runtime::search::project_choices::Memory;

use crate::session::{KnownConnection, KnownRuntime};
use crate::view::discover::{project_in, RuntimeOrigin, Scan};
use crate::view::discover_compare::project_of;
use crate::view::discover_context::Context;
use crate::view::types::{DiscoverView, ItemId, Tone};

/// The time of the views, in milliseconds since the Unix epoch.
pub(super) const NOW_MS: u64 = 1_760_000_000_000;

/// A finished search status of `sources` and `devices`, as `comm.search.status` answers it.
pub(super) fn status(sources: Value, devices: Value) -> Map<String, Value> {
    let mut body = Map::new();
    body.insert("search_id".to_owned(), json!("s-1"));
    body.insert("state".to_owned(), json!("done"));
    body.insert("sources".to_owned(), sources);
    body.insert("devices".to_owned(), devices);
    body
}

/// A device of the id `id` at `addresses` with the one result `find`.
pub(super) fn device(id: &str, addresses: &[&str], find: &Value) -> Value {
    json!({
        "id": id,
        "addresses": addresses,
        "last_seen_ms": NOW_MS - 3000,
        "finds": [find]
    })
}

/// A `TwinCAT` system of the AMS Net ID `net_id` that answers on port 851.
pub(super) fn ads_device(net_id: &str, name: &str) -> Value {
    let mut system = device(
        &format!("dev:netid:{net_id}"),
        &["10.0.0.5"],
        &json!({
            "protocol": "ads", "source": "ads_identify", "confidence": "confirmed", "simulated": false,
            "identity": { "kind": "ads", "ams_net_id": net_id, "computer_name": name },
            "params": {
                "ams_net_id": net_id, "host": "10.0.0.5", "name": name, "responding_ads_ports": [851]
            },
            "warnings": []
        }),
    );
    system["ams_net_ids"] = json!([net_id]);
    system
}

/// A Modbus TCP device at `host`, port 502, unit 1, that reports `product_code`.
pub(super) fn modbus_device(host: &str, product_code: Option<&str>) -> Value {
    let mut identity = json!({ "kind": "modbus", "unit_id": 1 });
    if let Some(code) = product_code {
        identity["product_code"] = json!(code);
    }
    device(
        &format!("dev:ip:{host}"),
        &[host],
        &json!({
            "protocol": "modbus_tcp", "source": "modbus_device_id", "confidence": "confirmed",
            "simulated": false, "identity": identity,
            "params": { "address": format!("{host}:502"), "unit_id": 1 },
            "warnings": []
        }),
    )
}

/// An `EtherCAT` `SubDevice` named `model` at `position` of the adapter `eth1`.
pub(super) fn ethercat_device(position: u16, model: &str) -> Value {
    device(
        &format!("dev:ethercat:eth1:{position}"),
        &[],
        &json!({
            "protocol": "ethercat", "source": "ethercat_bus", "confidence": "confirmed",
            "simulated": false,
            "identity": {
                "kind": "ethercat", "adapter": "eth1", "slot": position, "configured_address": position,
                "alias": 0, "model": model, "channels": 8, "vendor_id": 2, "product_id": 0,
                "revision": 0, "serial": 0, "input_bytes": 0, "output_bytes": 1
            },
            "params": { "adapter": "eth1", "slot": position, "model": model },
            "warnings": []
        }),
    )
}

/// An OPC UA server at `host` with one endpoint on port 4840 that announces no security.
pub(super) fn opcua_device(host: &str) -> Value {
    device(
        &format!("dev:ip:{host}"),
        &[host],
        &json!({
            "protocol": "opcua", "source": "opcua_get_endpoints", "confidence": "confirmed",
            "simulated": false,
            "identity": {
                "kind": "opcua", "endpoint_url": format!("opc.tcp://{host}:4840"),
                "security_policy": "none", "security_mode": "none", "unsupported": false
            },
            "params": { "endpoint_url": format!("opc.tcp://{host}:4840") },
            "warnings": []
        }),
    )
}

/// A USB debug probe, a result that the comparison does not cover.
pub(super) fn usb_device() -> Value {
    device(
        "dev:usb:066DFF",
        &[],
        &json!({
            "protocol": "usb", "source": "usb_descriptor", "confidence": "confirmed", "simulated": false,
            "identity": {
                "kind": "usb", "vid": 1155, "pid": 14155, "role": "debug_probe",
                "model": "ST-LINK/V2-1"
            },
            "params": {},
            "warnings": []
        }),
    )
}

/// The runtime `runtime:cell`, ready to scan, as the origin of the views.
pub(super) fn cell() -> Vec<RuntimeOrigin> {
    vec![RuntimeOrigin {
        id: "runtime:cell".to_owned(),
        name: "Cell".to_owned(),
        ready: true,
    }]
}

/// A runtime of the panel with the connections `(protocol, settings)`.
pub(super) fn runtime_with(connections: Vec<(&str, Value)>) -> KnownRuntime {
    KnownRuntime {
        id: ItemId("runtime:cell".to_owned()),
        name: "Cell".to_owned(),
        control_endpoint: None,
        answers: true,
        project: true,
        catalog: None,
        connections: connections
            .into_iter()
            .enumerate()
            .map(|(index, (protocol, params))| KnownConnection {
                id: ItemId(format!("connection:{index}")),
                instance_id: None,
                protocol: protocol.to_owned(),
                params: params.as_object().cloned().unwrap_or_default(),
                disabled: false,
            })
            .collect(),
    }
}

pub(super) fn view_with(answer: &Map<String, Value>, context: &Context) -> DiscoverView {
    project_in(answer, &Scan::ThisHost, &cell(), context, NOW_MS)
        .expect("the status reads as a search status")
}

/// The ids of the cards of `view`, in order.
fn card_ids(view: &DiscoverView) -> Vec<&str> {
    view.cards.iter().map(|card| card.id.as_str()).collect()
}

/// Each card's id, its project mark with its tone, and its rows.
type Marked<'a> = (&'a str, Option<(&'a str, Tone)>, Vec<(&'a str, &'a str)>);

fn marks(view: &DiscoverView) -> Vec<Marked<'_>> {
    view.cards
        .iter()
        .map(|card| {
            (
                card.id.as_str(),
                card.project
                    .as_ref()
                    .map(|mark| (mark.word.as_str(), mark.tone)),
                card.differs
                    .iter()
                    .map(|row| (row.label.as_str(), row.value.as_str()))
                    .collect(),
            )
        })
        .collect()
}

#[test]
fn compare_marks_on_discover_cards() {
    // Specification 25 §2, the project mark: a card is `In the project` (tone ok) when a connection of the
    // canvas names its device, `On canvas` when it is a TwinCAT system that is, `Not in the project` (neutral)
    // when none does, and `Differs from the project` (warning) with one row for each fact that differs; a
    // device of a protocol the comparison does not cover carries no mark. The canvas holds a TwinCAT system,
    // a Modbus drive that states product code DRV-1, a Modbus meter that states nothing, and an EtherCAT
    // EL2008 at position 2. The search finds that system and another one, the drive reporting DRV-2, the
    // meter, an unknown Modbus device, the EL2008 at position 3, and a USB probe. A wrong implementation
    // marks the unknown device in the project, forgets the rows, marks the USB probe, or marks the moved
    // EL2008 as unknown.
    let known = [runtime_with(vec![
        (
            "ads",
            json!({ "connections": [{
                "name": "line1", "target_net_id": "5.6.7.8.1.1", "host": "10.0.0.5", "ams_port": 851
            }] }),
        ),
        (
            "modbus_tcp",
            json!({ "address": "10.0.0.5:502", "unit_id": 1, "product_code": "DRV-1" }),
        ),
        ("modbus_tcp", json!({ "address": "10.0.0.6:502" })),
        (
            "ethercat",
            json!({ "adapter": "eth1", "modules": [{ "model": "EL2008", "slot": 2 }] }),
        ),
    ])];
    let answer = status(
        json!([
            { "id": "ads", "state": "found", "count": 2 },
            { "id": "modbus_tcp", "state": "found", "count": 3 },
            { "id": "ethercat", "state": "found", "count": 1 },
            { "id": "usb", "state": "found", "count": 1 }
        ]),
        json!([
            ads_device("5.6.7.8.1.1", "PLC-A"),
            ads_device("9.9.9.9.1.1", "PLC-B"),
            modbus_device("10.0.0.5", Some("DRV-2")),
            modbus_device("10.0.0.6", None),
            modbus_device("10.0.0.7", None),
            ethercat_device(3, "EL2008"),
            usb_device()
        ]),
    );
    let context = Context {
        project: project_of(&known),
        ..Context::default()
    };

    let view = view_with(&answer, &context);

    assert_eq!(
        marks(&view),
        vec![
            (
                "dev:netid:5.6.7.8.1.1",
                Some(("On canvas", Tone::Ok)),
                vec![]
            ),
            (
                "dev:netid:9.9.9.9.1.1",
                Some(("Not in the project", Tone::Neutral)),
                vec![]
            ),
            (
                "dev:ip:10.0.0.5",
                Some(("Differs from the project", Tone::Warning)),
                vec![(
                    "Product code",
                    "The project has DRV-1; the device reports DRV-2."
                )]
            ),
            (
                "dev:ip:10.0.0.6",
                Some(("In the project", Tone::Ok)),
                vec![]
            ),
            (
                "dev:ip:10.0.0.7",
                Some(("Not in the project", Tone::Neutral)),
                vec![]
            ),
            (
                "dev:ethercat:eth1:3",
                Some(("Differs from the project", Tone::Warning)),
                vec![("Position", "The project has 2; the device reports 3.")]
            ),
            ("dev:usb:066DFF", None, vec![]),
        ]
    );
    // The TwinCAT card that the canvas holds offers `Manage tags`, and the other one `Add to canvas`.
    assert_eq!(view.cards[0].actions[0].label, "Manage tags");
    assert_eq!(view.cards[1].actions[0].label, "Add to canvas");

    // While the panel shows no runtime there is no project, and no card is marked.
    let unmarked = view_with(&answer, &Context::default());
    assert!(unmarked
        .cards
        .iter()
        .all(|card| card.project.is_none() && card.differs.is_empty()));
    assert_eq!(project_of(&[]), None);
}

#[test]
fn an_ignored_find_keeps_its_project_mark() {
    // Decision O14 and specification 25 §2: an ignored find leaves the list until `Show ignored`, and then its
    // card still says what the project makes of it. A wrong implementation drops the mark of an ignored card.
    let known = [runtime_with(vec![(
        "modbus_tcp",
        json!({ "address": "10.0.0.6:502" }),
    )])];
    let answer = status(
        json!([{ "id": "modbus_tcp", "state": "found", "count": 1 }]),
        json!([modbus_device("10.0.0.7", None)]),
    );
    let ignored = Memory {
        protocols: None,
        ignored: BTreeSet::from(["dev:ip:10.0.0.7".to_owned()]),
    };
    let hidden = Context {
        project: project_of(&known),
        memory: ignored.clone(),
        ..Context::default()
    };
    assert_eq!(
        card_ids(&view_with(&answer, &hidden)),
        Vec::<&str>::new(),
        "the ignored find leaves the list"
    );

    let shown = Context {
        show_ignored: true,
        ..hidden
    };
    assert_eq!(
        marks(&view_with(&answer, &shown)),
        vec![(
            "dev:ip:10.0.0.7",
            Some(("Not in the project", Tone::Neutral)),
            vec![]
        )]
    );
}

#[test]
fn a_find_the_comparison_cannot_judge_carries_no_mark() {
    // Specification 25 §2 and specification 11, "Host names": a connection named by a host name that no result
    // carries may be any server of its protocol, so an OPC UA server that matches nothing is neither in the
    // project nor outside it. A wrong implementation marks it `Not in the project`, which would be false.
    let known = [runtime_with(vec![(
        "opcua_client",
        json!({ "connections": [{
            "endpoint_url": "opc.tcp://plc01:4840", "security_policy": "none", "security_mode": "none"
        }] }),
    )])];
    let answer = status(
        json!([{ "id": "opcua", "state": "found", "count": 1 }]),
        json!([opcua_device("10.0.0.6")]),
    );
    let context = Context {
        project: project_of(&known),
        ..Context::default()
    };

    let view = view_with(&answer, &context);

    assert_eq!(marks(&view), vec![("dev:ip:10.0.0.6", None, vec![])]);
}

#[test]
fn the_project_is_the_devices_of_the_enabled_connections_on_the_canvas() {
    // Specification 25 §2: the connections of the runtimes the panel shows name the devices; a disabled
    // connection names none, and neither does one whose settings name no device the way its driver reads them.
    // A wrong implementation counts a disabled connection, or lets one broken connection hide the others.
    let mut runtime = runtime_with(vec![
        (
            "modbus_tcp",
            json!({ "address": "10.0.0.5:502", "unit_id": 3 }),
        ),
        ("modbus_tcp", json!({ "address": "10.0.0.6:502" })),
        ("modbus_tcp", json!({ "address": "plc.local:502" })),
        ("gpio", json!({ "chip": "gpiochip0" })),
    ]);
    runtime.connections[1].disabled = true;

    let project = project_of(&[runtime]).expect("a runtime is shown");

    let facts: Vec<Value> = project
        .iter()
        .map(|device| Value::Object(device.facts()))
        .collect();
    assert_eq!(
        facts,
        vec![json!({ "address": "10.0.0.5:502", "unit_id": 3 })]
    );
}
