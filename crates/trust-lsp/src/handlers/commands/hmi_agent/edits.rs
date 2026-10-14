//! Typed draft edits (specification 43, AT-05): the operations an agent applies to a page, checked
//! and applied in order to a copy of the page.
//!
//! Port of `agent/edits.ts` (`editAgentPage`). Each operation is read from its JSON and refused
//! with the TypeScript's text when it is malformed. The first refusal discards the copy, so the
//! caller's page is never left half edited. Pipe operations live in `pipe_edits`, geometric
//! operations in `geometric_edits`, and the builder's own rules in `align`, `object_rules`,
//! `pipe_rules` and `clipboard`.

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use trust_runtime::hmi::catalogue::Catalogue;
use trust_runtime::hmi::page_files::{
    HmiBinding, HmiBindingDirection, HmiObject, HmiPage, PLACED_FACEPLATE,
};

use super::align::{align_objects, distribute_objects, AlignEdge, DistributeAxis};
use super::clipboard::duplicate_objects;
use super::error::{AgentError, AgentResult};
use super::geometric_edits::{arrange_region, equal_size};
use super::object_rules::delete_objects;
use super::pipe_edits::{connect_ports, connection_index, disconnect_ports, reconnect_pipe};

/// The most operations one edit may carry.
const MAX_OPERATIONS: usize = 500;

/// The most waypoints one pipe may carry.
const MAX_WAYPOINTS: usize = 64;

/// The object properties an `update` may set (`EDITABLE` in `edits.ts`).
const EDITABLE: &[&str] = &[
    "tag",
    "label",
    "text",
    "unit",
    "decimals",
    "min",
    "max",
    "limits",
    "span",
    "variant",
    "role",
    "confirm",
    "action",
    "pulse",
    "expect",
    "timeout",
    "options",
    "attach",
    "group",
    "locked",
    "arrange",
    "gap",
    "image",
    "faceplate",
    "instance",
    "click",
];

/// Applies the operations, in order, to a copy of `source` (`editAgentPage`). `page_size` is the
/// project's design size, which regional arrangement needs.
///
/// # Errors
///
/// Returns the first refusal: `LIMIT_EXCEEDED` for an empty or oversized list, `INVALID_ARGUMENT`
/// for a malformed operation, `CONFLICT` for a locked or conflicting object, and
/// `VALIDATION_FAILED` for a rule the page would break.
pub(super) fn edit_page(
    source: &HmiPage,
    operations: &[Value],
    catalogue: &Catalogue,
    page_size: Option<[u32; 2]>,
) -> AgentResult<HmiPage> {
    if operations.is_empty() || operations.len() > MAX_OPERATIONS {
        return Err(AgentError::limit("provide 1 to 500 operations."));
    }
    let mut page = source.clone();
    for operation in operations {
        apply(&mut page, operation, catalogue, page_size)?;
    }
    Ok(page)
}

/// Applies one operation to the page, by its kind.
fn apply(
    page: &mut HmiPage,
    operation: &Value,
    catalogue: &Catalogue,
    page_size: Option<[u32; 2]>,
) -> AgentResult<()> {
    let kind = operation
        .get("op")
        .and_then(Value::as_str)
        .ok_or_else(|| AgentError::invalid("operation kind is required."))?;
    match kind {
        "page" | "group" | "ungroup" | "remove" | "bind" | "unbind" | "update" | "add" => {
            apply_objects(page, kind, operation, catalogue)
        }
        "move" | "resize" | "align" | "distribute" | "equal-size" | "arrange" | "reorder"
        | "duplicate" => apply_geometry(page, kind, operation, page_size),
        "reroute" | "reconnect" | "connect" | "disconnect" => {
            apply_pipes(page, kind, operation, catalogue)
        }
        other => Err(unsupported(other)),
    }
}

/// The refusal for an operation kind the engine does not know.
fn unsupported(kind: &str) -> AgentError {
    AgentError::invalid(format!("unsupported operation {kind}."))
}

