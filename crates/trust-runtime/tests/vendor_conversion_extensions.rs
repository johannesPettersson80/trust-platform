use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

// docs/specs/07-standard-functions.md 2.9: the vendor-extension conversions compile and run in
// programs (specification 44), next to the IEC Table 24 bit string to BOOL conversion (2.2).
#[test]
fn vendor_extension_conversions_run_in_programs() {
    let source = r#"
PROGRAM Main
VAR
    speed : INT := 4;
    level : REAL := 127.6;
    status : BYTE := 16#80;
    out_byte : BYTE;
    out_word : WORD;
    running : BOOL;
    flagged : BOOL;
    low_bit : BOOL;
    idle : BOOL;
    gain : REAL;
END_VAR
out_byte := REAL_TO_BYTE(INT_TO_REAL(speed) / 10.0 * 255.0);
out_word := REAL_TO_WORD(level);
running := INT_TO_BOOL(speed);
flagged := BYTE_TO_BOOL(status AND BYTE#16#80);
low_bit := BYTE_TO_BOOL(status OR BYTE#16#01);
idle := REAL_TO_BOOL(level - 127.6);
gain := BOOL_TO_REAL(running) * 2.5;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.cycle();

    assert_eq!(harness.get_output("out_byte"), Some(Value::Byte(102)));
    assert_eq!(harness.get_output("out_word"), Some(Value::Word(128)));
    assert_eq!(harness.get_output("running"), Some(Value::Bool(true)));
    // IEC Table 24: the rightmost bit, which bit 7 of the mask leaves clear
    assert_eq!(harness.get_output("flagged"), Some(Value::Bool(false)));
    assert_eq!(harness.get_output("low_bit"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("idle"), Some(Value::Bool(false)));
    assert_eq!(harness.get_output("gain"), Some(Value::Real(2.5)));
}

// docs/specs/07-standard-functions.md 2.9, its examples: REAL_TO_BYTE rounds like REAL_TO_USINT,
// `REAL_TO_BYTE(200.4) = BYTE#200`; `INT_TO_BOOL(-3) = TRUE` and `INT_TO_BOOL(0) = FALSE`;
// BOOL_TO_LREAL gives 1.0. Catches a conversion that cuts a real instead of rounding it, or that
// reads only the rightmost bit of an integer (-3 has it set, 2 does not).
#[test]
fn vendor_extension_conversions_follow_the_examples_of_the_specification() {
    let source = r#"
PROGRAM Main
VAR
    reading : REAL := 200.4;
    negative : INT := -3;
    even : INT := 2;
    nothing : INT := 0;
    rounded : BYTE;
    from_negative : BOOL;
    from_even : BOOL;
    from_nothing : BOOL;
    one : LREAL;
END_VAR
rounded := REAL_TO_BYTE(reading);
from_negative := INT_TO_BOOL(negative);
from_even := INT_TO_BOOL(even);
from_nothing := INT_TO_BOOL(nothing);
one := BOOL_TO_LREAL(TRUE);
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    assert!(harness.cycle().errors.is_empty());

    assert_eq!(harness.get_output("rounded"), Some(Value::Byte(200)));
    assert_eq!(harness.get_output("from_negative"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("from_even"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("from_nothing"), Some(Value::Bool(false)));
    assert_eq!(harness.get_output("one"), Some(Value::LReal(1.0)));
}

// docs/specs/07-standard-functions.md 2.9: a real outside the target's range is an overflow, as
// `REAL_TO_BYTE(256.0)` is, and the variable keeps its value. Catches a conversion that keeps the
// low-order bits (0, or 65535 for -1) or saturates under the IEC rules.
#[test]
fn vendor_extension_conversions_refuse_a_value_outside_the_target() {
    for (expression, target, kept) in [
        ("REAL_TO_BYTE(REAL#256.0)", "out_byte", Value::Byte(7)),
        ("LREAL_TO_WORD(LREAL#-1.0)", "out_word", Value::Word(9)),
    ] {
        let source = format!(
            r#"
PROGRAM Main
VAR
    out_byte : BYTE := 7;
    out_word : WORD := 9;
END_VAR
{target} := {expression};
END_PROGRAM
"#
        );
        let mut harness = TestHarness::from_source(&source).unwrap();
        let cycle = harness.cycle();
        assert_eq!(cycle.errors, vec![RuntimeError::Overflow], "{expression}");
        assert_eq!(harness.get_output(target), Some(kept), "{expression}");
    }
}
