//! Decoding JSON with the same numbers whatever `serde_json`'s features are.
//!
//! A workspace build unifies the features of `serde_json`, and the runtime's OPC UA library turns on
//! `arbitrary_precision`. With it, a fractional number inside an untagged or internally tagged enum,
//! or inside a `#[serde(flatten)]` field, reaches serde as a map and no longer decodes as a number,
//! and a number beyond the range of `f64` decodes as infinity instead of being refused. Every stored
//! document and runtime answer is therefore parsed into a [`Value`] first: each fractional number is
//! rewritten as the shortest text of the `f64` it denotes, which decodes as a number with or without
//! the feature, and a number beyond the range of `f64` is refused, as it is without the feature.
//! Integers stay integers, and a fraction keeps the exact `f64` it denotes (the crate decodes with
//! `float_roundtrip`).

use std::collections::BTreeMap;

use schemars::{json_schema, Schema, SchemaGenerator};
use serde::de::{DeserializeOwned, Error as _};
use serde::{Deserialize, Deserializer};
use serde_json::{Number, Value};

/// Decodes `text` as `T`.
///
/// # Errors
/// The text is not JSON, holds a number beyond the range of `f64`, or is not a `T`.
pub fn decode_text<T: DeserializeOwned>(text: &str) -> Result<T, serde_json::Error> {
    decode_value(serde_json::from_str(text)?)
}

/// Decodes `value` as `T`.
///
/// # Errors
/// The value holds a number beyond the range of `f64`, or is not a `T`.
pub fn decode_value<T: DeserializeOwned>(value: Value) -> Result<T, serde_json::Error> {
    serde_json::from_value(normalized(value)?)
}

/// Parses `text` into a value whose fractional numbers are in their shortest form, so the value
/// decodes, compares and prints the same with or without `arbitrary_precision`.
///
/// # Errors
/// The text is not JSON, or holds a number beyond the range of `f64`.
pub fn parse_text(text: &str) -> Result<Value, serde_json::Error> {
    serde_json::from_str(text).and_then(normalized)
}

/// `value` with every number that is not an integer rewritten as the shortest text of its `f64`.
fn normalized(value: Value) -> Result<Value, serde_json::Error> {
    Ok(match value {
        Value::Number(number) => Value::Number(shortest(number)?),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(normalized)
                .collect::<Result<_, _>>()?,
        ),
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(key, item)| normalized(item).map(|item| (key, item)))
                .collect::<Result<_, _>>()?,
        ),
        other => other,
    })
}

/// An integer as it is; a fraction as the shortest text of its `f64`.
fn shortest(number: Number) -> Result<Number, serde_json::Error> {
    if number.is_i64() || number.is_u64() {
        return Ok(number);
    }
    number
        .as_f64()
        .filter(|value| value.is_finite())
        .and_then(Number::from_f64)
        .ok_or_else(|| serde_json::Error::custom(format!("number out of range: {number}")))
}

/// The JSON Schema of a number that the page may send as text, which its decoder reads (`duration_of_setting`, `number_of_entry`):
/// a number, or a string. The generated type of such a field is `number | string`.
#[must_use]
pub fn number_or_text_schema(_generator: &mut SchemaGenerator) -> Schema {
    json_schema!({ "anyOf": [{ "type": "number" }, { "type": "string" }] })
}

/// The JSON Schema of an optional number that the page may send as text (see [`number_or_text_schema`]): absent, null, a number or a
/// string.
#[must_use]
pub fn optional_number_or_text_schema(_generator: &mut SchemaGenerator) -> Schema {
    json_schema!({ "type": ["number", "string", "null"] })
}

/// The JSON Schema of a field that holds any JSON value (a `serde_json::Value` that is read and written as it is): the generated type
/// is `unknown`. A schema that carries only the field's description is generated as an object type, which most JSON values are not;
/// the declaration generator takes `tsType` as the field's type.
#[must_use]
pub fn any_value_schema(_generator: &mut SchemaGenerator) -> Schema {
    json_schema!({ "tsType": "unknown" })
}

/// The JSON Schema of an optional list of JSON objects that the engine keeps as they are (the validation scenarios a loop file declares):
/// the generated type is a list of objects. A schema that carries only the field's description is generated as one object, which a list
/// is not.
#[must_use]
pub fn object_list_schema(_generator: &mut SchemaGenerator) -> Schema {
    json_schema!({ "type": "array", "items": { "type": "object" } })
}

/// A number that the page sent as a JSON number or as the text it typed. Text that no finite number reads, and any other value, is NaN,
/// which no JSON number is, so that the checks refuse it under its field.
#[must_use]
pub fn number_or_text(value: &Value) -> f64 {
    match value {
        Value::Number(number) => number
            .as_f64()
            .filter(|value| value.is_finite())
            .unwrap_or(f64::NAN),
        Value::String(text) => text
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .unwrap_or(f64::NAN),
        _ => f64::NAN,
    }
}

