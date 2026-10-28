//! Specification 39, §12.2.4: type, capacity and non-finite checks of a written value.

use serde_json::json;

use super::value_for_target;
use crate::value::Value;

fn refused(current: Value, requested: serde_json::Value, words: &str) {
    let error = value_for_target(&current, &requested).expect_err("refused");
    assert!(error.contains(words), "{requested}: {error}");
}

#[test]
fn a_boolean_takes_true_or_false_only() {
    assert_eq!(
        value_for_target(&Value::Bool(false), &json!(true)),
        Ok(Value::Bool(true))
    );
    refused(Value::Bool(false), json!(1), "TRUE or FALSE");
    refused(Value::Bool(false), json!("TRUE"), "TRUE or FALSE");
}

#[test]
fn an_integer_takes_whole_numbers_that_fit_its_width() {
    assert_eq!(
        value_for_target(&Value::Int(0), &json!(-300)),
        Ok(Value::Int(-300))
    );
    assert_eq!(
        value_for_target(&Value::USInt(0), &json!(255)),
        Ok(Value::USInt(255))
    );
    assert_eq!(
        value_for_target(&Value::LInt(0), &json!(4.0)),
        Ok(Value::LInt(4)),
        "a whole number written as 4.0"
    );
    refused(Value::SInt(0), json!(128), "SINT");
    refused(Value::USInt(0), json!(-1), "USINT");
    refused(Value::DInt(0), json!(1.5), "whole number");
    refused(Value::Word(0), json!(70000), "WORD");
    refused(Value::Int(0), json!(true), "number");
}

#[test]
fn a_real_takes_finite_numbers_in_its_range() {
    assert_eq!(
        value_for_target(&Value::LReal(0.0), &json!(61.5)),
        Ok(Value::LReal(61.5))
    );
    assert_eq!(
        value_for_target(&Value::Real(0.0), &json!(2)),
        Ok(Value::Real(2.0))
    );
    refused(Value::Real(0.0), json!(1e39), "REAL");
    refused(Value::LReal(0.0), json!("61.5"), "number");
}

#[test]
fn a_value_that_is_not_finite_is_refused() {
    // JSON cannot carry NaN or infinity; a request that tries arrives as text or null.
    refused(Value::LReal(0.0), json!(null), "number");
    refused(Value::LReal(0.0), json!("NaN"), "number");
}

/// Specification 39, §12.8 item 5: a WSTRING takes text as a STRING does, up to 255 characters,
/// and its declared capacity then decides in UTF-16 code units. Catches a WSTRING point refused
/// whatever its text, a text turned into a STRING, and a text longer than the HMI writes.
#[test]
fn a_wstring_takes_text_of_any_language() {
    assert_eq!(
        value_for_target(&Value::WString("".into()), &json!("Kühlung geprüft")),
        Ok(Value::WString("Kühlung geprüft".into()))
    );
    refused(Value::WString("".into()), json!(1), "text");
    refused(Value::WString("".into()), json!("ä".repeat(256)), "255");
}

#[test]
fn other_types_cannot_be_written_from_the_hmi() {
    refused(Value::Char(b'x'), json!("y"), "cannot be written");
}

#[test]
fn a_time_takes_a_number_of_seconds() {
    use crate::value::Duration;
    assert_eq!(
        value_for_target(&Value::Time(Duration::from_nanos(0)), &json!(1.5)),
        Ok(Value::Time(Duration::from_nanos(1_500_000_000)))
    );
    refused(Value::Time(Duration::from_nanos(0)), json!(-1.0), "seconds");
    refused(
        Value::Time(Duration::from_nanos(0)),
        json!("T#1s"),
        "seconds",
    );
}

#[test]
fn a_string_takes_text_up_to_255_characters() {
    assert_eq!(
        value_for_target(&Value::String("".into()), &json!("Checked on site")),
        Ok(Value::String("Checked on site".into()))
    );
    refused(Value::String("".into()), json!(1), "text");
    refused(Value::String("".into()), json!("x".repeat(256)), "255");
}

#[test]
fn remote_enum_labels_remain_visible_in_command_readback_and_audit() {
    assert_eq!(
        super::json_of(&Value::String("Auto".into())),
        Some(json!("Auto"))
    );
}
