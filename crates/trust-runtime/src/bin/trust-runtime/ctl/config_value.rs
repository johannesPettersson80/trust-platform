//! The value of `ctl config-set <key> <value>` (specification 11, 6.8.1): what `config.set`
//! receives for it, and the note on the keys that take effect only at the next start, which
//! reads `runtime.toml`: `config-set` does not write it.

use serde_json::Value as JsonValue;

/// The value `config.set` receives: `null`, `true`, `false` and a whole number as that JSON
/// value; a value that begins with `[` or `{` as the JSON array or object it is, which
/// [`check_config_value`] checked before anything was sent; any other value as a string.
pub(super) fn parse_config_value(value: &str) -> JsonValue {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("null") {
        return JsonValue::Null;
    }
    if trimmed.eq_ignore_ascii_case("true") {
        return JsonValue::Bool(true);
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return JsonValue::Bool(false);
    }
    if let Ok(number) = trimmed.parse::<i64>() {
        return JsonValue::Number(number.into());
    }
    if is_structured(trimmed)
        && let Ok(structured) = serde_json::from_str(trimmed)
    {
        return structured;
    }
    JsonValue::String(trimmed.to_string())
}

/// Refuses, naming the key, a value that begins with `[` or `{` and is no JSON, before `ctl`
/// connects: such a value would otherwise reach the runtime as a string and be refused there
/// with a type error that does not show the typo.
pub(super) fn check_config_value(key: &str, value: &str) -> anyhow::Result<()> {
    let trimmed = value.trim();
    if is_structured(trimmed) {
        serde_json::from_str::<JsonValue>(trimmed).map_err(|error| {
            anyhow::anyhow!(
                "config-set {key}: a value that begins with '[' or '{{' is sent as JSON, and this one is not JSON: {error}"
            )
        })?;
    }
    Ok(())
}

/// The note on the keys of a `config.set` answer that take effect only after a restart: the
/// running service keeps its value, and the next start reads `runtime.toml`, which `config-set`
/// does not write. `None` when every key took effect at once.
pub(super) fn restart_required_note(response: &JsonValue) -> Option<String> {
    let keys: Vec<&str> = response
        .get("result")?
        .get("restart_required")?
        .as_array()?
        .iter()
        .filter_map(JsonValue::as_str)
        .collect();
    let verb = match keys.len() {
        0 => return None,
        1 => "takes",
        _ => "take",
    };
    Some(format!(
        "config-set: {} {verb} effect only after a restart, and the next start reads runtime.toml, \
         which config-set does not write: set the same value there.",
        keys.join(", ")
    ))
}

fn is_structured(value: &str) -> bool {
    value.starts_with('[') || value.starts_with('{')
}

#[cfg(test)]
#[path = "config_value_tests.rs"]
mod tests;
