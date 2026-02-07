//! Shared helpers for client settings lookup with alias fallback.

use serde_json::Value;

/// Returns the root LSP settings section (`stLsp`, `trust-lsp`, `trust_lsp`).
pub(crate) fn lsp_section(value: &Value) -> Option<&Value> {
    value
        .get("stLsp")
        .or_else(|| value.get("trust-lsp"))
        .or_else(|| value.get("trust_lsp"))
}

/// Returns the runtime subsection from client settings.
///
/// Supports both:
/// - nested: `{ "stLsp": { "runtime": { ... } } }`
/// - direct: `{ "runtime": { ... } }`
pub(crate) fn lsp_runtime_section(value: &Value) -> Option<&Value> {
    let section = lsp_section(value).or_else(|| value.get("runtime").is_some().then_some(value));
    section.and_then(|section| section.get("runtime"))
}

/// Returns the first key match (order is precedence).
pub(crate) fn value_with_aliases<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| value.get(*key))
}

/// Returns the first boolean key match (order is precedence).
pub(crate) fn bool_with_aliases(value: &Value, keys: &[&str]) -> Option<bool> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_bool))
}

/// Returns the first string key match (order is precedence).
pub(crate) fn string_with_aliases<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn lsp_section_prefers_stlsp_then_trust_lsp_aliases() {
        let value = json!({
            "stLsp": { "name": "st" },
            "trust-lsp": { "name": "hyphen" },
            "trust_lsp": { "name": "snake" },
        });
        assert_eq!(
            lsp_section(&value)
                .and_then(|section| section.get("name"))
                .and_then(Value::as_str),
            Some("st")
        );
    }

    #[test]
    fn runtime_section_supports_nested_and_top_level_runtime() {
        let nested = json!({
            "trust-lsp": {
                "runtime": { "enabled": true }
            }
        });
        assert_eq!(
            lsp_runtime_section(&nested)
                .and_then(|section| section.get("enabled"))
                .and_then(Value::as_bool),
            Some(true)
        );

        let top_level = json!({
            "runtime": { "enabled": false }
        });
        assert_eq!(
            lsp_runtime_section(&top_level)
                .and_then(|section| section.get("enabled"))
                .and_then(Value::as_bool),
            Some(false)
        );
    }

    #[test]
    fn alias_lookup_prefers_first_key_and_ignores_wrong_types() {
        let value = json!({
            "camel": true,
            "snake": false,
            "not_bool": "value",
            "alt_bool": true,
            "camel_text": "A",
            "snake_text": "B",
        });

        assert_eq!(bool_with_aliases(&value, &["camel", "snake"]), Some(true));
        assert_eq!(
            bool_with_aliases(&value, &["not_bool", "alt_bool"]),
            Some(true)
        );
        assert_eq!(
            string_with_aliases(&value, &["camel_text", "snake_text"]),
            Some("A")
        );
        assert_eq!(
            value_with_aliases(&value, &["snake", "camel"]).and_then(Value::as_bool),
            Some(false)
        );
    }
}
