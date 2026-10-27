//! Explicit TIME to DWORD millisecond codec under the uniform temporal ABI (spec 34 §4.3.1).
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.
#![cfg(feature = "full")]
use trust_runtime::{error::RuntimeError, harness::TestHarness, value::Value};

/// Spec 34 §4.3.1, uniform temporal storage ABI (lines 541-554): existing explicit I/O
/// codecs remain checked conversions with their existing widths. The TIME to DWORD
/// millisecond codec keeps all 32 bits, so the largest millisecond count is written as is.
#[test]
fn explicit_time_dword_millisecond_codec_accepts_the_largest_unsigned_value() {
    let mut harness = TestHarness::from_source(
        "PROGRAM Main VAR elapsed AT %QD0 : TIME; END_VAR elapsed := T#4294967295ms; END_PROGRAM",
    )
    .unwrap();
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(
        harness.get_direct_output("%QD0"),
        Ok(Value::DWord(u32::MAX))
    );
}

/// Spec 34 §4.3.1, uniform temporal storage ABI (lines 541-554): a checked conversion
/// refuses a TIME whose millisecond count does not fit the DWORD width. A negative count
/// and one that a truncating codec would wrap to 5000 ms are both refused, and no
/// wrapped value reaches the process image.
#[test]
fn explicit_time_dword_millisecond_codec_refuses_values_outside_the_dword_range() {
    for literal in ["T#-1ms", "T#4294972296ms"] {
        let source = format!(
            "PROGRAM Main VAR elapsed AT %QD0 : TIME; END_VAR elapsed := {literal}; END_PROGRAM"
        );
        let mut harness = TestHarness::from_source(&source).unwrap();
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
