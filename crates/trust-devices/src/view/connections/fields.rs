//! The summary of a connection's settings (specification 25 §5.2, §5.4 and §5.9): one row for
//! each field of the protocol's catalog entry that the protocol's summary shows and that its
//! `visible_when` does not hide, under its product label, with lists, points and allowed clients
//! in words. A secret field says only whether it is set, and a key that names a secret is left
//! out of every list.

use serde_json::{Map, Value};

use super::{row, servers};
use crate::panes::forms::{self, CatalogField, CatalogProtocol};
use crate::texts::connections as texts;
use crate::texts::graph;
use crate::view::types::Row;

/// The fields that the ADS server's summary shows (specification 25 §5.4).
const ADS_SERVER_SUMMARY: [&str; 10] = [
    "enabled",
    "listen",
    "ams_net_id",
    "ads_port",
    "insecure_transport",
    "writes_enabled",
    "expose",
    "writable",
    "allow_unpinned_clients",
    "clients",
];

/// The fields that the OPC UA server's summary shows.
const OPCUA_SERVER_SUMMARY: [&str; 8] = [
    "enabled",
    "listen",
    "endpoint_path",
    "namespace_uri",
    "expose",
    "security_policy",
    "security_mode",
    "allow_anonymous",
];

/// The words in a key that name a secret.
const SECRET_WORDS: [&str; 3] = ["password", "token", "secret"];

/// The value of a field: the connection's own when its settings have the key, else the
/// catalog's default.
fn value_of<'a>(field: &'a CatalogField, params: Option<&'a Value>) -> Option<&'a Value> {
    params
        .and_then(|params| params.get(&field.id))
        .or(Some(&field.default))
}

/// Whether the summary of `protocol` shows the field `field`: the servers show their allowlists,
/// every other protocol all its fields.
pub(super) fn summary_fields(protocol: &str, field: &str) -> bool {
    match protocol {
        "ads_server" => ADS_SERVER_SUMMARY.contains(&field),
        "opcua" => OPCUA_SERVER_SUMMARY.contains(&field),
        _ => true,
    }
}

/// The rows of the settings: each shown field with a value, under its product label.
pub(super) fn summary_rows(
    protocol: &str,
    schema: &CatalogProtocol,
    params: Option<&Value>,
) -> Vec<Row> {
    let empty = Map::new();
    let values = params.and_then(Value::as_object).unwrap_or(&empty);
    schema
        .fields
        .iter()
        .filter(|field| {
            summary_fields(protocol, &field.id) && forms::visible(schema, field, values)
        })
        .filter_map(|field| {
            let value = field_value(protocol, field, params);
            if value.is_empty() {
                return None;
            }
            let wide = value.contains('\n');
            Some(Row {
                wide,
                ..row(&field_label(protocol, field), value)
            })
        })
        .collect()
}

