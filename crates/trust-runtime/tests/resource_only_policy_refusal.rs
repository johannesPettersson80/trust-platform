//! Resource-only reconfiguration admits only unchanged declarations and access policy (spec 34 HOST-09).
use std::sync::Arc;
use trust_runtime::{
    bytecode::{BytecodeVersion, SectionData, SectionId, StorageRole},
    harness::CompileSession,
};
use trust_runtime_core::{
    error::RuntimeError,
    value::{Duration, Value},
    vm::{PreparationLimits, PreparedModule},
};

const REFUSAL: &str = "resource update changes executable or construction identity";

const SOURCE: &str = r#"
PROGRAM Main
VAR value : INT := INT#3; END_VAR
value := value + INT#1;
END_PROGRAM
CONFIGURATION Portability
RESOURCE Controller ON PLC
TASK Periodic (INTERVAL := T#25ms, PRIORITY := 0);
PROGRAM Plant WITH Periodic : Main;
VAR_ACCESS exposed : Plant.value : INT READ_WRITE; END_VAR
END_RESOURCE
END_CONFIGURATION
"#;

/// Spec 34 HOST-09, resource-only row: a candidate whose declaration template changed is
/// refused, and the installed resource, PLC values, images, task timing and time stay as
/// they were. The unchanged bytes are admitted, so the refusal is caused by the change.
#[test]
fn resource_only_update_refuses_a_changed_declaration_and_keeps_the_installed_state() {
    let limits = PreparationLimits::default();
    let mut module = CompileSession::from_source(SOURCE)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let current = module.encode().unwrap();
    let Some(SectionData::StorageLayout(layout)) = module.section_mut(SectionId::StorageLayout)
    else {
        panic!("storage layout")
    };
    let declaration = layout
        .entries
        .iter_mut()
        .find(|entry| entry.role == StorageRole::Variable && entry.retain == 0)
        .unwrap();
    declaration.retain = 1;
    let changed = module.encode().unwrap();

    let prepared = Arc::new(PreparedModule::from_bytes(&current, limits).unwrap());
    let candidate = Arc::new(PreparedModule::from_bytes(&changed, limits).unwrap());
    let mut state = Arc::clone(&prepared).instantiate_owned(0).unwrap();
    for ms in [25, 50, 75] {
        state.execute_cycle(Duration::from_millis(ms)).unwrap();
    }
    let Some(Value::Instance(plant)) = state.storage().get_global("Plant").cloned() else {
        panic!("program instance")
    };
    let value = state.storage().get_instance_var(plant, "value").cloned();
    assert!(matches!(value, Some(Value::Int(n)) if n > 3));
    let globals = state.storage().globals().clone();
    let outputs = state.outputs().to_vec();
    let timing = state
        .task_states()
        .iter()
        .map(|task| (task.last_run, task.last_single, task.overrun_count))
        .collect::<Vec<_>>();

    match state.prepare_resource_reconfiguration(candidate, 0, false) {
        Err(RuntimeError::InvalidConfig(reason)) => assert_eq!(reason.as_str(), REFUSAL),
        Err(other) => panic!("unexpected refusal: {other:?}"),
        Ok(_) => panic!("a changed declaration was admitted"),
    }
    assert!(std::ptr::eq(state.prepared(), prepared.as_ref()));
    assert_eq!(state.storage().globals(), &globals);
    assert_eq!(
        state.storage().get_instance_var(plant, "value"),
        value.as_ref()
    );
    assert_eq!(state.outputs(), outputs);
    assert_eq!(
        state
            .task_states()
            .iter()
            .map(|task| (task.last_run, task.last_single, task.overrun_count))
            .collect::<Vec<_>>(),
        timing
    );
    assert_eq!(state.current_time(), Duration::from_millis(75));
    assert!(state.fault().is_none());

    let same = Arc::new(PreparedModule::from_bytes(&current, limits).unwrap());
    state
        .prepare_resource_reconfiguration(same, 0, false)
        .unwrap()
        .commit()
        .unwrap();
}

/// Spec 34 HOST-09, resource-only row: a candidate whose access policy changed (the
/// `VAR_ACCESS` alias loses its write permission) is refused, and the installed resource,
/// PLC values, images, task timing and time stay as they were.
#[test]
fn resource_only_update_refuses_a_changed_access_policy_and_keeps_the_installed_state() {
    let limits = PreparationLimits::default();
    let mut module = CompileSession::from_source(SOURCE)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let current = module.encode().unwrap();
    let Some(SectionData::AccessBindings(aliases)) = module.section_mut(SectionId::AccessBindings)
    else {
        panic!("access bindings")
    };
    let alias = &mut aliases.entries[0];
    assert_eq!(alias.flags & 1, 1, "the source alias is READ_WRITE");
    alias.flags = 0;
    let changed = module.encode().unwrap();

    let prepared = Arc::new(PreparedModule::from_bytes(&current, limits).unwrap());
    let candidate = Arc::new(PreparedModule::from_bytes(&changed, limits).unwrap());
    let mut state = Arc::clone(&prepared).instantiate_owned(0).unwrap();
    for ms in [25, 50, 75] {
        state.execute_cycle(Duration::from_millis(ms)).unwrap();
    }
    let Some(Value::Instance(plant)) = state.storage().get_global("Plant").cloned() else {
        panic!("program instance")
    };
    let value = state.storage().get_instance_var(plant, "value").cloned();
    assert!(matches!(value, Some(Value::Int(n)) if n > 3));
    let globals = state.storage().globals().clone();
    let outputs = state.outputs().to_vec();
    let timing = state
        .task_states()
        .iter()
        .map(|task| (task.last_run, task.last_single, task.overrun_count))
        .collect::<Vec<_>>();

    match state.prepare_resource_reconfiguration(candidate, 0, false) {
        Err(RuntimeError::InvalidConfig(reason)) => assert_eq!(reason.as_str(), REFUSAL),
        Err(other) => panic!("unexpected refusal: {other:?}"),
        Ok(_) => panic!("a changed access policy was admitted"),
    }
    assert!(std::ptr::eq(state.prepared(), prepared.as_ref()));
    assert_eq!(state.storage().globals(), &globals);
    assert_eq!(
        state.storage().get_instance_var(plant, "value"),
        value.as_ref()
    );
    assert_eq!(state.outputs(), outputs);
    assert_eq!(
        state
            .task_states()
            .iter()
            .map(|task| (task.last_run, task.last_single, task.overrun_count))
            .collect::<Vec<_>>(),
        timing
    );
    assert_eq!(state.current_time(), Duration::from_millis(75));
    assert!(state.fault().is_none());

    let same = Arc::new(PreparedModule::from_bytes(&current, limits).unwrap());
    state
        .prepare_resource_reconfiguration(same, 0, false)
        .unwrap()
        .commit()
        .unwrap();
}
