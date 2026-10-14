//! TIME-08 TEXT: the canonical pass of a structure charges 1 unit per UTF-8 byte of each
//! bounded text field it checks, before it counts the characters.
use std::sync::Arc;

use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::collections::OrderedMap;
use trust_runtime_core::error::{ExecutionLimitKind, RuntimeError};
use trust_runtime_core::value::{StructValue, Value};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

// ST identifiers are case-insensitive, so the global must not share the type's name.
const SOURCE: &str = "TYPE Caption : STRUCT text : STRING[5]; END_STRUCT; END_TYPE
VAR_GLOBAL shown : Caption; END_VAR
PROGRAM Main END_PROGRAM
";

fn artifact() -> Vec<u8> {
    CompileSession::from_source(SOURCE)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("accepted source")
        .encode()
        .unwrap()
}

fn caption(text: &str) -> Value {
    let mut fields = OrderedMap::default();
    fields.insert("text".into(), Value::String(text.into()));
    Value::Struct(Arc::new(StructValue::from_canonical_parts(
        "Caption".into(),
        fields,
    )))
}

/// Stores `value` into the `shown` global as one engineering write under `max_work`.
fn write_caption(artifact: &[u8], value: &Value, max_work: usize) -> Result<(), RuntimeError> {
    let limits = PreparationLimits {
        max_work,
        ..Default::default()
    };
    let prepared = PreparedModule::from_bytes(artifact, limits).expect("admit the artifact");
    let mut state = prepared
        .instantiate(0)
        .expect("the default construction allowance admits the artifact");
    state.write_global("shown", value.clone())
}

/// The least engineering allowance that admits storing `value`. Admission is monotone in it.
/// Preparation rejects an allowance of 0, so the search starts at 1; a structure write visits
/// its own node, so its least admitted allowance is at least 1 in any case.
fn least_admitted_work(artifact: &[u8], value: &Value) -> usize {
    let (mut low, mut high) = (1usize, 1_000_000usize);
    assert!(write_caption(artifact, value, high).is_ok());
    while low < high {
        let middle = low + (high - low) / 2;
        match write_caption(artifact, value, middle) {
            Ok(()) => high = middle,
            Err(RuntimeError::ExecutionLimit {
                kind: ExecutionLimitKind::Work,
                ..
            }) => low = middle + 1,
            Err(error) => panic!("a refusal other than work: {error:?}"),
        }
    }
    low
}

#[test]
fn canonical_structure_pass_charges_one_unit_per_byte_of_bounded_text() {
    // Spec 34 TIME-08. An empty field is canonical and its check charges nothing, so the
    // least allowance for it is the lifetime-check cost alone. "åäö" is canonical in
    // STRING[5] (3 characters ≤ 5) and its check charges its 6 UTF-8 bytes.
    let artifact = artifact();
    let empty = least_admitted_work(&artifact, &caption(""));
    let text = least_admitted_work(&artifact, &caption("åäö"));
    assert_eq!(text - empty, 6);
}

#[test]
fn canonical_structure_pass_refuses_at_the_byte_charge() {
    // With the allowance one unit below the admitted total, the canonical check of "åäö"
    // is the refused charge: 6 units requested after the `empty` units already used.
    let artifact = artifact();
    let empty = least_admitted_work(&artifact, &caption(""));
    let outcome = write_caption(&artifact, &caption("åäö"), empty + 5);
    assert!(matches!(
        outcome,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used,
            requested: 6,
            limit,
        }) if used == empty && limit == empty + 5
    ));
}
