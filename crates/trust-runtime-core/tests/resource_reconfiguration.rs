#![cfg(feature = "std")]
use std::sync::Arc;
use trust_runtime_core::{
    bytecode::{SectionData, SectionId},
    error::RuntimeError,
    value::Duration,
    vm::{PreparationLimits, PreparedModule},
};
const FIXTURE: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc");

#[test]
fn resource_only_reconfiguration_cannot_change_the_installed_storage_abi() {
    use trust_runtime_core::bytecode::{
        BytecodeModule, BytecodeVersion, HEADER_FLAG_CRC32, HEADER_FLAG_LEGACY_TEMPORAL32,
    };
    use trust_runtime_core::value::StorageAbi;
    let limits = PreparationLimits::default();
    let mut raw = BytecodeModule::decode(FIXTURE).unwrap();
    raw.version = BytecodeVersion::SOURCE_FREE;
    raw.flags = HEADER_FLAG_CRC32 | HEADER_FLAG_LEGACY_TEMPORAL32;
    let prepared = Arc::new(PreparedModule::from_decoded(&raw, limits).unwrap());
    let mut state = Arc::clone(&prepared).instantiate_owned(0).unwrap();
    state.execute_cycle(Duration::from_millis(30)).unwrap();
    let globals = state.storage().globals().clone();
    let outputs = state.outputs().to_vec();
    let timing = state
        .task_states()
        .iter()
        .map(|task| (task.last_run, task.last_single, task.overrun_count))
        .collect::<Vec<_>>();

    raw.flags = HEADER_FLAG_CRC32;
    let current = Arc::new(PreparedModule::from_decoded(&raw, limits).unwrap());
    assert_eq!(current.storage_abi(), StorageAbi::UniformTemporal64);
    assert!(state
        .prepare_resource_reconfiguration(current, 0, false)
        .is_err());
    assert!(std::ptr::eq(state.prepared(), prepared.as_ref()));
    assert_eq!(state.prepared().storage_abi(), StorageAbi::LegacyTemporal32);
    assert_eq!(state.storage().globals(), &globals);
    assert_eq!(state.outputs(), outputs);
    assert_eq!(
        state
            .task_states()
            .iter()
            .map(|task| (task.last_run, task.last_single, task.overrun_count))
            .collect::<Vec<_>>(),
        timing
    );
    assert_eq!(state.current_time(), Duration::from_millis(30));
    assert!(state.fault().is_none());
}

#[test]
fn reconfiguration_preserves_owner_values_and_fault_and_rejects_changed_limits() {
    let limits = PreparationLimits::default();
    let (prepared, mut raw) = PreparedModule::from_bytes_with_artifact(FIXTURE, limits).unwrap();
    let mut state = Arc::new(prepared).instantiate_owned(0).unwrap();
    state.execute_cycle(Duration::from_millis(30)).unwrap();
    let before = state.storage().globals().clone();
    state.latch_fault(RuntimeError::DivisionByZero);
    let Some(SectionData::ResourceMeta(meta)) = raw.section_mut(SectionId::ResourceMeta) else {
        panic!("resource")
    };
    meta.resources[0].memory_size += 8;
    let next = Arc::new(PreparedModule::from_decoded(&raw, limits).unwrap());
    state
        .prepare_resource_reconfiguration(next, 0, true)
        .unwrap()
        .commit()
        .unwrap();
    assert_eq!(state.storage().globals(), &before);
    assert!(matches!(state.fault(), Some(RuntimeError::DivisionByZero)));
    assert_eq!(state.current_time(), Duration::from_millis(30));
    let wrong = Arc::new(
        PreparedModule::from_decoded(
            &raw,
            PreparationLimits {
                max_work: limits.max_work + 1,
                ..limits
            },
        )
        .unwrap(),
    );
    assert!(state
        .prepare_resource_reconfiguration(wrong, 0, true)
        .is_err());
    assert_eq!(state.storage().globals(), &before);
}

#[test]
fn discarded_image_candidate_keeps_live_bytes_and_binding_only_update_keeps_task_timing() {
    let limits = PreparationLimits::default();
    let (prepared, mut raw) = PreparedModule::from_bytes_with_artifact(FIXTURE, limits).unwrap();
    let mut state = Arc::new(prepared).instantiate_owned(0).unwrap();
    state.execute_cycle(Duration::from_millis(30)).unwrap();
    let timing = state
        .task_states()
        .iter()
        .map(|task| (task.last_run, task.last_single, task.overrun_count))
        .collect::<Vec<_>>();
    let images = state.outputs().to_vec();
    let Some(SectionData::ResourceMeta(meta)) = raw.section_mut(SectionId::ResourceMeta) else {
        panic!("resource")
    };
    meta.resources[0].outputs_size += 8;
    let next = Arc::new(PreparedModule::from_decoded(&raw, limits).unwrap());
    drop(
        state
            .prepare_resource_reconfiguration(Arc::clone(&next), 0, false)
            .unwrap(),
    );
    assert_eq!(state.outputs(), images);
    state
        .prepare_resource_reconfiguration(next, 0, false)
        .unwrap()
        .commit()
        .unwrap();
    assert_eq!(
        state
            .task_states()
            .iter()
            .map(|task| (task.last_run, task.last_single, task.overrun_count))
            .collect::<Vec<_>>(),
        timing
    );
    assert_eq!(&state.outputs()[..images.len()], images);
    assert!(state.outputs()[images.len()..]
        .iter()
        .all(|byte| *byte == 0));
}

