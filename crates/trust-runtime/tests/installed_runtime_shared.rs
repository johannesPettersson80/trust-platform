//! Public hosted entries execute the same saved artifact as the portable engine.
use std::{
    sync::{Arc, Mutex},
    time::{Duration as PhysicalDuration, Instant},
};
use trust_runtime::{
    debug::RuntimeEvent,
    error::RuntimeError,
    io::{IoAddress, IoDriver, IoSafeState},
    value::{Duration, Value},
    Runtime,
};

const GPIO: &[u8] = include_bytes!("fixtures/portability/f401/gpio.stbc");
const PERIODIC: &[u8] = include_bytes!("fixtures/portability/stbc-2.0/program-v2.stbc");

#[test]
fn public_host_loads_saved_v2_without_compile_session_and_preserves_nominal_deadlines() {
    let mut runtime = Runtime::from_bytes(PERIODIC, Default::default()).unwrap();
    let before = runtime.vm_tier1_specialized_executor_snapshot();
    for now in (0..=1000).step_by(10) {
        runtime.set_current_time(Duration::from_millis(now));
        runtime.execute_cycle().unwrap();
    }
    let Some(Value::Instance(plant)) = runtime.storage().get_global("Plant") else {
        panic!("admitted program root")
    };
    assert_eq!(
        runtime.storage().get_instance_var(*plant, "activations"),
        Some(&Value::DInt(40))
    );
    assert_eq!(runtime.task_overrun_count("Periodic"), Some(0));
    let after = runtime.vm_tier1_specialized_executor_snapshot();
    assert_eq!(
        after.compile_attempts, before.compile_attempts,
        "ordinary cycles never author/lower code"
    );
    assert_eq!(after.compile_successes, before.compile_successes);
    assert!(!runtime.faulted());
    assert_eq!(runtime.prepared().resources().len(), 1);
}

#[test]
fn rejected_queued_write_is_observable_without_faulting_the_plc() {
    let mut runtime = Runtime::from_bytes(GPIO, Default::default()).unwrap();
    let debug = runtime.enable_debug();
    debug.enqueue_global_write("probe_depth", Value::Bool(true));
    runtime.set_current_time(Duration::from_millis(10));
    runtime.execute_cycle().unwrap();
    assert_eq!(
        runtime.storage().get_global("probe_depth"),
        Some(&Value::Int(1))
    );
    assert!(!runtime.faulted());
    assert!(debug
        .drain_runtime_events()
        .iter()
        .any(|event| matches!(event,
        RuntimeEvent::WriteRejected { target, .. } if target == "probe_depth")));
}

#[test]
fn force_after_mapped_output_does_not_resample_the_output_image() {
    let mut runtime = Runtime::from_bytes(GPIO, Default::default()).unwrap();
    let debug = runtime.enable_debug();
    let Some(Value::Instance(plant)) = runtime.storage().get_global("Plant") else {
        panic!("admitted program root")
    };
    let plant = *plant;
    debug.force_instance(plant, "led", Value::Bool(true));
    runtime
        .write_io(&IoAddress::parse("%IX0.0").unwrap(), Value::Bool(false))
        .unwrap();
    runtime.set_current_time(Duration::from_millis(30));
    runtime.execute_cycle().unwrap();
    assert_eq!(
        runtime.storage().get_instance_var(plant, "led"),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        runtime.io().outputs(),
        &[0],
        "force occurs after normal mapped output sampling"
    );
}

