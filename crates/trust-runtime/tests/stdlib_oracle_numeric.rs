//! Independent-oracle cases for arithmetic, named numerical functions, comparison and
//! shifts/rotates.
//!
//! Source: A13/A27 numeric and standard-library audit, section 6 (prioritized missing cases)
//! and the weak-row list, families A (weak operator rows), C (operators), D (numerical
//! functions), E (shifts and rotates), F14 (EQ on signed zero) and I9 (IS_VALID on a REAL
//! value). Authorities: IEC 61131-3 Ed.3 Table 28 (numerical and arithmetic functions),
//! Table 29 (arithmetic functions, MOD), Table 30 (bit shift functions), specification 07
//! sections 3, 3.1, 5 and 10, specification 10 sections 6.4-6.6, and IEEE 754 for signed
//! zeros and the binary32 and binary64 bit patterns.
//!
//! Expected values come from the audit oracle (python3 exact fractions and IEEE rounding),
//! not from the implementation. A failing case is a finding: report it, do not change the
//! expectation. Where a typed literal cannot be written in Structured Text, the ST form is
//! kept as the audit gives it and a native companion asserts the same function through
//! `StandardLibrary::call`.

use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::stdlib::StandardLibrary;
use trust_runtime::value::Value;

fn compile(source: &str) -> TestHarness {
    TestHarness::from_source(source).unwrap_or_else(|error| panic!("fixture must compile: {error}"))
}

fn run_one_cycle(source: &str) -> TestHarness {
    let mut harness = compile(source);
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    harness
}

fn output_after_cycle(source: &str, name: &str) -> Value {
    run_one_cycle(source).try_get_output(name).unwrap()
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
fn oracle_c8_dint_division_minimum_by_minus_one_overflows_and_keeps_destination() {
    // C8: spec 10 section 6.4. The quotient 2^31 is outside DINT; checked in the result type.
    assert_fault_keeps(
        "PROGRAM Main\nVAR a : DINT := DINT#-2147483648; b : DINT := DINT#-1; q : DINT := 5; \
         END_VAR\nq := a / b;\nEND_PROGRAM",
        RuntimeError::Overflow,
        "q",
        Value::DInt(5),
    );
}

#[test]
fn oracle_a18_dint_addition_overflow_keeps_destination() {
    // A18 (weak row): spec 10 section 6.4 and 6.8. DINT MAX plus one is not a DINT.
    assert_fault_keeps(
        "PROGRAM Main\nVAR a : DINT := DINT#2147483647; r : DINT := 3; END_VAR\n\
         r := a + DINT#1;\nEND_PROGRAM",
        RuntimeError::Overflow,
        "r",
        Value::DInt(3),
    );
}

#[test]
fn oracle_a19_uint_subtraction_underflow_keeps_destination() {
    // A19 (weak row): spec 10 section 6.10. Unsigned subtraction below zero is an overflow.
    assert_fault_keeps(
        "PROGRAM Main\nVAR u : UINT := UINT#0; r : UINT := UINT#9; END_VAR\n\
         r := u - UINT#1;\nEND_PROGRAM",
        RuntimeError::Overflow,
        "r",
        Value::UInt(9),
    );
}

#[test]
fn oracle_c17_expt_negative_base_with_integer_valued_exponent_is_exact() {
    // C17: spec 07 section 3 (ANY_REAL base). (-2)^3 = -8 exactly.
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR r : REAL := 0.0; END_VAR\nr := EXPT(REAL#-2.0, REAL#3.0);\n\
             END_PROGRAM",
            "r",
        ),
        Value::Real(-8.0)
    );
}

#[test]
fn oracle_c18_expt_negative_base_with_fractional_exponent_overflows() {
    // C18 (weak row): spec 07 section 3.1. (-2)^0.5 has no real result, a domain error that
    // raises Overflow; the destination keeps its value.
    assert_fault_keeps(
        "PROGRAM Main\nVAR r : REAL := 7.0; END_VAR\nr := EXPT(REAL#-2.0, REAL#0.5);\n\
         END_PROGRAM",
        RuntimeError::Overflow,
        "r",
        Value::Real(7.0),
    );
    assert_eq!(
        StandardLibrary::new().call("EXPT", &[Value::Real(-2.0), Value::Real(0.5)]),
        Err(RuntimeError::Overflow)
    );
}

#[test]
fn oracle_d3_abs_of_minimum_plus_one_is_maximum() {
    // D3: spec 07 section 3.1. MIN + 1 is representable; its absolute value is MAX.
    let harness = run_one_cycle(
        "PROGRAM Main\nVAR s : SINT := 0; i : INT := 0; END_VAR\n\
         s := ABS(SINT#-127);\ni := ABS(INT#-32767);\nEND_PROGRAM",
    );
    assert_eq!(harness.try_get_output("s").unwrap(), Value::SInt(127));
    assert_eq!(harness.try_get_output("i").unwrap(), Value::Int(32767));
}

#[test]
fn oracle_d4_abs_of_unsigned_maximum_is_unchanged() {
    // D4: spec 07 section 3.1, "unsigned values are returned unchanged". ST form first; the
    // ULINT literal above i64::MAX may not compile in truST's literal lowering.
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR u : ULINT := 0; END_VAR\nu := ABS(ULINT#18446744073709551615);\n\
             END_PROGRAM",
            "u",
        ),
        Value::ULInt(18_446_744_073_709_551_615)
    );
}

#[test]
fn oracle_d4_abs_of_unsigned_maximum_native() {
    // D4 (native form): the same function without the ULINT literal.
    assert_eq!(
        StandardLibrary::new().call("ABS", &[Value::ULInt(u64::MAX)]),
        Ok(Value::ULInt(u64::MAX))
    );
}