#[test]
fn expiration_at_commit_and_preparation_budget_failure_preserve_the_owner() {
    use std::{cell::Cell, rc::Rc};
    struct Deadline(Rc<Cell<bool>>);
    impl trust_runtime_core::vm::ExecutionServices for Deadline {
        fn deadline_exceeded(&self) -> bool {
            self.0.get()
        }
        fn has_wall_clock(&self) -> bool {
            false
        }
        fn current_dt(&self) -> Result<trust_runtime_core::value::DateTimeValue, RuntimeError> {
            Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
        }
    }
    let limits = PreparationLimits {
        max_preparation_bytes: 1024 * 1024,
        ..Default::default()
    };
    let (prepared, mut raw) = PreparedModule::from_bytes_with_artifact(FIXTURE, limits).unwrap();
    let prepared = Arc::new(prepared);
    let expired = Rc::new(Cell::new(false));
    let mut state = Arc::clone(&prepared)
        .instantiate_owned_with_services(0, Deadline(Rc::clone(&expired)))
        .unwrap();
    state.execute_cycle(Duration::from_millis(30)).unwrap();
    state.latch_fault(RuntimeError::DivisionByZero);
    let values = state.storage().globals().clone();
    let image = state.outputs().to_vec();
    let timing = state
        .task_states()
        .iter()
        .map(|t| (t.last_run, t.last_single, t.overrun_count))
        .collect::<Vec<_>>();
    let Some(SectionData::ResourceMeta(meta)) = raw.section_mut(SectionId::ResourceMeta) else {
        panic!("resource")
    };
    meta.resources[0].memory_size += 8;
    let next = Arc::new(PreparedModule::from_decoded(&raw, limits).unwrap());
    let plan = state
        .prepare_resource_reconfiguration(next, 0, true)
        .unwrap();
    expired.set(true);
    assert_eq!(plan.commit(), Err(RuntimeError::ExecutionTimeout));
    assert!(std::ptr::eq(state.prepared(), prepared.as_ref()));
    assert_eq!(state.storage().globals(), &values);
    assert_eq!(state.outputs(), image);
    assert_eq!(
        state
            .task_states()
            .iter()
            .map(|t| (t.last_run, t.last_single, t.overrun_count))
            .collect::<Vec<_>>(),
        timing
    );
    assert!(matches!(state.fault(), Some(RuntimeError::DivisionByZero)));
    expired.set(false);
    let Some(SectionData::ResourceMeta(meta)) = raw.section_mut(SectionId::ResourceMeta) else {
        panic!("resource")
    };
    meta.resources[0].memory_size = 2 * 1024 * 1024;
    let next = Arc::new(PreparedModule::from_decoded(&raw, limits).unwrap());
    assert!(matches!(
        state.prepare_resource_reconfiguration(next, 0, true),
        Err(RuntimeError::PreparationLimit)
    ));
    assert!(std::ptr::eq(state.prepared(), prepared.as_ref()));
    assert_eq!(state.storage().globals(), &values);
    assert_eq!(state.outputs(), image);
    assert_eq!(
        state
            .task_states()
            .iter()
            .map(|t| (t.last_run, t.last_single, t.overrun_count))
            .collect::<Vec<_>>(),
        timing
    );
    assert!(matches!(state.fault(), Some(RuntimeError::DivisionByZero)));
}

#[test]
fn exact_resource_identity_preserves_the_sign_bit_of_float_constants() {
    use trust_runtime_core::bytecode::TypeData;
    let limits = PreparationLimits::default();
    let (_, mut raw) = PreparedModule::from_bytes_with_artifact(
        include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/numeric-v2.stbc"),
        limits,
    )
    .unwrap();
    let Some(SectionData::TypeTable(types)) = raw.section(SectionId::TypeTable) else {
        panic!("types")
    };
    let float = types
        .entries
        .iter()
        .position(|entry| matches!(entry.data, TypeData::Primitive { prim_id: 15, .. }))
        .unwrap() as u32;
    let Some(SectionData::ConstPool(pool)) = raw.section_mut(SectionId::ConstPool) else {
        panic!("constants")
    };
    let index = pool
        .entries
        .iter()
        .position(|entry| entry.type_id == float)
        .unwrap();
    pool.entries[index].payload = (-0.0f64).to_le_bytes().to_vec();
    let negative = Arc::new(PreparedModule::from_decoded(&raw, limits).unwrap());
    let mut state = Arc::clone(&negative).instantiate_owned(0).unwrap();
    let same = Arc::new(PreparedModule::from_decoded(&raw, limits).unwrap());
    state
        .prepare_resource_reconfiguration(same, 0, false)
        .unwrap()
        .commit()
        .unwrap();
    let Some(SectionData::ConstPool(pool)) = raw.section_mut(SectionId::ConstPool) else {
        panic!("constants")
    };
    pool.entries[index].payload = 0.0f64.to_le_bytes().to_vec();
    let positive = Arc::new(PreparedModule::from_decoded(&raw, limits).unwrap());
    assert!(state
        .prepare_resource_reconfiguration(positive, 0, false)
        .is_err());
}
