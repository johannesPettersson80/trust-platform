//! Relative (`AT %B`) structure wire extents with a negative lower array bound. The external
//! image stride is the complete codec extent (spec 10 §1.2: greatest leaf end, rounded to bytes).
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.
#![cfg(feature = "full")]
use trust_runtime::{harness::TestHarness, value::Value};

const SOURCE: &str = r#"
TYPE TimedPair : STRUCT first AT %B0 : TIME; second AT %B8 : TIME; END_STRUCT; END_TYPE
VAR_GLOBAL pairs AT %QB0 : ARRAY[-1..1] OF TimedPair; END_VAR
PROGRAM Main
    pairs[-1].first := T#1s; pairs[-1].second := T#2s;
    pairs[0].first := T#3s; pairs[0].second := T#4s;
    pairs[1].first := T#5s; pairs[1].second := T#6s;
END_PROGRAM
"#;

/// The `len` bytes of the output image from `%QB0`.
fn output_bytes(harness: &TestHarness, len: usize) -> Vec<u8> {
    (0..len)
        .map(
            |byte| match harness.get_direct_output(&format!("%QB{byte}")) {
                Ok(Value::Byte(value)) => value,
                other => panic!("%QB{byte}: {other:?}"),
            },
        )
        .collect()
}

/// Three elements at a 12-byte wire stride make a 36-byte image. Element -1 starts at byte 0,
/// element 0 at byte 12 and element 1 at byte 24, each with its second TIME 8 bytes after its
/// first.
#[test]
fn negative_bound_relative_array_places_each_element_at_its_wire_stride() {
    let mut harness = TestHarness::from_source(SOURCE).unwrap();
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let mut expected = [0_u8; 36];
    for (offset, milliseconds) in [
        (0, 1_000_u32),
        (8, 2_000),
        (12, 3_000),
        (20, 4_000),
        (24, 5_000),
        (32, 6_000),
    ] {
        expected[offset..offset + 4].copy_from_slice(&milliseconds.to_le_bytes());
    }
    assert_eq!(output_bytes(&harness, expected.len()), expected);
}
