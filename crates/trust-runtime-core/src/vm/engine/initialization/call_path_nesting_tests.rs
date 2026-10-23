//! Structural nesting: 64 edges from the root construct; the 65th edge is refused and leaves no type on the stack.
use super::*;
use crate::bytecode::{TypeData, TypeEntry, TypeKind};
use crate::vm::{construction::values::construct_intrinsic_value, PreparationLimits};

fn fixture() -> PreparedModule {
    PreparedModule::from_bytes(
        include_bytes!(
            "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        PreparationLimits::default(),
    )
    .unwrap()
}

/// A forged chain of `edges` one-element arrays ending at the fixture's first primitive. The module bypasses
/// admission on purpose, so the construction path itself must refuse what admission would.
/// Returns the module and the root array's type id.
fn chained_module(edges: usize) -> (PreparedModule, u32) {
    let mut prepared = fixture();
    let leaf = prepared
        .vm
        .types
        .entries
        .iter()
        .position(|entry| matches!(entry.data, TypeData::Primitive { .. }))
        .expect("fixture primitive") as u32;
    let root = prepared.vm.types.entries.len() as u32;
    for level in 0..edges {
        let elem_type_id = if level + 1 == edges {
            leaf
        } else {
            root + level as u32 + 1
        };
        prepared.vm.types.entries.push(TypeEntry {
            kind: TypeKind::Array,
            name_idx: None,
            data: TypeData::Array {
                elem_type_id,
                dims: alloc::vec![(0, 0)],
            },
        });
    }
    (prepared, root)
}

/// Spec 12 §4.1: at most 64 structural edges (65 nodes) are accepted from the root.
#[test]
fn call_path_structural_chain_of_64_edges_constructs() {
    let (prepared, root) = chained_module(64);
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    state.data.resources.construction_bytes.set(0);
    state
        .data
        .resources
        .work_budget
        .reset(prepared.limits.max_work);
    let value = construct_intrinsic_value(&prepared.vm.types, &mut state, root);
    assert!(
        matches!(value, Ok(Value::Array(_))),
        "64 edges are admitted"
    );
    assert!(state.data.construction.types.is_empty());
}

/// Spec 12 §4.1: edge 65 is refused before construction, and the refused construction leaves no active type.
#[test]
fn call_path_structural_chain_of_65_edges_is_refused_and_unwinds() {
    let (prepared, root) = chained_module(65);
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    state.data.resources.construction_bytes.set(0);
    state
        .data
        .resources
        .work_budget
        .reset(prepared.limits.max_work);
    assert_eq!(
        construct_intrinsic_value(&prepared.vm.types, &mut state, root),
        Err(RuntimeError::TypeMismatch)
    );
    assert!(state.data.construction.types.is_empty());
    assert!(state
        .data
        .construction
        .type_operations
        .iter()
        .all(|bits| *bits == 0));
}
