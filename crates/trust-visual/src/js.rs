//! The JavaScript rules the visual editors applied before their logic moved to Rust (specification
//! 25 §11.4): `String.prototype.trim`, `Number(text)`, `parseInt`, the ordering of
//! `String.prototype.localeCompare`, and how a value counts as present (truthiness), compares
//! (`===`), becomes text (`String(value)`) and lists its members (array-index names first). The editors' files keep their meaning only when Rust reads
//! names, numbers and orders the way the editor did; JSON layout and number text are in
//! [`crate::json`].

use std::cmp::Ordering;

use crate::json::number_text;
use crate::Json;

/// Whether `ch` is white space or a line terminator for JavaScript (`WhiteSpace` and
/// `LineTerminator` of ECMA-262): tab, vertical tab, form feed, space, no-break space, the byte
/// order mark, every `Zs` space, line feed, carriage return, and the line and paragraph separators.
#[must_use]
pub fn is_space(ch: char) -> bool {
    matches!(
        ch,
        '\u{9}' | '\u{a}' | '\u{b}' | '\u{c}' | '\u{d}' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

/// `text.trim()` as JavaScript trims: [`is_space`] at both ends.
#[must_use]
pub fn trim(text: &str) -> &str {
    text.trim_matches(is_space)
}

/// `Number(text)` for a string, as JavaScript reads it (`StringNumericLiteral` of ECMA-262): blank
/// text is 0; otherwise a decimal number with an optional sign, fraction and exponent,
/// `Infinity` with an optional sign, or an unsigned `0x`, `0o` or `0b` integer. Anything else is
/// `NaN`, answered as `None`.
#[must_use]
pub fn number_from_text(text: &str) -> Option<f64> {
    let text = trim(text);
    if text.is_empty() {
        return Some(0.0);
    }
    let (negative, unsigned) = match text.as_bytes()[0] {
        b'+' => (false, &text[1..]),
        b'-' => (true, &text[1..]),
        _ => (false, text),
    };
    let signed = |value: f64| if negative { -value } else { value };
    if unsigned == "Infinity" {
        return Some(signed(f64::INFINITY));
    }
    // An unsigned `0x`, `0o` or `0b` literal; a sign before one makes `NaN`.
    if unsigned.len() == text.len()
        && let Some((radix, digits)) = radix_prefix(unsigned)
    {
        return radix_value(radix, digits);
    }
    if !decimal_literal(unsigned) {
        return None;
    }
    unsigned.parse::<f64>().ok().map(signed)
}

/// The radix and digits of a text starting `0x`, `0o` or `0b` (any case).
fn radix_prefix(text: &str) -> Option<(u32, &str)> {
    let bytes = text.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'0' {
        return None;
    }
    let radix = match bytes[1] {
        b'x' | b'X' => 16,
        b'o' | b'O' => 8,
        b'b' | b'B' => 2,
        _ => return None,
    };
    Some((radix, &text[2..]))
}

/// The value of the digits of a `0x`, `0o` or `0b` literal, or `None` (`NaN`) when they are
/// missing or not digits of the radix.
fn radix_value(radix: u32, digits: &str) -> Option<f64> {
    if digits.is_empty() || !digits.chars().all(|ch| ch.is_digit(radix)) {
        return None;
    }
    // Exact up to 2^128, then rounded once; longer literals accumulate in floating point.
    if let Ok(value) = u128::from_str_radix(digits, radix) {
        // Rounds to the nearest double, as JavaScript does.
        #[allow(clippy::cast_precision_loss)]
        return Some(value as f64);
    }
    let mut value = 0.0f64;
    for ch in digits.chars() {
        value = value * f64::from(radix) + f64::from(ch.to_digit(radix).unwrap_or(0));
    }
    Some(value)
}

/// Whether `text` is an unsigned decimal literal: digits with an optional fraction, or a fraction
/// alone, then an optional exponent.
fn decimal_literal(text: &str) -> bool {
    let bytes = text.as_bytes();
    let digits = |from: usize| {
        bytes[from..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count()
    };
    let mut at = digits(0);
    let integer = at;
    let mut fraction = 0;
    if bytes.get(at) == Some(&b'.') {
        fraction = digits(at + 1);
        at += 1 + fraction;
    }
    if integer == 0 && fraction == 0 {
        return false;
    }
    if matches!(bytes.get(at), Some(b'e' | b'E')) {
        at += 1;
        if matches!(bytes.get(at), Some(b'+' | b'-')) {
            at += 1;
        }
        let exponent = digits(at);
        if exponent == 0 {
            return false;
        }
        at += exponent;
    }
    at == bytes.len()
}

/// `a.localeCompare(b)` as the editors ordered element ids, for the ASCII text ids are made of:
/// the root collation of Unicode (CLDR), where white space and punctuation come first, then
/// digits, then letters in alphabetical order regardless of case; only when two texts are equal
/// that way does a lower-case letter come before its upper-case letter. Other characters follow
/// the letters in code-point order.
#[must_use]
pub fn locale_compare(a: &str, b: &str) -> Ordering {
    let primary = |text: &str| text.chars().map(primary_weight).collect::<Vec<u32>>();
    primary(a)
        .cmp(&primary(b))
        .then_with(|| {
            let tertiary = |text: &str| {
                text.chars()
                    .map(|ch| u8::from(ch.is_ascii_uppercase()))
                    .collect::<Vec<u8>>()
            };
            tertiary(a).cmp(&tertiary(b))
        })
        .then_with(|| a.cmp(b))
}

/// The ASCII white space and punctuation in the order of the root collation.
const PUNCTUATION_ORDER: &str = "\t\n\u{b}\u{c}\r _-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$";

fn primary_weight(ch: char) -> u32 {
    if let Some(rank) = PUNCTUATION_ORDER
        .chars()
        .position(|candidate| candidate == ch)
    {
        // At most the length of the table, so it fits.
        #[allow(clippy::cast_possible_truncation)]
        return rank as u32;
    }
    if let Some(digit) = ch.to_digit(10) {
        return 100 + digit;
    }
    if ch.is_ascii_alphabetic() {
        return 200 + u32::from(ch.to_ascii_lowercase()) - u32::from('a');
    }
    1000 + u32::from(ch)
}

/// Whether `value` is truthy in JavaScript; `None` stands for `undefined`: `undefined`, `null`,
/// `false`, `0`, `NaN` and `""` are false.
#[must_use]
pub fn truthy(value: Option<&Json>) -> bool {
    match value {
        None | Some(Json::Null) => false,
        Some(Json::Bool(value)) => *value,
        Some(Json::Number(number)) => *number != 0.0 && !number.is_nan(),
        Some(Json::String(text)) => !text.is_empty(),
        Some(Json::Array(_) | Json::Object(_)) => true,
    }
}

/// `String(value)`, as a template literal also writes it; `None` stands for `undefined`.
#[must_use]
pub fn string(value: Option<&Json>) -> String {
    match value {
        None => "undefined".to_owned(),
        Some(Json::Null) => "null".to_owned(),
        Some(Json::Bool(value)) => value.to_string(),
        Some(Json::Number(number)) => number_text(*number),
        Some(Json::String(text)) => text.clone(),
        // `Array.prototype.join`: `null` items are empty.
        Some(Json::Array(items)) => items
            .iter()
            .map(|item| match item {
                Json::Null => String::new(),
                other => string(Some(other)),
            })
            .collect::<Vec<_>>()
            .join(","),
        Some(Json::Object(_)) => "[object Object]".to_owned(),
    }
}

/// The member `key` of `value` (property access: `undefined` on anything but an object).
#[must_use]
pub fn member<'a>(value: Option<&'a Json>, key: &str) -> Option<&'a Json> {
    value.and_then(|value| value.get(key))
}

/// `value || fallback`.
#[must_use]
pub fn or<'a>(value: Option<&'a Json>, fallback: &'a Json) -> &'a Json {
    match value {
        Some(value) if truthy(Some(value)) => value,
        _ => fallback,
    }
}

/// `value ?? fallback`: the value unless it is `undefined` or `null`.
#[must_use]
pub fn nullish_or<'a>(value: Option<&'a Json>, fallback: Option<&'a Json>) -> Option<&'a Json> {
    match value {
        None | Some(Json::Null) => fallback,
        Some(value) => Some(value),
    }
}

/// `left === right`. Two objects or arrays read from a file are never the same object.
#[must_use]
pub fn strict_eq(left: Option<&Json>, right: Option<&Json>) -> bool {
    match (left, right) {
        (None, None) => true,
        // Numbers compare as doubles: NaN is never equal and -0 equals 0.
        (Some(left), Some(right)) => {
            !matches!(left, Json::Array(_) | Json::Object(_)) && left == right
        }
        _ => false,
    }
}

/// `Number.parseInt(text, 10)`: leading white space, an optional sign and the leading decimal
/// digits; `None` (`NaN`) without digits.
#[must_use]
pub fn parse_int(text: &str) -> Option<f64> {
    let rest = text.trim_start_matches(is_space);
    let (negative, rest) = match rest.as_bytes().first() {
        Some(b'-') => (true, &rest[1..]),
        Some(b'+') => (false, &rest[1..]),
        _ => (false, rest),
    };
    let digits: &str = &rest[..rest.bytes().take_while(u8::is_ascii_digit).count()];
    if digits.is_empty() {
        return None;
    }
    // Every digit string reads as a double; a very long one reads as infinity, as in JavaScript.
    let magnitude: f64 = digits.parse().unwrap_or(f64::INFINITY);
    Some(if negative { -magnitude } else { magnitude })
}

/// The number of a value for a comparison (`a > b`): anything but a number compares as `NaN`.
#[must_use]
pub fn as_number(value: Option<&Json>) -> f64 {
    value.and_then(Json::as_f64).unwrap_or(f64::NAN)
}

/// Whether `value.length > 0` holds in JavaScript for a stored list: a non-empty array or text.
#[must_use]
pub fn has_length(value: Option<&Json>) -> bool {
    match value {
        Some(Json::Array(items)) => !items.is_empty(),
        Some(Json::String(text)) => !text.is_empty(),
        _ => false,
    }
}

/// The array index a member name stands for: the canonical decimal of 0 to 2³² − 2.
fn array_index(key: &str) -> Option<u32> {
    if key.is_empty()
        || (key.len() > 1 && key.starts_with('0'))
        || !key.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    key.parse::<u64>()
        .ok()
        .filter(|index| *index < u64::from(u32::MAX))
        .and_then(|index| u32::try_from(index).ok())
}

/// The members of an object in the order JavaScript enumerates them (`Object.keys`,
/// `Object.entries`, `JSON.stringify`): array-index names ascending, then the others in order.
#[must_use]
pub fn ordered(members: &[(String, Json)]) -> Vec<&(String, Json)> {
    let mut indexed: Vec<(u32, &(String, Json))> = members
        .iter()
        .filter_map(|member| array_index(&member.0).map(|index| (index, member)))
        .collect();
    indexed.sort_by_key(|(index, _)| *index);
    indexed
        .into_iter()
        .map(|(_, member)| member)
        .chain(
            members
                .iter()
                .filter(|member| array_index(&member.0).is_none()),
        )
        .collect()
}

/// `Object.entries(value)` for an object (in [`ordered`] order) or an array (index, item); other
/// values have no entries here.
#[must_use]
pub fn entries(value: &Json) -> Vec<(String, &Json)> {
    match value {
        Json::Object(members) => ordered(members)
            .into_iter()
            .map(|(name, value)| (name.clone(), value))
            .collect(),
        Json::Array(items) => items
            .iter()
            .enumerate()
            .map(|(index, item)| (index.to_string(), item))
            .collect(),
        _ => Vec::new(),
    }
}

/// `value` with the members of every object in the order `JSON.stringify` writes them.
#[must_use]
pub fn normalized(value: &Json) -> Json {
    match value {
        Json::Array(items) => Json::Array(items.iter().map(normalized).collect()),
        Json::Object(members) => Json::Object(
            ordered(members)
                .into_iter()
                .map(|(name, value)| (name.clone(), normalized(value)))
                .collect(),
        ),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(text: &str) -> Json {
        Json::parse(text).expect("valid JSON")
    }

    #[test]
    fn truthiness_follows_javascript() {
        // Catches a port that treats `0`, `""` or `null` as present, or `[]`/`{}` as absent; that
        // changes default positions and names of SFC files and which Blockly fields count.
        assert!(!truthy(None));
        assert!(!truthy(Some(&Json::Null)));
        assert!(!truthy(Some(&Json::Number(0.0))));
        assert!(!truthy(Some(&Json::str(""))));
        assert!(!truthy(Some(&Json::Bool(false))));
        assert!(truthy(Some(&value("[]"))));
        assert!(truthy(Some(&value("{}"))));
        assert!(truthy(Some(&Json::str("0"))));
        assert!(truthy(Some(&Json::Number(-1.5))));
    }

    #[test]
    fn values_become_text_as_string_writes_them() {
        // String(undefined) is "undefined", String([1, null, [2, 3]]) is "1,,2,3", String({}) is
        // "[object Object]", String(1.0) is "1".
        assert_eq!(string(None), "undefined");
        assert_eq!(string(Some(&Json::Null)), "null");
        assert_eq!(string(Some(&value("[1, null, [2, 3]]"))), "1,,2,3");
        assert_eq!(string(Some(&value("{\"a\": 1}"))), "[object Object]");
        assert_eq!(string(Some(&value("1.0"))), "1");
        assert_eq!(string(Some(&Json::Bool(true))), "true");
    }

    #[test]
    fn strict_equality_tells_undefined_from_null() {
        // `undefined === undefined` but `undefined !== null`; catches a port that merges them.
        assert!(strict_eq(None, None));
        assert!(!strict_eq(None, Some(&Json::Null)));
        assert!(strict_eq(Some(&Json::str("a")), Some(&Json::str("a"))));
        assert!(!strict_eq(Some(&Json::str("1")), Some(&Json::Number(1.0))));
        assert!(!strict_eq(Some(&Json::object()), Some(&Json::object())));
        assert_eq!(
            or(Some(&Json::Number(0.0)), &Json::Number(5.0)),
            &Json::Number(5.0)
        );
        assert_eq!(
            nullish_or(Some(&Json::Number(0.0)), None),
            Some(&Json::Number(0.0))
        );
    }

    #[test]
    fn parse_int_reads_leading_digits() {
        // parseInt(" 12x") is 12, parseInt("-3") is -3, parseInt("x1") is NaN.
        assert_eq!(parse_int(" 12x"), Some(12.0));
        assert_eq!(parse_int("-3"), Some(-3.0));
        assert_eq!(parse_int("+4"), Some(4.0));
        assert_eq!(parse_int("x1"), None);
        assert_eq!(parse_int(""), None);
    }

    #[test]
    fn array_index_names_come_first_in_ascending_order() {
        // JSON.stringify({"b":1,"2":2,"a":3,"10":4,"01":5}) is {"2":2,"10":4,"b":1,"a":3,"01":5}:
        // "01" is no array index. Catches a writer that keeps the file order for numeric names.
        let object = value("{\"b\":1,\"2\":2,\"a\":3,\"10\":4,\"01\":5,\"4294967295\":6}");
        assert_eq!(
            normalized(&object).to_compact(),
            "{\"2\":2,\"10\":4,\"b\":1,\"a\":3,\"01\":5,\"4294967295\":6}"
        );
        let names: Vec<String> = entries(&object).into_iter().map(|(name, _)| name).collect();
        assert_eq!(names, ["2", "10", "b", "a", "01", "4294967295"]);
    }

    #[test]
    fn only_arrays_and_text_have_a_length() {
        assert!(has_length(Some(&value("[\"x\"]"))));
        assert!(has_length(Some(&Json::str("x"))));
        assert!(!has_length(Some(&value("[]"))));
        assert!(!has_length(Some(&value("{\"a\":1}"))));
        assert!(!has_length(Some(&Json::Number(3.0))));
        assert!(!has_length(None));
    }

    #[test]
    fn trim_removes_the_javascript_space_set() {
        // JavaScript trims the byte order mark and no-break space but keeps U+0085 (NEL), which
        // Rust's `trim` removes; catches a port that uses `str::trim` for names.
        assert_eq!(trim("\u{feff}\u{a0} a b \t\n"), "a b");
        assert_eq!(trim("\u{85}x"), "\u{85}x");
    }

    #[test]
    fn number_reads_text_as_javascript_number_does() {
        // Worked from ECMA-262 StringToNumber.
        assert_eq!(number_from_text(""), Some(0.0));
        assert_eq!(number_from_text("  12  "), Some(12.0));
        assert_eq!(number_from_text("-1.5e2"), Some(-150.0));
        assert_eq!(number_from_text(".5"), Some(0.5));
        assert_eq!(number_from_text("5."), Some(5.0));
        assert_eq!(number_from_text("+7"), Some(7.0));
        assert_eq!(number_from_text("0x10"), Some(16.0));
        assert_eq!(number_from_text("0b101"), Some(5.0));
        assert_eq!(number_from_text("0o17"), Some(15.0));
        assert_eq!(number_from_text("-Infinity"), Some(f64::NEG_INFINITY));
        // Rust's parser accepts these; JavaScript does not.
        assert_eq!(number_from_text("inf"), None);
        assert_eq!(number_from_text("NaN"), None);
        assert_eq!(number_from_text("-0x10"), None);
        assert_eq!(number_from_text("1_000"), None);
        assert_eq!(number_from_text("12abc"), None);
        assert_eq!(number_from_text("."), None);
        assert_eq!(number_from_text("1e"), None);
    }

    #[test]
    fn locale_order_puts_punctuation_then_digits_then_letters_case_last() {
        // Root collation: `_` before `-` before digits before letters; case decides only a tie.
        // Catches byte order, which puts `Z` before `a` and digits before `_`.
        let mut ids = vec!["b", "B", "a_2", "a-2", "a2", "A", "Z", "a", "10", "2"];
        ids.sort_by(|left, right| locale_compare(left, right));
        assert_eq!(
            ids,
            vec!["10", "2", "a", "A", "a_2", "a-2", "a2", "b", "B", "Z"]
        );
        assert_eq!(locale_compare("coil_10", "coil_2"), Ordering::Less);
        assert_eq!(locale_compare("x", "x"), Ordering::Equal);
    }
}
