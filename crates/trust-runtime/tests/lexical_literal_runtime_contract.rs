use trust_runtime::harness::{CompileSession, TestHarness};
use trust_runtime::value::Value;

fn run(source: &str) -> TestHarness {
    let mut harness =
        TestHarness::from_source(source).unwrap_or_else(|error| panic!("fixture: {error}"));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    harness
}

fn compile_error(source: &str) -> String {
    match CompileSession::from_source(source).build_runtime() {
        Ok(_) => panic!("malformed or out-of-range literal must reject compilation"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn lexical_literal_runtime_decimal_and_based_separators_materialize_exact_value() {
    let harness = run(r#"
PROGRAM Main
VAR result : DINT; END_VAR
result := 123_4 + 2#1010 + 8#77 + 16#FF;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("result"), Some(Value::DInt(1562)));
}

#[test]
fn lexical_literal_runtime_typed_signed_bit_and_boolean_values_preserve_tags() {
    let harness = run(r#"
PROGRAM Main
VAR signed : INT; bits : WORD; flag : BOOL; END_VAR
signed := INT#-123;
bits := WORD#16#0AFF;
flag := BOOL#TRUE;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("signed"), Some(Value::Int(-123)));
    assert_eq!(harness.get_output("bits"), Some(Value::Word(0x0AFF)));
    assert_eq!(harness.get_output("flag"), Some(Value::Bool(true)));
}

#[test]
fn lexical_literal_runtime_real_exponent_materializes_decimal_power() {
    let harness = run(r#"
PROGRAM Main
VAR result : REAL; END_VAR
result := 1.25E2;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("result"), Some(Value::Real(125.0)));
}

#[test]
fn lexical_literal_runtime_narrow_string_escapes_decode_in_source_order() {
    let harness = run(r#"
PROGRAM Main
VAR result : STRING[16]; END_VAR
result := 'A$$B$L$0A$''';
END_PROGRAM
"#);
    assert_eq!(
        harness.get_output("result"),
        Some(Value::String("A$B\n\n'".into()))
    );
}

#[test]
fn lexical_literal_runtime_wide_string_escapes_decode_unicode_code_unit() {
    let harness = run(r#"
PROGRAM Main
VAR result : WSTRING[16]; END_VAR
result := "A$$B$N$00C4$"";
END_PROGRAM
"#);
    assert_eq!(
        harness.get_output("result"),
        Some(Value::WString("A$B\nÄ\"".to_string()))
    );
}

#[test]
fn lexical_literal_runtime_duration_allows_most_significant_unit_overflow() {
    let harness = run(r#"
PROGRAM Main
VAR equal : BOOL; END_VAR
equal := T#25h_15m = T#1d_1h_15m;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("equal"), Some(Value::Bool(true)));
}

#[test]
fn lexical_literal_runtime_duration_fraction_on_least_unit_is_exact() {
    let harness = run(r#"
PROGRAM Main
VAR equal : BOOL; END_VAR
equal := LTIME#1s_500.25ms = LTIME#1500250us;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("equal"), Some(Value::Bool(true)));
}

#[test]
fn lexical_literal_runtime_negative_duration_applies_sign_to_complete_composition() {
    let harness = run(r#"
PROGRAM Main
VAR equal : BOOL; END_VAR
equal := TIME#-1h_30m = TIME#-90m;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("equal"), Some(Value::Bool(true)));
}

#[test]
fn lexical_literal_runtime_date_prefix_aliases_have_same_value() {
    let harness = run(r#"
PROGRAM Main
VAR shortEqual : BOOL; longEqual : BOOL; END_VAR
shortEqual := D#1984-06-25 = DATE#1984-06-25;
longEqual := LD#1984-06-25 = LDATE#1984-06-25;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("shortEqual"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("longEqual"), Some(Value::Bool(true)));
}

#[test]
fn lexical_literal_runtime_time_of_day_prefix_aliases_have_same_value() {
    let harness = run(r#"
PROGRAM Main
VAR shortEqual : BOOL; longEqual : BOOL; END_VAR
shortEqual := TOD#15:36:55.36 = TIME_OF_DAY#15:36:55.36;
longEqual := LTOD#15:36:55.360_227_400 = LTIME_OF_DAY#15:36:55.360_227_400;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("shortEqual"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("longEqual"), Some(Value::Bool(true)));
}

#[test]
fn lexical_literal_runtime_date_time_prefix_aliases_have_same_value() {
    let harness = run(r#"
PROGRAM Main
VAR shortEqual : BOOL; longEqual : BOOL; END_VAR
shortEqual := DT#1984-06-25-15:36:55.36 = DATE_AND_TIME#1984-06-25-15:36:55.36;
longEqual := LDT#1984-06-25-15:36:55.360_227_400 = LDATE_AND_TIME#1984-06-25-15:36:55.360_227_400;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("shortEqual"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("longEqual"), Some(Value::Bool(true)));
}

#[test]
fn lexical_literal_runtime_accepts_gregorian_century_leap_day() {
    let harness = run(r#"
PROGRAM Main
VAR equal : BOOL; END_VAR
equal := D#2000-02-29 = DATE#2000-02-29;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("equal"), Some(Value::Bool(true)));
}

#[test]
fn lexical_literal_runtime_rejects_nonleap_february_29() {
    let error =
        compile_error("PROGRAM Main\nVAR value : DATE := D#2023-02-29; END_VAR\nEND_PROGRAM");
    assert!(error.to_ascii_lowercase().contains("date"), "{error}");
}

#[test]
fn lexical_literal_runtime_rejects_century_not_divisible_by_400_as_leap() {
    let error =
        compile_error("PROGRAM Main\nVAR value : DATE := D#1900-02-29; END_VAR\nEND_PROGRAM");
    assert!(error.to_ascii_lowercase().contains("date"), "{error}");
}

#[test]
fn lexical_literal_runtime_rejects_day_beyond_month_length() {
    let error =
        compile_error("PROGRAM Main\nVAR value : DATE := D#2024-04-31; END_VAR\nEND_PROGRAM");
    assert!(error.to_ascii_lowercase().contains("date"), "{error}");
}

#[test]
fn lexical_literal_runtime_rejects_time_of_day_hour_24() {
    let error =
        compile_error("PROGRAM Main\nVAR value : TOD := TOD#24:00:00; END_VAR\nEND_PROGRAM");
    assert!(
        error.to_ascii_lowercase().contains("time") || error.to_ascii_lowercase().contains("tod"),
        "{error}"
    );
}

#[test]
fn lexical_literal_runtime_rejects_time_of_day_minute_60() {
    let error =
        compile_error("PROGRAM Main\nVAR value : TOD := TOD#12:60:00; END_VAR\nEND_PROGRAM");
    assert!(
        error.to_ascii_lowercase().contains("time") || error.to_ascii_lowercase().contains("tod"),
        "{error}"
    );
}

#[test]
fn lexical_literal_runtime_rejects_time_of_day_second_60() {
    let error =
        compile_error("PROGRAM Main\nVAR value : TOD := TOD#12:00:60; END_VAR\nEND_PROGRAM");
    assert!(
        error.to_ascii_lowercase().contains("time") || error.to_ascii_lowercase().contains("tod"),
        "{error}"
    );
}

#[test]
fn lexical_literal_runtime_rejects_invalid_date_component_in_datetime() {
    let error = compile_error(
        "PROGRAM Main\nVAR value : DT := DT#2023-02-29-12:00:00; END_VAR\nEND_PROGRAM",
    );
    assert!(error.to_ascii_lowercase().contains("date"), "{error}");
}

#[test]
fn lexical_literal_runtime_rejects_invalid_clock_component_in_datetime() {
    let error = compile_error(
        "PROGRAM Main\nVAR value : DT := DT#2024-01-01-24:00:00; END_VAR\nEND_PROGRAM",
    );
    assert!(
        error.to_ascii_lowercase().contains("time") || error.to_ascii_lowercase().contains("tod"),
        "{error}"
    );
}

#[test]
fn lexical_literal_runtime_rejects_typed_integer_payload_out_of_range() {
    let error = compile_error("PROGRAM Main\nVAR value : INT := INT#32768; END_VAR\nEND_PROGRAM");
    assert!(
        error.to_ascii_lowercase().contains("range")
            || error.to_ascii_lowercase().contains("overflow"),
        "{error}"
    );
}

#[test]
fn lexical_literal_runtime_rejects_boolean_numeric_payload_other_than_zero_or_one() {
    let error = compile_error("PROGRAM Main\nVAR value : BOOL := BOOL#2; END_VAR\nEND_PROGRAM");
    assert!(
        error.to_ascii_lowercase().contains("bool") || error.to_ascii_lowercase().contains("range"),
        "{error}"
    );
}

#[test]
fn lexical_literal_runtime_rejects_unknown_typed_literal_prefix() {
    let error = compile_error("PROGRAM Main\nVAR value : INT := MISSING#1; END_VAR\nEND_PROGRAM");
    assert!(
        error.to_ascii_lowercase().contains("missing")
            || error.to_ascii_lowercase().contains("type"),
        "{error}"
    );
}
