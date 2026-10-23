//! TIME-08 TEXT: reading one character charges 1 unit per byte through a host cycle access and
//! 2 units per byte in a program after the parent copy (a WSTRING parent copy adds 1 unit plus
//! its bytes). Creating a reference to a dynamic character index charges 1 unit per byte.
use super::*;
use crate::error::ExecutionLimitKind;
use crate::value::{RefSegment, Value, ValueRef};
use crate::vm::dispatch_refs::{dynamic_load_ref, dynamic_ref_index};
use crate::vm::{FrameStack, PreparedModule, VmTrap};

/// Runs `operation` against a fixture engine whose global `probe_depth` holds `text`. The
/// operation receives a reference to the whole global. The work allowance is `budget` for that
/// one operation. Returns the outcome and the allowance left afterwards.
fn with_probe<T>(
    text: Value,
    budget: usize,
    operation: impl FnOnce(&mut EngineState<'_>, ValueRef) -> Result<T, RuntimeError>,
) -> (Result<T, RuntimeError>, usize) {
    let prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let services = &super::super::services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    data.storage.set_global("probe_depth", text);
    let reference = data.storage.ref_for_global("probe_depth").unwrap();
    data.resources.construction_bytes.set(0);
    data.resources.work_budget.reset(budget);
    let mut state = EngineState::borrow(&mut data, &prepared, services);
    let outcome = operation(&mut state, reference);
    let remaining = state.data.resources.work_budget.remaining();
    (outcome, remaining)
}

/// The reference to character `index` (1-based) of the global.
fn element(mut reference: ValueRef, index: i64) -> ValueRef {
    reference.path.push(RefSegment::Index(alloc::vec![index]));
    reference
}

#[test]
fn host_cycle_character_read_charges_one_unit_per_byte_of_the_text() {
    // "åäö" is 3 characters in 6 UTF-8 bytes. Reading character 2 through a host cycle access
    // admits with 6 units and is refused with 5, so the read costs one unit per byte.
    let (outcome, remaining) = with_probe(Value::String("åäö".into()), 6, |state, reference| {
        CycleAccess {
            engine: state,
            phase: CyclePhase::BeforeInputSample,
            output_undo: None,
        }
        .read_reference(&element(reference, 2))
    });
    assert_eq!(outcome.unwrap(), Value::Char(0xE4));
    assert_eq!(remaining, 0);

    let (outcome, remaining) = with_probe(Value::String("åäö".into()), 5, |state, reference| {
        CycleAccess {
            engine: state,
            phase: CyclePhase::BeforeInputSample,
            output_undo: None,
        }
        .read_reference(&element(reference, 2))
    });
    assert!(matches!(
        outcome,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 0,
            requested: 6,
            limit: 5,
        })
    ));
    assert_eq!(remaining, 5);
}

#[test]
fn program_string_character_read_charges_two_units_per_byte_of_the_text() {
    // A STRING parent copy charges no work units, so reading character 2 of "åäö" (6 bytes)
    // costs 2 × 6 = 12 units.
    let (outcome, remaining) =
        with_probe(Value::String("åäö".into()), 12, |state, reference| {
            dynamic_load_ref(&*state, &FrameStack::default(), &element(reference, 2))
                .map_err(VmTrap::into_runtime_error)
        });
    assert_eq!(outcome.unwrap(), Value::Char(0xE4));
    assert_eq!(remaining, 0);

    let (outcome, remaining) =
        with_probe(Value::String("åäö".into()), 11, |state, reference| {
            dynamic_load_ref(&*state, &FrameStack::default(), &element(reference, 2))
                .map_err(VmTrap::into_runtime_error)
        });
    assert!(matches!(
        outcome,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 0,
            requested: 12,
            limit: 11,
        })
    ));
    assert_eq!(remaining, 11);
}

