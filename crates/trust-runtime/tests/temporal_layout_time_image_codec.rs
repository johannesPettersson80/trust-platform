//! TIME to DWORD process-image codec boundaries (spec 12 §4.1: a TIME in a DWORD image is unsigned
//! 32-bit milliseconds with its checked range; spec 07 §2.4: TIME_TO_DWORD counts whole
//! milliseconds and rejects negative or greater-than-u32::MAX values). The codec is the explicit
//! external transport, so it must give the same bytes under the 1.1/2.0 and 3.0 storage ABIs.
#![cfg(feature = "full")]
use trust_runtime::{
    bytecode::BytecodeVersion,
    error::RuntimeError,
    harness::{CompileSession, TestHarness},
    value::{Duration, Value},
    Runtime,
};

fn program(literal: &str) -> String {
    format!(
        "VAR_GLOBAL elapsed AT %QD0 : TIME; END_VAR PROGRAM Main elapsed := {literal}; END_PROGRAM"
    )
}

fn install(source: &str, version: BytecodeVersion) -> Runtime {
    let session = CompileSession::from_source(source);
    let module = session.build_bytecode_module_for_version(version).unwrap();
    if version == BytecodeVersion::LEGACY {
        session.build_runtime_from_bytecode(&module, None).unwrap()
    } else {
        Runtime::from_bytes(&module.encode().unwrap(), Default::default()).unwrap()
    }
}

/// Output: the image holds the whole-millisecond count, little-endian. The bottom (0 ms), the
/// first fractional values (1.5 ms and 0.999999 ms truncate toward zero) and the top of the range
/// (4294967295 ms, and 4294967295.999 ms, which truncates to it) all encode exactly.
#[test]
fn time_dword_image_writes_whole_milliseconds_at_the_bottom_and_top_of_the_range() {
    for version in [BytecodeVersion::LEGACY, BytecodeVersion::SOURCE_FREE] {
        for (literal, expected) in [
            ("T#0ms", 0_u32),
            ("T#1ms", 1),
            ("T#1500us", 1),
            ("T#999999ns", 0),
            ("T#4294967295ms", u32::MAX),
            ("T#4294967295999us", u32::MAX),
        ] {
            let mut runtime = install(&program(literal), version);
            runtime.execute_cycle().unwrap();
            assert_eq!(
                runtime.io().outputs(),
                &expected.to_le_bytes(),
                "{version:?} {literal}"
            );
        }
    }
}

/// Output refusal: a negative count, including a negative value under one millisecond (which
/// truncation toward zero would otherwise report as 0 ms), and a count of 2^32 ms are refused
/// as Overflow. No byte reaches the image.
#[test]
fn time_dword_image_refuses_negative_and_above_32_bit_values_without_writing() {
    for version in [BytecodeVersion::LEGACY, BytecodeVersion::SOURCE_FREE] {
        for literal in ["T#-1ms", "T#-999999ns", "T#4294967296ms"] {
            let mut runtime = install(&program(literal), version);
            assert_eq!(
                runtime.execute_cycle(),
                Err(RuntimeError::Overflow),
                "{version:?} {literal}"
            );
            assert_eq!(runtime.io().outputs(), &[0_u8; 4], "{version:?} {literal}");
        }
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
        let runtime = harness.into_runtime();
        assert_eq!(
            runtime.storage().get_global("elapsed"),
            Some(&Value::Time(Duration::from_millis(i64::from(raw)))),
            "{raw}"
        );
    }
}
