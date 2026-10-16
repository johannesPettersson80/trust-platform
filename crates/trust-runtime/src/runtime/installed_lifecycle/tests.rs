use super::*;
use crate::{
    harness::CompileSession,
    value::{StructValue, Value},
};
use std::sync::Arc;

fn runtime(source: &str) -> Runtime {
    CompileSession::from_source(source).build_runtime().unwrap()
}
const SOURCE: &str = "VAR_GLOBAL RETAIN count : DINT := DINT#3; END_VAR VAR_GLOBAL transient : DINT := DINT#4; END_VAR PROGRAM Main count := count + DINT#1; END_PROGRAM";

#[test]
fn contextual_legacy_binding_reconfigures_from_admitted_archive_and_retires_export_copy() {
    let source = "VAR_GLOBAL used : DINT; unused : DINT; END_VAR PROGRAM Main used := used + DINT#1; END_PROGRAM";
    let session = CompileSession::from_source(source);
    let original = session
        .build_bytecode_module_for_version(crate::bytecode::BytecodeVersion::LEGACY)
        .unwrap();
    let mut installed = session
        .build_runtime_from_bytecode(&original, None)
        .unwrap();
    assert_eq!(
        installed.deployment_artifact().unwrap().version,
        crate::bytecode::BytecodeVersion::LEGACY
    );
    assert_eq!(
        installed.construction_artifact().unwrap().version,
        crate::bytecode::BytecodeVersion::SOURCE_FREE
    );

    for name in ["used", "unused"] {
        let reference = installed.storage().ref_for_global(name).unwrap();
        installed
            .bind_checked_io(
                reference,
                crate::io::IoAddress::parse(if name == "used" { "%QD0" } else { "%QD4" }).unwrap(),
                name.into(),
            )
            .unwrap();
    }
    assert!(
        installed.original_artifact.is_none(),
        "successful re-admission retires preserved legacy export bytes"
    );
    assert_eq!(
        installed.deployment_artifact().unwrap().version,
        crate::bytecode::BytecodeVersion::SOURCE_FREE
    );
    installed.execute_cycle().unwrap();
    assert_eq!(
        installed.storage().get_global("used"),
        Some(&Value::DInt(1))
    );
}

#[test]
fn failed_contextual_legacy_binding_keeps_archive_values_and_task_readiness() {
    let source = r#"
VAR_GLOBAL used : DINT; unused : DINT; END_VAR
PROGRAM Main
    used := used + DINT#1;
END_PROGRAM
CONFIGURATION Config
    RESOURCE CPU ON PLC
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM A WITH Fast : Main;
    END_RESOURCE
END_CONFIGURATION
"#;
    let session = CompileSession::from_source(source);
    let original = session
        .build_bytecode_module_for_version(crate::bytecode::BytecodeVersion::LEGACY)
        .unwrap();
    let mut installed = session
        .build_runtime_from_bytecode(&original, None)
        .unwrap();
    let export = Arc::clone(installed.original_artifact.as_ref().unwrap());
    let admitted = Arc::clone(installed.artifact.as_ref().unwrap());
    installed.write_global("used", Value::DInt(8)).unwrap();
    installed.set_current_time(Duration::from_millis(10));
    installed.execute_cycle().unwrap();
    assert_eq!(
        installed.storage().get_global("used"),
        Some(&Value::DInt(9))
    );
    let readiness = installed.state.task_states()[0].clone();
    let cycle = installed.cycle_counter;
    let now = installed.current_time();

    // The candidate can be staged, but this safe-output address does not fit
    // the process image grown for the requested mapping. Rejection must leave
    // both legacy export identity and installed execution state untouched.
    installed.set_io_safe_state(crate::io::IoSafeState {
        outputs: vec![(
            crate::io::IoAddress::parse("%QX20.0").unwrap(),
            Value::Bool(false),
        )],
    });
    let reference = installed.storage().ref_for_global("used").unwrap();
    let error = installed
        .bind_checked_io(
            reference,
            crate::io::IoAddress::parse("%QD0").unwrap(),
            "used".into(),
        )
        .expect_err("candidate policy must reject the out-of-range safe output");
    assert!(
        matches!(error, RuntimeError::InvalidIoAddress(_)),
        "{error:?}"
    );
    assert!(Arc::ptr_eq(
        installed.original_artifact.as_ref().unwrap(),
        &export
    ));
    assert!(Arc::ptr_eq(installed.artifact.as_ref().unwrap(), &admitted));
    assert_eq!(
        installed.deployment_artifact().unwrap().version,
        crate::bytecode::BytecodeVersion::LEGACY
    );
    assert_eq!(
        installed.storage().get_global("used"),
        Some(&Value::DInt(9))
    );
    assert_eq!(installed.cycle_counter, cycle);
    assert_eq!(installed.current_time(), now);
    let after = &installed.state.task_states()[0];
    assert_eq!(after.last_run, readiness.last_run);
    assert_eq!(after.last_single, readiness.last_single);
    assert_eq!(after.overrun_count, readiness.overrun_count);
    assert!(installed.io().outputs().is_empty());
}

