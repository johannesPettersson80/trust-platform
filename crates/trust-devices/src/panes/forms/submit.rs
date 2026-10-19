//! What a submitted form saves (specification 25 §5.9): the value of every field that is shown,
//! converted to the field's schema type. A field whose `visible_when` predicate does not hold is
//! ignored, and a value that does not fit its type is a field error of its field, never text.
//! The items of a list that the catalog describes are checked as the runtime's configuration
//! parser reads them (specification 11).

use serde_json::{Map, Value};

use super::catalog::{value_text, CatalogField, CatalogProtocol, ItemField, ListTexts};
use crate::texts::forms as texts;
use crate::view::types::FieldError;

/// Whether `field` of `protocol` is shown for the submitted `values`: it has no `visible_when`
/// predicate, or the value of the field the predicate names, or else that field's default,
/// equals the predicate's value (specification 25 §5.9).
#[must_use]
pub fn visible(
    protocol: &CatalogProtocol,
    field: &CatalogField,
    values: &Map<String, Value>,
) -> bool {
    let Some(rule) = &field.visible_when else {
        return true;
    };
    let controlling = values
        .get(&rule.field)
        .or_else(|| protocol.field(&rule.field).map(|field| &field.default));
    controlling.map_or_else(String::new, value_text) == rule.equals
}

/// The parameters that the submitted `values` of a form of `protocol` save: the value of every
/// field that is shown, converted to the field's schema type (specification 25 §5.9). Values of
/// hidden fields and of fields the protocol does not have are left out, and so is a secret left
/// empty as null.
///
/// # Errors
///
/// Every value that does not fit its field's type, as a field error of that field: text in a
/// number field, a value that is not one of a choice's options, a list or an object that is not
/// valid JSON, and the first item of a described list that its description refuses. Such a value
/// is never sent as text.
pub fn coerce_submission(
    protocol: &CatalogProtocol,
    values: &Map<String, Value>,
) -> Result<Map<String, Value>, Vec<FieldError>> {
    let mut params = Map::new();
    let mut errors = Vec::new();
    for field in &protocol.fields {
        let Some(value) = values.get(&field.id) else {
            continue;
        };
        if !visible(protocol, field, values) {
            continue;
        }
        match coerce(protocol, field, value, values) {
            Ok(Some(converted)) => {
                params.insert(field.id.clone(), converted);
            }
            Ok(None) => {}
            Err(message) => errors.push(FieldError {
                field: field.id.clone(),
                message,
            }),
        }
    }
    if errors.is_empty() {
        Ok(params)
    } else {
        Err(errors)
    }
}

/// `value` of `field` converted to its type; `None` for a value left out.
fn coerce(
    protocol: &CatalogProtocol,
    field: &CatalogField,
    value: &Value,
    values: &Map<String, Value>,
) -> Result<Option<Value>, String> {
    match field.field_type.as_str() {
        "number" => whole_number(value).map(Some).map_err(str::to_owned),
        "boolean" => on_off(value).map(Some).map_err(str::to_owned),
        "enum" => value
            .as_str()
            .map(str::trim)
            .filter(|text| field.options.iter().any(|option| option == text))
            .map(|text| Some(Value::String(text.to_owned())))
            .ok_or_else(|| texts::CHOOSE_LISTED_VALUE.to_owned()),
        "json_array" => {
            let items = parsed(value, Value::Array(Vec::new()))
                .and_then(|value| match value {
                    Value::Array(items) => Some(items),
                    _ => None,
                })
                .ok_or_else(|| texts::ENTER_JSON_ARRAY.to_owned())?;
            list_items(protocol, field, &items, values).map(|items| Some(Value::Array(items)))
        }
        "json_object" => parsed(value, Value::Object(Map::new()))
            .filter(Value::is_object)
            .map(Some)
            .ok_or_else(|| texts::ENTER_JSON_OBJECT.to_owned()),
        "string" | "secret" | "path" | "endpoint" => match value {
            Value::String(_) => Ok(Some(value.clone())),
            Value::Null => Ok(None),
            _ => Err(texts::ENTER_TEXT.to_owned()),
        },
        _ => Ok(Some(value.clone())),
    }
}

/// The items of the list `field`: by the catalog's description of its items when it has one,
/// else by the shape of its default items.
fn list_items(
    protocol: &CatalogProtocol,
    field: &CatalogField,
    items: &[Value],
    values: &Map<String, Value>,
) -> Result<Vec<Value>, String> {
    match field.list.as_ref() {
        Some(words) if !words.item_fields.is_empty() => {
            let is_on = |id: &str| switched_on(protocol, values, id);
            items
                .iter()
                .map(|item| described_item(words, item, &is_on))
                .collect()
        }
        Some(words) if words.item_type.is_some() => items.iter().map(unsigned).collect(),
        _ => {
            let shape = item_shape(field);
            items
                .iter()
                .map(|item| coerce_item(&shape, item).map_err(str::to_owned))
                .collect()
        }
    }
}