/// A map of numbers that the page may send as text, each read by [`number_or_text`]: an entry that no number reads is NaN.
///
/// # Errors
/// The field is present but is not a map.
pub fn number_or_text_map<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<BTreeMap<String, f64>>, D::Error> {
    Ok(
        Option::<BTreeMap<String, Value>>::deserialize(deserializer)?.map(|entries| {
            entries
                .iter()
                .map(|(symbol, value)| (symbol.clone(), number_or_text(value)))
                .collect()
        }),
    )
}

/// The JSON Schema of an optional map of numbers that the page may send as text (see [`number_or_text_map`]).
#[must_use]
pub fn optional_number_or_text_map_schema(_generator: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": ["object", "null"],
        "additionalProperties": { "anyOf": [{ "type": "number" }, { "type": "string" }] }
    })
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde_json::json;

    use super::*;

    #[test]
    fn a_map_of_numbers_reads_text_as_its_number_and_no_text_as_nan() {
        #[derive(Debug, Deserialize)]
        struct Holder {
            #[serde(default, deserialize_with = "number_or_text_map")]
            settings: Option<BTreeMap<String, f64>>,
        }
        let holder: Holder = decode_value(json!({
            "settings": { "Main.Kp": "1.5", "Main.Ki": 2, "Main.Kd": "", "Main.Kt": "x" }
        }))
        .expect("a map of numbers and text decodes");
        let settings = holder.settings.expect("the map is sent");
        assert!((settings["Main.Kp"] - 1.5).abs() < 1e-12);
        assert!((settings["Main.Ki"] - 2.0).abs() < 1e-12);
        assert!(settings["Main.Kd"].is_nan(), "a blank entry is no number");
        assert!(
            settings["Main.Kt"].is_nan(),
            "text no number reads is refused"
        );
        let absent: Holder = decode_value(json!({})).expect("an absent map decodes");
        assert!(absent.settings.is_none());
    }

    #[derive(Debug, PartialEq, Deserialize)]
    #[serde(tag = "kind", rename_all = "lowercase")]
    enum Law {
        Pi { gain: f64 },
    }

    #[derive(Debug, PartialEq, Deserialize)]
    #[serde(untagged)]
    enum NumberOrBoolean {
        Number(f64),
        Boolean(bool),
    }

    #[derive(Debug, PartialEq, Deserialize)]
    struct Outer {
        #[serde(flatten)]
        inner: Inner,
    }

    #[derive(Debug, PartialEq, Deserialize)]
    struct Inner {
        value: f64,
    }

    // The crate's tests run with `arbitrary_precision` (a dev-dependency feature), so each case
    // below fails through serde_json::from_str when the module is bypassed.
    #[test]
    fn fractions_decode_inside_tagged_and_untagged_enums_and_flattened_fields() {
        for text in [
            r#"{"kind":"pi","gain":1.5}"#,
            r#"{"kind":"pi","gain":1.50}"#,
            r#"{"kind":"pi","gain":15e-1}"#,
        ] {
            assert_eq!(decode_text::<Law>(text).expect(text), Law::Pi { gain: 1.5 });
        }
        assert_eq!(
            decode_text::<Law>(r#"{"kind":"pi","gain":2}"#).expect("an integer"),
            Law::Pi { gain: 2.0 }
        );
        assert_eq!(
            decode_text::<NumberOrBoolean>("0.25").expect("a fraction"),
            NumberOrBoolean::Number(0.25)
        );
        assert_eq!(
            decode_text::<NumberOrBoolean>("true").expect("a Boolean"),
            NumberOrBoolean::Boolean(true)
        );
        assert_eq!(
            decode_text::<Outer>(r#"{"value":0.1}"#).expect("flattened"),
            Outer {
                inner: Inner { value: 0.1 }
            }
        );
        assert_eq!(
            decode_value::<Law>(json!({ "kind": "pi", "gain": 0.75 })).expect("a value"),
            Law::Pi { gain: 0.75 }
        );
    }

    #[test]
    fn a_parsed_value_keeps_integers_and_writes_fractions_in_their_shortest_form() {
        let value =
            parse_text(r#"{"count":3,"gain":1.50,"small":1e-3,"list":[2.0,7]}"#).expect("json");
        assert_eq!(value["count"].to_string(), "3");
        assert_eq!(value["gain"].to_string(), "1.5");
        assert_eq!(value["small"].to_string(), "0.001");
        assert_eq!(value["list"].to_string(), "[2.0,7]");
        assert_eq!(
            value,
            json!({ "count": 3, "gain": 1.5, "list": [2.0, 7], "small": 0.001 })
        );
    }

    #[test]
    fn a_number_out_of_the_f64_range_is_refused() {
        assert!(decode_text::<Inner>(r#"{"value":1e400}"#).is_err());
    }
}
