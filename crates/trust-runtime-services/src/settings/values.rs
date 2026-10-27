//! The exact value checks of `config.set` (specification 11, the runtime configuration contract):
//! a value of the wrong type or outside its range is refused with the key, and a check changes
//! nothing.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde_json::Value;
use trust_runtime_core::value::Duration;

/// The refusal of a value of the wrong type for `key`.
#[must_use]
pub fn config_type_error(key: &str, expected: &str) -> String {
    format!("invalid config value for '{key}': expected {expected}")
}

/// The refusal of a value of the right type that `key` does not take.
#[must_use]
pub fn config_value_error(key: &str, message: &str) -> String {
    format!("invalid config value for '{key}': {message}")
}

/// A JSON Boolean.
///
/// # Errors
///
/// [`config_type_error`] for any other value.
pub fn expect_bool(key: &str, value: &Value) -> Result<bool, String> {
    value
        .as_bool()
        .ok_or_else(|| config_type_error(key, "boolean"))
}

/// A string that is not empty once trimmed; the trimmed text.
///
/// # Errors
///
/// [`config_type_error`] for a value that is no string, [`config_value_error`] for an empty one.
pub fn expect_non_empty_string<'a>(key: &str, value: &'a Value) -> Result<&'a str, String> {
    let value = value
        .as_str()
        .ok_or_else(|| config_type_error(key, "string"))?;
    let value = value.trim();
    if value.is_empty() {
        return Err(config_value_error(key, "must not be empty"));
    }
    Ok(value)
}

/// An integer from 1 to `i64::MAX`, exactly representable.
///
/// # Errors
///
/// [`config_type_error`] for a value that is no such integer, [`config_value_error`] below 1.
pub fn expect_positive_i64(key: &str, value: &Value) -> Result<i64, String> {
    let number = value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|n| i64::try_from(n).ok()))
        .ok_or_else(|| config_type_error(key, "integer >= 1"))?;
    if number < 1 {
        return Err(config_value_error(key, "must be >= 1"));
    }
    Ok(number)
}

/// A positive millisecond count that also fits the signed nanosecond duration range.
///
/// # Errors
///
/// The errors of [`expect_positive_i64`], and [`config_value_error`] above the range.
pub fn expect_duration_millis(key: &str, value: &Value) -> Result<Duration, String> {
    let millis = expect_positive_i64(key, value)?;
    Duration::checked_from_millis(millis)
        .ok_or_else(|| config_value_error(key, &format!("must be <= {}", i64::MAX / 1_000_000)))
}

/// An array of strings, each trimmed and not empty, in order.
///
/// # Errors
///
/// [`config_type_error`] for a value that is no array, [`config_value_error`] naming the first
/// entry that is no string or is empty.
pub fn expect_string_array(key: &str, value: &Value) -> Result<Vec<String>, String> {
    let values = value
        .as_array()
        .ok_or_else(|| config_type_error(key, "array of strings"))?;
    let mut output = Vec::with_capacity(values.len());
    for (index, item) in values.iter().enumerate() {
        let Some(text) = item.as_str() else {
            return Err(config_value_error(
                key,
                &format!("entry {index} must be a string"),
            ));
        };
        let text = text.trim();
        if text.is_empty() {
            return Err(config_value_error(
                key,
                &format!("entry {index} must not be empty"),
            ));
        }
        output.push(text.to_string());
    }
    Ok(output)
}

/// An object of strings, keys and values trimmed and not empty, in key order.
///
/// # Errors
///
/// [`config_type_error`] for a value that is no object, [`config_value_error`] for an empty key
/// and naming the first entry that is no string or is empty.
pub fn expect_string_map(key: &str, value: &Value) -> Result<Vec<(String, String)>, String> {
    let values = value
        .as_object()
        .ok_or_else(|| config_type_error(key, "object of strings"))?;
    let mut output = Vec::with_capacity(values.len());
    for (map_key, map_value) in values {
        if map_key.trim().is_empty() {
            return Err(config_value_error(key, "map keys must not be empty"));
        }
        let Some(text) = map_value.as_str() else {
            return Err(config_value_error(
                key,
                &format!("entry '{map_key}' must be a string"),
            ));
        };
        let text = text.trim();
        if text.is_empty() {
            return Err(config_value_error(
                key,
                &format!("entry '{map_key}' must not be empty"),
            ));
        }
        output.push((map_key.trim().to_string(), text.to_string()));
    }
    output.sort();
    Ok(output)
}