/// Applies a page property, group, removal, binding or object addition or update.
fn apply_objects(
    page: &mut HmiPage,
    kind: &str,
    operation: &Value,
    catalogue: &Catalogue,
) -> AgentResult<()> {
    match kind {
        "page" => apply_page_properties(page, operation.get("properties")),
        "group" | "ungroup" => {
            let ids = selected(page, operation.get("ids"))?;
            ensure_movable(page, &ids)?;
            if kind == "ungroup" {
                for object in objects_mut(page, &ids) {
                    object.group = None;
                }
            } else {
                let group = name(operation.get("group"))?;
                for object in objects_mut(page, &ids) {
                    object.group = Some(group.clone());
                }
            }
            Ok(())
        }
        "remove" => {
            let ids = selected(page, operation.get("ids"))?;
            ensure_movable(page, &ids)?;
            let dependent = page.pipes.iter().any(|pipe| {
                ids.contains(&first_segment(&pipe.from)) || ids.contains(&first_segment(&pipe.to))
            }) || page.objects.iter().any(|object| {
                object
                    .attach
                    .as_ref()
                    .is_some_and(|attach| ids.contains(attach))
            });
            if dependent && operation.get("removeDependents") != Some(&Value::Bool(true)) {
                return Err(AgentError::conflict(
                    "dependent pipes or attachments must be included in removal.",
                ));
            }
            *page = delete_objects(page, &ids);
            Ok(())
        }
        "bind" => bind(page, operation, catalogue),
        "unbind" => {
            let index = object_index(page, operation.get("id"))?;
            ensure_object_movable(&page.objects[index])?;
            let property = property_name(operation.get("property"))?;
            page.objects[index].bind.remove(&property);
            Ok(())
        }
        "update" => update(page, operation),
        "add" => add(page, operation, catalogue),
        other => Err(unsupported(other)),
    }
}

/// Applies a move, resize, alignment, distribution, equal sizing, regional arrangement, reorder
/// or duplication.
fn apply_geometry(
    page: &mut HmiPage,
    kind: &str,
    operation: &Value,
    page_size: Option<[u32; 2]>,
) -> AgentResult<()> {
    match kind {
        "move" => {
            let ids = selected(page, operation.get("ids"))?;
            ensure_movable(page, &ids)?;
            let delta = pair(operation.get("delta"), i64::MIN)?;
            for object in objects_mut(page, &ids) {
                object.at = [
                    object.at[0].saturating_add(delta[0]),
                    object.at[1].saturating_add(delta[1]),
                ];
            }
            Ok(())
        }
        "resize" => {
            let index = object_index(page, operation.get("id"))?;
            ensure_object_movable(&page.objects[index])?;
            let size = pair(operation.get("size"), 8)?;
            page.objects[index].size = [to_size(size[0])?, to_size(size[1])?];
            Ok(())
        }
        "align" => {
            let ids = selected(page, operation.get("ids"))?;
            ensure_movable(page, &ids)?;
            let edge = operation
                .get("edge")
                .and_then(Value::as_str)
                .and_then(AlignEdge::parse)
                .ok_or_else(|| AgentError::invalid("unknown alignment."))?;
            *page = align_objects(page, &ids, edge);
            Ok(())
        }
        "distribute" => {
            let ids = selected(page, operation.get("ids"))?;
            ensure_movable(page, &ids)?;
            let axis = operation
                .get("axis")
                .and_then(Value::as_str)
                .and_then(DistributeAxis::parse)
                .ok_or_else(|| AgentError::invalid("unknown distribution axis."))?;
            *page = distribute_objects(page, &ids, axis);
            Ok(())
        }
        "equal-size" => {
            let ids = selected(page, operation.get("ids"))?;
            ensure_movable(page, &ids)?;
            let reference = object_id(page, operation.get("reference"))?;
            *page = equal_size(page, &ids, &reference, operation.get("axis"))?;
            Ok(())
        }
        "arrange" => {
            let ids = selected(page, operation.get("ids"))?;
            *page = arrange_region(page, &ids, operation, page_size)?;
            Ok(())
        }
        "reorder" => reorder(page, operation),
        "duplicate" => duplicate(page, operation),
        other => Err(unsupported(other)),
    }
}

/// Moves the selected objects to the front or the back of the z-order. The selected objects keep
/// the order in which the request named them.
fn reorder(page: &mut HmiPage, operation: &Value) -> AgentResult<()> {
    let ids = selected(page, operation.get("ids"))?;
    ensure_movable(page, &ids)?;
    let front = match operation.get("position").and_then(Value::as_str) {
        Some("front") => true,
        Some("back") => false,
        _ => return Err(AgentError::invalid("choose front or back.")),
    };
    let chosen: Vec<HmiObject> = ids
        .iter()
        .filter_map(|id| page.objects.iter().find(|object| object.id == *id).cloned())
        .collect();
    let remaining: Vec<HmiObject> = page
        .objects
        .iter()
        .filter(|object| !ids.contains(&object.id))
        .cloned()
        .collect();
    page.objects = if front {
        [remaining, chosen].concat()
    } else {
        [chosen, remaining].concat()
    };
    Ok(())
}

