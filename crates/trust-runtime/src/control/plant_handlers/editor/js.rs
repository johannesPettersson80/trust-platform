//! The plant editor's draft as the VS Code editor holds it: a JSON document whose numbers come from
//! JavaScript. A field the user left empty is absent (`undefined`), and a number that is not finite
//! (`NaN` from unparsable text, an infinity) arrives as `null`, because JSON has no such numbers. The
//! rules read the draft with these meanings, so that an empty optional field and an invalid entry are
//! told apart as the editor tells them apart.

use serde_json::{Map, Number, Value};

/// A draft number with its JavaScript meaning.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Num {
    /// The field is absent: the user left it empty or never set it.
    Absent,
    /// A value that is not a finite number: `null` (JavaScript's `NaN` or an infinity), or a value of
    /// another type.
    NotFinite,
    /// A finite number.
    Finite(f64),
}

impl Num {
    /// The finite value, `None` for an absent field and for one that is not a finite number.
    pub(super) fn finite(self) -> Option<f64> {
        match self {
            Self::Finite(value) => Some(value),
            Self::Absent | Self::NotFinite => None,
        }
    }

    /// Whether the field is present, finite or not (`value !== undefined` in the editor).
    pub(super) fn present(self) -> bool {
        !matches!(self, Self::Absent)
    }

    /// The JavaScript comparison `value < limit`: false for an absent field and for one that is not
    /// finite, as a comparison with `undefined` or `NaN` is.
    pub(super) fn below(self, limit: f64) -> bool {
        self.finite().is_some_and(|value| value < limit)
    }

    /// `Number.isSafeInteger(value)`.
    pub(super) fn safe_integer(self) -> Option<i64> {
        self.finite().and_then(safe_integer)
    }
}

/// The meaning of `value`: absent, not finite, or finite.
pub(super) fn num(value: Option<&Value>) -> Num {
    match value {
        None => Num::Absent,
        Some(Value::Number(number)) => number
            .as_f64()
            .filter(|value| value.is_finite())
            .map_or(Num::NotFinite, Num::Finite),
        Some(_) => Num::NotFinite,
    }
}

/// The meaning of the member `key` of `object`.
pub(super) fn field(object: &Map<String, Value>, key: &str) -> Num {
    num(object.get(key))
}

/// `Number.isSafeInteger(value)` for a finite `value`: whole and within 2^53 − 1.
pub(super) fn safe_integer(value: f64) -> Option<i64> {
    const MAX_SAFE: f64 = 9_007_199_254_740_991.0;
    (value.fract() == 0.0 && value.abs() <= MAX_SAFE).then_some(value as i64)
}

/// A number as JavaScript's `JSON.stringify` writes it: a whole number without a fraction, any other
/// finite number as the nearest double, and `null` for a value that is not finite.
pub(super) fn js_number(value: f64) -> Value {
    if !value.is_finite() {
        return Value::Null;
    }
    if value.fract() == 0.0 && value.abs() < 9_007_199_254_740_992.0 {
        return Value::from(value as i64);
    }
    Number::from_f64(value).map_or(Value::Null, Value::Number)
}

/// JavaScript's `Number(text)`: surrounding whitespace ignored, empty text 0, a decimal number with
/// an optional sign and exponent, an unsigned `0x`, `0o` or `0b` integer, and `NaN` for anything
/// else (an infinity is not finite, so it reads as `NaN` here too).
pub(super) fn number_from_text(text: &str) -> f64 {
    let text = text.trim();
    if text.is_empty() {
        return 0.0;
    }
    for (prefix, radix) in [("0x", 16), ("0o", 8), ("0b", 2)] {
        if text.len() > 2 && text[..2].eq_ignore_ascii_case(prefix) {
            return u64::from_str_radix(&text[2..], radix).map_or(f64::NAN, |value| value as f64);
        }
    }
    let decimal = text
        .chars()
        .all(|ch| ch.is_ascii_digit() || matches!(ch, '+' | '-' | '.' | 'e' | 'E'));
    if !decimal {
        return f64::NAN;
    }
    text.parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .unwrap_or(f64::NAN)
}

