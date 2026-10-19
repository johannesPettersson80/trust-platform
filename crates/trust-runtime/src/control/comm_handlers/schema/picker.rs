//! The words of the add picker and of the setup forms, which the communication catalog carries
//! (specification 11, "Communication schema, capabilities, and Test"; specification 25 §5.4 and
//! §5.9). The tools show these words instead of keeping a copy of them.
//!
//! - `PICKER_GROUPS`: the picker's groups in their canonical order, each with the protocols it
//!   holds, in their order.
//! - `picker_entry`: a protocol's entry in the picker, with the picker's title, badge and
//!   purpose. The protocol's own title and purpose stay the words of its form. `discovery` has
//!   none: the runtime discovery settings are edited from a runtime's `Runtime discovery`
//!   connection, not added as a protocol card.
//! - `describe`: gives a protocol its entry, its `enum` fields the labels of their options, and
//!   its `json_array` fields the words and the seed of their list editors.
//! - `item_description`: the items of the lists whose items the runtime's configuration parser
//!   reads as numbers or objects, so that adding an item gives one the parser accepts once it is
//!   filled in.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use super::{CommFieldSchema, CommProtocolSchema};

/// A group of the add picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct PickerGroup {
    /// Its stable identity.
    id: &'static str,
    /// Its name.
    label: &'static str,
    /// Whether the picker shows it only on request.
    advanced: bool,
    /// The protocols it holds, in their order.
    protocols: &'static [&'static str],
}

/// The add picker's groups, in their canonical order (specification 25 §5.9).
pub(super) const PICKER_GROUPS: [PickerGroup; 5] = [
    PickerGroup {
        id: "devices_io",
        label: "Devices and I/O",
        advanced: false,
        protocols: &["modbus_tcp", "ethercat", "gpio", "simulated", "loopback"],
    },
    PickerGroup {
        id: "read_tags",
        label: "Read tags from another PLC or server",
        advanced: false,
        protocols: &["opcua_client", "ads"],
    },
    PickerGroup {
        id: "share_values",
        label: "Share truST values",
        advanced: false,
        protocols: &["opcua", "ads_server"],
    },
    PickerGroup {
        id: "messages",
        label: "Send and receive messages",
        advanced: false,
        protocols: &["mqtt"],
    },
    PickerGroup {
        id: "advanced",
        label: "Advanced integrations",
        advanced: true,
        protocols: &["mesh", "openot", "realtime_t0", "runtime_cloud"],
    },
];

/// A protocol's entry in the add picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct PickerEntry {
    /// The entry's title.
    title: &'static str,
    /// Its badge.
    badge: &'static str,
    /// What the protocol is for, in the picker's words.
    purpose: &'static str,
}

/// The picker entry of `protocol`. `discovery` has none, and neither has a protocol that no
/// group holds.
pub(super) fn picker_entry(protocol: &str) -> Option<PickerEntry> {
    let (title, badge, purpose) = match protocol {
        "modbus_tcp" => ("Modbus", "MB", "Connect PLCs, drives, meters, and remote I/O."),
        "ethercat" => (
            "EtherCAT",
            "EC",
            "Connect EtherCAT drives, terminals, or remote I/O.",
        ),
        "gpio" => ("GPIO", "IO", "Use Raspberry Pi or Linux controller pins."),
        "simulated" => ("Simulated I/O", "SIM", "Practice and test without hardware."),
        "loopback" => (
            "Loopback I/O",
            "LOOP",
            "Echo outputs back into inputs for quick checks.",
        ),
        "opcua_client" => (
            "OPC UA client",
            "UA IN",
            "Read selected tags from another OPC UA server.",
        ),
        "ads" => ("ADS client", "ADS IN", "Read tags from a TwinCAT or ADS PLC."),
        "opcua" => (
            "OPC UA server",
            "UA OUT",
            "Share truST values with SCADA, HMI, or historians.",
        ),
        "ads_server" => (
            "ADS server",
            "ADS OUT",
            "Share truST values with TwinCAT or ADS clients.",
        ),
        "mqtt" => (
            "MQTT broker",
            "MQTT",
            "Send and receive process values through MQTT topics.",
        ),
        "mesh" => (
            "Mesh / Zenoh",
            "MESH",
            "Configure a peer network; no live link is active until a runtime reports one.",
        ),
        "openot" => (
            "OpenOT",
            "OT",
            "Configure OpenOT evidence output; no evidence is published until a runtime reports one.",
        ),
        "realtime_t0" => (
            "Realtime T0",
            "RT",
            "Configure deterministic same-host exchange; no live exchange runs until a runtime reports one.",
        ),
        "runtime_cloud" => (
            "Runtime cloud",
            "CLOUD",
            "Configure federation policy; no live link is active until a runtime reports one.",
        ),
        _ => return None,
    };
    Some(PickerEntry {
        title,
        badge,
        purpose,
    })
}