/// Copies the selected objects by the given offset, each copy taking the ID the request chose
/// for its original (`duplicate` operation).
fn duplicate(page: &mut HmiPage, operation: &Value) -> AgentResult<()> {
    let ids = selected(page, operation.get("ids"))?;
    let delta = pair(operation.get("delta"), i64::MIN)?;
    let new_ids = new_ids(operation.get("newIds"), ids.len())?;
    let taken = new_ids
        .iter()
        .enumerate()
        .any(|(index, id)| new_ids[..index].contains(id) || has_object(page, id));
    if taken {
        return Err(AgentError::conflict("new ids must be unique."));
    }
    *page = duplicate_objects(page, &ids, &new_ids, delta)?;
    Ok(())
}

/// Applies a pipe operation: reroute, reconnect, connect or disconnect.
fn apply_pipes(
    page: &mut HmiPage,
    kind: &str,
    operation: &Value,
    catalogue: &Catalogue,
) -> AgentResult<()> {
    match kind {
        "reroute" => {
            let index = connection_index(page, operation.get("from"), operation.get("to"))?;
            let waypoints = operation
                .get("waypoints")
                .and_then(Value::as_array)
                .filter(|list| list.len() <= MAX_WAYPOINTS)
                .ok_or_else(|| AgentError::limit("provide up to 64 pipe waypoints."))?;
            let points = waypoints
                .iter()
                .map(|value| pair(Some(value), i64::MIN))
                .collect::<AgentResult<Vec<[i64; 2]>>>()?;
            page.pipes[index].waypoints = points;
            Ok(())
        }
        "reconnect" => {
            let index = connection_index(page, operation.get("from"), operation.get("to"))?;
            *page = reconnect_pipe(
                page,
                index,
                operation.get("newFrom"),
                operation.get("newTo"),
                catalogue,
            )?;
            Ok(())
        }
        "connect" => {
            *page = connect_ports(page, operation.get("from"), operation.get("to"), catalogue)?;
            Ok(())
        }
        "disconnect" => {
            *page = disconnect_ports(page, operation.get("from"), operation.get("to"))?;
            Ok(())
        }
        other => Err(unsupported(other)),
    }
}

/// Reads one JSON value as a page field's type, refusing a value of the wrong kind.
fn page_field<T: DeserializeOwned>(value: Value) -> AgentResult<T> {
    serde_json::from_value(value).map_err(|_| AgentError::invalid("unsupported page property."))
}

/// Sets page properties: title, order, icon, group, hidden and incomplete (`page` operation).
fn apply_page_properties(page: &mut HmiPage, properties: Option<&Value>) -> AgentResult<()> {
    let object = properties
        .and_then(Value::as_object)
        .ok_or_else(|| AgentError::invalid("unsupported page property."))?;
    if object.keys().any(|key| {
        !matches!(
            key.as_str(),
            "title" | "order" | "icon" | "group" | "hidden" | "incomplete"
        )
    }) {
        return Err(AgentError::invalid("unsupported page property."));
    }
    for (key, value) in object {
        let value = value.clone();
        match key.as_str() {
            "title" => page.title = page_field(value)?,
            "order" => page.order = page_field(value)?,
            "icon" => page.icon = page_field(value)?,
            "group" => page.group = page_field(value)?,
            "hidden" => page.hidden = page_field(value)?,
            _ => page.incomplete = page_field(value)?,
        }
    }
    Ok(())
}

/// Binds a property of an object to a catalogue property with a direction the property allows
/// (`bind` operation).
fn bind(page: &mut HmiPage, operation: &Value, catalogue: &Catalogue) -> AgentResult<()> {
    let index = object_index(page, operation.get("id"))?;
    ensure_object_movable(&page.objects[index])?;
    let property = property_name(operation.get("property"))?;
    let refused = || AgentError::validation("unsupported binding property or direction.");
    let component = catalogue.component(&page.objects[index].kind);
    let definition = component.and_then(|component| {
        component
            .properties
            .get(&property)
            .or(component.additional.as_ref())
    });
    let binding = operation
        .get("binding")
        .filter(|value| value.is_object())
        .ok_or_else(refused)?;
    let path_ok = binding
        .get("path")
        .and_then(Value::as_str)
        .is_some_and(|path| !path.trim().is_empty());
    let direction: HmiBindingDirection = binding
        .get("direction")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .ok_or_else(refused)?;
    let allowed = definition.is_some_and(|definition| definition.directions.contains(&direction));
    if !path_ok || !allowed {
        return Err(refused());
    }
    let parsed: HmiBinding = serde_json::from_value(binding.clone()).map_err(|_| refused())?;
    page.objects[index].bind.insert(property, parsed);
    Ok(())
}

