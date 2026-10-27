//! Nested relative array extents of the temporal wire image (spec 34 §4.3.1, uniform temporal ABI).
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.
#![cfg(feature = "full")]
use trust_runtime::{harness::TestHarness, value::Value};

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

/// Spec 34 §4.3.1, uniform temporal storage ABI (lines 541-554): nested relative array
/// extents. A row is 12 wire bytes (second TIME at %B8), a bank holds two rows (24 bytes)
/// and the outer array strides by one bank, so every leaf lands at its complete offset.
#[test]
fn nested_relative_array_extents_place_every_leaf_at_its_complete_wire_offset() {
    let source = r#"
TYPE TimedPair : STRUCT first AT %B0 : TIME; second AT %B8 : TIME; END_STRUCT; END_TYPE
TYPE TimedBank : STRUCT rows AT %B0 : ARRAY[0..1] OF TimedPair; END_STRUCT; END_TYPE
VAR_GLOBAL banks AT %QB0 : ARRAY[0..1] OF TimedBank; END_VAR
PROGRAM Main
    banks[0].rows[0].first := T#1s; banks[0].rows[0].second := T#2s;
    banks[0].rows[1].first := T#3s; banks[0].rows[1].second := T#4s;
    banks[1].rows[0].first := T#5s; banks[1].rows[0].second := T#6s;
    banks[1].rows[1].first := T#7s; banks[1].rows[1].second := T#8s;
END_PROGRAM
"#;
    let mut expected = [0_u8; 48];
    for (offset, milliseconds) in [
        (0, 1_000_u32),
        (8, 2_000),
        (12, 3_000),
        (20, 4_000),
        (24, 5_000),
        (32, 6_000),
        (36, 7_000),
        (44, 8_000),
    ] {
        expected[offset..offset + 4].copy_from_slice(&milliseconds.to_le_bytes());
    }
    let mut harness = TestHarness::from_source(source).unwrap();
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(output_bytes(&harness, expected.len()), expected);
}
