//! The formatting of a logpoint's message (specification 13; specification 11 §6.13.2).

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec;

use smol_str::SmolStr;
use trust_runtime_core::value::{ArrayValue, EnumValue, Value};

use super::{format_message, log_text, LogPart, MAX_MESSAGE_BYTES};
use crate::debug::expression::{Expression, Names};

/// The variables `depth` (INT 3), `ready` (TRUE) and `label` (STRING `vat`).
struct Plant;

impl Names for Plant {
    fn read(&self, name: &str) -> Option<Value> {
        match name.to_ascii_lowercase().as_str() {
            "depth" => Some(Value::Int(3)),
            "ready" => Some(Value::Bool(true)),
            "label" => Some(Value::String(SmolStr::new("vat"))),
            _ => None,
        }
    }

    fn enumeration(&self, _type_name: &str, _value: &str) -> Option<Value> {
        None
    }
}

fn name(text: &str) -> LogPart {
    LogPart::Expression(Expression::Name(SmolStr::new(text)))
}

fn text(text: &str) -> LogPart {
    LogPart::Text(String::from(text))
}

/// Specification 13: the parts in order, a BOOL as `TRUE`, a STRING unquoted, another value in its
/// stable debug representation, and an expression that fails as `<error: detail>` without losing
/// the text around it: `depth Int(3), ready TRUE, label vat, x <error: 'missing' names no
/// variable>!`. Catches parts out of order, a quoted text, `true` for a BOOL, and a message dropped
/// at the first failing part.
#[test]
fn a_message_joins_its_texts_and_values_in_order() {
    let parts = [
        text("depth "),
        name("depth"),
        text(", ready "),
        name("ready"),
        text(", label "),
        name("label"),
        text(", x "),
        name("missing"),
        text("!"),
    ];
    assert_eq!(
        format_message(&parts, &Plant),
        "depth Int(3), ready TRUE, label vat, x <error: 'missing' names no variable>!"
    );
}

/// Specification 13: an array shows its element count, an enumeration `<type>::<value>`, a BOOL
/// `FALSE` and a CHAR its character. Catches an array listed element by element and an enumeration
/// shown by its number.
#[test]
fn values_show_as_a_logpoint_shows_them() {
    let array = Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![Value::Int(1), Value::Int(2), Value::Int(3)],
        vec![(1, 3)],
    )));
    assert_eq!(log_text(&array), "[3]");
    let mode = Value::Enum(Box::new(EnumValue::from_canonical_parts(
        SmolStr::new("Mode"),
        SmolStr::new("Run"),
        1,
    )));
    assert_eq!(log_text(&mode), "Mode::Run");
    assert_eq!(log_text(&Value::Bool(false)), "FALSE");
    assert_eq!(log_text(&Value::Char(b'A')), "A");
}

/// Specification 11 §6.13.2: a message longer than 1,024 bytes is cut at a character boundary:
/// 1,023 `a` and an `ä` (2 bytes) keep the 1,023 `a`. Catches a cut in the middle of a character,
/// which would not be UTF-8, and a message without a bound.
#[test]
fn a_long_message_is_cut_at_a_character_boundary() {
    let mut long = "a".repeat(MAX_MESSAGE_BYTES - 1);
    long.push('ä');
    let message = format_message(&[LogPart::Text(long)], &Plant);
    assert_eq!(message.len(), MAX_MESSAGE_BYTES - 1);
    assert!(message.bytes().all(|byte| byte == b'a'));
}
