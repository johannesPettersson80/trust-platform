//! JSON as the editors' JavaScript sees it (specification 25 §11.4).
//!
//! A visual program file is read and written the way the webview reads and writes it: an object
//! keeps the order of its members, a number is a double, and a file is written as
//! `JSON.stringify(value, null, 2)` writes it. A file an editor rewrites therefore keeps its own
//! member order, and every number rule matches the one the editor applied before the rules moved to
//! Rust. The reader is a small parser of its own, so it does not depend on how `serde_json` is
//! configured in the build (its `arbitrary_precision` feature changes how numbers decode).

use std::fmt::Write as _;

/// The deepest nesting a file may have; deeper input is refused instead of exhausting the stack.
const MAX_DEPTH: usize = 512;

/// One JSON value, with the member order of each object kept.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    /// `null`.
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// A number, a double as in JavaScript.
    Number(f64),
    /// A string.
    String(String),
    /// An array.
    Array(Vec<Json>),
    /// An object, its members in file order.
    Object(Vec<(String, Json)>),
}

impl Json {
    /// Reads a JSON text, as `JSON.parse` does: a repeated member name keeps the place of its first
    /// occurrence and the value of its last.
    ///
    /// # Errors
    /// The text is not JSON; the message names the line and column.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut reader = Reader {
            text: text.as_bytes(),
            at: 0,
            depth: 0,
        };
        reader.skip_space();
        let value = reader.value()?;
        reader.skip_space();
        if reader.at < reader.text.len() {
            return Err(reader.error("unexpected text after the JSON value"));
        }
        Ok(value)
    }

    /// The text `JSON.stringify(value, null, 2)` writes.
    #[must_use]
    pub fn to_pretty(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, Some(0));
        out
    }

    /// The text `JSON.stringify(value)` writes.
    #[must_use]
    pub fn to_compact(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, None);
        out
    }

    /// A string value.
    #[must_use]
    pub fn str(text: impl Into<String>) -> Self {
        Self::String(text.into())
    }

    /// An empty object.
    #[must_use]
    pub fn object() -> Self {
        Self::Object(Vec::new())
    }

    /// The member `key` of an object.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Self::Object(members) => members
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    /// The member `key` of an object, to change.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Json> {
        match self {
            Self::Object(members) => members
                .iter_mut()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    /// Sets the member `key` of an object as a JavaScript assignment does: an existing member keeps
    /// its place, a new one is added at the end. A value that is not an object is left unchanged.
    pub fn set(&mut self, key: &str, value: Json) {
        if let Self::Object(members) = self {
            if let Some(slot) = members.iter_mut().find(|(name, _)| name == key) {
                slot.1 = value;
            } else {
                members.push((key.to_owned(), value));
            }
        }
    }

    /// Removes the member `key` of an object and answers its value.
    pub fn remove(&mut self, key: &str) -> Option<Json> {
        match self {
            Self::Object(members) => {
                let index = members.iter().position(|(name, _)| name == key)?;
                Some(members.remove(index).1)
            }
            _ => None,
        }
    }

    /// The text of a string value.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(text) => Some(text),
            _ => None,
        }
    }

    /// The number of a number value.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Number(number) => Some(*number),
            _ => None,
        }
    }

    /// The number of a number value that is finite (`Number.isFinite`).
    #[must_use]
    pub fn as_finite(&self) -> Option<f64> {
        self.as_f64().filter(|number| number.is_finite())
    }

    /// The value of a Boolean.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// The items of an array.
    #[must_use]
    pub fn as_array(&self) -> Option<&Vec<Json>> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    /// The items of an array, to change.
    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Json>> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    /// The members of an object.
    #[must_use]
    pub fn as_object(&self) -> Option<&Vec<(String, Json)>> {
        match self {
            Self::Object(members) => Some(members),
            _ => None,
        }
    }

    /// Whether the value is an object (not an array and not `null`).
    #[must_use]
    pub fn is_object(&self) -> bool {
        matches!(self, Self::Object(_))
    }

    /// The string member `key` of an object.
    #[must_use]
    pub fn str_at(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Self::as_str)
    }

    /// The value as a `serde_json` value, for a reply of the language server. Member order is that
    /// of `serde_json`'s map; a text whose order matters is sent as text instead.
    #[must_use]
    pub fn to_serde(&self) -> serde_json::Value {
        match self {
            Self::Null => serde_json::Value::Null,
            Self::Bool(value) => serde_json::Value::Bool(*value),
            Self::Number(number) => number_to_serde(*number),
            Self::String(text) => serde_json::Value::String(text.clone()),
            Self::Array(items) => {
                serde_json::Value::Array(items.iter().map(Self::to_serde).collect())
            }
            Self::Object(members) => serde_json::Value::Object(
                members
                    .iter()
                    .map(|(name, value)| (name.clone(), value.to_serde()))
                    .collect(),
            ),
        }
    }

    /// A `serde_json` value (an argument of a language-server command) as a JSON value.
    #[must_use]
    pub fn from_serde(value: &serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => Self::Null,
            serde_json::Value::Bool(value) => Self::Bool(*value),
            serde_json::Value::Number(number) => number.as_f64().map_or(Self::Null, Self::Number),
            serde_json::Value::String(text) => Self::String(text.clone()),
            serde_json::Value::Array(items) => {
                Self::Array(items.iter().map(Self::from_serde).collect())
            }
            serde_json::Value::Object(members) => Self::Object(
                members
                    .iter()
                    .map(|(name, value)| (name.clone(), Self::from_serde(value)))
                    .collect(),
            ),
        }
    }

    fn write(&self, out: &mut String, indent: Option<usize>) {
        match self {
            Self::Null => out.push_str("null"),
            Self::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
            Self::Number(number) => {
                if number.is_finite() {
                    out.push_str(&number_text(*number));
                } else {
                    out.push_str("null");
                }
            }
            Self::String(text) => write_string(out, text),
            Self::Array(items) => {
                if items.is_empty() {
                    out.push_str("[]");
                    return;
                }
                out.push('[');
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    if let Some(level) = indent {
                        new_line(out, level + 1);
                        item.write(out, Some(level + 1));
                    } else {
                        item.write(out, None);
                    }
                }
                if let Some(level) = indent {
                    new_line(out, level);
                }
                out.push(']');
            }
            Self::Object(members) => {
                if members.is_empty() {
                    out.push_str("{}");
                    return;
                }
                out.push('{');
                for (index, (name, value)) in members.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    if let Some(level) = indent {
                        new_line(out, level + 1);
                        write_string(out, name);
                        out.push_str(": ");
                        value.write(out, Some(level + 1));
                    } else {
                        write_string(out, name);
                        out.push(':');
                        value.write(out, None);
                    }
                }
                if let Some(level) = indent {
                    new_line(out, level);
                }
                out.push('}');
            }
        }
    }
}