/// The words and the seed of a `json_array` field's list editor.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(super) struct ListTexts {
    /// What one item is called.
    item_noun: String,
    /// What shows while the list is empty.
    empty_text: String,
    /// The item that adding appends when the list holds no item to copy; null when items come
    /// only from Discover or Browse.
    item_template: Value,
    /// The type of each item of a list of numbers.
    #[serde(skip_serializing_if = "Option::is_none")]
    item_type: Option<&'static str>,
    /// The properties of each item of a list of objects, in their order.
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    item_fields: &'static [ItemField],
    /// The groups of properties of which an item sets exactly one.
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    item_one_of: &'static [OneOf],
}

/// A property of the items of a list of objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct ItemField {
    /// Its key in an item.
    id: &'static str,
    /// Its label.
    label: &'static str,
    /// `string` or `enum`.
    #[serde(rename = "type")]
    field_type: &'static str,
    /// Whether an item needs it.
    required: bool,
    /// The choices of an `enum`.
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    options: &'static [&'static str],
}

/// Properties of which an item sets exactly one; a new item holds the first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct OneOf {
    /// The properties.
    fields: &'static [&'static str],
    /// The protocol's boolean field that, while on, lets an item set none of them.
    #[serde(skip_serializing_if = "Option::is_none")]
    unless: Option<&'static str>,
}

/// How the items of a list are described: the type of a list of numbers, or the properties of a
/// list of objects with their groups.
#[derive(Debug, Clone, Copy, Default)]
struct ItemDescription {
    item_type: Option<&'static str>,
    fields: &'static [ItemField],
    one_of: &'static [OneOf],
}

impl ItemDescription {
    /// Whether a new item holds the property `id`: every property but the later ones of a group.
    fn starts_with(&self, id: &str) -> bool {
        !self
            .one_of
            .iter()
            .any(|group| group.fields.iter().skip(1).any(|field| *field == id))
    }
}

const fn text_field(id: &'static str, label: &'static str, required: bool) -> ItemField {
    ItemField {
        id,
        label,
        field_type: "string",
        required,
        options: &[],
    }
}

/// `[[runtime.ads_server.clients]]`: `AdsServerClientSection` (`config/parser.rs`) requires
/// `ams_net_id`; `parse_ads_server_client_source`
/// (`config/parser/validation/runtime/ads_server.rs`) requires one of `source_ip` and
/// `source_cidr`, refuses both, and accepts neither only with `allow_unpinned_clients`.
const ADS_CLIENT_FIELDS: [ItemField; 3] = [
    text_field("ams_net_id", "AMS Net ID", true),
    text_field("source_ip", "Source IP", false),
    text_field("source_cidr", "Source CIDR", false),
];
const ADS_CLIENT_ONE_OF: [OneOf; 1] = [OneOf {
    fields: &["source_ip", "source_cidr"],
    unless: Some("allow_unpinned_clients"),
}];