#[test]
fn oracle_d11_ln_of_negative_overflows_and_keeps_destination() {
    // D11: spec 07 section 3.1 (domain error gives Overflow).
    assert_fault_keeps(
        "PROGRAM Main\nVAR a : LREAL := 7.0; END_VAR\na := LN(LREAL#-1.0);\nEND_PROGRAM",
        RuntimeError::Overflow,
        "a",
        Value::LReal(7.0),
    );
}

#[test]
fn oracle_d12_ln_of_negative_zero_overflows_and_keeps_destination() {
    // D12: spec 07 section 13 (non-positive argument). -0.0 is not positive.
    assert_fault_keeps(
        "PROGRAM Main\nVAR a : LREAL := 7.0; END_VAR\na := LN(LREAL#-0.0);\nEND_PROGRAM",
        RuntimeError::Overflow,
        "a",
        Value::LReal(7.0),
    );
}

#[test]
fn oracle_d14_log_of_zero_overflows_and_keeps_destination() {
    // D14: spec 07 section 3.1. LOG(0) is minus infinity, which is not a finite result.
    assert_fault_keeps(
        "PROGRAM Main\nVAR a : LREAL := 7.0; END_VAR\na := LOG(LREAL#0.0);\nEND_PROGRAM",
        RuntimeError::Overflow,
        "a",
        Value::LReal(7.0),
    );
}

#[test]
fn oracle_d20_asin_of_one_is_pi_over_two_single() {
    // D20: IEC 61131-3 Table 28, range -pi/2 to pi/2. The binary32 nearest to pi/2 is
    // 0x3FC90FDB.
    let bits = real_bits(&output_after_cycle(
        "PROGRAM Main\nVAR a : REAL := 0.0; END_VAR\na := ASIN(REAL#1.0);\nEND_PROGRAM",
        "a",
    ));
    assert_eq!(bits, 0x3FC9_0FDB);
}

#[test]
fn oracle_d21_asin_just_above_one_overflows_and_keeps_destination() {
    // D21: spec 07 section 3.1. 1 + 2^-52 is outside [-1, 1].
    assert_fault_keeps(
        "PROGRAM Main\nVAR a : LREAL := 7.0; END_VAR\na := ASIN(LREAL#1.0000000000000002);\n\
         END_PROGRAM",
        RuntimeError::Overflow,
        "a",
        Value::LReal(7.0),
    );
}

#[test]
fn oracle_d27_atan2_signed_zero_quadrants() {
    // D27: IEEE 754 atan2 quadrant rules. atan2(-0, -1) = -pi; atan2(+0, -0) = +pi.
    let negative_pi = lreal_bits(&output_after_cycle(
        "PROGRAM Main\nVAR a : LREAL := 0.0; END_VAR\na := ATAN2(LREAL#-0.0, LREAL#-1.0);\n\
         END_PROGRAM",
        "a",
    ));
    assert_eq!(negative_pi, 0xC009_21FB_5444_2D18);
    let positive_pi = lreal_bits(&output_after_cycle(
        "PROGRAM Main\nVAR a : LREAL := 0.0; END_VAR\na := ATAN2(LREAL#0.0, LREAL#-0.0);\n\
         END_PROGRAM",
        "a",
    ));
    assert_eq!(positive_pi, 0x4009_21FB_5444_2D18);
}

#[test]
fn oracle_i9_is_valid_accepts_negative_zero() {
    // I9: spec 07 section 10. -0.0 is finite, so it is valid.
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR ok : BOOL := FALSE; END_VAR\n\
             ok := IS_VALID(REAL#-0.0);\nEND_PROGRAM",
            "ok",
        ),
        Value::Bool(true)
    );
}

#[test]
fn oracle_f14_eq_treats_negative_zero_as_equal_to_positive_zero() {
    // F14: spec 07 section 13 (finite IEEE ordering). -0.0 compares equal to +0.0.
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR b : BOOL := FALSE; END_VAR\nb := EQ(REAL#0.0, REAL#-0.0);\n\
             END_PROGRAM",
            "b",
        ),
        Value::Bool(true)
    );
}

#[test]
fn oracle_e4_shl_moves_the_top_bit_of_a_byte_to_bit_seven() {
    // E4: spec 07 section 5. 0x81 shifted left by 7 keeps only bit 0, which lands on bit 7.
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR b : BYTE := BYTE#16#81; END_VAR\nb := SHL(IN := b, N := 7);\n\
             END_PROGRAM",
            "b",
        ),
        Value::Byte(0x80)
    );
}

#[test]
fn oracle_e5_shl_word_one_to_msb_then_out_of_width() {
    // E5: spec 07 section 5. 1 << 15 is 0x8000; 1 << 16 shifts the bit out of the WORD.
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR w : WORD := WORD#0; END_VAR\nw := SHL(IN := WORD#1, N := 15);\n\
             END_PROGRAM",
            "w",
        ),
        Value::Word(0x8000)
    );
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR w : WORD := WORD#0; END_VAR\nw := SHL(IN := WORD#1, N := 16);\n\
             END_PROGRAM",
            "w",
        ),
        Value::Word(0x0000)
    );
}

#[test]
fn oracle_e7_rol_count_beyond_width_is_taken_modulo_width() {
    // E7: spec 07 section 5. Rotating 0x81 by 9 on an 8-bit value equals rotating by 1.
    assert_eq!(
        output_after_cycle(
            "PROGRAM Main\nVAR b : BYTE := BYTE#16#81; END_VAR\nb := ROL(IN := b, N := 9);\n\
             END_PROGRAM",
            "b",
        ),
        Value::Byte(0x03)
    );
}
