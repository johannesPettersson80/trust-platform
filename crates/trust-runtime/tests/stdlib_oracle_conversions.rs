//! Independent-oracle cases for integer and REAL/LREAL conversions.
//!
//! Source: A13/A27 numeric and standard-library audit, section 6 (prioritized missing
//! cases) and the weak-row list, families A (integer conversions, widths, bit transfers,
//! text) and B (REAL/LREAL conversions and non-finite values). Authorities: IEC 61131-3
//! Ed.3 Tables 22-27 and section 6.6.2.5 (rounding to nearest, ties to even), specification
//! 07 section 2 and 2.8, specification 10 sections 6.7-6.9, and IEEE 754 binary32/binary64
//! rounding for the REAL and LREAL bit patterns.
//!
//! Expected values come from the audit oracle (python3 exact fractions and IEEE rounding),
//! not from the implementation. A failing case is a finding: report it, do not change the
//! expectation. Where a typed literal cannot be written in Structured Text (a ULINT literal
//! above i64::MAX), the ST form is kept as the audit gives it and a native companion asserts
//! the same conversion through `StandardLibrary::call`.

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

fn real_bits(value: &Value) -> u32 {
    match value {
        Value::Real(number) => number.to_bits(),
        other => panic!("expected REAL, got {other:?}"),
    }
}

fn lreal_bits(value: &Value) -> u64 {
    match value {
        Value::LReal(number) => number.to_bits(),
        other => panic!("expected LREAL, got {other:?}"),
    }
}

#[test]
fn oracle_a3_dint_to_sint_minimum_is_representable() {
    // A3: -128 is the SINT minimum (IEC Table 10); the conversion must store it.
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR s : SINT := 7; END_VAR\ns := DINT_TO_SINT(DINT#-128);\nEND_PROGRAM",
            "s",
        ),
        Value::SInt(-128)
    );
}

#[test]
fn oracle_a4_dint_to_sint_below_minimum_overflows_and_keeps_destination() {
    // A4: spec 10 section 6.8. DINT -129 is outside SINT and must not be stored.
    assert_fault_keeps(
        "PROGRAM Main\nVAR s : SINT := 7; END_VAR\ns := DINT_TO_SINT(DINT#-129);\nEND_PROGRAM",
        RuntimeError::Overflow,
        "s",
        Value::SInt(7),
    );
}

#[test]
fn oracle_a7_ulint_to_lint_above_i64_max_overflows_st() {
    // A7 (ST form as given by the audit). ULINT#18446744073709551615 is a valid IEC literal;
    // truST's literal lowering parses integers as i64, so this fixture may fail to compile.
    assert_fault_keeps(
        "PROGRAM Main\nVAR l : LINT := 7; u : ULINT := ULINT#18446744073709551615; END_VAR\n\
         l := ULINT_TO_LINT(u);\nEND_PROGRAM",
        RuntimeError::Overflow,
        "l",
        Value::LInt(7),
    );
}

#[test]
fn oracle_a7_ulint_to_lint_above_i64_max_overflows_native() {
    // A7 (native form): the same conversion without relying on the ULINT literal.
    assert_eq!(
        StandardLibrary::new().call("ULINT_TO_LINT", &[Value::ULInt(u64::MAX)]),
        Err(RuntimeError::Overflow)
    );
}

#[test]
fn oracle_a8_lint_to_ulint_negative_overflows_and_keeps_destination() {
    // A8: spec 10 section 6.10. A negative LINT has no ULINT value.
    assert_fault_keeps(
        "PROGRAM Main\nVAR u : ULINT := 7; l : LINT := -1; END_VAR\nu := LINT_TO_ULINT(l);\n\
         END_PROGRAM",
        RuntimeError::Overflow,
        "u",
        Value::ULInt(7),
    );
}

#[test]
fn oracle_a9_usint_to_sint_above_sint_maximum_overflows_and_keeps_destination() {
    // A9: spec 10 section 6.8. USINT 255 is outside SINT.
    assert_fault_keeps(
        "PROGRAM Main\nVAR s : SINT := 7; b : USINT := USINT#255; END_VAR\ns := USINT_TO_SINT(b);\n\
         END_PROGRAM",
        RuntimeError::Overflow,
        "s",
        Value::SInt(7),
    );
}

#[test]
fn oracle_a21_string_to_ulint_accepts_unsigned_maximum() {
    // A21: spec 07 section 2.8. The text form reaches the full ULINT range.
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR u : ULINT := 0; END_VAR\n\
             u := STRING_TO_ULINT('18446744073709551615');\nEND_PROGRAM",
            "u",
        ),
        Value::ULInt(18_446_744_073_709_551_615)
    );
}

