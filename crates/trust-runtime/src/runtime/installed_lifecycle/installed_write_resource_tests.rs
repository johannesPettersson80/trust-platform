use super::*;
use crate::{harness::CompileSession, value::Value};

const TRIGGERED_SOURCE: &str = "VAR_GLOBAL trigger : BOOL; count : DINT; END_VAR PROGRAM Main count := count + DINT#1; END_PROGRAM";
const TRANSIENT_SOURCE: &str =
    "VAR_GLOBAL transient : DINT := DINT#4; END_VAR PROGRAM Main transient := transient; END_PROGRAM";

fn runtime(source: &str) -> Runtime {
    CompileSession::from_source(source).build_runtime().unwrap()
}

/// Spec 34 §4.3.1 resource-only row: replacing task metadata resets task readiness to the current logical time and samples the SINGLE input as its baseline.
#[test]
fn installed_write_task_metadata_replacement_resets_readiness_to_the_current_time() {
    let mut installed = runtime(TRIGGERED_SOURCE);
    installed
        .write_global("trigger", Value::Bool(true))
        .unwrap();
    installed.set_current_time(Duration::from_millis(35));
    let mut resource = installed
        .artifact
        .as_ref()
        .unwrap()
        .metadata()
        .unwrap()
        .resources[0]
        .clone();
    resource.tasks = vec![crate::task::TaskConfig {
        name: "Event".into(),
        interval: Duration::ZERO,
        single: Some("trigger".into()),
        priority: 0,
        programs: vec!["Main".into()],
        fb_instances: vec![],
    }];
    installed.apply_resource_metadata(&resource).unwrap();
    let timing = installed.state.task_states()[0].clone();
    assert_eq!(timing.last_run, Duration::from_millis(35));
    assert!(timing.last_single);
}

/// Spec 34 §4.3.1 resource-only row: a resource-only update keeps the live value and does not rerun the declared initializer.
#[test]
fn installed_write_resource_only_update_keeps_live_values_without_rerunning_initializers() {
    let mut installed = runtime(TRANSIENT_SOURCE);
    installed
        .write_global("transient", Value::DInt(99))
        .unwrap();
    let resource = installed
        .artifact
        .as_ref()
        .unwrap()
        .metadata()
        .unwrap()
        .resources[0]
        .clone();
    installed.apply_resource_metadata(&resource).unwrap();
    assert_eq!(
        installed.storage().get_global("transient"),
        Some(&Value::DInt(99))
    );
}

/// Spec 34 §4.3.1 code and layout row: a refused candidate neither recompiles the live module nor repairs a live write.
#[test]
fn installed_write_refused_candidate_leaves_the_live_module_and_compile_counts_unchanged() {
    let mut installed = runtime(TRANSIENT_SOURCE);
    installed
        .write_global("transient", Value::DInt(99))
        .unwrap();
    let prepared_before = std::ptr::from_ref(installed.prepared());
    let before = installed.vm_tier1_specialized_executor_snapshot();
    assert!(installed
        .apply_bytecode_bytes(b"not bytecode", None)
        .is_err());
    installed.execute_cycle().unwrap();
    let after = installed.vm_tier1_specialized_executor_snapshot();
    assert_eq!(after.compile_attempts, before.compile_attempts);
    assert_eq!(after.compile_successes, before.compile_successes);
    assert!(std::ptr::eq(prepared_before, installed.prepared()));
    assert_eq!(
        installed.storage().get_global("transient"),
        Some(&Value::DInt(99))
    );
}
