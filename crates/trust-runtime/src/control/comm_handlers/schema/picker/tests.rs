//! The words of the add picker and of the setup forms in the communication catalog
//! (specification 11, "Communication schema, capabilities, and Test").

use std::collections::BTreeSet;

use serde_json::{json, Value};

use super::super::{communication_protocol_schemas, static_comm_schema_value};
use super::{CONNECTIONS_EMPTY, EXPOSE_EMPTY, LINK_TRANSPORTS, PICKER_GROUPS};
use crate::config::RuntimeCloudPreferredTransport;

#[test]
fn picker_groups_hold_every_protocol_but_discovery_once() {
    let protocols = communication_protocol_schemas(&[]);
    let placed: Vec<&str> = PICKER_GROUPS
        .iter()
        .flat_map(|group| group.protocols.iter().copied())
        .collect();
    let unique: BTreeSet<&str> = placed.iter().copied().collect();
    assert_eq!(unique.len(), placed.len(), "each protocol is in one group");
    assert_eq!(
        unique.len() + 1,
        protocols.len(),
        "every protocol but discovery is placed"
    );
    for protocol in &protocols {
        let placed = unique.contains(protocol.id);
        assert_eq!(
            placed,
            protocol.id != "discovery",
            "{} placement",
            protocol.id
        );
        assert_eq!(
            protocol.picker.is_some(),
            placed,
            "{} has a picker entry exactly when a group holds it",
            protocol.id
        );
    }
}

#[test]
fn option_labels_and_list_texts_cover_every_enum_and_list_field() {
    for protocol in communication_protocol_schemas(&[]) {
        for field in &protocol.fields {
            let labels = field.option_labels.as_ref();
            match &field.options {
                Some(options) => {
                    let keys: Vec<&str> = labels
                        .map(|labels| labels.keys().copied().collect())
                        .unwrap_or_default();
                    let mut sorted = options.clone();
                    sorted.sort_unstable();
                    assert_eq!(keys, sorted, "{}.{} labels", protocol.id, field.id);
                }
                None => assert_eq!(labels, None, "{}.{} has no options", protocol.id, field.id),
            }
            assert_eq!(
                field.list.is_some(),
                field.field_type == "json_array",
                "{}.{} has list texts exactly when it is a list",
                protocol.id,
                field.id
            );
        }
    }
}

/// Every list of the catalog: its protocol and field, and its noun, empty text and template.
fn lists() -> Vec<(&'static str, &'static str, &'static str, String, Value)> {
    let empty = |label: &str| format!("No {label} yet. Add an item below.");
    vec![
        ("mqtt", "tls_alpn", "TLS ALPN", empty("TLS ALPN"), json!("")),
        (
            "ethercat",
            "modules",
            "module",
            empty("modules"),
            json!({ "model": "EK1100", "slot": 0 }),
        ),
        (
            "ethercat",
            "mock_inputs",
            "mock frame",
            empty("mock input frames"),
            json!(""),
        ),
        (
            "ethercat",
            "selected_channels",
            "channel",
            empty("selected channels"),
            json!(""),
        ),
        (
            "gpio",
            "inputs",
            "input",
            empty("inputs"),
            json!({ "address": "%IX0.0", "line": 17 }),
        ),
        (
            "gpio",
            "outputs",
            "output",
            empty("outputs"),
            json!({ "address": "%QX0.0", "line": 27 }),
        ),
        (
            "opcua",
            "expose",
            "global",
            EXPOSE_EMPTY.to_owned(),
            json!("global.*"),
        ),
        (
            "opcua_client",
            "connections",
            "connection",
            CONNECTIONS_EMPTY.to_owned(),
            Value::Null,
        ),
        (
            "openot",
            "producer_instances",
            "ST producer block",
            empty("ST producer blocks"),
            json!(""),
        ),
        (
            "discovery",
            "interfaces",
            "interface",
            empty("interfaces"),
            json!(""),
        ),
        (
            "mesh",
            "connect",
            "peer address",
            empty("peer addresses"),
            json!(""),
        ),
        (
            "mesh",
            "publish",
            "publish topic",
            empty("publish topics"),
            json!(""),
        ),
        (
            "realtime_t0",
            "cpu_affinity",
            "CPU index",
            empty("CPU affinity"),
            json!(""),
        ),
        (
            "runtime_cloud",
            "wan_allow_write",
            "WAN write rule",
            empty("allowed WAN writes"),
            json!({ "action": "", "target": "" }),
        ),
        (
            "runtime_cloud",
            "link_transports",
            "link transport",
            empty("preferred link transports"),
            json!({ "source": "", "target": "", "transport": "" }),
        ),
        (
            "ads_server",
            "expose",
            "global",
            EXPOSE_EMPTY.to_owned(),
            json!("global.*"),
        ),
        (
            "ads_server",
            "writable",
            "writable global",
            empty("writable globals"),
            json!(""),
        ),
        (
            "ads_server",
            "clients",
            "allowed client",
            empty("allowed clients"),
            json!({ "ams_net_id": "", "source_ip": "" }),
        ),
        (
            "ads",
            "connections",
            "connection",
            CONNECTIONS_EMPTY.to_owned(),
            Value::Null,
        ),
    ]
}