#[test]
fn checked_code_replacement_preserves_retained_values_and_restarts_nonretained_state() {
    let mut installed = runtime(SOURCE);
    installed.write_global("count", Value::DInt(12)).unwrap();
    installed
        .write_global("transient", Value::DInt(99))
        .unwrap();
    installed.set_current_time(Duration::from_millis(35));
    let replacement =
        CompileSession::from_source(SOURCE.replace("count + DINT#1", "count + DINT#2"))
            .build_bytecode_module()
            .unwrap();
    installed.apply_bytecode_module(&replacement, None).unwrap();
    assert_eq!(installed.current_time(), Duration::from_millis(35));
    assert_eq!(
        installed.storage().get_global("count"),
        Some(&Value::DInt(12))
    );
    assert_eq!(
        installed.storage().get_global("transient"),
        Some(&Value::DInt(4))
    );
    installed.execute_cycle().unwrap();
    assert_eq!(
        installed.storage().get_global("count"),
        Some(&Value::DInt(14))
    );
}

#[test]
fn rejected_candidate_and_snapshot_leave_values_debug_mutations_and_counter_intact() {
    let mut installed = runtime(SOURCE);
    installed.write_global("count", Value::DInt(12)).unwrap();
    installed.cycle_counter = 7;
    let debug = installed.enable_debug();
    debug.force_global("count", Value::DInt(44));
    assert!(installed
        .apply_bytecode_bytes(b"not bytecode", None)
        .is_err());
    let mut bad = RetainSnapshot::default();
    bad.insert("count", Value::String("not an integer".into()));
    assert!(installed.apply_retain_snapshot(&bad).is_err());
    assert_eq!(
        installed.storage().get_global("count"),
        Some(&Value::DInt(12))
    );
    assert_eq!(installed.cycle_counter, 7);
    assert!(!installed.faulted());
    installed.execute_cycle().unwrap();
    assert_eq!(
        installed.storage().get_global("count"),
        Some(&Value::DInt(44))
    );
}

#[test]
fn retain_transaction_rolls_back_earlier_keys_and_uses_shared_member_default_recipe() {
    let mut installed = runtime("TYPE Holder : STRUCT x : DINT; added : DINT := DINT#5; END_STRUCT; END_TYPE VAR_GLOBAL RETAIN first : DINT := DINT#1; item : Holder; END_VAR PROGRAM Main END_PROGRAM");
    let mut bad = RetainSnapshot::default();
    bad.insert("first", Value::DInt(8));
    bad.insert("item", Value::Bool(false));
    assert!(installed.apply_retain_snapshot(&bad).is_err());
    assert_eq!(
        installed.storage().get_global("first"),
        Some(&Value::DInt(1))
    );
    let mut good = RetainSnapshot::default();
    good.insert(
        "item",
        Value::Struct(Arc::new(StructValue::from_untyped_parts(
            "HOLDER".into(),
            [("X".into(), Value::DInt(11))].into_iter().collect(),
        ))),
    );
    installed.apply_retain_snapshot(&good).unwrap();
    let Some(Value::Struct(value)) = installed.storage().get_global("item") else {
        panic!("holder")
    };
    assert_eq!(value.type_name(), "Holder");
    assert_eq!(value.fields().get("x"), Some(&Value::DInt(11)));
    assert_eq!(value.fields().get("added"), Some(&Value::DInt(5)));
}

