//! Installed owner engineering operations preserve state on rejected references.
use trust_runtime_core::{
    bytecode::{BytecodeModule, SectionData, SectionId, StorageOwner},
    error::RuntimeError,
    memory::{FrameId, InstanceId, MemoryLocation},
    value::{Duration, RefSegment, Value, ValueRef},
    vm::{PreparationLimits, PreparedModule},
};

const GPIO: &[u8] = include_bytes!("../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");
const PROGRAM: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc");

fn constant_module() -> BytecodeModule {
    let mut module = BytecodeModule::decode(GPIO).unwrap();
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        panic!("strings")
    };
    let name = strings
        .entries
        .iter()
        .position(|name| name == "probe_depth")
        .unwrap() as u32;
    let Some(SectionData::StorageLayout(layout)) = module.section_mut(SectionId::StorageLayout)
    else {
        panic!("layout")
    };
    layout
        .entries
        .iter_mut()
        .find(|entry| entry.owner == StorageOwner::Global && entry.name_idx == name)
        .unwrap()
        .flags |= 1;
    module
}

macro_rules! reference_checks {
    ($state:expr) => {{
        let state = &mut $state;
        let reference = state.storage().ref_for_global("probe_depth").unwrap();
        let before_run = state.task_states()[0].last_run;
        let before_image = state.outputs().to_vec();
        assert_eq!(state.read_reference(&reference).unwrap(), Value::Int(1));
        state
            .write_reference(reference.as_view(), Value::Int(3))
            .unwrap();
        assert_eq!(state.read_reference(&reference).unwrap(), Value::Int(3));
        assert!(state
            .write_reference(reference.as_view(), Value::Bool(false))
            .is_err());
        assert_eq!(state.read_reference(&reference).unwrap(), Value::Int(3));
        for location in [
            MemoryLocation::Local(FrameId(u32::MAX)),
            MemoryLocation::Instance(InstanceId(u32::MAX)),
        ] {
            let stale = ValueRef {
                location,
                offset: 0,
                path: Vec::new(),
            };
            assert_eq!(
                state.write_reference(stale.as_view(), Value::Int(9)),
                Err(RuntimeError::ReferenceLifetime)
            );
            assert_eq!(
                state.read_reference(&stale),
                Err(RuntimeError::ReferenceLifetime)
            );
        }
        let mut invalid = reference.clone();
        invalid.path.push(RefSegment::Field("missing".into()));
        assert!(state
            .write_reference(invalid.as_view(), Value::Int(9))
            .is_err());
        assert!(state.read_reference(&invalid).is_err());
        let mut invalid = reference.clone();
        invalid.offset = usize::MAX;
        assert!(state
            .write_reference(invalid.as_view(), Value::Int(9))
            .is_err());
        assert!(state.read_reference(&invalid).is_err());
        assert_eq!(
            state.storage().get_global("probe_depth"),
            Some(&Value::Int(3))
        );
        assert_eq!(state.task_states()[0].last_run, before_run);
        assert_eq!(state.outputs(), before_image);
        assert!(state.fault().is_none());
    }};
}

#[test]
fn borrowed_reference_operations_are_checked_and_never_reinitialize_the_program() {
    let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    reference_checks!(state);
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    let Some(Value::Instance(plant)) = state.storage().get_global("Plant") else {
        panic!("root")
    };
    assert_eq!(
        state.storage().get_instance_var(*plant, "depth_result"),
        Some(&Value::Int(3))
    );
}

#[test]
#[cfg(feature = "std")]
fn owned_reference_operations_use_the_same_checked_algorithm() {
    let prepared = std::sync::Arc::new(
        PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap(),
    );
    let mut state = prepared.instantiate_owned(0).unwrap();
    reference_checks!(state);
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    assert!(state.fault().is_none());
}

#[test]
fn constant_and_program_root_replacement_are_rejected_without_state_or_readiness_changes() {
    let module = constant_module();
    let prepared = PreparedModule::from_decoded(&module, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let reference = state.storage().ref_for_global("probe_depth").unwrap();
    let before = state.read_reference(&reference).unwrap();
    assert_eq!(
        state.write_reference(reference.as_view(), Value::Int(3)),
        Err(RuntimeError::ConstantWrite)
    );
    assert_eq!(state.read_reference(&reference).unwrap(), before);
    let root = state.storage().ref_for_global("Plant").unwrap();
    let before_root = state.read_reference(&root).unwrap();
    assert_eq!(
        state.write_reference(root.as_view(), Value::Null),
        Err(RuntimeError::ProgramRootReplacement)
    );
    assert_eq!(state.read_reference(&root).unwrap(), before_root);
    assert_eq!(state.current_time(), Duration::ZERO);
    assert_eq!(state.task_states()[0].last_run, Duration::ZERO);
    assert_eq!(state.outputs(), &[0]);
    assert!(state.fault().is_none());
}

#[test]
fn rejected_compound_reference_write_preserves_current_value_and_process_images() {
    let prepared = PreparedModule::from_bytes(PROGRAM, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let mut reference = state.storage().ref_for_global("Plant").unwrap();
    reference.path.extend([
        RefSegment::Field("counter".into()),
        RefSegment::Field("history".into()),
        RefSegment::Index(vec![4]),
    ]);
    let before = state.outputs().to_vec();
    assert!(state
        .write_reference(reference.as_view(), Value::Int(99))
        .is_err());
    reference.path.pop();
    reference.path.push(RefSegment::Index(vec![1]));
    assert_eq!(state.read_reference(&reference).unwrap(), Value::Int(1));
    assert_eq!(state.outputs(), before);
    assert_eq!(state.task_states()[0].last_run, Duration::ZERO);
    assert!(state.fault().is_none());
}

macro_rules! timing_checks {
    ($state:expr) => {{
        let state = &mut $state;
        state.execute_cycle(Duration::from_millis(35)).unwrap();
        assert_eq!(state.task_states()[0].last_run, Duration::from_millis(30));
        assert_eq!(state.task_states()[0].overrun_count, 2);
        state.set_current_time(Duration::from_millis(-5));
        assert_eq!(state.current_time(), Duration::from_millis(-5));
        assert_eq!(state.task_states()[0].last_run, Duration::from_millis(30));
        assert_eq!(state.task_states()[0].overrun_count, 2);
        assert_eq!(state.profile().resolution, Duration::from_millis(1));
        let before = state.outputs().to_vec();
        state.reset_task_timing(Duration::from_millis(100));
        assert_eq!(state.current_time(), Duration::from_millis(100));
        assert_eq!(state.task_states()[0].last_run, Duration::from_millis(100));
        assert_eq!(state.task_states()[0].overrun_count, 0);
        assert!(!state.task_states()[0].last_single);
        assert_eq!(state.outputs(), before);
        assert!(state.fault().is_none());
    }};
}

#[test]
fn explicit_clock_set_and_task_timing_reset_are_distinct_on_borrowed_state() {
    let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    timing_checks!(state);
}

#[test]
#[cfg(feature = "std")]
fn explicit_clock_set_and_task_timing_reset_are_distinct_on_owned_state() {
    let prepared = std::sync::Arc::new(
        PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap(),
    );
    let mut state = prepared.instantiate_owned(0).unwrap();
    timing_checks!(state);
}
