//! The object-level review of an edit (specification 43, AT-05: "Return object-level before/after
//! changes and findings"). Port of `agent/review.ts` (`describePageChanges`).
//!
//! The native models are compared field by field. An object keeps its identity by its ID, so a
//! changed property is reported against the same object, and the page's own properties are
//! compared apart from its objects.

use std::collections::BTreeSet;

use serde::Serialize;
use serde_json::{Map, Value};
use trust_runtime::hmi::page_files::{HmiObject, HmiPage};

/// What an edit changed on a page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PageChanges {
    /// IDs of the objects the edit added, in page order.
    pub(super) added: Vec<String>,
    /// IDs of the objects the edit removed, in the order they were before.
    pub(super) removed: Vec<String>,
    /// Objects that remain and whose authored properties changed.
    pub(super) modified: Vec<ModifiedObject>,
    /// Page-level properties that changed (the page's objects are compared above).
    pub(super) page_properties: Vec<String>,
    /// Whether the objects that remain are in another order.
    pub(super) order_changed: bool,
}

/// One object whose authored properties changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct ModifiedObject {
    /// The object's ID.
    pub(super) id: String,
    /// The changed property names, sorted.
    pub(super) properties: Vec<String>,
}

/// Compares two pages (`describePageChanges`). A missing `before` is an empty page.
#[must_use]
pub(super) fn describe_page_changes(before: Option<&HmiPage>, after: &HmiPage) -> PageChanges {
    let previous: Vec<&HmiObject> = before
        .map(|page| page.objects.iter().collect())
        .unwrap_or_default();
    let next_ids: Vec<&str> = after
        .objects
        .iter()
        .map(|object| object.id.as_str())
        .collect();
    let previous_ids: Vec<&str> = previous.iter().map(|object| object.id.as_str()).collect();
    let added = after
        .objects
        .iter()
        .filter(|object| !previous_ids.contains(&object.id.as_str()))
        .map(|object| object.id.clone())
        .collect();
    let removed = previous
        .iter()
        .filter(|object| !next_ids.contains(&object.id.as_str()))
        .map(|object| object.id.clone())
        .collect();
    let modified = after
        .objects
        .iter()
        .filter_map(|object| {
            let original = previous.iter().find(|known| known.id == object.id)?;
            let properties = changed_properties(&fields_of(original), &fields_of(object), &["id"]);
            (!properties.is_empty()).then(|| ModifiedObject {
                id: object.id.clone(),
                properties,
            })
        })
        .collect();
    let page_properties = changed_properties(
        &before.map(fields_of_page).unwrap_or_default(),
        &fields_of_page(after),
        &["object"],
    );
    let kept_before: Vec<&str> = previous_ids
        .iter()
        .copied()
        .filter(|id| next_ids.contains(id))
        .collect();
    let kept_after: Vec<&str> = next_ids
        .iter()
        .copied()
        .filter(|id| previous_ids.contains(id))
        .collect();
    PageChanges {
        added,
        removed,
        modified,
        page_properties,
        order_changed: kept_before != kept_after,
    }
}

/// The keys whose values differ, sorted, leaving out the excluded keys (`changedProperties`). A
/// key that one side lacks counts as changed.
fn changed_properties(
    before: &Map<String, Value>,
    after: &Map<String, Value>,
    excluded: &[&str],
) -> Vec<String> {
    let keys: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    keys.into_iter()
        .filter(|key| !excluded.contains(&key.as_str()) && before.get(*key) != after.get(*key))
        .cloned()
        .collect()
}

/// An object's fields as its file writes them.
fn fields_of(object: &HmiObject) -> Map<String, Value> {
    match serde_json::to_value(object) {
        Ok(Value::Object(fields)) => fields,
        _ => Map::new(),
    }
}

/// A page's fields as its file writes them.
fn fields_of_page(page: &HmiPage) -> Map<String, Value> {
    match serde_json::to_value(page) {
        Ok(Value::Object(fields)) => fields,
        _ => Map::new(),
    }
}

#[cfg(test)]
#[path = "review_tests.rs"]
mod tests;