#[test]
fn resource_resize_preserves_intersections_and_zeroes_new_capacity() {
    let mut installed = runtime(SOURCE);
    let mut resource = installed
        .artifact
        .as_ref()
        .unwrap()
        .metadata()
        .unwrap()
        .resources[0]
        .clone();
    resource.process_image.memory = 4;
    installed.apply_resource_metadata(&resource).unwrap();
    installed
        .write_io(
            &crate::io::IoAddress::parse("%MB0").unwrap(),
            Value::Byte(37),
        )
        .unwrap();
    resource.process_image.memory = 8;
    installed.apply_resource_metadata(&resource).unwrap();
    assert_eq!(installed.io().memory(), &[37, 0, 0, 0, 0, 0, 0, 0]);
    resource.process_image.memory = 2;
    installed.apply_resource_metadata(&resource).unwrap();
    assert_eq!(installed.io().memory(), &[37, 0]);
}

#[test]
fn replacement_cannot_invalidate_a_registered_safe_output() {
    let mut installed = runtime(SOURCE);
    let mut resource = installed
        .artifact
        .as_ref()
        .unwrap()
        .metadata()
        .unwrap()
        .resources[0]
        .clone();
    resource.process_image.outputs = 8;
    installed.apply_resource_metadata(&resource).unwrap();
    installed.set_io_safe_state(crate::io::IoSafeState {
        outputs: vec![(crate::io::IoAddress::parse("%QB7").unwrap(), Value::Byte(0))],
    });
    installed
        .write_io(
            &crate::io::IoAddress::parse("%QB0").unwrap(),
            Value::Byte(13),
        )
        .unwrap();
    resource.process_image.outputs = 2;
    assert!(installed.apply_resource_metadata(&resource).is_err());
    assert_eq!(installed.io().outputs().len(), 8);
    assert_eq!(installed.io().outputs()[0], 13);
    assert!(!installed.faulted());
}

#[test]
fn durable_restart_failure_is_atomic_and_cold_restart_skips_store() {
    struct BadStore;
    impl crate::retain::RetainStore for BadStore {
        fn load(&self) -> Result<RetainSnapshot, RuntimeError> {
            let mut snapshot = RetainSnapshot::default();
            snapshot.insert("count", Value::String("wrong type".into()));
            Ok(snapshot)
        }
        fn store(&self, _: &RetainSnapshot) -> Result<(), RuntimeError> {
            Ok(())
        }
    }
    let mut installed = runtime(SOURCE);
    installed.write_global("count", Value::DInt(18)).unwrap();
    installed
        .write_global("transient", Value::DInt(19))
        .unwrap();
    installed.set_retain_store(Some(Box::new(BadStore)), None);
    assert!(installed
        .restart_with_retain_store(RestartMode::Warm)
        .is_err());
    assert_eq!(
        installed.storage().get_global("count"),
        Some(&Value::DInt(18))
    );
    assert_eq!(
        installed.storage().get_global("transient"),
        Some(&Value::DInt(19))
    );
    installed
        .restart_with_retain_store(RestartMode::Cold)
        .unwrap();
    assert_eq!(
        installed.storage().get_global("count"),
        Some(&Value::DInt(3))
    );
}

#[test]
fn schema_change_rejects_retained_program_object_without_dropping_graph_or_debug_queue() {
    let source = "FUNCTION_BLOCK CounterFb VAR count : INT := INT#3; END_VAR END_FUNCTION_BLOCK PROGRAM Main VAR RETAIN block : CounterFb; END_VAR END_PROGRAM";
    let mut installed = runtime(source);
    installed
        .write_named_path(
            "Main",
            &[
                crate::value::RefSegment::Field("block".into()),
                crate::value::RefSegment::Field("count".into()),
            ],
            Value::Int(7),
        )
        .unwrap();
    let Some(Value::Instance(main)) = installed.storage().get_global("Main") else {
        panic!("Main")
    };
    let Some(Value::Instance(block)) = installed.storage().get_instance_var(*main, "block") else {
        panic!("block")
    };
    let block = *block;
    let debug = installed.enable_debug();
    debug.force_instance(block, "count", Value::Int(99));
    let changed = CompileSession::from_source(
        source.replace("count : INT := INT#3", "count : DINT := DINT#3"),
    )
    .build_bytecode_module()
    .unwrap();
    let error = installed.apply_bytecode_module(&changed, None).unwrap_err();
    assert!(matches!(error, RuntimeError::RetainMigration(_)));
    assert_eq!(
        installed.storage().get_instance_var(block, "count"),
        Some(&Value::Int(7))
    );
    assert!(!installed.faulted());
    installed.execute_cycle().unwrap();
    assert_eq!(
        installed.storage().get_instance_var(block, "count"),
        Some(&Value::Int(99))
    );
}

