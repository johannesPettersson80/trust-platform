//! Independent-oracle cases for string functions, BCD conversions and the IS_VALID family.
//!
//! Source: A13/A27 numeric and standard-library audit, section 6 (prioritized missing cases)
//! and families G (strings, excluding the length clamps and the empty-needle rows that are
//! owned by a separate change), I (BCD and validate). Authorities: IEC 61131-3 Ed.3 Table 34
//! (string functions, one-based positions), Table 39 (validate), spec 07 sections 8.1, 10
//! and 13 (BCD: every nibble must be 0 through 9; a destination range check applies).
//!
//! Expected values come from the audit oracle (exact integer and string arithmetic), not from
//! the implementation. A failing case is a finding: report it, do not change the expectation.
//! Where a value cannot be written as an ST literal (a 32767-character string, a 2^32
//! capacity), the case is asserted through `StandardLibrary::call` instead.

use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::stdlib::StandardLibrary;
use trust_runtime::value::Value;

fn compile(source: &str) -> TestHarness {
    TestHarness::from_source(source).unwrap_or_else(|error| panic!("fixture must compile: {error}"))
}

fn output_after_cycle(source: &str, name: &str) -> Value {
    let mut harness = compile(source);
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    harness.try_get_output(name).unwrap()
}

fn assert_fault_keeps(source: &str, error: RuntimeError, name: &str, kept: Value) {
    let mut harness = compile(source);
    let cycle = harness.cycle();
    assert_eq!(cycle.errors, [error]);
    assert_eq!(harness.try_get_output(name).unwrap(), kept);
}

#[test]
fn oracle_g3_left_with_length_beyond_end_returns_whole_string() {
    // G3: spec 07 section 8.1. A length past the end is clamped to the string.
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR s : STRING := ''; END_VAR\ns := LEFT(IN := 'ABC', L := 99);\n\
             END_PROGRAM",
            "s",
        ),
        Value::String("ABC".into())
    );
}

#[test]
fn oracle_g5_mid_position_zero_clamps_to_first_element() {
    // G5: spec 07 section 8.1. A position below one is clamped to the first element, so two
    // characters from position 1 are 'AB'.
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR s : STRING := ''; END_VAR\n\
             s := MID(IN := 'ABCDE', L := 2, P := 0);\nEND_PROGRAM",
            "s",
        ),
        Value::String("AB".into())
    );
}

#[test]
fn oracle_g12_delete_position_zero_clamps_to_first_element() {
    // G12: spec 07 section 8.1. Deleting two characters at the clamped position 1 leaves 'CDE'.
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR s : STRING := ''; END_VAR\n\
             s := DELETE(IN := 'ABCDE', L := 2, P := 0);\nEND_PROGRAM",
            "s",
        ),
        Value::String("CDE".into())
    );
}

#[test]
fn oracle_g14_delete_start_beyond_end_leaves_input_unchanged() {
    // G14: spec 07 section 8.1. A start beyond the final element leaves the input unchanged.
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR s : STRING := 'ABCDE'; END_VAR\n\
             s := DELETE(IN := s, L := 2, P := 99);\nEND_PROGRAM",
            "s",
        ),
        Value::String("ABCDE".into())
    );
}

#[test]
fn oracle_g17_replace_position_zero_clamps_to_first_element() {
    // G17: spec 07 section 8.1. Replacing two characters at the clamped position 1 with 'X'.
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR s : STRING := ''; END_VAR\n\
             s := REPLACE(IN1 := 'ABCDE', IN2 := 'X', L := 2, P := 0);\nEND_PROGRAM",
            "s",
        ),
        Value::String("XCDE".into())
    );
}

#[test]
fn oracle_g20_find_match_beyond_int_range_overflows() {
    // G20: spec 07 section 8.1. The only match is at one-based position 32768, which is not
    // representable as INT, so the result is Overflow. Native form because the 32767-character
    // operand is not a practical ST literal.
    let haystack = format!("{}B", "A".repeat(32_767));
    assert_eq!(
        StandardLibrary::new().call(
            "FIND",
            &[Value::String(haystack.into()), Value::String("B".into())],
        ),
        Err(RuntimeError::Overflow)
    );
}

#[test]
fn oracle_g23_limit_string_capacity_beyond_u32_overflows() {
    // G23: spec 07 section 8.1. The internal capacity is not truncated to u32; 2^32 overflows.
    assert_eq!(
        StandardLibrary::new().call(
            "__TRUST_LIMIT_STRING",
            &[Value::String("ABC".into()), Value::LInt(1_i64 << 32)],
        ),
        Err(RuntimeError::Overflow)
    );
}