/// `runtime.cloud.wan.allow_write`: `RuntimeCloudWanAllowRuleSection` (`config/parser.rs`)
/// requires `action` and `target`, and `parse_runtime_cloud_section`
/// (`config/parser/validation/runtime/transport.rs`) refuses an empty one.
const WAN_RULE_FIELDS: [ItemField; 2] = [
    text_field("action", "Action", true),
    text_field("target", "Target", true),
];

/// The transports that `RuntimeCloudPreferredTransport::parse` (`config/contracts.rs`) accepts.
pub(super) const LINK_TRANSPORTS: [&str; 8] = [
    "realtime",
    "zenoh",
    "mesh",
    "mqtt",
    "modbus-tcp",
    "opcua",
    "discovery",
    "web",
];

/// `runtime.cloud.links.transports`: `RuntimeCloudLinkPreferenceRuleSection`
/// (`config/parser.rs`) requires all three, `parse_runtime_cloud_section` refuses an empty
/// source or target, and the transport is one of `LINK_TRANSPORTS`.
const LINK_TRANSPORT_FIELDS: [ItemField; 3] = [
    text_field("source", "Source", true),
    text_field("target", "Target", true),
    ItemField {
        id: "transport",
        label: "Transport",
        field_type: "enum",
        required: true,
        options: &LINK_TRANSPORTS,
    },
];

/// The items of the list `field` of `protocol`, as the runtime's configuration parser reads
/// them. `runtime.realtime.cpu_affinity` is `Option<Vec<usize>>` in `RealtimeSection`
/// (`config/parser.rs`).
fn item_description(protocol: &str, field: &str) -> ItemDescription {
    match (protocol, field) {
        ("realtime_t0", "cpu_affinity") => ItemDescription {
            item_type: Some("unsigned"),
            ..ItemDescription::default()
        },
        ("ads_server", "clients") => ItemDescription {
            item_type: None,
            fields: &ADS_CLIENT_FIELDS,
            one_of: &ADS_CLIENT_ONE_OF,
        },
        ("runtime_cloud", "wan_allow_write") => ItemDescription {
            fields: &WAN_RULE_FIELDS,
            ..ItemDescription::default()
        },
        ("runtime_cloud", "link_transports") => ItemDescription {
            fields: &LINK_TRANSPORT_FIELDS,
            ..ItemDescription::default()
        },
        _ => ItemDescription::default(),
    }
}

/// The empty text of the connection lists, whose items come from Discover or Browse.
const CONNECTIONS_EMPTY: &str =
    "No connections yet. Use Discover or Browse to add one instead of typing JSON.";

/// The empty text of the exposed globals.
const EXPOSE_EMPTY: &str =
    "No globals selected yet. Use Choose globals to pick project variables, or add a pattern manually.";

/// Gives `protocol` its picker entry, its `enum` fields the labels of their options, and its
/// `json_array` fields their list texts.
pub(super) fn describe(protocol: &mut CommProtocolSchema) {
    protocol.picker = picker_entry(protocol.id);
    let protocol_id = protocol.id;
    for field in &mut protocol.fields {
        let id = field.id;
        if let Some(options) = &field.options {
            field.option_labels = Some(
                options
                    .iter()
                    .map(|&option| (option, option_label(id, option)))
                    .collect::<BTreeMap<_, _>>(),
            );
        }
        if field.field_type == "json_array" {
            let list = list_texts(field, item_description(protocol_id, id));
            field.list = Some(list);
        }
    }
}