#[test]
fn oracle_a22_string_to_sint_above_maximum_overflows_and_keeps_destination() {
    // A22: spec 07 section 2.8 and spec 10 section 6.8.
    assert_fault_keeps(
        "PROGRAM Main\nVAR s : SINT := 7; END_VAR\ns := STRING_TO_SINT('128');\nEND_PROGRAM",
        RuntimeError::Overflow,
        "s",
        Value::SInt(7),
    );
}

#[test]
fn oracle_a23_string_to_uint_accepts_radix_and_separators() {
    // A23: spec 07 section 2.8. Base 36 uses letters; a single underscore between digits is
    // allowed in the text form (IEC 61131-3 section 6.3.2).
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR u : UINT := 0; END_VAR\nu := STRING_TO_UINT('36#Z');\nEND_PROGRAM",
            "u",
        ),
        Value::UInt(35)
    );
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR u : UINT := 0; END_VAR\n\
             u := STRING_TO_UINT('2#1111_1111');\nEND_PROGRAM",
            "u",
        ),
        Value::UInt(255)
    );
}

#[test]
fn oracle_a31_byte_to_sint_reinterprets_the_sign_bit() {
    // A31: spec 07 section 2.8. The bit pattern 0xFF is -1 as SINT.
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR s : SINT := 0; END_VAR\ns := BYTE_TO_SINT(BYTE#16#FF);\nEND_PROGRAM",
            "s",
        ),
        Value::SInt(-1)
    );
}

#[test]
fn oracle_b2_lreal_to_int_ties_at_zero_round_to_even_for_both_signs() {
    // B2: IEC 61131-3 Table 23 (ties to even). 0.5 and -0.5 both round to 0.
    for literal in ["LREAL#0.5", "LREAL#-0.5"] {
        assert_eq!(
            output_after_cycle(
                &format!(
                    "PROGRAM Main\nVAR i : INT := 7; END_VAR\ni := LREAL_TO_INT({literal});\n\
                     END_PROGRAM"
                ),
                "i",
            ),
            Value::Int(0),
            "{literal}"
        );
    }
}

#[test]
fn oracle_b3_real_to_int_tie_inside_range_rounds_to_even() {
    // B3: IEC 61131-3 Table 23. 32766.5 is a tie; the even neighbour 32766 is in range.
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR i : INT := 7; END_VAR\ni := REAL_TO_INT(REAL#32766.5);\nEND_PROGRAM",
            "i",
        ),
        Value::Int(32766)
    );
}

#[test]
fn oracle_b4_real_to_int_tie_above_int_maximum_overflows_and_keeps_destination() {
    // B4: spec 07 section 2.8 (complete destination range). 32767.5 rounds to 32768, which
    // is outside INT, so the store must not happen.
    assert_fault_keeps(
        "PROGRAM Main\nVAR r : INT := 7; END_VAR\nr := REAL_TO_INT(REAL#32767.5);\nEND_PROGRAM",
        RuntimeError::Overflow,
        "r",
        Value::Int(7),
    );
}

#[test]
fn oracle_b5_lreal_to_int_negative_tie_at_int_minimum_is_in_range() {
    // B5: IEC 61131-3 Table 23. -32768.5 rounds to the even value -32768, which is INT MIN.
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR i : INT := 7; END_VAR\ni := LREAL_TO_INT(LREAL#-32768.5);\n\
             END_PROGRAM",
            "i",
        ),
        Value::Int(-32768)
    );
}

#[test]
fn oracle_b7_lreal_to_ulint_largest_exact_value_below_2_pow_64() {
    // B7: spec 07 section 2.8. 2^64 - 2048 is exact in binary64 and fits ULINT.
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR u : ULINT := 0; END_VAR\n\
             u := LREAL_TO_ULINT(LREAL#18446744073709549568.0);\nEND_PROGRAM",
            "u",
        ),
        Value::ULInt(18_446_744_073_709_549_568)
    );
}

#[test]
fn oracle_b8_lreal_to_lint_two_pow_63_overflows_and_negative_two_pow_63_is_minimum() {
    // B8: spec 07 section 2.8. 2^63 is one above LINT MAX; -2^63 is LINT MIN.
    assert_fault_keeps(
        "PROGRAM Main\nVAR l : LINT := 7; END_VAR\n\
         l := LREAL_TO_LINT(LREAL#9223372036854775808.0);\nEND_PROGRAM",
        RuntimeError::Overflow,
        "l",
        Value::LInt(7),
    );
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR l : LINT := 7; END_VAR\n\
             l := LREAL_TO_LINT(LREAL#-9223372036854775808.0);\nEND_PROGRAM",
            "l",
        ),
        Value::LInt(i64::MIN)
    );
}

#[test]
fn oracle_b10_lreal_to_real_largest_finite_single_is_exact() {
    // B10: spec 10 section 6.7. 2^128 - 2^104 is exactly the binary32 maximum 0x7F7FFFFF.
    let bits = real_bits(&output_after_cycle(
        "PROGRAM Main\nVAR x : REAL := 0.0; y : LREAL := LREAL#0.0; END_VAR\n\
         y := LREAL#340282346638528859811704183484516925440.0;\n\
         x := LREAL_TO_REAL(y);\nEND_PROGRAM",
        "x",
    ));
    assert_eq!(bits, 0x7F7F_FFFF);
}

