//! TIME-08 TEXT: storing a bounded STRING or WSTRING normalizes it at 2 units per UTF-8
//! byte of the value it receives, truncated or not. A type without a declared length is not
//! normalized and charges nothing.
use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::error::{ExecutionLimitKind, RuntimeError};
use trust_runtime_core::value::Value;
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

const SOURCE: &str = "VAR_GLOBAL
    text : STRING[3];
    wide : WSTRING[3];
    free_text : STRING;
    free_wide : WSTRING;
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

/// Stores `value` into `name` as one engineering write whose allowance is `max_work`.
/// Returns the write's outcome and the destination before and after it.
fn store(name: &str, value: Value, max_work: usize) -> (Result<(), RuntimeError>, Value, Value) {
    let limits = PreparationLimits {
        max_work,
        ..Default::default()
    };
    let prepared = PreparedModule::from_bytes(&artifact(), limits).expect("admit the artifact");
    let mut state = prepared
        .instantiate(0)
        .expect("the default construction allowance admits the artifact");
    let before = state
        .storage()
        .get_global(name)
        .expect("declared global")
        .clone();
    let outcome = state.write_global(name, value);
    let after = state
        .storage()
        .get_global(name)
        .expect("declared global")
        .clone();
    (outcome, before, after)
}

#[test]
fn bounded_string_store_charges_two_units_per_byte_when_not_truncated() {
    // Spec 34 TIME-08. "åäö" is 3 characters in 6 UTF-8 bytes (each letter is two bytes),
    // so storing it in STRING[3] costs 2 × 6 = 12 units and keeps all three characters.
    let text = "åäö";
    assert_eq!(text.len(), 6);
    let (outcome, before, after) = store("text", Value::String(text.into()), 12);
    outcome.unwrap();
    assert_eq!(before, Value::String("".into()));
    assert_eq!(after, Value::String(text.into()));

    let (outcome, _, after) = store("text", Value::String(text.into()), 11);
    assert!(matches!(
        outcome,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 0,
            requested: 12,
            limit: 11,
        })
    ));
    assert_eq!(after, Value::String("".into()));
}

#[test]
fn bounded_string_store_charges_two_units_per_byte_of_the_truncated_input() {
    // "åäöü" is 4 characters in 8 UTF-8 bytes. The charge is on the whole input, so the
    // write costs 2 × 8 = 16 units even though only "åäö" (3 characters) is kept.
    let text = "åäöü";
    assert_eq!(text.len(), 8);
    let (outcome, _, after) = store("text", Value::String(text.into()), 16);
    outcome.unwrap();
    assert_eq!(after, Value::String("åäö".into()));

    let (outcome, _, after) = store("text", Value::String(text.into()), 15);
    assert!(matches!(
        outcome,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 0,
            requested: 16,
            limit: 15,
        })
    ));
    assert_eq!(after, Value::String("".into()));
}

#[test]
fn bounded_wstring_store_charges_two_units_per_byte_of_the_truncated_input() {
    // "αβγδ" is 4 characters in 8 UTF-8 bytes; the write costs 2 × 8 = 16 units and keeps "αβγ".
    let text = "αβγδ";
    assert_eq!(text.len(), 8);
    let (outcome, _, after) = store("wide", Value::WString(text.into()), 16);
    outcome.unwrap();
    assert_eq!(after, Value::WString("αβγ".into()));

    let (outcome, _, after) = store("wide", Value::WString(text.into()), 15);
    assert!(matches!(
        outcome,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 0,
            requested: 16,
            limit: 15,
        })
    ));
    assert_eq!(after, Value::WString("".into()));
}

#[test]
fn bounded_wstring_store_charges_two_units_per_byte_when_not_truncated() {
    // "αβγ" is 3 characters in 6 UTF-8 bytes, so storing it in WSTRING[3] costs 12 units.
    let text = "αβγ";
    assert_eq!(text.len(), 6);
    let (outcome, _, after) = store("wide", Value::WString(text.into()), 12);
    outcome.unwrap();
    assert_eq!(after, Value::WString(text.into()));

    let (outcome, _, after) = store("wide", Value::WString(text.into()), 11);
    assert!(matches!(
        outcome,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 0,
            requested: 12,
            limit: 11,
        })
    ));
    assert_eq!(after, Value::WString("".into()));
}

#[test]
fn unbounded_string_store_charges_nothing() {
    // A STRING without a declared length is not normalized. The smallest allowance that
    // preparation admits is 1 unit, and this 8-byte store is admitted with it.
    let text = "åäöü";
    let (outcome, _, after) = store("free_text", Value::String(text.into()), 1);
    outcome.unwrap();
    assert_eq!(after, Value::String(text.into()));
}

#[test]
fn unbounded_wstring_store_charges_nothing() {
    // A WSTRING without a declared length is not normalized. The smallest allowance that
    // preparation admits is 1 unit, and this 12-byte store is admitted with it.
    let text = "αβγδεζ";
    let (outcome, _, after) = store("free_wide", Value::WString(text.into()), 1);
    outcome.unwrap();
    assert_eq!(after, Value::WString(text.into()));
}

#[test]
fn two_mebibyte_store_charges_two_units_per_byte_of_the_input() {
    // The input is 2 MiB of ASCII, so its charge is 2 × 2,097,152 = 4,194,304 units. Only the
    // three-character prefix is retained, but the traversal is charged in full.
    let text = "x".repeat(2 * 1024 * 1024);
    let (outcome, _, after) = store("text", Value::String(text.clone().into()), 4_194_304);
    outcome.unwrap();
    assert_eq!(after, Value::String("xxx".into()));

    let (outcome, _, after) = store("text", Value::String(text.into()), 4_194_303);
    assert!(matches!(
        outcome,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 0,
            requested: 4_194_304,
            limit: 4_194_303,
        })
    ));
    assert_eq!(after, Value::String("".into()));
}