/// The product label of a field (specification 25 §5.2): `Connections`, `Writes enabled`,
/// `Exposed globals`, `Enabled`, `Connection file` or `Config file`, `Polling`, and else the
/// form's label without the `OPC UA client` or `OPC UA server` it starts with.
pub(super) fn field_label(protocol: &str, field: &CatalogField) -> String {
    let fixed = match field.id.as_str() {
        "connections" => Some(texts::LABEL_CONNECTIONS),
        "writes_enabled" => Some(texts::LABEL_WRITES_ENABLED),
        "expose" if matches!(protocol, "ads_server" | "opcua") => {
            Some(texts::LABEL_EXPOSED_GLOBALS)
        }
        _ => None,
    };
    if let Some(fixed) = fixed {
        return fixed.to_owned();
    }
    let id = field.id.to_ascii_lowercase();
    let label = field.label.to_ascii_lowercase();
    if starts_with_enable(&id, is_separator) || starts_with_enable(&label, char::is_whitespace) {
        return texts::LABEL_ENABLED.to_owned();
    }
    if names_config_path(&id) || label.contains("config path") {
        let label = if protocol == "opcua_client" {
            texts::LABEL_CONNECTION_FILE
        } else {
            texts::LABEL_CONFIG_FILE
        };
        return label.to_owned();
    }
    if names_poll_interval(&id) || label.contains("poll interval") {
        return texts::LABEL_POLLING.to_owned();
    }
    let label = without_phrase(&field.label, "opc ua client");
    let label = without_phrase(&label, "opc ua server");
    graph::capitalized(&label.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn is_separator(character: char) -> bool {
    character == '_' || character == '-' || character.is_whitespace()
}

/// Whether `text` starts with `enable` and then a character that `separator` accepts.
fn starts_with_enable(text: &str, separator: fn(char) -> bool) -> bool {
    text.strip_prefix("enable")
        .and_then(|rest| rest.chars().next())
        .is_some_and(separator)
}

/// Whether a key ends with `config path`, with or without one separator between the words, as a
/// word of its own.
fn names_config_path(id: &str) -> bool {
    let Some(rest) = id.strip_suffix("path") else {
        return false;
    };
    let rest = rest.strip_suffix(is_separator).unwrap_or(rest);
    rest.strip_suffix("config")
        .is_some_and(|before| before.is_empty() || before.ends_with(is_separator))
}

/// Whether a key names a poll interval, with or without one separator between the words.
fn names_poll_interval(id: &str) -> bool {
    id.match_indices("poll").any(|(at, word)| {
        let rest = &id[at + word.len()..];
        rest.strip_prefix(is_separator)
            .unwrap_or(rest)
            .starts_with("interval")
    })
}

/// `label` without the first `phrase` (in lower case) that starts a word and is followed by white
/// space, and without that white space, ignoring case.
fn without_phrase(label: &str, phrase: &str) -> String {
    let lower = label.to_ascii_lowercase();
    let mut from = 0;
    while let Some(found) = lower[from..].find(phrase) {
        let at = from + found;
        let rest = &label[at + phrase.len()..];
        let spaces = rest.len() - rest.trim_start().len();
        let starts_word = label[..at]
            .chars()
            .next_back()
            .is_none_or(|before| !(before.is_ascii_alphanumeric() || before == '_'));
        if starts_word && spaces > 0 {
            return format!("{}{}", &label[..at], &rest[spaces..]);
        }
        from = at + 1;
    }
    label.to_owned()
}

/// The summary value of a field: lists, points and allowed clients in words, the `EtherCAT`
/// fields in their units, a secret only as set or not, a switch as `On` or `Off`, and an empty
/// list as `None`.
fn field_value(protocol: &str, field: &CatalogField, params: Option<&Value>) -> String {
    let value = value_of(field, params);
    if field.secret {
        return secret_value(value);
    }
    match (protocol, field.id.as_str()) {
        (_, "connections") => connection_lines(value),
        ("ads_server", "clients") => servers::ads_server_clients(value, params),
        ("ads_server" | "opcua", "expose") => servers::exposed_globals(value),
        ("ethercat", "modules") => ethercat_modules(value),
        ("ethercat", "selected_channels") => ethercat_channels(value),
        ("ethercat", "mock_inputs") => {
            let frames = array_of(value);
            if frames.is_empty() {
                texts::NONE.to_owned()
            } else {
                texts::mock_frames(frames.len())
            }
        }
        ("ethercat", "timeout_ms" | "cycle_warn_ms") => {
            let text = text_of(value);
            if text.is_empty() {
                text
            } else {
                graph::milliseconds(&text)
            }
        }
        ("ethercat", "on_error") => on_error(text_of(value)),
        _ => plain_value(&field.field_type, value),
    }
}

/// A value of no special field: an empty list is `None` and a switch `On` or `Off`.
fn plain_value(kind: &str, value: Option<&Value>) -> String {
    let text = text_of(value);
    match kind {
        "json_array" if text.trim().is_empty() || text.trim() == "[]" => texts::NONE.to_owned(),
        "boolean" | "bool" => switch(value).map_or(text, str::to_owned),
        _ => text,
    }
}

/// `On` or `Off` for a switch's value.
fn switch(value: Option<&Value>) -> Option<&'static str> {
    let on = match value? {
        Value::Bool(on) => *on,
        Value::String(text) if text.trim().eq_ignore_ascii_case("true") => true,
        Value::String(text) if text.trim().eq_ignore_ascii_case("false") => false,
        _ => return None,
    };
    Some(if on { texts::ON } else { texts::OFF })
}

/// A secret's value: never shown, only whether it is set (specification 25 §5.4).
fn secret_value(value: Option<&Value>) -> String {
    let shown = if text_of(value).is_empty() {
        texts::SECRET_NOT_SET
    } else {
        texts::SECRET_SET
    };
    shown.to_owned()
}

/// What the `EtherCAT` `on_error` choice does.
fn on_error(choice: String) -> String {
    let word = match choice.as_str() {
        "fault" => texts::ON_ERROR_FAULT,
        "warn" => texts::ON_ERROR_WARN,
        "ignore" => texts::ON_ERROR_IGNORE,
        _ => return choice,
    };
    word.to_owned()
}

/// A value as the form shows it: nothing for a missing or null value, the text of a string, and
/// JSON for anything else, without the keys that name a secret.
pub(super) fn text_of(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(text)) => text.clone(),
        Some(other) => without_secrets(other).to_string(),
    }
}

/// A scalar as text, trimmed: nothing for a missing or null value.
pub(super) fn scalar(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(text)) => text.trim().to_owned(),
        Some(other) => without_secrets(other).to_string(),
    }
}

/// `text` unless it is empty.
pub(super) fn non_empty(text: String) -> Option<String> {
    (!text.is_empty()).then_some(text)
}

