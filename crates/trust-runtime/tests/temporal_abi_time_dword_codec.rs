//! Explicit TIME to DWORD millisecond codec under the uniform temporal ABI (spec 34 §4.3.1).
#![cfg(feature = "full")]
use trust_runtime::{
    bytecode::BytecodeVersion, error::RuntimeError, harness::CompileSession, Runtime,
};

fn install_source_free(source: &str) -> Runtime {
    let module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    Runtime::from_bytes(&module.encode().unwrap(), Default::default()).unwrap()
}

/// Spec 34 §4.3.1, uniform temporal storage ABI (lines 541-554): existing explicit I/O
/// codecs remain checked conversions with their existing widths. The TIME to DWORD
/// millisecond codec keeps all 32 bits, so the largest millisecond count is written as is.
#[test]
fn explicit_time_dword_millisecond_codec_accepts_the_largest_unsigned_value() {
    let mut runtime = install_source_free(
        "PROGRAM Main VAR elapsed AT %QD0 : TIME; END_VAR elapsed := T#4294967295ms; END_PROGRAM",
    );
    runtime.execute_cycle().unwrap();
    assert_eq!(runtime.io().outputs(), &u32::MAX.to_le_bytes());
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
        let mut runtime = install_source_free(&source);
        assert_eq!(
            runtime.execute_cycle(),
            Err(RuntimeError::Overflow),
            "{literal}"
        );
        assert_eq!(runtime.io().outputs(), &[0_u8; 4], "{literal}");
    }
}