#[test]
fn resource_only_update_preserves_live_fb_input_identity_and_pending_writes() {
    let mut installed = runtime("FUNCTION_BLOCK FB VAR_INPUT IN : BOOL; END_VAR VAR_OUTPUT OUT : BOOL; END_VAR OUT := IN; END_FUNCTION_BLOCK PROGRAM P VAR fb : FB; END_VAR END_PROGRAM CONFIGURATION C VAR_GLOBAL trigger : BOOL := FALSE; END_VAR PROGRAM P : P; END_CONFIGURATION");
    let Some(Value::Instance(program)) = installed.storage().get_global("P").cloned() else {
        panic!("program")
    };
    let fb_ref = installed.storage().ref_for_instance(program, "fb").unwrap();
    let Some(Value::Instance(fb)) = installed.storage().read_by_ref(fb_ref.clone()).cloned() else {
        panic!("FB")
    };
    let input = installed.storage().ref_for_instance(fb, "IN").unwrap();
    installed
        .write_reference(&input, Value::Bool(true))
        .unwrap();
    let identity = installed.storage().get_instance(fb).unwrap() as *const _;
    installed.cycle_counter = 7;
    installed.set_current_time(Duration::from_millis(35));
    let observations = installed.vm_register_profile_snapshot();
    let debug = installed.enable_debug();
    debug.force_global("trigger", Value::Bool(true));
    let resource = crate::bytecode::ResourceMetadata {
        name: "R".into(),
        process_image: Default::default(),
        tasks: vec![crate::task::TaskConfig {
            name: "T".into(),
            interval: Duration::ZERO,
            single: Some("trigger".into()),
            priority: 0,
            programs: Vec::new(),
            fb_instances: vec![fb_ref],
        }],
    };
    installed.apply_resource_metadata(&resource).unwrap();
    assert_eq!(
        installed.storage().get_instance(fb).unwrap() as *const _,
        identity
    );
    assert_eq!(
        installed.storage().get_instance_var(fb, "IN"),
        Some(&Value::Bool(true))
    );
    assert_eq!(installed.cycle_counter, 7);
    assert_eq!(installed.vm_register_profile_snapshot(), observations);
    assert_eq!(installed.current_time(), Duration::from_millis(35));
    installed.execute_cycle().unwrap();
    assert_eq!(
        installed.storage().get_instance_var(fb, "OUT"),
        Some(&Value::Bool(true))
    );
}

#[test]
fn resource_only_fault_and_invalid_task_or_binding_preserve_the_installed_candidate() {
    let mut installed = runtime(SOURCE);
    installed
        .write_global("transient", Value::DInt(99))
        .unwrap();
    installed.state.latch_fault(RuntimeError::DivisionByZero);
    installed.cycle_counter = 9;
    let mut resource = installed
        .artifact
        .as_ref()
        .unwrap()
        .metadata()
        .unwrap()
        .resources[0]
        .clone();
    resource.process_image.outputs = 8;
    installed.apply_resource_metadata(&resource).unwrap();
    assert_eq!(
        installed.storage().get_global("transient"),
        Some(&Value::DInt(99))
    );
    assert!(matches!(
        installed.last_fault(),
        Some(RuntimeError::DivisionByZero)
    ));
    assert_eq!(installed.cycle_counter, 9);
    let artifact = Arc::clone(installed.artifact.as_ref().unwrap());
    resource.tasks.push(crate::task::TaskConfig {
        name: "bad".into(),
        interval: Duration::from_millis(10),
        single: None,
        priority: 0,
        programs: vec!["missing".into()],
        fb_instances: vec![],
    });
    assert!(installed.apply_resource_metadata(&resource).is_err());
    assert!(Arc::ptr_eq(installed.artifact.as_ref().unwrap(), &artifact));
    let target = installed.storage().ref_for_global("transient").unwrap();
    assert!(installed
        .bind_checked_io(
            target,
            crate::io::IoAddress::parse("%IX0.0").unwrap(),
            "transient".into()
        )
        .is_err());
    assert!(Arc::ptr_eq(installed.artifact.as_ref().unwrap(), &artifact));
    assert_eq!(
        installed.storage().get_global("transient"),
        Some(&Value::DInt(99))
    );
    assert!(matches!(
        installed.last_fault(),
        Some(RuntimeError::DivisionByZero)
    ));
}