/// The items of a list value, also when it is written as JSON text.
pub(super) fn array_of(value: Option<&Value>) -> Vec<Value> {
    match value {
        Some(Value::Array(items)) => items.clone(),
        Some(Value::String(text)) if text.trim_start().starts_with('[') => {
            serde_json::from_str(text).unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

/// A copy of `value` without the keys that name a secret, at any depth.
fn without_secrets(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .filter(|(key, _)| !names_secret(key))
                .map(|(key, value)| (key.clone(), without_secrets(value)))
                .collect::<Map<String, Value>>(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(without_secrets).collect()),
        other => other.clone(),
    }
}

fn names_secret(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    SECRET_WORDS.iter().any(|word| key.contains(word))
}

/// A list of connections, one line each: its name, address and ADS identity, then a line for
/// each of its points (specification 25 §5.4).
fn connection_lines(value: Option<&Value>) -> String {
    array_of(value)
        .iter()
        .enumerate()
        .map(|(index, connection)| connection_line(index, connection))
        .collect::<Vec<_>>()
        .join("\n")
}

fn connection_line(index: usize, connection: &Value) -> String {
    let fallback = || texts::numbered(texts::CONNECTION_NOUN, index + 1);
    let Some(entry) = connection.as_object() else {
        return fallback();
    };
    let mut head = vec![non_empty(scalar(entry.get("name"))).unwrap_or_else(fallback)];
    head.extend(
        ["endpoint_url", "host", "address", "broker"]
            .iter()
            .map(|key| scalar(entry.get(*key)))
            .find(|address| !address.is_empty()),
    );
    head.extend(ads_identity(connection));
    let head = head.join(" · ");
    let points: Vec<String> = entry
        .get("points")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|point| point.is_object())
        .map(point_line)
        .filter(|line| !line.is_empty())
        .map(|line| format!("- {line}"))
        .collect();
    if points.is_empty() {
        head
    } else {
        format!("{head}\n{}", points.join("\n"))
    }
}

/// The AMS Net ID and the ADS port of an ADS connection.
fn ads_identity(connection: &Value) -> Vec<String> {
    let text = |key: &str| non_empty(scalar(connection.get(key)));
    let net_id = text("ams_net_id").or_else(|| text("target_net_id"));
    let port = text("ams_port");
    net_id
        .map(|net_id| texts::ams_net_id(&net_id))
        .into_iter()
        .chain(port.map(|port| texts::ads_port(&port)))
        .collect()
}

/// What a point becomes in Structured Text and where it comes from (`point_line`), such as
/// `TankLevel (%IW0) ← ns=2;s=Tank · REAL · read`.
pub(super) fn point_line(point: &Value) -> String {
    let text = |key: &str| scalar(point.get(key));
    let name = ["var", "address", "name"]
        .into_iter()
        .map(text)
        .find(|name| !name.is_empty())
        .unwrap_or_default();
    let address = text("address");
    let source = point_source(point);
    let meta = [
        text("type"),
        non_empty(text("access")).unwrap_or_else(|| text("direction")),
    ]
    .into_iter()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join(" · ");
    if name.is_empty() && source.is_empty() {
        return String::new();
    }
    let mut line = if name.is_empty() {
        texts::UNNAMED.to_owned()
    } else {
        name.clone()
    };
    if !address.is_empty() && address != name {
        line = format!("{line} ({address})");
    }
    if !source.is_empty() {
        line = format!("{line} ← {source}");
    }
    if !meta.is_empty() {
        line = format!("{line} · {meta}");
    }
    line
}

/// Where a point comes from: its node, symbol, register, topic, channel, tag or index.
fn point_source(point: &Value) -> String {
    let text = |key: &str| scalar(point.get(key));
    for key in ["node_id", "symbol"] {
        let source = text(key);
        if !source.is_empty() {
            return source;
        }
    }
    if point.get("register").is_some() {
        return texts::register(&text("register"));
    }
    for key in ["topic", "channel", "tag"] {
        let source = text(key);
        if !source.is_empty() {
            return source;
        }
    }
    if point.get("index").is_some() {
        return texts::index(&text("index"));
    }
    String::new()
}

/// The modules of an `EtherCAT` bus: model, slot and channels of each (`ethercat_rows`).
fn ethercat_modules(value: Option<&Value>) -> String {
    let modules = array_of(value);
    if modules.is_empty() {
        return texts::NONE.to_owned();
    }
    modules
        .iter()
        .enumerate()
        .map(|(index, module)| module_line(index, module))
        .collect::<Vec<_>>()
        .join("; ")
}

fn module_line(index: usize, module: &Value) -> String {
    let fallback = || texts::numbered(texts::MODULE_NOUN, index + 1);
    if !module.is_object() {
        return fallback();
    }
    let mut parts = vec![non_empty(scalar(module.get("model"))).unwrap_or_else(fallback)];
    parts.extend(non_empty(scalar(module.get("slot"))).map(|slot| texts::slot(&slot)));
    parts.extend(count_of(module.get("channels")).map(texts::module_channels));
    parts.join(" · ")
}

/// A count, also when it is written as text.
pub(super) fn count_of(value: Option<&Value>) -> Option<u64> {
    match value? {
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

/// The selected channels of an `EtherCAT` bus.
fn ethercat_channels(value: Option<&Value>) -> String {
    let names: Vec<String> = array_of(value)
        .iter()
        .map(|channel| scalar(Some(channel)))
        .filter(|name| !name.is_empty())
        .collect();
    if names.is_empty() {
        texts::NONE.to_owned()
    } else {
        texts::channels(&names)
    }
}
