//! TIME-08: copy-on-write detach of a shared structure charges each field as a copy.
use super::*;
use crate::collections::OrderedMap;
use crate::error::ExecutionLimitKind;
use crate::value::{ArrayValue, StructValue};
use alloc::{boxed::Box, string::String, sync::Arc, vec};

fn probe_structure() -> Arc<StructValue> {
    let mut fields = OrderedMap::default();
    fields.insert("small".into(), Value::DInt(9));
    fields.insert("name".into(), Value::WString(String::from("xy")));
    fields.insert(
        "items".into(),
        Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![Value::DInt(0); 3],
            vec![(0, 2)],
        ))),
    );
    Arc::new(StructValue::from_canonical_parts("Probe".into(), fields))
}

#[test]
fn shared_structure_detach_charges_each_field_copy_work() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!(
            "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let services = &super::super::services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, services);
    let shared = probe_structure();
    // The second owner makes the structure shared, so a field write must detach it.
    let root = Value::Struct(Arc::clone(&shared));
    let path = [RefSegment::Field("small".into())];

    // TIME-08 COPY: detaching copies every field. "name" is a WSTRING (1 unit), "items" is an
    // array (1 + 3 elements) and "small" is a scalar (nothing), so the detach costs 5 units.
    state.data.resources.work_budget.reset(5);
    state.data.resources.construction_bytes.set(0);
    state
        .charge_path_write(&root, &path, &Value::DInt(90))
        .unwrap();
    assert_eq!(state.data.resources.work_budget.remaining(), 0);

    state.data.resources.work_budget.reset(4);
    let error = state
        .charge_path_write(&root, &path, &Value::DInt(90))
        .unwrap_err();
    assert!(matches!(
        error,
        RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            limit: 4,
            ..
        }
    ));
}

#[test]
fn unshared_structure_write_detaches_nothing() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!(
            "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let services = &super::super::services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, services);
    // The only owner writes in place: there is no backing to detach, so no field is copied.
    let root = Value::Struct(probe_structure());
    let path = [RefSegment::Field("small".into())];

    state.data.resources.work_budget.reset(0);
    state.data.resources.construction_bytes.set(0);
    state
        .charge_path_write(&root, &path, &Value::DInt(90))
        .unwrap();
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
    assert_eq!(state.data.resources.construction_bytes.get(), 0);
}