#[test]
fn resource_registration_samples_single_and_binding_updates_keep_nominal_deadline() {
    let mut installed = runtime("VAR_GLOBAL trigger : BOOL; count : DINT; END_VAR PROGRAM Main count := count + DINT#1; END_PROGRAM");
    let background = installed.metadata_snapshot().background_thread_id();
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
    let thread = installed.metadata.task_thread_ids["Event"];
    assert!(timing.last_single);
    installed.execute_cycle().unwrap();
    assert_eq!(
        installed.storage().get_global("count"),
        Some(&Value::DInt(0))
    );
    let reference = installed.storage().ref_for_global("count").unwrap();
    installed
        .bind_checked_io(
            reference,
            crate::io::IoAddress::parse("%QD0").unwrap(),
            "count".into(),
        )
        .unwrap();
    assert_eq!(installed.state.task_states()[0].last_run, timing.last_run);
    assert_eq!(
        installed.state.task_states()[0].last_single,
        timing.last_single
    );
    assert_eq!(installed.metadata.task_thread_ids["Event"], thread);
    installed
        .write_global("trigger", Value::Bool(false))
        .unwrap();
    installed.execute_cycle().unwrap();
    installed
        .write_global("trigger", Value::Bool(true))
        .unwrap();
    installed.execute_cycle().unwrap();
    assert_eq!(
        installed.storage().get_global("count"),
        Some(&Value::DInt(1))
    );
    assert_eq!(
        installed
            .metadata_snapshot()
            .task_thread_id(&"Event".into()),
        Some(thread)
    );
    assert_eq!(
        installed.metadata_snapshot().background_thread_id(),
        background
    );
    let resource = installed
        .artifact
        .as_ref()
        .unwrap()
        .metadata()
        .unwrap()
        .resources[0]
        .clone();
    let mut removed = resource.clone();
    removed.tasks.clear();
    installed.apply_resource_metadata(&removed).unwrap();
    assert_eq!(
        installed
            .metadata_snapshot()
            .task_thread_id(&"Event".into()),
        None
    );
    assert_eq!(
        installed.metadata_snapshot().background_thread_id(),
        background
    );
    installed.apply_resource_metadata(&resource).unwrap();
    assert_eq!(
        installed
            .metadata_snapshot()
            .task_thread_id(&"Event".into()),
        Some(thread)
    );
    assert_eq!(
        installed.metadata_snapshot().background_thread_id(),
        background
    );
}

#[test]
fn resource_transaction_refuses_code_change_without_touching_live_state() {
    let mut installed = runtime(SOURCE);
    installed
        .write_global("transient", Value::DInt(99))
        .unwrap();
    let original = Arc::clone(installed.artifact.as_ref().unwrap());
    let changed = CompileSession::from_source(SOURCE.replace("count + DINT#1", "count + DINT#2"))
        .build_bytecode_module()
        .unwrap();
    let metadata = Arc::clone(&installed.metadata);
    assert!(installed
        .reconfigure_resource_module(changed, metadata, true)
        .is_err());
    assert!(Arc::ptr_eq(installed.artifact.as_ref().unwrap(), &original));
    assert_eq!(
        installed.storage().get_global("transient"),
        Some(&Value::DInt(99))
    );
}