#[test]
fn schema_reply_carries_the_picker_and_form_words() {
    let schema = static_comm_schema_value(None).expect("the static catalog");
    assert_eq!(
        schema["picker_groups"],
        json!([
            { "id": "devices_io", "label": "Devices and I/O", "advanced": false,
              "protocols": ["modbus_tcp", "ethercat", "gpio", "simulated", "loopback"] },
            { "id": "read_tags", "label": "Read tags from another PLC or server", "advanced": false,
              "protocols": ["opcua_client", "ads"] },
            { "id": "share_values", "label": "Share truST values", "advanced": false,
              "protocols": ["opcua", "ads_server"] },
            { "id": "messages", "label": "Send and receive messages", "advanced": false,
              "protocols": ["mqtt"] },
            { "id": "advanced", "label": "Advanced integrations", "advanced": true,
              "protocols": ["mesh", "openot", "realtime_t0", "runtime_cloud"] },
        ])
    );
    let protocol = |id: &str| {
        schema["protocols"]
            .as_array()
            .and_then(|protocols| protocols.iter().find(|protocol| protocol["id"] == id))
            .unwrap_or_else(|| panic!("protocol {id}"))
            .clone()
    };
    let field = |protocol_id: &str, field_id: &str| {
        protocol(protocol_id)["fields"]
            .as_array()
            .and_then(|fields| fields.iter().find(|field| field["id"] == field_id))
            .unwrap_or_else(|| panic!("field {protocol_id}.{field_id}"))
            .clone()
    };
    assert_eq!(
        protocol("modbus_tcp")["picker"],
        json!({ "title": "Modbus", "badge": "MB",
                "purpose": "Connect PLCs, drives, meters, and remote I/O." })
    );
    assert_eq!(
        protocol("modbus_tcp")["purpose"],
        "Read and write register-oriented devices or PLC endpoints.",
        "the form keeps its own words"
    );
    assert_eq!(protocol("discovery").get("picker"), None);
    assert_eq!(
        field("modbus_tcp", "on_error")["option_labels"],
        json!({ "fault": "Stop with fault", "warn": "Warn and continue", "ignore": "Ignore" })
    );
    assert_eq!(
        field("realtime_t0", "scheduler")["option_labels"],
        json!({ "fifo": "FIFO", "rr": "Round robin", "other": "Default" })
    );
    assert_eq!(
        field("gpio", "backend")["option_labels"],
        json!({ "libgpiod": "libgpiod", "sysfs": "sysfs" }),
        "an option without a product label reads as its value"
    );
    assert_eq!(field("modbus_tcp", "address").get("list"), None);
    let mut seen = BTreeSet::new();
    for (protocol_id, field_id, noun, empty_text, template) in lists() {
        let list = &field(protocol_id, field_id)["list"];
        assert_eq!(
            (
                &list["item_noun"],
                &list["empty_text"],
                &list["item_template"]
            ),
            (&json!(noun), &json!(empty_text), &template),
            "{protocol_id}.{field_id}"
        );
        seen.insert((protocol_id, field_id));
    }
    let protocols = communication_protocol_schemas(&[]);
    let catalog_lists: BTreeSet<(&str, &str)> = protocols
        .iter()
        .flat_map(|protocol| {
            protocol
                .fields
                .iter()
                .filter(|field| field.field_type == "json_array")
                .map(move |field| (protocol.id, field.id))
        })
        .collect();
    assert_eq!(seen, catalog_lists, "every list of the catalog is pinned");
}