#[test]
fn oracle_b11_lreal_to_real_exact_tie_at_single_limit_overflows_and_keeps_destination() {
    // B11: IEC 61131-3 section 6.6.2.5.3 with ties to even. 2^128 - 2^103 lies exactly
    // halfway between the binary32 maximum (odd mantissa) and 2^128, so it rounds to 2^128,
    // which is infinite, and the conversion faults.
    assert_fault_keeps(
        "PROGRAM Main\nVAR x : REAL := 7.0; \
         y : LREAL := LREAL#340282356779733661637539395458142568448.0; END_VAR\n\
         x := LREAL_TO_REAL(y);\nEND_PROGRAM",
        RuntimeError::Overflow,
        "x",
        Value::Real(7.0),
    );
}

#[test]
fn oracle_b14_dword_to_real_smallest_subnormal_is_finite() {
    // B14: spec 10 section 6.7 and spec 34 section 12.1. Bit pattern 0x00000001 is the
    // smallest positive binary32 subnormal and must be kept, not flushed to zero.
    let bits = real_bits(&output_after_cycle(
        "PROGRAM Main\nVAR r : REAL := 0.0; END_VAR\nr := DWORD_TO_REAL(DWORD#16#00000001);\n\
         END_PROGRAM",
        "r",
    ));
    assert_eq!(bits, 0x0000_0001);
}

#[test]
fn oracle_b18_string_to_lreal_below_halfway_rounds_to_f64_maximum() {
    // B18: spec 07 section 2.8 (correctly rounded). 1.7976931348623158E308 is below the
    // halfway point 2^1024 - 2^970 and rounds to f64::MAX (0x7FEFFFFFFFFFFFFF).
    let bits = lreal_bits(&output_after_cycle(
        "PROGRAM Main\nVAR d : LREAL := 0.0; END_VAR\n\
         d := STRING_TO_LREAL('1.7976931348623158E308');\nEND_PROGRAM",
        "d",
    ));
    assert_eq!(bits, 0x7FEF_FFFF_FFFF_FFFF);
}

#[test]
fn oracle_b19_string_to_lreal_above_halfway_overflows_and_keeps_destination() {
    // B19: spec 07 section 2.8. 1.7976931348623159E308 is above the halfway point and
    // rounds to infinity.
    assert_fault_keeps(
        "PROGRAM Main\nVAR d : LREAL := 7.0; END_VAR\n\
         d := STRING_TO_LREAL('1.7976931348623159E308');\nEND_PROGRAM",
        RuntimeError::Overflow,
        "d",
        Value::LReal(7.0),
    );
}

#[test]
fn oracle_b20_string_to_lreal_signed_zero_and_smallest_subnormal() {
    // B20: spec 07 section 2.8, IEEE 754 subnormal and signed-zero results. An underflowing
    // negative decimal is -0.0 (sign bit set); 4.9406564584124654E-324 is 2^-1074.
    let negative_zero = lreal_bits(&output_after_cycle(
        "PROGRAM Main\nVAR d : LREAL := 7.0; END_VAR\n\
         d := STRING_TO_LREAL('-1E-400');\nEND_PROGRAM",
        "d",
    ));
    assert_eq!(negative_zero, 0x8000_0000_0000_0000);
    let smallest = lreal_bits(&output_after_cycle(
        "PROGRAM Main\nVAR d : LREAL := 7.0; END_VAR\n\
         d := STRING_TO_LREAL('4.9406564584124654E-324');\nEND_PROGRAM",
        "d",
    ));
    assert_eq!(smallest, 0x0000_0000_0000_0001);
}

#[test]
fn oracle_b21_string_to_real_single_width_limit() {
    // B21: spec 07 section 2.8 and spec 10 section 6.7. 3.4028236E38 is above the halfway
    // point to 2^128 and overflows. 3.4028234E38 is below the binary32 maximum and rounds to
    // it (0x7F7FFFFF) because the nearest binary32 value is f32::MAX.
    assert_fault_keeps(
        "PROGRAM Main\nVAR r : REAL := 7.0; END_VAR\nr := STRING_TO_REAL('3.4028236E38');\n\
         END_PROGRAM",
        RuntimeError::Overflow,
        "r",
        Value::Real(7.0),
    );
    let bits = real_bits(&output_after_cycle(
        "PROGRAM Main\nVAR r : REAL := 0.0; END_VAR\nr := STRING_TO_REAL('3.4028234E38');\n\
         END_PROGRAM",
        "r",
    ));
    assert_eq!(bits, 0x7F7F_FFFF);
}