struct OutputDriver {
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    fail: bool,
}
impl IoDriver for OutputDriver {
    fn read_inputs(&mut self, _: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn write_outputs(&mut self, bytes: &[u8]) -> Result<(), RuntimeError> {
        self.writes.lock().unwrap().push(bytes.to_vec());
        if self.fail {
            Err(RuntimeError::IoDriver("injected output failure".into()))
        } else {
            Ok(())
        }
    }
}

#[test]
fn expired_watchdog_still_delivers_safe_outputs_to_every_driver_and_preserves_root_fault() {
    let mut runtime = Runtime::from_bytes(GPIO, Default::default()).unwrap();
    let first = Arc::new(Mutex::new(Vec::new()));
    let second = Arc::new(Mutex::new(Vec::new()));
    runtime.add_io_driver(
        "first",
        Box::new(OutputDriver {
            writes: Arc::clone(&first),
            fail: true,
        }),
    );
    runtime.add_io_driver(
        "second",
        Box::new(OutputDriver {
            writes: Arc::clone(&second),
            fail: false,
        }),
    );
    runtime.set_io_safe_state(IoSafeState {
        outputs: vec![(IoAddress::parse("%QX0.0").unwrap(), Value::Bool(false))],
    });
    runtime
        .write_io(&IoAddress::parse("%QX0.0").unwrap(), Value::Bool(true))
        .unwrap();
    let deadline = Instant::now() - PhysicalDuration::from_millis(1);
    runtime.set_execution_deadline(Some(deadline));
    let error = runtime.watchdog_timeout();
    assert!(matches!(error, RuntimeError::SafeStateFailed { .. }));
    assert_eq!(runtime.last_fault(), Some(&RuntimeError::WatchdogTimeout));
    assert!(runtime.faulted());
    assert_eq!(runtime.execution_deadline(), Some(deadline));
    assert_eq!(runtime.io().outputs(), &[0]);
    assert_eq!(first.lock().unwrap().as_slice(), &[vec![0]]);
    assert_eq!(second.lock().unwrap().as_slice(), &[vec![0]]);
    assert_eq!(runtime.execute_cycle(), Err(RuntimeError::ResourceFaulted));
}

#[test]
fn export_preserves_initializers_instead_of_serializing_current_plc_values() {
    let source = "PROGRAM Main VAR counter : INT := 7; END_VAR counter := counter + 1; END_PROGRAM";
    let mut runtime = trust_runtime::harness::CompileSession::from_source(source)
        .build_runtime()
        .unwrap();
    runtime
        .write_named_path(
            "Main",
            &[trust_runtime::value::RefSegment::Field("counter".into())],
            Value::Int(13),
        )
        .unwrap();
    let exported = trust_runtime::bytecode::build_module_from_runtime(&runtime).unwrap();
    assert_eq!(
        exported.version,
        trust_runtime::bytecode::BytecodeVersion::SOURCE_FREE
    );
    let restored = Runtime::from_bytes(&exported.encode().unwrap(), Default::default()).unwrap();
    let Some(Value::Instance(original)) = runtime.storage().get_global("Main") else {
        panic!("original root")
    };
    let Some(Value::Instance(fresh)) = restored.storage().get_global("Main") else {
        panic!("fresh root")
    };
    assert_eq!(
        runtime.storage().get_instance_var(*original, "counter"),
        Some(&Value::Int(13))
    );
    assert_eq!(
        restored.storage().get_instance_var(*fresh, "counter"),
        Some(&Value::Int(7))
    );
    assert!(
        trust_runtime::bytecode::build_module_from_runtime_with_sources(&restored, &[source])
            .is_err(),
        "plain export cannot silently create or reassociate unavailable debug positions"
    );
    assert!(
        trust_runtime::bytecode::build_module_from_runtime_with_sources(
            &restored,
            &["unrelated source"]
        )
        .is_err()
    );
    assert!(
        trust_runtime::bytecode::build_module_from_runtime_with_sources(
            &runtime,
            &["unrelated source"]
        )
        .is_err()
    );
    assert_eq!(
        runtime.storage().get_instance_var(*original, "counter"),
        Some(&Value::Int(13))
    );
    let with_debug =
        trust_runtime::bytecode::build_module_from_runtime_with_sources(&runtime, &[source])
            .unwrap();
    let saved = Runtime::from_bytes(&with_debug.encode().unwrap(), Default::default()).unwrap();
    assert!(
        trust_runtime::bytecode::build_module_from_runtime_with_sources(
            &saved,
            &["unrelated source"]
        )
        .is_err()
    );
    assert!(
        trust_runtime::bytecode::build_module_from_runtime_with_sources(&saved, &[source]).is_err(),
        "a file_0 label does not prove original source identity even when supplied text is correct"
    );
}

#[test]
fn source_free_io_projection_keeps_the_declared_enum_alias_identity() {
    use trust_runtime::bytecode::{SectionData, SectionId, TypeData};
    let source = "TYPE Mode : (Idle, Active); ModeAlias : Mode; END_TYPE PROGRAM Main VAR mode AT %IW0 : ModeAlias; END_VAR END_PROGRAM";
    let mut module = trust_runtime::harness::CompileSession::from_source(source)
        .build_bytecode_module()
        .unwrap();
    let strings = match module.section(SectionId::StringTable).unwrap() {
        SectionData::StringTable(strings) => strings.entries.clone(),
        other => panic!("{other:?}"),
    };
    let alias = match module.section(SectionId::TypeTable).unwrap() {
        SectionData::TypeTable(types) => types
            .entries
            .iter()
            .position(|entry| {
                matches!(entry.data, TypeData::Alias { .. })
                    && entry.name_idx.is_some_and(|name| {
                        strings[name as usize].eq_ignore_ascii_case("ModeAlias")
                    })
            })
            .unwrap() as u32,
        other => panic!("{other:?}"),
    };
    // Carry the declaration identity explicitly, independent of wire-codec
    // normalization. It is a TYPE_TABLE index, not a HIR TypeId.
    let Some(SectionData::IoMap(io)) = module.section_mut(SectionId::IoMap) else {
        panic!("I/O map")
    };
    io.bindings[0].type_id = Some(alias);
    let runtime = Runtime::from_bytes(&module.encode().unwrap(), Default::default()).unwrap();
    let expected = runtime.registry().lookup("ModeAlias").unwrap();
    let snapshot = runtime.io().snapshot();
    assert_eq!(snapshot.inputs[0].value_type, Some(expected));
    assert_eq!(snapshot.inputs[0].name.as_deref(), Some("Main.mode"));
    assert_eq!(
        runtime.metadata_snapshot().artifact_type(alias),
        Some(expected)
    );
}

#[test]
fn ordinary_producer_keeps_wire_types_and_source_free_nested_io_labels() {
    let source = "TYPE Mode : (Idle, Active); ModeAlias : Mode; Packet : STRUCT words : ARRAY[1..2] OF WORD; END_STRUCT; END_TYPE PROGRAM Main VAR mode AT %IW0 : ModeAlias; packet AT %IW2 : Packet; END_VAR END_PROGRAM";
    let installed = trust_runtime::harness::CompileSession::from_source(source)
        .build_runtime()
        .unwrap();
    let module = trust_runtime::bytecode::build_module_from_runtime(&installed).unwrap();
    let saved = Runtime::from_bytes(&module.encode().unwrap(), Default::default()).unwrap();
    let original_view = installed.io();
    let saved_view = saved.io();
    assert_eq!(original_view.bindings().len(), 3);
    assert_eq!(saved_view.bindings().len(), 3);
    for (original, restored) in original_view.bindings().iter().zip(saved_view.bindings()) {
        assert_eq!(original.address, restored.address);
        let original_type = original.value_type.unwrap();
        let restored_type = restored.value_type.unwrap();
        assert_eq!(
            installed.registry().get(original_type),
            saved.registry().get(restored_type),
            "ordinary producer preserves its existing primitive wire-codec type contract"
        );
    }
    let names = saved_view
        .bindings()
        .iter()
        .map(|binding| binding.display_name.as_deref().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        ["Main.mode", "Main.packet.words[1]", "Main.packet.words[2]"]
    );
}

#[test]
fn fresh_task_registration_samples_initial_single_without_changing_restart_baseline() {
    use std::sync::Arc;
    use trust_runtime::runtime_core::vm::{PreparationLimits, PreparedModule};
    let source = "VAR_GLOBAL trigger : BOOL := TRUE; count : DINT; END_VAR PROGRAM Main count := count + DINT#1; END_PROGRAM CONFIGURATION C TASK Event (SINGLE := trigger, INTERVAL := T#0ms, PRIORITY := 0); PROGRAM Main WITH Event : Main; END_CONFIGURATION";
    let module = trust_runtime::harness::CompileSession::from_source(source)
        .build_bytecode_module()
        .unwrap();
    let bytes = module.encode().unwrap();
    let prepared =
        Arc::new(PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap());
    let mut borrowed = prepared.instantiate(0).unwrap();
    let mut owned = Arc::clone(&prepared).instantiate_owned(0).unwrap();
    let mut hosted = Runtime::from_bytes(&bytes, Default::default()).unwrap();
    assert!(borrowed.task_states()[0].last_single);
    assert!(owned.task_states()[0].last_single);
    for (ms, trigger, expected) in [(0, true, 0), (10, false, 0), (20, true, 1)] {
        borrowed
            .write_global("trigger", Value::Bool(trigger))
            .unwrap();
        owned.write_global("trigger", Value::Bool(trigger)).unwrap();
        hosted
            .write_global("trigger", Value::Bool(trigger))
            .unwrap();
        let time = Duration::from_millis(ms);
        borrowed.execute_cycle(time).unwrap();
        owned.execute_cycle(time).unwrap();
        hosted.set_current_time(time);
        hosted.execute_cycle().unwrap();
        assert_eq!(
            borrowed.storage().get_global("count"),
            Some(&Value::DInt(expected))
        );
        assert_eq!(
            owned.storage().get_global("count"),
            Some(&Value::DInt(expected))
        );
        assert_eq!(
            hosted.storage().get_global("count"),
            Some(&Value::DInt(expected))
        );
    }
    borrowed.restart(trust_runtime::RestartMode::Warm).unwrap();
    owned.restart(trust_runtime::RestartMode::Warm).unwrap();
    hosted.restart(trust_runtime::RestartMode::Warm).unwrap();
    assert!(!borrowed.task_states()[0].last_single);
    assert!(!owned.task_states()[0].last_single);
    borrowed.execute_cycle(Duration::from_millis(20)).unwrap();
    owned.execute_cycle(Duration::from_millis(20)).unwrap();
    hosted.execute_cycle().unwrap();
    assert_eq!(
        borrowed.storage().get_global("count"),
        Some(&Value::DInt(1))
    );
    assert_eq!(owned.storage().get_global("count"), Some(&Value::DInt(1)));
    assert_eq!(hosted.storage().get_global("count"), Some(&Value::DInt(1)));
}

#[test]
fn retained_debug_projection_uses_declared_globals_and_program_fields() {
    let source = "VAR_GLOBAL RETAIN saved : INT := 3; END_VAR PROGRAM Main VAR RETAIN remembered : INT := 7; END_VAR END_PROGRAM";
    let mut runtime = trust_runtime::harness::CompileSession::from_source(source)
        .build_runtime()
        .unwrap();
    let bindings = runtime
        .metadata_snapshot()
        .retained_references(runtime.storage());
    assert!(
        runtime.storage().retain().is_empty(),
        "retained state is not duplicated for debugging"
    );
    let saved = bindings
        .iter()
        .find(|(name, _)| name == "saved")
        .unwrap()
        .1
        .clone();
    let remembered = bindings
        .iter()
        .find(|(name, _)| name == "@program/Main/remembered")
        .unwrap()
        .1
        .clone();
    assert_eq!(runtime.read_reference(&saved).unwrap(), Value::Int(3));
    assert_eq!(runtime.read_reference(&remembered).unwrap(), Value::Int(7));
    runtime.write_reference(&remembered, Value::Int(9)).unwrap();
    assert_eq!(
        runtime
            .retain_snapshot()
            .unwrap()
            .values()
            .get("@program/Main/remembered"),
        Some(&Value::Int(9))
    );
}
