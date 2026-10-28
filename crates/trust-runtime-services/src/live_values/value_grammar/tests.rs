//! The scalar grammar contract: the exact values and the exact refusal text that `set`,
//! `var.force`, `io.write` and `io.force` answer today (specification 11, "Live Values";
//! specification 34 §4.3.1).

use alloc::format;

use trust_runtime_core::value::Value;

use super::*;

#[test]
fn parse_value_accepts_boolean_case_and_surrounding_whitespace() {
    // A grammar that forgot to trim or upper-case would reject the padded and mixed-case texts.
    for (text, expected) in [
        ("TRUE", Value::Bool(true)),
        ("false", Value::Bool(false)),
        (" \tTrUe\n", Value::Bool(true)),
        ("\rfAlSe ", Value::Bool(false)),
    ] {
        assert_eq!(parse_value(text), Ok(expected), "text={text:?}");
    }
}

#[test]
fn parse_value_accepts_signed_i64_boundaries() {
    // A grammar that trimmed only whitespace, dropped a leading sign or narrowed the width would
    // reject one of these.
    for (text, expected) in [
        ("0", Value::LInt(0)),
        ("+0007", Value::LInt(7)),
        ("-0007", Value::LInt(-7)),
        ("9223372036854775807", Value::LInt(i64::MAX)),
        ("-9223372036854775808", Value::LInt(i64::MIN)),
    ] {
        assert_eq!(parse_value(text), Ok(expected), "text={text:?}");
    }
}

#[test]
fn parse_value_refuses_unsupported_text_with_its_own_text() {
    // A grammar that accepted IEC integer separators, a hexadecimal prefix, a decimal point or an
    // exponent would return a value here; one that widened i64 would accept the overflows.
    for text in [
        "",
        " \t\n",
        "1_000",
        "16#FF",
        "1.0",
        "1e3",
        "\"TRUE\"",
        "9223372036854775808",
        "-9223372036854775809",
    ] {
        assert_eq!(
            parse_value(text),
            Err(format!("unsupported value '{text}'")),
            "text={text:?}"
        );
    }
}