/// Sets the allowed properties of one object (`update` operation). A locked object changes only
/// by being unlocked.
fn update(page: &mut HmiPage, operation: &Value) -> AgentResult<()> {
    let index = object_index(page, operation.get("id"))?;
    let unsupported = || AgentError::invalid("unsupported object property.");
    let properties = operation
        .get("properties")
        .and_then(Value::as_object)
        .filter(|properties| {
            properties
                .keys()
                .all(|key| EDITABLE.contains(&key.as_str()))
        })
        .ok_or_else(unsupported)?;
    let only_unlock =
        properties.len() == 1 && properties.get("locked") == Some(&Value::Bool(false));
    if page.objects[index].locked && !only_unlock {
        return Err(AgentError::conflict("a selected object is locked."));
    }
    for (key, value) in properties {
        page.objects[index] = with_property(&page.objects[index], key, value)?;
    }
    Ok(())
}

/// The object with one property replaced, checked by the object's own model.
fn with_property(object: &HmiObject, key: &str, value: &Value) -> AgentResult<HmiObject> {
    let unsupported = || AgentError::invalid("unsupported object property.");
    let mut fields: Map<String, Value> = match serde_json::to_value(object) {
        Ok(Value::Object(fields)) => fields,
        _ => return Err(unsupported()),
    };
    fields.insert(key.to_owned(), value.clone());
    serde_json::from_value(Value::Object(fields)).map_err(|_| unsupported())
}

/// Adds an object to the page (`add` operation): an unused identifier, a catalogue type or the
/// placed faceplate, and a size of at least 8 pixels on each side.
fn add(page: &mut HmiPage, operation: &Value, catalogue: &Catalogue) -> AgentResult<()> {
    let value = operation
        .get("object")
        .ok_or_else(|| AgentError::invalid("invalid object id."))?;
    let object = object_from_json(value)?;
    name_text(&object.id)?;
    if has_object(page, &object.id) {
        return Err(AgentError::conflict("object id exists."));
    }
    if catalogue.component(&object.kind).is_none() && object.kind != PLACED_FACEPLATE {
        return Err(AgentError::invalid("unknown component type."));
    }
    if object.size.iter().any(|side| *side < 8) {
        return Err(AgentError::invalid("expected two integer coordinates."));
    }
    page.objects.push(object);
    Ok(())
}

/// Reads an object from its JSON, as its own model reads it.
///
/// # Errors
///
/// Returns `INVALID_ARGUMENT` naming the field the object model refuses.
pub(super) fn object_from_json(value: &Value) -> AgentResult<HmiObject> {
    serde_json::from_value(value.clone())
        .map_err(|error| AgentError::invalid(format!("invalid object: {error}.")))
}

/// Reads a page from its JSON, as its own model reads it.
///
/// # Errors
///
/// Returns `INVALID_ARGUMENT` naming the field the page model refuses.
pub(super) fn page_from_json(value: &Value) -> AgentResult<HmiPage> {
    serde_json::from_value(value.clone())
        .map_err(|error| AgentError::invalid(format!("invalid page: {error}.")))
}

/// The IDs of the selected objects, in the order they were named (`selected` in `edits.ts`).
fn selected(page: &HmiPage, value: Option<&Value>) -> AgentResult<Vec<String>> {
    let list = value
        .and_then(Value::as_array)
        .filter(|list| !list.is_empty() && has_no_repeats(list))
        .ok_or_else(|| AgentError::invalid("provide unique selected object ids."))?;
    list.iter()
        .map(|id| object_index(page, Some(id)).map(|index| page.objects[index].id.clone()))
        .collect()
}

/// Whether a list of JSON values has no repeated value.
fn has_no_repeats(list: &[Value]) -> bool {
    list.iter()
        .enumerate()
        .all(|(index, value)| !list[..index].contains(value))
}

