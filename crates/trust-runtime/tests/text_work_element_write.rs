//! TIME-08 TEXT: writing one character charges 3 units per UTF-8 byte of the whole text plus
//! 12 (that is 3 × (B + 4)) before the commit. A refused write leaves the text unchanged.
use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::error::{ExecutionLimitKind, RuntimeError};
use trust_runtime_core::value::{RefSegment, Value};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

const SOURCE: &str = "VAR_GLOBAL
    text : STRING[6];
    wide : WSTRING[6];
    marker : CHAR;
    wide_marker : WCHAR;
END_VAR
PROGRAM Main END_PROGRAM
";

fn artifact() -> Vec<u8> {
    CompileSession::from_source(SOURCE)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("accepted source")
        .encode()
        .unwrap()
}

/// Seeds `name` with `seed`, then writes `value` at the 1-based character `index`. Each of the
/// two writes is an engineering operation with its own allowance of `max_work`. Returns the
/// element write's outcome and the destination after it.
fn write_element(
    name: &str,
    seed: Value,
    index: i64,
    value: Value,
    max_work: usize,
) -> (Result<(), RuntimeError>, Value) {
    let limits = PreparationLimits {
        max_work,
        ..Default::default()
    };
    let prepared = PreparedModule::from_bytes(&artifact(), limits).expect("admit the artifact");
    let mut state = prepared
        .instantiate(0)
        .expect("the default construction allowance admits the artifact");
    state
        .write_global(name, seed)
        .expect("the seed fits the allowance");
    let outcome = state.write_named_path(name, &[RefSegment::Index(vec![index])], value);
    let after = state
        .storage()
        .get_global(name)
        .expect("declared global")
        .clone();
    (outcome, after)
}

#[test]
fn char_write_is_refused_one_unit_below_three_units_per_padded_byte() {
    // "åäö" is 6 UTF-8 bytes, so the write costs 3 × 6 + 12 = 30 units. Nothing before it
    // charges work, so the refusal reports 0 used. The text must stay "åäö".
    let (outcome, after) = write_element(
        "text",
        Value::String("åäö".into()),
        2,
        Value::Char(b'x'),
        29,
    );
    assert!(matches!(
        outcome,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 0,
            requested: 30,
            limit: 29,
        })
    ));
    assert_eq!(after, Value::String("åäö".into()));
}

#[test]
fn char_write_is_admitted_at_three_units_per_padded_byte() {
    let (outcome, after) = write_element(
        "text",
        Value::String("åäö".into()),
        2,
        Value::Char(b'x'),
        30,
    );
    outcome.unwrap();
    assert_eq!(after, Value::String("åxö".into()));
}

#[test]
fn wide_char_write_is_refused_one_unit_below_three_units_per_padded_byte() {
    // "αβγ" is 6 UTF-8 bytes, so the WSTRING element write costs 3 × 6 + 12 = 30 units.
    let (outcome, after) = write_element(
        "wide",
        Value::WString("αβγ".into()),
        2,
        Value::WChar(u16::from(b'x')),
        29,
    );
    assert!(matches!(
        outcome,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 0,
            requested: 30,
            limit: 29,
        })
    ));
    assert_eq!(after, Value::WString("αβγ".into()));

    let (outcome, after) = write_element(
        "wide",
        Value::WString("αβγ".into()),
        2,
        Value::WChar(u16::from(b'x')),
        30,
    );
    outcome.unwrap();
    assert_eq!(after, Value::WString("αxγ".into()));
}
