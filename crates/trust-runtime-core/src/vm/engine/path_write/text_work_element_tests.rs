//! TIME-08 TEXT: writing one character charges 3 units per byte of the whole text plus 12, and
//! 16 allocation bytes per byte plus 64, before the commit. The work charge is made first.
use super::*;
use crate::error::ExecutionLimitKind;

fn fixture() -> PreparedModule {
    PreparedModule::from_bytes(
        include_bytes!(
            "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap()
}

#[test]
fn char_element_write_charges_three_units_per_padded_byte() {
    // "åäö" is 6 UTF-8 bytes, so writing element 2 costs 3 × (6 + 4) = 30 units. The work
    // refusal comes before any allocation, so the allocation counter stays at zero.
    let prepared = fixture();
    let services = &super::super::services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, services);
    let root = Value::String("åäö".into());
    let path = [RefSegment::Index(vec![2])];
    let value = Value::Char(b'x');

    state.data.resources.work_budget.reset(29);
    state.data.resources.construction_bytes.set(0);
    let error = state.charge_path_write(&root, &path, &value).unwrap_err();
    assert!(matches!(
        error,
        RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 0,
            requested: 30,
            limit: 29,
        }
    ));
    assert_eq!(state.data.resources.construction_bytes.get(), 0);

    state.data.resources.work_budget.reset(30);
    state.charge_path_write(&root, &path, &value).unwrap();
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
}

#[test]
fn char_element_write_allocates_sixteen_bytes_per_padded_byte() {
    // "åäö" is 6 UTF-8 bytes, so writing one character allocates 16 × (6 + 4) = 160 bytes.
    let prepared = fixture();
    let services = &super::super::services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, services);
    let root = Value::String("åäö".into());
    let path = [RefSegment::Index(vec![2])];
    let value = Value::Char(b'x');
    let limit = prepared.limits.max_construction_bytes;

    state.data.resources.work_budget.reset(1_000);
    state.data.resources.construction_bytes.set(limit - 159);
    let error = state.charge_path_write(&root, &path, &value).unwrap_err();
    assert!(matches!(
        error,
        RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::AllocationBytes,
            used,
            requested: 160,
            limit: refused_limit,
        } if used == limit - 159 && refused_limit == limit
    ));
    assert_eq!(state.data.resources.construction_bytes.get(), limit - 159);

    state.data.resources.construction_bytes.set(limit - 160);
    state.charge_path_write(&root, &path, &value).unwrap();
    assert_eq!(state.data.resources.construction_bytes.get(), limit);
}

#[test]
fn wide_char_element_write_charges_three_units_per_padded_byte() {
    // "αβγ" is 6 UTF-8 bytes, so writing a WCHAR element costs 3 × (6 + 4) = 30 units.
    let prepared = fixture();
    let services = &super::super::services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, services);
    let root = Value::WString("αβγ".into());
    let path = [RefSegment::Index(vec![2])];
    let value = Value::WChar(u16::from(b'x'));

    state.data.resources.work_budget.reset(29);
    let error = state.charge_path_write(&root, &path, &value).unwrap_err();
    assert!(matches!(
        error,
        RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 0,
            requested: 30,
            limit: 29,
        }
    ));

    state.data.resources.work_budget.reset(30);
    state.charge_path_write(&root, &path, &value).unwrap();
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
}
