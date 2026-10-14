//! The metadata of an inspected page (specification 43, AT-03): its groups, the provenance of each
//! object's bindings, units and ranges, the faceplates it places, its navigation and the references that
//! do not resolve.
//!
//! Port of `agent/queryMetadata.ts` (`inspectMetadata`). The unit and format rules are those of
//! `drawing/valueFormats.ts` (`withValueFormat`) restricted to what an inspection reports: no runtime
//! values are present, so only the object, the binding and the project format decide a unit.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use super::argument::Argument;
use super::error::AgentResult;
use super::native::{self, text_of};
use crate::state::ServerState;

/// The metadata of one page's model (`inspectMetadata`). `text` is the page's text the faceplates are
/// resolved from; `draft` says whether that text is a draft's, which the open documents then carry too.
///
/// # Errors
///
/// `VALIDATION_FAILED` when the catalogue or the placed faceplates cannot be read.
pub(super) fn inspect_metadata(
    state: &ServerState,
    argument: &Argument,
    model: &Value,
    file: &str,
    text: &str,
    draft: bool,
) -> AgentResult<Value> {
    let mut open = native::unsaved_pages(&argument.open);
    if draft {
        if let Value::Object(pages) = &mut open {
            pages.insert(file.to_owned(), Value::String(text.to_owned()));
        }
    }
    let catalogue = native::catalogue(state, &argument.root_uri, &open)?;
    let formats = catalogue
        .get("formats")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let objects: Vec<Value> = model
        .get("object")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut provenance = Map::new();
    for object in &objects {
        let id = text_of(object, "id").to_owned();
        if let Some(group) = object.get("group").and_then(Value::as_str) {
            if !group.is_empty() {
                groups.entry(group.to_owned()).or_default().push(id.clone());
            }
        }
        provenance.insert(id, provenance_of(object, &formats));
    }
    let placed = if objects
        .iter()
        .any(|object| object.get("type").and_then(Value::as_str) == Some("faceplate"))
    {
        native::placed(state, &argument.root_uri, file, text, &open)?
    } else {
        json!({})
    };
    let unresolved: Vec<Value> = objects
        .iter()
        .filter(|object| {
            object.get("type").and_then(Value::as_str) == Some("faceplate")
                && placed.get(text_of(object, "id")).is_none_or(Value::is_null)
        })
        .map(|object| {
            json!({
                "object": text_of(object, "id"),
                "reason": "missing-or-incompatible-faceplate",
            })
        })
        .collect();
    Ok(json!({
        "groups": groups,
        "provenance": provenance,
        "placed": placed,
        "navigation": {
            "order": model.get("order").cloned().unwrap_or(Value::Null),
            "group": model.get("group").cloned().unwrap_or(Value::Null),
            "hidden": model.get("hidden").and_then(Value::as_bool).unwrap_or(false),
            "icon": model.get("icon").cloned().unwrap_or(Value::Null),
        },
        "unresolved": unresolved,
    }))
}

/// The provenance of one object: its bindings with the path's format key, its unit and range, and the
/// token source (`provenance` in `inspectMetadata`).
fn provenance_of(object: &Value, formats: &Value) -> Value {
    let bindings: Map<String, Value> = object
        .get("bind")
        .and_then(Value::as_object)
        .map(|bind| {
            bind.iter()
                .map(|(key, binding)| {
                    let path = binding.get("path").and_then(Value::as_str).unwrap_or("");
                    let mut entry = binding.clone();
                    if let Value::Object(fields) = &mut entry {
                        fields.insert("source".into(), json!("authored"));
                        fields.insert("formatKey".into(), json!(format_path(path)));
                    }
                    (key.clone(), entry)
                })
                .collect()
        })
        .unwrap_or_default();
    let unit = resolved_unit(object, formats);
    let unit_source = if object.get("unit").is_some() {
        "object"
    } else if value_binding(object)
        .and_then(|binding| binding.get("unit"))
        .is_some()
    {
        "binding"
    } else if unit.is_some() {
        "project-format"
    } else {
        "unspecified"
    };
    json!({
        "bindings": bindings,
        "unit": { "value": unit.map_or(Value::Null, Value::String), "source": unit_source },
        "range": {
            "min": object.get("min").cloned().unwrap_or(Value::Null),
            "max": object.get("max").cloned().unwrap_or(Value::Null),
            "source": "authored-or-unspecified",
        },
        "tokens": { "source": "shared-theme-and-project-overrides" },
    })
}

/// The unit an object shows: its own unit, else its value binding's, else the project format of the
/// bound path (`withValueFormat` with no runtime values). Without a value binding only the object's own
/// unit applies.
#[must_use]
fn resolved_unit(object: &Value, formats: &Value) -> Option<String> {
    if let Some(unit) = object.get("unit").and_then(Value::as_str) {
        return Some(unit.to_owned());
    }
    let binding = value_binding(object)?;
    if let Some(unit) = binding.get("unit").and_then(Value::as_str) {
        return Some(unit.to_owned());
    }
    let path = binding.get("path").and_then(Value::as_str)?;
    formats
        .get(format_path(path))
        .and_then(|format| format.get("unit"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// The binding of an object's `value` property, when it has one.
#[must_use]
fn value_binding(object: &Value) -> Option<&Value> {
    object.get("bind").and_then(|bind| bind.get("value"))
}

/// A bound path in the form the format table keys it by: its ASCII letters in lower case, since IEC
/// member lookup is case-insensitive (`formatPath` in `valueFormats.ts`).
#[must_use]
pub(super) fn format_path(path: &str) -> String {
    path.to_ascii_lowercase()
}

#[cfg(test)]
#[path = "query_metadata_tests.rs"]
mod tests;