/// A number as JavaScript writes it (`String(number)`, `` `${number}` ``): integers without a
/// fraction, `-0` as `0`, and exponent notation from `1e21` up and below `1e-6`.
#[must_use]
pub fn number_text(number: f64) -> String {
    if number.is_nan() {
        return "NaN".to_owned();
    }
    if number.is_infinite() {
        return if number > 0.0 {
            "Infinity"
        } else {
            "-Infinity"
        }
        .to_owned();
    }
    if number == 0.0 {
        return "0".to_owned();
    }
    // Rust's exponent form holds the shortest digits that read back as the same double, as
    // JavaScript's algorithm does; only the placement of the decimal point differs.
    let scientific = format!("{:e}", number.abs());
    let (mantissa, exponent) = scientific
        .split_once('e')
        .unwrap_or((scientific.as_str(), "0"));
    let exponent: i64 = exponent.parse().unwrap_or(0);
    let digits: String = mantissa.chars().filter(|ch| *ch != '.').collect();
    let count = i64::try_from(digits.len()).unwrap_or(i64::MAX);
    // `number = 0.digits × 10^point`.
    let point = exponent + 1;
    let mut out = String::new();
    if number < 0.0 {
        out.push('-');
    }
    if count <= point && point <= 21 {
        out.push_str(&digits);
        out.push_str(&"0".repeat(usize::try_from(point - count).unwrap_or(0)));
    } else if 0 < point && point <= 21 {
        let split = usize::try_from(point).unwrap_or(0);
        out.push_str(&digits[..split]);
        out.push('.');
        out.push_str(&digits[split..]);
    } else if -6 < point && point <= 0 {
        out.push_str("0.");
        out.push_str(&"0".repeat(usize::try_from(-point).unwrap_or(0)));
        out.push_str(&digits);
    } else {
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        let shown = point - 1;
        let _ = write!(
            out,
            "e{}{}",
            if shown >= 0 { '+' } else { '-' },
            shown.abs()
        );
    }
    out
}

