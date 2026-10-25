//! TIME to DWORD process-image codec boundaries (spec 12 §4.1: a TIME in a DWORD image is unsigned
//! 32-bit milliseconds with its checked range; spec 07 §2.4: TIME_TO_DWORD counts whole
//! milliseconds and rejects negative or greater-than-u32::MAX values). The codec is the explicit
//! external transport of a compiled program's process image.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.
#![cfg(feature = "full")]
use trust_runtime::{
    error::RuntimeError,
    harness::TestHarness,
    value::{Duration, Value},
};

fn program(literal: &str) -> String {
    format!(
        "VAR_GLOBAL elapsed AT %QD0 : TIME; END_VAR PROGRAM Main elapsed := {literal}; END_PROGRAM"
    )
}

/// Output: the image holds the whole-millisecond count, little-endian. The bottom (0 ms), the
/// first fractional values (1.5 ms and 0.999999 ms truncate toward zero) and the top of the range
/// (4294967295 ms, and 4294967295.999 ms, which truncates to it) all encode exactly.
#[test]
fn time_dword_image_writes_whole_milliseconds_at_the_bottom_and_top_of_the_range() {
    for (literal, expected) in [
        ("T#0ms", 0_u32),
        ("T#1ms", 1),
        ("T#1500us", 1),
        ("T#999999ns", 0),
        ("T#4294967295ms", u32::MAX),
        ("T#4294967295999us", u32::MAX),
    ] {
        let mut harness = TestHarness::from_source(&program(literal)).unwrap();
        let result = harness.cycle();
        assert!(result.errors.is_empty(), "{literal}: {:?}", result.errors);
        assert_eq!(
            harness.get_direct_output("%QD0"),
            Ok(Value::DWord(expected)),
            "{literal}"
        );
    }
}

/// Output refusal: a negative count, including a negative value under one millisecond (which
/// truncation toward zero would otherwise report as 0 ms), and a count of 2^32 ms are refused
/// as Overflow. No byte reaches the image.
#[test]
fn time_dword_image_refuses_negative_and_above_32_bit_values_without_writing() {
    for literal in ["T#-1ms", "T#-999999ns", "T#4294967296ms"] {
        let mut harness = TestHarness::from_source(&program(literal)).unwrap();
        assert_eq!(
            harness.cycle().errors,
            [RuntimeError::Overflow],
            "{literal}"
        );
        assert_eq!(
            harness.get_direct_output("%QD0"),
            Ok(Value::DWord(0)),
            "{literal}"
        );
    }
}

/// Input: a DWORD image is read as unsigned whole milliseconds. Zero, one, 1500 and the 32-bit
/// maximum (4294967295 ms, which needs the full unsigned width) decode to exact TIME values with
/// no sign and no wrap.
#[test]
fn time_dword_image_reads_unsigned_milliseconds_exactly() {
    for raw in [0_u32, 1, 1_500, u32::MAX] {
        let mut harness = TestHarness::from_source(
            "VAR_GLOBAL elapsed AT %ID0 : TIME; END_VAR PROGRAM Main END_PROGRAM",
        )
        .unwrap();
        harness.set_direct_input("%ID0", Value::DWord(raw)).unwrap();
        harness.cycle();
        assert_eq!(
            harness.get_output("elapsed"),
            Some(Value::Time(Duration::from_millis(i64::from(raw)))),
            "{raw}"
        );
    }
}
