//! Direct (%I/%Q) byte, word, double-word and bit views of temporal process-image values.
//! A TIME in a DWORD image is little-endian unsigned milliseconds (spec 12 §4.1). Adjacent TIME
//! images must not alias (spec 10 §1.2 direct-address layout). TIME is not an ANY_BIT type, so it
//! has no partial access (spec 02 §1, Partial Access to ANY_BIT Variables).
#![cfg(feature = "full")]
use trust_runtime::{
    harness::{CompileSession, TestHarness},
    value::Value,
};

/// 258 ms is 0x0000_0102: byte 0 = 0x02, byte 1 = 0x01, bytes 2 and 3 = 0. Every direct view of
/// the same four bytes must agree with that little-endian layout.
#[test]
fn time_image_bytes_words_and_bits_read_back_as_little_endian_direct_values() {
    let mut harness = TestHarness::from_source(
        "VAR_GLOBAL elapsed AT %QD0 : TIME; END_VAR PROGRAM Main elapsed := T#258ms; END_PROGRAM",
    )
    .unwrap();
    harness.cycle();
    assert_eq!(harness.get_direct_output("%QB0"), Ok(Value::Byte(0x02)));
    assert_eq!(harness.get_direct_output("%QB1"), Ok(Value::Byte(0x01)));
    assert_eq!(harness.get_direct_output("%QB2"), Ok(Value::Byte(0x00)));
    assert_eq!(harness.get_direct_output("%QW0"), Ok(Value::Word(0x0102)));
    assert_eq!(harness.get_direct_output("%QD0"), Ok(Value::DWord(258)));
    assert_eq!(harness.get_direct_output("%QX0.0"), Ok(Value::Bool(false)));
    assert_eq!(harness.get_direct_output("%QX0.1"), Ok(Value::Bool(true)));
    assert_eq!(harness.get_direct_output("%QX1.0"), Ok(Value::Bool(true)));
    assert_eq!(harness.get_direct_output("%QX1.1"), Ok(Value::Bool(false)));
}

/// Two TIME images at %QD0 and %QD4 keep their own four bytes. 65536 ms is 0x0001_0000, so its
/// only set byte is byte 2 of the image (address %QB6), and the first image stays untouched.
#[test]
fn adjacent_time_images_do_not_alias_and_expose_their_own_bytes() {
    let mut harness = TestHarness::from_source(
        r#"
VAR_GLOBAL first AT %QD0 : TIME; second AT %QD4 : TIME; END_VAR
PROGRAM Main
    first := T#258ms;
    second := T#65536ms;
END_PROGRAM
"#,
    )
    .unwrap();
    harness.cycle();
    assert_eq!(harness.get_direct_output("%QD0"), Ok(Value::DWord(258)));
    assert_eq!(harness.get_direct_output("%QD4"), Ok(Value::DWord(65_536)));
    assert_eq!(harness.get_direct_output("%QB0"), Ok(Value::Byte(0x02)));
    assert_eq!(harness.get_direct_output("%QB1"), Ok(Value::Byte(0x01)));
    assert_eq!(harness.get_direct_output("%QB2"), Ok(Value::Byte(0x00)));
    assert_eq!(harness.get_direct_output("%QB3"), Ok(Value::Byte(0x00)));
    assert_eq!(harness.get_direct_output("%QB4"), Ok(Value::Byte(0x00)));
    assert_eq!(harness.get_direct_output("%QB5"), Ok(Value::Byte(0x00)));
    assert_eq!(harness.get_direct_output("%QB6"), Ok(Value::Byte(0x01)));
    assert_eq!(harness.get_direct_output("%QB7"), Ok(Value::Byte(0x00)));
}

/// Spec 02 §1: only BYTE, WORD, DWORD and LWORD variables take a `.%X`, `.%B`, `.%W` or `.%D`
/// selector. A TIME or LTIME source is not one of them, so its partial access is a compile error.
#[test]
fn time_and_ltime_variables_reject_partial_bit_and_byte_access() {
    for declaration in ["elapsed : TIME", "elapsed : LTIME"] {
        for (selector, target) in [("%X0", "BOOL"), ("%B0", "BYTE")] {
            let source = format!(
                "VAR_GLOBAL {declaration}; END_VAR \
                 PROGRAM Main VAR picked : {target}; END_VAR \
                 picked := elapsed.{selector}; END_PROGRAM"
            );
            assert!(
                CompileSession::from_source(&source)
                    .build_runtime()
                    .is_err(),
                "{declaration} {selector}"
            );
        }
    }
}

/// Spec 10 line 165 (direct-I/O layout): "8-, 16-, 32-, and 64-bit elementary families map to
/// byte, word, double-word, and long-word I/O widths". LTIME is a 64-bit family, so an LTIME at
/// %QL0 uses one long word of eight little-endian bytes. The LDATE, LTOD and LDT forms follow the
/// same rule in `temporal_layout_direct_long_forms.rs`.
#[test]
fn ltime_at_a_long_word_address_uses_eight_little_endian_image_bytes() {
    let mut harness = TestHarness::from_source(
        "VAR_GLOBAL span AT %QL0 : LTIME; END_VAR PROGRAM Main span := LTIME#5s; END_PROGRAM",
    )
    .unwrap();
    harness.cycle();
    assert_eq!(
        harness.get_direct_output("%QL0"),
        Ok(Value::LWord(5_000_000_000))
    );
}