/// Whether the boolean field `id` of the submitted form is on, or else its default.
fn switched_on(protocol: &CatalogProtocol, values: &Map<String, Value>, id: &str) -> bool {
    values
        .get(id)
        .or_else(|| protocol.field(id).map(|field| &field.default))
        .is_some_and(|value| on_off(value) == Ok(Value::Bool(true)))
}

/// An item of a list of objects that the catalog describes: each described property converted
/// to its type, an empty one left out unless it is required, and of each group exactly one set,
/// unless the group's field is on. Any other property keeps its value.
fn described_item(
    words: &ListTexts,
    item: &Value,
    switched_on: &dyn Fn(&str) -> bool,
) -> Result<Value, String> {
    let Value::Object(given) = item else {
        return Err(texts::ENTER_JSON_OBJECT.to_owned());
    };
    let mut converted = given.clone();
    for property in &words.item_fields {
        let value = match given.get(&property.id) {
            Some(value) => described_value(property, value)?,
            None => None,
        };
        match value {
            Some(value) => {
                converted.insert(property.id.clone(), value);
            }
            None if property.required => return Err(texts::item_required(&property.label)),
            None => {
                converted.remove(&property.id);
            }
        }
    }
    for group in &words.item_one_of {
        let set = group
            .fields
            .iter()
            .filter(|id| converted.contains_key(id.as_str()))
            .count();
        let labels: Vec<&str> = group
            .fields
            .iter()
            .map(|id| {
                words
                    .item_fields
                    .iter()
                    .find(|property| property.id == *id)
                    .map_or(id.as_str(), |property| property.label.as_str())
            })
            .collect();
        match set {
            1 => {}
            0 if group.unless.as_deref().is_some_and(switched_on) => {}
            0 => return Err(texts::one_of_missing(&labels)),
            _ => return Err(texts::one_of_both(&labels)),
        }
    }
    Ok(Value::Object(converted))
}

/// A described property's value converted to its type: `None` when it is empty, an `enum` only
/// as one of its options.
fn described_value(property: &ItemField, value: &Value) -> Result<Option<Value>, String> {
    let text = match value {
        Value::String(text) => text.trim(),
        Value::Null => return Ok(None),
        _ => return Err(texts::item_text(&property.label)),
    };
    if text.is_empty() {
        return Ok(None);
    }
    if property.field_type != "enum" {
        return Ok(Some(value.clone()));
    }
    if property.options.iter().any(|option| option == text) {
        Ok(Some(Value::String(text.to_owned())))
    } else {
        Err(texts::item_choice(&property.label))
    }
}

/// An item of a list of numbers that the catalog describes: a whole number of 0 or more.
fn unsigned(value: &Value) -> Result<Value, String> {
    match value {
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.trim().parse::<u64>().ok(),
        _ => None,
    }
    .map(Value::from)
    .ok_or_else(|| texts::ENTER_UNSIGNED.to_owned())
}

/// A JSON value, or the JSON text that a text box holds; `empty` for an empty text box.
fn parsed(value: &Value, empty: Value) -> Option<Value> {
    match value {
        Value::String(text) if text.trim().is_empty() => Some(empty),
        Value::String(text) => serde_json::from_str(text).ok(),
        other => Some(other.clone()),
    }
}

fn whole_number(value: &Value) -> Result<Value, &'static str> {
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
    .map(Value::from)
    .ok_or(texts::ENTER_WHOLE_NUMBER)
}

fn on_off(value: &Value) -> Result<Value, &'static str> {
    match value {
        Value::Bool(_) => Ok(value.clone()),
        Value::String(text) if text.trim() == "true" => Ok(Value::Bool(true)),
        Value::String(text) if text.trim() == "false" => Ok(Value::Bool(false)),
        _ => Err(texts::CHOOSE_LISTED_VALUE),
    }
}

/// The shape a list's items are converted to: its default items, whose objects' properties are
/// merged, and else its template.
fn item_shape(field: &CatalogField) -> Value {
    let defaults = field
        .default
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    let template = field.list.as_ref().map(|list| &list.item_template);
    let mut shape = Value::Null;
    for item in defaults.iter().chain(template) {
        if shape.is_null() {
            shape = item.clone();
        } else if let (Value::Object(merged), Value::Object(more)) = (&mut shape, item) {
            for (key, value) in more {
                merged.entry(key.clone()).or_insert_with(|| value.clone());
            }
        }
    }
    shape
}

/// `item` converted to `shape`: a number or an on/off value of the shape is converted, and so is
/// such a property of an object; anything else keeps its value.
fn coerce_item(shape: &Value, item: &Value) -> Result<Value, &'static str> {
    match shape {
        Value::Number(_) => whole_number(item),
        Value::Bool(_) => on_off(item),
        Value::Object(properties) => {
            let Value::Object(values) = item else {
                return Err(texts::ENTER_JSON_OBJECT);
            };
            values
                .iter()
                .map(|(key, value)| {
                    let converted = match properties.get(key) {
                        Some(property @ (Value::Number(_) | Value::Bool(_))) => {
                            coerce_item(property, value)?
                        }
                        _ => value.clone(),
                    };
                    Ok((key.clone(), converted))
                })
                .collect::<Result<Map<_, _>, _>>()
                .map(Value::Object)
        }
        _ => Ok(item.clone()),
    }
}