/// The label of `option` of the field `field`; an option without a product label reads as its
/// value.
fn option_label(field: &str, option: &'static str) -> &'static str {
    match (field, option) {
        ("on_error", "fault") => "Stop with fault",
        ("on_error", "warn") => "Warn and continue",
        ("on_error", "ignore") => "Ignore",
        ("scheduler", "fifo") => "FIFO",
        ("scheduler", "rr") => "Round robin",
        ("scheduler", "other") => "Default",
        ("source", "heartbeat") => "Heartbeat",
        ("source", "st-fb") => "ST producer",
        ("fence_mode", "fenced") => "Fenced",
        ("fence_mode", "unfenced") => "Unfenced",
        ("profile", "dev") => "Development",
        ("profile", "plant") => "Plant",
        ("profile", "wan") => "WAN",
        _ => option,
    }
}

/// Whether `field` is a list of connections, whose items come only from Discover or Browse.
fn lists_connections(field: &CommFieldSchema) -> bool {
    field.id.to_ascii_lowercase().contains("connections")
}

/// The list texts of `field`, whose items `description` describes. A described list starts a
/// new item from its description: an empty string for a number, or every property as an empty
/// string, of each group only the first. Any other list copies the first item of its default.
fn list_texts(field: &CommFieldSchema, description: ItemDescription) -> ListTexts {
    let connections = lists_connections(field);
    let first_default = field
        .default
        .as_array()
        .and_then(|items| items.first())
        .cloned();
    let item_template = if !description.fields.is_empty() {
        Value::Object(
            description
                .fields
                .iter()
                .filter(|property| description.starts_with(property.id))
                .map(|property| (property.id.to_owned(), Value::String(String::new())))
                .collect(),
        )
    } else if description.item_type.is_some() {
        Value::String(String::new())
    } else {
        match first_default {
            Some(item) => item,
            None if connections => Value::Null,
            None => Value::String(String::new()),
        }
    };
    let empty_text = if connections {
        CONNECTIONS_EMPTY.to_owned()
    } else if field.id == "expose" {
        EXPOSE_EMPTY.to_owned()
    } else {
        format!("No {} yet. Add an item below.", sentence_label(field.label))
    };
    ListTexts {
        item_noun: item_noun(field),
        empty_text,
        item_template,
        item_type: description.item_type,
        item_fields: description.fields,
        item_one_of: description.one_of,
    }
}

/// What one item of the list `field` is called.
fn item_noun(field: &CommFieldSchema) -> String {
    let noun = match field.id {
        "connect" => "peer address",
        "producer_instances" => "ST producer block",
        "wan_allow_write" => "WAN write rule",
        "link_transports" => "link transport",
        "cpu_affinity" => "CPU index",
        "expose" => "global",
        "modules" => "module",
        "mock_inputs" => "mock frame",
        "selected_channels" => "channel",
        _ => return label_noun(field.label),
    };
    noun.to_owned()
}

/// A label as the noun of one item: in the singular, with its acronyms kept in capitals and its
/// other words in lower case, such as `allowed client` for `Allowed clients`.
fn label_noun(label: &str) -> String {
    let phrase = label
        .split_whitespace()
        .map(|word| {
            if is_acronym(word) {
                word.to_owned()
            } else {
                word.to_lowercase()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    let noun = if let Some(stem) = phrase.strip_suffix("ies") {
        format!("{stem}y")
    } else if let Some(stem) = phrase.strip_suffix('s') {
        stem.to_owned()
    } else {
        phrase
    };
    if noun.is_empty() {
        "item".to_owned()
    } else {
        noun
    }
}

/// A label inside a sentence: its first letter in lower case unless its first word is an
/// acronym.
fn sentence_label(label: &str) -> String {
    let label = label.trim();
    if label.is_empty() {
        return "items".to_owned();
    }
    if label.split_whitespace().next().is_some_and(is_acronym) {
        return label.to_owned();
    }
    let mut characters = label.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(characters).collect()
    })
}

/// A word in capitals and digits with at least one letter, such as `TLS` or `CPU`.
fn is_acronym(word: &str) -> bool {
    word.chars().any(|character| character.is_ascii_uppercase())
        && word
            .chars()
            .all(|character| character.is_ascii_uppercase() || character.is_ascii_digit())
}

#[cfg(test)]
mod tests;