/// The numbers of an array, each with its meaning; an absent or non-array value has none.
pub(super) fn numbers(value: Option<&Value>) -> Option<Vec<Num>> {
    value
        .and_then(Value::as_array)
        .map(|values| values.iter().map(|value| num(Some(value))).collect())
}

/// Every number in a value, arrays flattened at any depth (`Array.flat(Infinity)`): a scalar is one
/// number, an empty array none.
pub(super) fn flattened(value: &Value) -> Vec<Num> {
    match value {
        Value::Array(values) => values.iter().flat_map(flattened).collect(),
        other => vec![num(Some(other))],
    }
}

/// A string member, or `""` when it is absent or not a string.
pub(super) fn text<'a>(object: &'a Map<String, Value>, key: &str) -> &'a str {
    object.get(key).and_then(Value::as_str).unwrap_or("")
}

/// A boolean member, `default` when it is absent or not a boolean.
pub(super) fn flag(object: &Map<String, Value>, key: &str, default: bool) -> bool {
    object.get(key).and_then(Value::as_bool).unwrap_or(default)
}

/// The string array member `key`; absent or not an array gives an empty list, and an entry that is
/// not a string reads as `""` (an unbound port).
pub(super) fn strings(object: &Map<String, Value>, key: &str) -> Vec<String> {
    object
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .map(|value| value.as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// The object member `key`, or an empty object.
pub(super) fn object<'a>(value: &'a Map<String, Value>, key: &str) -> &'a Map<String, Value> {
    static EMPTY: std::sync::OnceLock<Map<String, Value>> = std::sync::OnceLock::new();
    value
        .get(key)
        .and_then(Value::as_object)
        .unwrap_or_else(|| EMPTY.get_or_init(Map::new))
}

/// The array member `key` as objects (rows); a row that is not an object reads as an empty one.
pub(super) fn rows(value: &Map<String, Value>, key: &str) -> Vec<Map<String, Value>> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .map(|value| value.as_object().cloned().unwrap_or_default())
                .collect()
        })
        .unwrap_or_default()
}

/// A value as JavaScript's `JSON.stringify` shows it in a message: the editor's summary repeats the
/// offending value this way.
pub(super) fn shown(value: Option<&Value>) -> String {
    match value {
        None => "undefined".to_string(),
        Some(value) => serde_json::to_string(value).unwrap_or_else(|_| "null".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_draft_number_reads_as_absent_not_finite_or_finite() {
        // The editor sends `NaN` as `null` and leaves an empty field out (JSON has no NaN or
        // undefined). Catches a reader that takes `null` for zero or for an empty field.
        let object = json!({"a": 1.5, "b": null, "c": "1"});
        let object = object.as_object().unwrap();
        assert_eq!(field(object, "a"), Num::Finite(1.5));
        assert_eq!(field(object, "b"), Num::NotFinite);
        assert_eq!(field(object, "c"), Num::NotFinite);
        assert_eq!(field(object, "missing"), Num::Absent);
        assert!(!Num::NotFinite.below(0.0), "NaN < 0 is false in JavaScript");
        assert!(Num::Finite(-1.0).below(0.0));
    }

    #[test]
    fn numbers_are_written_as_javascript_writes_them() {
        // `JSON.stringify(2)` is `2`, `JSON.stringify(0.5)` is `0.5` and `JSON.stringify(NaN)` is
        // `null`. Catches a whole number written as `2.0`, which TOML would then read as a float for
        // an integer field such as `at_ms`.
        assert_eq!(serde_json::to_string(&js_number(2.0)).unwrap(), "2");
        assert_eq!(serde_json::to_string(&js_number(-3.0)).unwrap(), "-3");
        assert_eq!(serde_json::to_string(&js_number(0.5)).unwrap(), "0.5");
        assert_eq!(js_number(f64::NAN), Value::Null);
        assert_eq!(
            safe_integer(9_007_199_254_740_991.0),
            Some(9_007_199_254_740_991)
        );
        assert_eq!(safe_integer(9_007_199_254_740_992.0), None);
        assert_eq!(safe_integer(1.5), None);
    }
}