#[test]
fn program_wstring_character_read_charges_the_parent_copy_then_two_units_per_byte() {
    // "αβγ" is 3 characters in 6 UTF-8 bytes. The WSTRING parent copy costs 1 unit, then the
    // character read costs 2 × 6 = 12 units, so the whole read costs 13 units.
    let (outcome, remaining) =
        with_probe(Value::WString("αβγ".into()), 13, |state, reference| {
            dynamic_load_ref(&*state, &FrameStack::default(), &element(reference, 2))
                .map_err(VmTrap::into_runtime_error)
        });
    assert_eq!(outcome.unwrap(), Value::WChar(0x03B2));
    assert_eq!(remaining, 0);

    // With no allowance, the parent copy is the first refused charge.
    let (outcome, _) = with_probe(Value::WString("αβγ".into()), 0, |state, reference| {
        dynamic_load_ref(&*state, &FrameStack::default(), &element(reference, 2))
            .map_err(VmTrap::into_runtime_error)
    });
    assert!(matches!(
        outcome,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 0,
            requested: 1,
            limit: 0,
        })
    ));

    // The parent copy leaves 11 units, so the 12-unit character read is the refused charge.
    let (outcome, remaining) =
        with_probe(Value::WString("αβγ".into()), 12, |state, reference| {
            dynamic_load_ref(&*state, &FrameStack::default(), &element(reference, 2))
                .map_err(VmTrap::into_runtime_error)
        });
    assert!(matches!(
        outcome,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 1,
            requested: 12,
            limit: 12,
        })
    ));
    assert_eq!(remaining, 11);
}

#[test]
fn dynamic_character_index_reference_charges_the_text_bytes_and_its_new_segment() {
    // "αβγδ" is 4 characters in 8 UTF-8 bytes. Creating the reference to character 2 first
    // charges the text scan: 8 units. It then adds one index segment with one subscript. The
    // growth charge is one unit per added segment and one per added subscript
    // (`dispatch_refs.rs`, `reserve_reference_segment`), so the new segment costs 2 units and
    // the creation costs 8 + 2 = 10. Spec 34 TIME-08 ("Extending a reference") says "1 unit and
    // the allocation of one element per added segment or subscript"; the per-subscript unit is
    // the lead's reading of that row and needs confirming against the spec wording.
    let (outcome, remaining) =
        with_probe(Value::WString("αβγδ".into()), 10, |state, reference| {
            dynamic_ref_index(&*state, &FrameStack::default(), reference, 2)
                .map(|_| ())
                .map_err(VmTrap::into_runtime_error)
        });
    outcome.unwrap();
    assert_eq!(remaining, 0);

    // With 9 units the text scan is admitted (1 unit left), and the 2-unit segment is refused.
    let (outcome, remaining) =
        with_probe(Value::WString("αβγδ".into()), 9, |state, reference| {
            dynamic_ref_index(&*state, &FrameStack::default(), reference, 2)
                .map(|_| ())
                .map_err(VmTrap::into_runtime_error)
        });
    assert!(matches!(
        outcome,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 8,
            requested: 2,
            limit: 9,
        })
    ));
    assert_eq!(remaining, 1);
}

#[test]
fn program_wstring_parent_copy_charges_its_bytes_with_its_unit() {
    // The WSTRING parent copy of "αβγ" costs 1 unit and allocates one Value plus its 6 bytes.
    // The character read itself allocates nothing, so the allocation counter ends there.
    let prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let services = &super::super::services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    data.storage
        .set_global("probe_depth", Value::WString("αβγ".into()));
    let reference = data.storage.ref_for_global("probe_depth").unwrap();
    data.resources.construction_bytes.set(0);
    data.resources.work_budget.reset(13);
    let state = EngineState::borrow(&mut data, &prepared, services);
    let value = dynamic_load_ref(&state, &FrameStack::default(), &element(reference, 2))
        .map_err(VmTrap::into_runtime_error)
        .unwrap();
    assert_eq!(value, Value::WChar(0x03B2));
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
    assert_eq!(
        state.data.resources.construction_bytes.get(),
        core::mem::size_of::<Value>() + 6
    );
}