#[test]
fn oracle_i3_uint_to_bcd_word_five_digits_overflows_and_keeps_destination() {
    // I3: spec 07 section 13. 10000 needs five BCD digits and a WORD holds four.
    assert_fault_keeps(
        "PROGRAM Main\nVAR b : WORD := WORD#16#1234; u : UINT := UINT#10000; END_VAR\n\
         b := UINT_TO_BCD_WORD(u);\nEND_PROGRAM",
        RuntimeError::Overflow,
        "b",
        Value::Word(0x1234),
    );
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR b : WORD := WORD#0; u : UINT := UINT#9999; END_VAR\n\
             b := UINT_TO_BCD_WORD(u);\nEND_PROGRAM",
            "b",
        ),
        Value::Word(0x9999)
    );
}

#[test]
fn oracle_i4_ulint_to_bcd_lword_seventeen_digits_overflows_and_keeps_destination() {
    // I4: spec 07 section 13. 10^16 needs seventeen digits; sixteen nines are the maximum.
    assert_fault_keeps(
        "PROGRAM Main\nVAR b : LWORD := LWORD#16#1234; u : ULINT := ULINT#10000000000000000; \
         END_VAR\nb := ULINT_TO_BCD_LWORD(u);\nEND_PROGRAM",
        RuntimeError::Overflow,
        "b",
        Value::LWord(0x1234),
    );
    assert_eq!(
        StandardLibrary::new().call("ULINT_TO_BCD_LWORD", &[Value::ULInt(9_999_999_999_999_999)]),
        Ok(Value::LWord(0x9999_9999_9999_9999))
    );
}

#[test]
fn oracle_i6_bcd_low_nibble_above_nine_is_type_mismatch() {
    // I6: spec 07 section 13 ("every nibble above nine"). 0x9A has a low nibble of 10.
    assert_fault_keeps(
        "PROGRAM Main\nVAR u : UINT := UINT#0; END_VAR\nu := BYTE_BCD_TO_UINT(BYTE#16#9A);\n\
         END_PROGRAM",
        RuntimeError::TypeMismatch,
        "u",
        Value::UInt(0),
    );
}

#[test]
fn oracle_i7_word_bcd_to_usint_above_usint_maximum_overflows_and_keeps_destination() {
    // I7: spec 07 section 13 ("checks the unsigned destination range"). BCD 999 is not a USINT.
    assert_fault_keeps(
        "PROGRAM Main\nVAR s : USINT := USINT#7; END_VAR\ns := WORD_BCD_TO_USINT(WORD#16#0999);\n\
         END_PROGRAM",
        RuntimeError::Overflow,
        "s",
        Value::USInt(7),
    );
}

#[test]
fn oracle_i11_is_valid_bcd_rejects_low_nibble_above_nine() {
    // I11: spec 07 section 10 ("every nibble"). 0x9A is not valid BCD.
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR ok : BOOL := TRUE; END_VAR\nok := IS_VALID_BCD(BYTE#16#9A);\n\
             END_PROGRAM",
            "ok",
        ),
        Value::Bool(false)
    );
}

#[test]
fn oracle_i12_is_valid_bcd_rejects_bool_input_and_keeps_destination() {
    // I12 (ST form): spec 07 section 10 has the type checker reject a BOOL argument to
    // IS_VALID_BCD at compile time, so the fixture stops before the runtime. The diagnostic
    // names the accepted argument types; the runtime TypeMismatch is the native companion below.
    let error = TestHarness::from_source(
        "PROGRAM Main\nVAR ok : BOOL := TRUE; END_VAR\nok := IS_VALID_BCD(TRUE);\nEND_PROGRAM",
    )
    .err()
    .expect("IS_VALID_BCD(TRUE) must be rejected at compile time");
    let message = error.to_string();
    assert!(
        message.contains("expected BYTE, WORD, DWORD, or LWORD type"),
        "unexpected compile error: {message}"
    );
}

#[test]
fn oracle_i12_is_valid_bcd_rejects_bool_input_native() {
    // I12 (native form): the runtime function reports TypeMismatch for a BOOL operand.
    assert_eq!(
        StandardLibrary::new().call("IS_VALID_BCD", &[Value::Bool(true)]),
        Err(RuntimeError::TypeMismatch)
    );
}
