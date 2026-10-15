//! Relative (`AT %B`) structure wire extents with a negative lower array bound. The external
//! image stride is the complete codec extent (spec 10 §1.2: greatest leaf end, rounded to bytes),
//! which is the same in both ABIs. SIZEOF differs: spec 12 §4.1 says the 3.0 uniform layout
//! measures the complete relative extent per element (2 × 8 bytes = 16 bytes), while the 1.1/2.0
//! layout sums the four-byte TIME fields (2 × 4 bytes = 8 bytes per element).
#![cfg(feature = "full")]
use trust_runtime::{bytecode::BytecodeVersion, harness::CompileSession, value::Value, Runtime};

const SOURCE: &str = r#"
TYPE TimedPair : STRUCT first AT %B0 : TIME; second AT %B8 : TIME; END_STRUCT; END_TYPE
VAR_GLOBAL pairs AT %QB0 : ARRAY[-1..1] OF TimedPair; storage_width : DINT; END_VAR
PROGRAM Main
    pairs[-1].first := T#1s; pairs[-1].second := T#2s;
    pairs[0].first := T#3s; pairs[0].second := T#4s;
    pairs[1].first := T#5s; pairs[1].second := T#6s;
    storage_width := SIZEOF(ARRAY[-1..1] OF TimedPair);
END_PROGRAM
"#;

fn install(source: &str, version: BytecodeVersion) -> Runtime {
    let session = CompileSession::from_source(source);
    let module = session.build_bytecode_module_for_version(version).unwrap();
    if version == BytecodeVersion::LEGACY {
        session.build_runtime_from_bytecode(&module, None).unwrap()
    } else {
        Runtime::from_bytes(&module.encode().unwrap(), Default::default()).unwrap()
    }
}

/// Three elements at a 12-byte wire stride make a 36-byte image. Element -1 starts at byte 0,
/// element 0 at byte 12 and element 1 at byte 24, each with its second TIME 8 bytes after its
/// first. The image is identical in both ABIs, while SIZEOF is 48 bytes (uniform) or 24 (legacy).
#[test]
fn negative_bound_relative_array_places_each_element_at_its_wire_stride_in_both_abis() {
    for (version, storage_width) in [
        (BytecodeVersion::LEGACY, 24),
        (BytecodeVersion::SOURCE_FREE, 48),
    ] {
        let mut runtime = install(SOURCE, version);
        runtime.execute_cycle().unwrap();
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
        assert_eq!(runtime.io().outputs(), &expected, "{version:?}");
        assert_eq!(
            runtime.storage().get_global("storage_width"),
            Some(&Value::DInt(storage_width)),
            "{version:?}"
        );
    }
}