/// The `list` of the field `field_id` of `protocol_id` in the static catalog.
fn list_of(schema: &Value, protocol_id: &str, field_id: &str) -> Value {
    schema["protocols"]
        .as_array()
        .and_then(|protocols| {
            protocols
                .iter()
                .find(|protocol| protocol["id"] == protocol_id)
        })
        .and_then(|protocol| protocol["fields"].as_array())
        .and_then(|fields| fields.iter().find(|field| field["id"] == field_id))
        .map(|field| field["list"].clone())
        .unwrap_or_else(|| panic!("the list {protocol_id}.{field_id}"))
}

#[test]
fn described_list_items_follow_the_runtime_parser() {
    let schema = static_comm_schema_value(None).expect("the static catalog");
    // `RealtimeSection::cpu_affinity` is `Option<Vec<usize>>`.
    let cpus = list_of(&schema, "realtime_t0", "cpu_affinity");
    assert_eq!(cpus["item_type"], "unsigned");
    assert_eq!(cpus.get("item_fields"), None);
    // `AdsServerClientSection` and `parse_ads_server_client_source`.
    let clients = list_of(&schema, "ads_server", "clients");
    assert_eq!(
        clients["item_fields"],
        json!([
            { "id": "ams_net_id", "label": "AMS Net ID", "type": "string", "required": true },
            { "id": "source_ip", "label": "Source IP", "type": "string", "required": false },
            { "id": "source_cidr", "label": "Source CIDR", "type": "string", "required": false },
        ])
    );
    assert_eq!(
        clients["item_one_of"],
        json!([{ "fields": ["source_ip", "source_cidr"], "unless": "allow_unpinned_clients" }])
    );
    // `RuntimeCloudWanAllowRuleSection` and `RuntimeCloudLinkPreferenceRuleSection`.
    assert_eq!(
        list_of(&schema, "runtime_cloud", "wan_allow_write")["item_fields"],
        json!([
            { "id": "action", "label": "Action", "type": "string", "required": true },
            { "id": "target", "label": "Target", "type": "string", "required": true },
        ])
    );
    assert_eq!(
        list_of(&schema, "runtime_cloud", "link_transports")["item_fields"],
        json!([
            { "id": "source", "label": "Source", "type": "string", "required": true },
            { "id": "target", "label": "Target", "type": "string", "required": true },
            { "id": "transport", "label": "Transport", "type": "enum", "required": true,
              "options": LINK_TRANSPORTS },
        ])
    );
    // The transports are the ones the parser reads, each in its own spelling.
    for transport in LINK_TRANSPORTS {
        let parsed = RuntimeCloudPreferredTransport::parse(transport).expect("a transport");
        assert_eq!(parsed.as_str(), transport);
    }
    assert!(RuntimeCloudPreferredTransport::parse("carrier-pigeon").is_err());
    // No other list is described.
    let described: Vec<(&str, &str)> = communication_protocol_schemas(&[])
        .iter()
        .flat_map(|protocol| {
            protocol
                .fields
                .iter()
                .filter(|field| {
                    field.list.as_ref().is_some_and(|list| {
                        list.item_type.is_some() || !list.item_fields.is_empty()
                    })
                })
                .map(move |field| (protocol.id, field.id))
        })
        .collect();
    assert_eq!(
        described,
        [
            ("realtime_t0", "cpu_affinity"),
            ("runtime_cloud", "wan_allow_write"),
            ("runtime_cloud", "link_transports"),
            ("ads_server", "clients"),
        ]
    );
}