/// JavaScript's `Math.round`: the nearest integer, a half rounding up (towards positive infinity).
#[must_use]
pub fn js_round(number: f64) -> f64 {
    let floor = number.floor();
    if number - floor >= 0.5 {
        floor + 1.0
    } else {
        floor
    }
}

/// A number for a JSON value: an integer when JavaScript would write one, so a value read back
/// from `serde_json` compares equal to one the editor wrote.
fn number_to_serde(number: f64) -> serde_json::Value {
    const SAFE: f64 = 9_007_199_254_740_991.0;
    if number.fract() == 0.0 && number.abs() <= SAFE {
        // Exact: the value is an integer within the safe range of a double.
        #[allow(clippy::cast_possible_truncation)]
        let integer = number as i64;
        serde_json::Value::from(integer)
    } else {
        serde_json::Number::from_f64(number)
            .map_or(serde_json::Value::Null, serde_json::Value::Number)
    }
}

fn new_line(out: &mut String, level: usize) {
    out.push('\n');
    for _ in 0..level {
        out.push_str("  ");
    }
}

fn write_string(out: &mut String, text: &str) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if u32::from(ch) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(ch));
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
}

struct Reader<'a> {
    text: &'a [u8],
    at: usize,
    depth: usize,
}

impl Reader<'_> {
    fn error(&self, what: &str) -> String {
        let before = &self.text[..self.at.min(self.text.len())];
        let line = before.split(|byte| *byte == b'\n').count();
        let column = before
            .iter()
            .rev()
            .take_while(|byte| **byte != b'\n')
            .count()
            + 1;
        format!("Invalid JSON at line {line}, column {column}: {what}.")
    }

    fn peek(&self) -> Option<u8> {
        self.text.get(self.at).copied()
    }

    fn skip_space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn expect_word(&mut self, word: &str, value: Json) -> Result<Json, String> {
        if self.text[self.at..].starts_with(word.as_bytes()) {
            self.at += word.len();
            Ok(value)
        } else {
            Err(self.error("unexpected character"))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        match self.peek() {
            None => Err(self.error("the text ends before a value")),
            Some(b'{') => self.nested(Self::object),
            Some(b'[') => self.nested(Self::array),
            Some(b'"') => self.string().map(Json::String),
            Some(b't') => self.expect_word("true", Json::Bool(true)),
            Some(b'f') => self.expect_word("false", Json::Bool(false)),
            Some(b'n') => self.expect_word("null", Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.error("unexpected character")),
        }
    }

    fn nested(&mut self, read: fn(&mut Self) -> Result<Json, String>) -> Result<Json, String> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.error("the values are nested too deeply"));
        }
        let value = read(self);
        self.depth -= 1;
        value
    }

    fn object(&mut self) -> Result<Json, String> {
        self.at += 1;
        let mut members: Vec<(String, Json)> = Vec::new();
        self.skip_space();
        if self.peek() == Some(b'}') {
            self.at += 1;
            return Ok(Json::Object(members));
        }
        loop {
            self.skip_space();
            if self.peek() != Some(b'"') {
                return Err(self.error("expected a member name in double quotes"));
            }
            let name = self.string()?;
            self.skip_space();
            if self.peek() != Some(b':') {
                return Err(self.error("expected ':' after a member name"));
            }
            self.at += 1;
            self.skip_space();
            let value = self.value()?;
            if let Some(slot) = members.iter_mut().find(|(existing, _)| *existing == name) {
                slot.1 = value;
            } else {
                members.push((name, value));
            }
            self.skip_space();
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b'}') => {
                    self.at += 1;
                    return Ok(Json::Object(members));
                }
                _ => return Err(self.error("expected ',' or '}' in an object")),
            }
        }
    }

    fn array(&mut self) -> Result<Json, String> {
        self.at += 1;
        let mut items = Vec::new();
        self.skip_space();
        if self.peek() == Some(b']') {
            self.at += 1;
            return Ok(Json::Array(items));
        }
        loop {
            self.skip_space();
            items.push(self.value()?);
            self.skip_space();
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b']') => {
                    self.at += 1;
                    return Ok(Json::Array(items));
                }
                _ => return Err(self.error("expected ',' or ']' in an array")),
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self
            .text
            .get(self.at..self.at + 4)
            .ok_or_else(|| self.error("a \\u escape needs four hexadecimal digits"))?;
        let text = std::str::from_utf8(digits)
            .map_err(|_| self.error("a \\u escape needs four hexadecimal digits"))?;
        let value = u32::from_str_radix(text, 16)
            .map_err(|_| self.error("a \\u escape needs four hexadecimal digits"))?;
        self.at += 4;
        Ok(value)
    }

    fn string(&mut self) -> Result<String, String> {
        self.at += 1;
        let mut bytes: Vec<u8> = Vec::new();
        loop {
            let Some(byte) = self.peek() else {
                return Err(self.error("the text ends inside a string"));
            };
            match byte {
                b'"' => {
                    self.at += 1;
                    return String::from_utf8(bytes)
                        .map_err(|_| self.error("the string is not UTF-8"));
                }
                b'\\' => {
                    self.at += 1;
                    let Some(escape) = self.peek() else {
                        return Err(self.error("the text ends inside a string"));
                    };
                    self.at += 1;
                    let ch = match escape {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let first = self.hex4()?;
                            let code = if (0xD800..0xDC00).contains(&first)
                                && self.text[self.at..].starts_with(b"\\u")
                            {
                                let saved = self.at;
                                self.at += 2;
                                let second = self.hex4()?;
                                if (0xDC00..0xE000).contains(&second) {
                                    0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00)
                                } else {
                                    self.at = saved;
                                    first
                                }
                            } else {
                                first
                            };
                            // A lone surrogate cannot be held in a Rust string.
                            char::from_u32(code).unwrap_or('\u{FFFD}')
                        }
                        _ => return Err(self.error("unknown escape in a string")),
                    };
                    let mut buffer = [0u8; 4];
                    bytes.extend_from_slice(ch.encode_utf8(&mut buffer).as_bytes());
                }
                0..=0x1F => return Err(self.error("a control character inside a string")),
                _ => {
                    bytes.push(byte);
                    self.at += 1;
                }
            }
        }
    }

    fn digits(&mut self) -> usize {
        let start = self.at;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.at += 1;
        }
        self.at - start
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.at;
        if self.peek() == Some(b'-') {
            self.at += 1;
        }
        match self.peek() {
            Some(b'0') => self.at += 1,
            Some(b'1'..=b'9') => {
                self.digits();
            }
            _ => return Err(self.error("a number needs digits")),
        }
        if self.peek() == Some(b'.') {
            self.at += 1;
            if self.digits() == 0 {
                return Err(self.error("a number needs digits after '.'"));
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.at += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.at += 1;
            }
            if self.digits() == 0 {
                return Err(self.error("a number needs digits in its exponent"));
            }
        }
        let text = std::str::from_utf8(&self.text[start..self.at])
            .map_err(|_| self.error("a number is not ASCII"))?;
        text.parse::<f64>()
            .map(Json::Number)
            .map_err(|_| self.error("a number cannot be read"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_keeps_its_member_order_and_is_written_as_json_stringify_writes_it() {
        // Catches a reader that sorts members (as serde_json's map does): the order of the file
        // must survive a rewrite, and the layout must be the two-space layout of the webview.
        let text = "{\"b\": 1, \"a\": [true, null, \"x\"], \"c\": {}, \"d\": []}";
        let value = Json::parse(text).expect("valid JSON");
        assert_eq!(
            value.to_pretty(),
            "{\n  \"b\": 1,\n  \"a\": [\n    true,\n    null,\n    \"x\"\n  ],\n  \"c\": {},\n  \"d\": []\n}"
        );
        assert_eq!(
            value.to_compact(),
            "{\"b\":1,\"a\":[true,null,\"x\"],\"c\":{},\"d\":[]}"
        );
    }

    #[test]
    fn a_repeated_member_keeps_its_first_place_and_its_last_value() {
        // JSON.parse('{"a":1,"b":2,"a":3}') is {a: 3, b: 2}; catches a reader that keeps both or the
        // first value.
        let value = Json::parse("{\"a\":1,\"b\":2,\"a\":3}").expect("valid JSON");
        assert_eq!(value.to_compact(), "{\"a\":3,\"b\":2}");
    }

    #[test]
    fn numbers_are_written_as_javascript_writes_them() {
        // Worked from ECMAScript Number::toString: integers have no fraction, -0 is "0", and the
        // exponent form starts at 1e21 and below 1e-6.
        let cases = [
            (100.0, "100"),
            (-0.0, "0"),
            (0.1, "0.1"),
            (1.5, "1.5"),
            (-2.25, "-2.25"),
            (123_456.789, "123456.789"),
            (1e20, "100000000000000000000"),
            (1e21, "1e+21"),
            (0.000_001, "0.000001"),
            (0.000_000_1, "1e-7"),
            (1.5e-7, "1.5e-7"),
        ];
        for (number, expected) in cases {
            assert_eq!(number_text(number), expected, "{number}");
        }
        assert_eq!(Json::parse("100.0").expect("number").to_compact(), "100");
    }

    #[test]
    fn math_round_rounds_a_half_up() {
        // Math.round(2.5) = 3, Math.round(-2.5) = -2, Math.round(-0.6) = -1; catches Rust's
        // round-half-away-from-zero.
        assert_eq!(js_round(2.5), 3.0);
        assert_eq!(js_round(-2.5), -2.0);
        assert_eq!(js_round(-0.6), -1.0);
        assert_eq!(js_round(10.4), 10.0);
    }

    #[test]
    fn strings_read_escapes_and_write_them_back() {
        // "é" is é, a surrogate pair is one character, control characters are written as
        // JSON.stringify writes them.
        let value = Json::parse(r#""a\"b\\c\né😀\u0001""#).expect("string");
        assert_eq!(value.as_str(), Some("a\"b\\c\né\u{1F600}\u{1}"));
        assert_eq!(value.to_compact(), "\"a\\\"b\\\\c\\né\u{1F600}\\u0001\"");
    }

    #[test]
    fn malformed_text_is_refused_with_its_place() {
        // Catches a reader that accepts trailing text, a missing value or a bare control character.
        let error = Json::parse("{\"a\": }").expect_err("no value");
        assert!(error.contains("line 1, column 7"), "{error}");
        assert!(Json::parse("[1] x").is_err());
        assert!(Json::parse("\"a\nb\"").is_err());
        assert!(Json::parse("01").is_err());
        assert!(Json::parse(&"[".repeat(MAX_DEPTH + 1)).is_err());
    }

    #[test]
    fn set_keeps_the_place_of_an_existing_member() {
        // A JavaScript assignment to an existing property keeps its place; a new one goes last.
        let mut value = Json::parse("{\"a\":1,\"b\":2}").expect("object");
        value.set("a", Json::Number(3.0));
        value.set("c", Json::Bool(true));
        assert_eq!(value.to_compact(), "{\"a\":3,\"b\":2,\"c\":true}");
        assert_eq!(value.remove("b"), Some(Json::Number(2.0)));
        assert_eq!(value.to_compact(), "{\"a\":3,\"c\":true}");
    }
}
