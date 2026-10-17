//! TIME-08 TEXT: storing a STRING or WSTRING normalizes it at 2 units per UTF-8 byte of the
//! value it receives, truncated or not. Every STRING and WSTRING is bounded: STRING[n] holds n
//! bytes of UTF-8, WSTRING[n] n UTF-16 code units, and a type without a declared length holds
//! 80 (docs/IEC_DECISIONS.md 2026-10-17).
use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::error::{ExecutionLimitKind, RuntimeError};
use trust_runtime_core::value::Value;
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

const SOURCE: &str = "VAR_GLOBAL
    text : STRING[6];
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
    // so storing it in STRING[6] costs 2 × 6 = 12 units and keeps all three characters.
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
    // write costs 2 × 8 = 16 units even though only "åäö" (the 6 bytes of STRING[6]) is kept.
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
    // "αβγδ" is 4 characters in 8 UTF-8 bytes and 4 UTF-16 code units; the write costs
    // 2 × 8 = 16 units and keeps the 3 code units "αβγ" of WSTRING[3].
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
fn a_string_without_a_length_is_normalized_like_string_80() {
    // A STRING without a declared length is STRING[80]: storing the 8-byte "åäöü" costs
    // 2 × 8 = 16 units and keeps it whole, and 15 units refuse the write.
    let text = "åäöü";
    let (outcome, _, after) = store("free_text", Value::String(text.into()), 16);
    outcome.unwrap();
    assert_eq!(after, Value::String(text.into()));

    let (outcome, _, after) = store("free_text", Value::String(text.into()), 15);
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

    // 50 "ä" are 100 bytes: the store keeps the 40 that fill 80 bytes and charges all 100.
    let long = "ä".repeat(50);
    let (outcome, _, after) = store("free_text", Value::String(long.as_str().into()), 200);
    outcome.unwrap();
    assert_eq!(after, Value::String("ä".repeat(40).into()));
}

#[test]
fn a_wstring_without_a_length_is_normalized_like_wstring_80() {
    // A WSTRING without a declared length is WSTRING[80]: "αβγδεζ" is 12 UTF-8 bytes and 6
    // code units, so the store costs 2 × 12 = 24 units and keeps it whole.
    let text = "αβγδεζ";
    let (outcome, _, after) = store("free_wide", Value::WString(text.into()), 24);
    outcome.unwrap();
    assert_eq!(after, Value::WString(text.into()));

    let (outcome, _, _) = store("free_wide", Value::WString(text.into()), 23);
    assert!(matches!(
        outcome,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            requested: 24,
            limit: 23,
            ..
        })
    ));

    // 81 "🙂" are 162 code units: the store keeps the 40 that fill 80 code units.
    let long = "🙂".repeat(81);
    let (outcome, _, after) = store("free_wide", Value::WString(long.clone()), 2 * long.len());
    outcome.unwrap();
    assert_eq!(after, Value::WString("🙂".repeat(40)));
}

#[test]
fn two_mebibyte_store_charges_two_units_per_byte_of_the_input() {
    // The input is 2 MiB of ASCII, so its charge is 2 × 2,097,152 = 4,194,304 units. Only the
    // six-byte prefix of STRING[6] is retained, but the traversal is charged in full.
    let text = "x".repeat(2 * 1024 * 1024);
    let (outcome, _, after) = store("text", Value::String(text.clone().into()), 4_194_304);
    outcome.unwrap();
    assert_eq!(after, Value::String("xxxxxx".into()));

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
