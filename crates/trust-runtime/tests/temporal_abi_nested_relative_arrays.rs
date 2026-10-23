//! Nested relative array extents of the temporal wire image (spec 34 §4.3.1, uniform temporal ABI).
#![cfg(feature = "full")]
use trust_runtime::{bytecode::BytecodeVersion, harness::CompileSession, Runtime};

fn install(source: &str, version: BytecodeVersion) -> Runtime {
    let session = CompileSession::from_source(source);
    let module = session.build_bytecode_module_for_version(version).unwrap();
    if version == BytecodeVersion::LEGACY {
        session.build_runtime_from_bytecode(&module, None).unwrap()
    } else {
        Runtime::from_bytes(&module.encode().unwrap(), Default::default()).unwrap()
    }
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
    for version in [BytecodeVersion::LEGACY, BytecodeVersion::SOURCE_FREE] {
        let mut runtime = install(source, version);
        runtime.execute_cycle().unwrap();
        assert_eq!(runtime.io().outputs(), &expected, "{version:?}");
    }
}