/// The index of the object with the given ID.
fn object_index(page: &HmiPage, value: Option<&Value>) -> AgentResult<usize> {
    value
        .and_then(Value::as_str)
        .and_then(|id| page.objects.iter().position(|object| object.id == id))
        .ok_or_else(|| AgentError::invalid(format!("unknown object {}.", js_text(value))))
}

/// The ID of an object that the page holds.
fn object_id(page: &HmiPage, value: Option<&Value>) -> AgentResult<String> {
    object_index(page, value).map(|index| page.objects[index].id.clone())
}

/// The mutable objects with the given IDs, in page order.
fn objects_mut<'a>(
    page: &'a mut HmiPage,
    ids: &'a [String],
) -> impl Iterator<Item = &'a mut HmiObject> + 'a {
    page.objects
        .iter_mut()
        .filter(move |object| ids.contains(&object.id))
}

/// Whether the page holds an object with this ID.
fn has_object(page: &HmiPage, id: &str) -> bool {
    page.objects.iter().any(|object| object.id == id)
}

/// Refuses a selection that holds a locked object.
fn ensure_movable(page: &HmiPage, ids: &[String]) -> AgentResult<()> {
    if page
        .objects
        .iter()
        .any(|object| object.locked && ids.contains(&object.id))
    {
        return Err(AgentError::conflict("a selected object is locked."));
    }
    Ok(())
}

/// Refuses a locked object.
fn ensure_object_movable(object: &HmiObject) -> AgentResult<()> {
    if object.locked {
        return Err(AgentError::conflict("a selected object is locked."));
    }
    Ok(())
}

/// The new IDs of a duplication: one per object, each a valid identifier.
fn new_ids(value: Option<&Value>, count: usize) -> AgentResult<Vec<String>> {
    let list = value
        .and_then(Value::as_array)
        .filter(|list| list.len() == count)
        .ok_or_else(|| AgentError::invalid("provide one new id per object."))?;
    list.iter().map(|id| name(Some(id))).collect()
}

/// A valid object identifier, read from a JSON value (`name` in `edits.ts`).
fn name(value: Option<&Value>) -> AgentResult<String> {
    let text = value
        .and_then(Value::as_str)
        .ok_or_else(|| AgentError::invalid("invalid object id."))?;
    name_text(text)?;
    Ok(text.to_owned())
}

/// Checks an object identifier: a letter, then letters, digits, `_` or `-`.
fn name_text(text: &str) -> AgentResult<()> {
    let mut characters = text.chars();
    let starts = characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic());
    if starts
        && characters.all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        })
    {
        return Ok(());
    }
    Err(AgentError::invalid("invalid object id."))
}

/// A binding property name: an identifier, as `ID` in `edits.ts` requires.
fn property_name(value: Option<&Value>) -> AgentResult<String> {
    let text = value
        .and_then(Value::as_str)
        .ok_or_else(|| AgentError::invalid("invalid binding property."))?;
    name_text(text).map_err(|_| AgentError::invalid("invalid binding property."))?;
    Ok(text.to_owned())
}

/// The text before the first dot of a port (`pipe.from.split(".")[0]`).
fn first_segment(port: &str) -> String {
    port.split('.').next().unwrap_or_default().to_owned()
}

/// Two whole coordinates, each at least `minimum` (`pair` in `edits.ts`).
pub(super) fn pair(value: Option<&Value>, minimum: i64) -> AgentResult<[i64; 2]> {
    let invalid = || AgentError::invalid("expected two integer coordinates.");
    let list = value
        .and_then(Value::as_array)
        .filter(|list| list.len() == 2)
        .ok_or_else(invalid)?;
    let first = list[0]
        .as_i64()
        .filter(|number| *number >= minimum)
        .ok_or_else(invalid)?;
    let second = list[1]
        .as_i64()
        .filter(|number| *number >= minimum)
        .ok_or_else(invalid)?;
    Ok([first, second])
}

/// A side of an object as the model stores it.
fn to_size(side: i64) -> AgentResult<u32> {
    u32::try_from(side).map_err(|_| AgentError::invalid("expected two integer coordinates."))
}

/// A JSON value as the text a message shows (`String(value)` in the TypeScript).
pub(super) fn js_text(value: Option<&Value>) -> String {
    match value {
        None => "undefined".to_owned(),
        Some(Value::String(text)) => text.clone(),
        Some(Value::Null) => "null".to_owned(),
        Some(other) => other.to_string(),
    }
}

#[cfg(test)]
#[path = "edits_tests.rs"]
mod tests;
