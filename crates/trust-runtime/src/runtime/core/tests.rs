use super::*;

use crate::debug::SourceLocation;
use crate::memory::MemoryLocation;
use crate::program_model::{ClassDef, FunctionBlockDef, FunctionDef, InterfaceDef, LValue};
use crate::task::ProgramDef;
use crate::value::{PartialAccess, RefPath, ValueRef};
use trust_hir::TypeId;

#[test]
fn runtime_core_new_establishes_default_state_and_builtin_function_blocks() {
    let runtime = Runtime::new();

    assert_eq!(runtime.resource_name(), "RESOURCE");
    assert_eq!(runtime.execution_backend(), ExecutionBackend::BytecodeVm);
    assert_eq!(runtime.current_time(), Duration::ZERO);
    assert_eq!(runtime.cycle_counter(), 0);
    assert!(runtime.storage().globals().is_empty());
    assert!(runtime.tasks().is_empty());
    assert!(runtime.programs().is_empty());
    assert!(runtime.debug_control().is_none());
    assert_eq!(runtime.profile().epoch.ticks(), 0);
    assert_eq!(runtime.profile().resolution, Duration::from_millis(1));
    assert!(runtime.function_blocks().contains_key("TON"));
    assert!(matches!(
        runtime
            .registry()
            .lookup("TON")
            .and_then(|type_id| runtime.registry().get(type_id)),
        Some(Type::FunctionBlock { name }) if name == "TON"
    ));
}

#[test]
fn runtime_core_declaration_registration_uses_case_insensitive_lookup_keys() {
    let mut runtime = Runtime::new();

    runtime.register_function(function("mixFunction"));
    runtime.register_function_block(function_block("mixBlock"));
    runtime.register_class(class("mixClass"));
    runtime.register_interface(interface("mixInterface"));

    assert_eq!(runtime.functions()["MIXFUNCTION"].name, "mixFunction");
    assert_eq!(runtime.function_blocks()["MIXBLOCK"].name, "mixBlock");
    assert_eq!(runtime.classes()["MIXCLASS"].name, "mixClass");
    assert_eq!(runtime.interfaces()["MIXINTERFACE"].name, "mixInterface");
}

#[test]
fn runtime_core_program_registration_materializes_and_publishes_instance() {
    let mut runtime = Runtime::new();
    let program = program("Main");

    runtime.register_program(program).unwrap();

    assert_eq!(runtime.programs()["Main"].name, "Main");
    let Value::Instance(instance_id) = runtime
        .storage()
        .get_global("Main")
        .cloned()
        .expect("published program instance")
    else {
        panic!("program global must contain its instance");
    };
    let instance = runtime
        .storage()
        .get_instance(instance_id)
        .expect("materialized program instance");
    assert_eq!(instance.type_name, "Main");
}

#[test]
fn runtime_core_task_registration_preserves_single_sample_and_thread_ids() {
    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("Start", Value::Bool(true));
    runtime.register_task(task("Fast", Some("Start"), &["Main"]));

    assert_eq!(runtime.task_thread_ids.get("Fast"), Some(&1));
    assert!(runtime.task_state["Fast"].last_single);
    assert_eq!(runtime.task_state["Fast"].last_run, Duration::ZERO);

    runtime.register_task(task("Slow", None, &["Main"]));
    runtime.register_task(task("Fast", Some("Start"), &["Main"]));
    assert_eq!(runtime.task_thread_ids.get("Fast"), Some(&1));
    assert_eq!(runtime.task_thread_ids.get("Slow"), Some(&2));
    assert_eq!(runtime.next_thread_id, 3);
}

#[test]
fn runtime_core_background_thread_is_allocated_only_for_unscheduled_programs() {
    let mut runtime = Runtime::new();
    runtime.register_program(program("Main")).unwrap();
    runtime.register_program(program("Aux")).unwrap();
    runtime.register_task(task("Fast", None, &["Main", "Aux"]));

    assert!(!runtime.has_background_programs());
    assert_eq!(runtime.ensure_background_thread_id(), None);

    runtime.tasks[0].programs.pop();
    assert!(runtime.has_background_programs());
    let first = runtime
        .ensure_background_thread_id()
        .expect("unscheduled program gets a background thread");
    assert_eq!(first, 2);
    assert_eq!(runtime.ensure_background_thread_id(), Some(first));
}

#[test]
fn runtime_core_task_timing_reset_preserves_threads_and_clears_history() {
    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("Start", Value::Bool(true));
    runtime.register_task(task("Fast", Some("Start"), &[]));
    runtime.task_state.get_mut("Fast").unwrap().overrun_count = 9;
    let thread_id = runtime.task_thread_ids["Fast"];

    runtime.reset_task_timing(Duration::from_millis(42));

    let state = &runtime.task_state["Fast"];
    assert_eq!(runtime.current_time(), Duration::from_millis(42));
    assert_eq!(state.last_run, Duration::from_millis(42));
    assert!(!state.last_single);
    assert_eq!(state.overrun_count, 0);
    assert_eq!(runtime.task_thread_ids["Fast"], thread_id);
}

#[test]
fn runtime_core_logical_time_advances_with_signed_saturation() {
    let mut runtime = Runtime::new();

    runtime.set_current_time(Duration::from_nanos(i64::MAX - 2));
    runtime.advance_time(Duration::from_nanos(10));
    assert_eq!(runtime.current_time(), Duration::from_nanos(i64::MAX));

    runtime.set_current_time(Duration::from_nanos(i64::MIN + 2));
    runtime.advance_time(Duration::from_nanos(-10));
    assert_eq!(runtime.current_time(), Duration::from_nanos(i64::MIN));
}

#[test]
fn runtime_core_deadlines_round_trip_without_debug_pause() {
    let mut runtime = Runtime::new();
    let execution = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let output = execution + std::time::Duration::from_secs(1);

    runtime.set_execution_deadline(Some(execution));
    runtime.set_output_commit_deadline(Some(output));
    assert_eq!(runtime.execution_deadline(), Some(execution));
    assert_eq!(runtime.effective_execution_deadline(), Some(execution));
    assert_eq!(runtime.output_commit_deadline(), Some(output));
    assert_eq!(runtime.effective_output_commit_deadline(), Some(output));

    runtime.set_execution_deadline(None);
    runtime.set_output_commit_deadline(None);
    assert_eq!(runtime.effective_execution_deadline(), None);
    assert_eq!(runtime.effective_output_commit_deadline(), None);
}

#[test]
fn runtime_core_metadata_snapshot_isolated_from_later_runtime_mutation() {
    let mut runtime = Runtime::new();
    runtime.register_program(program("Main")).unwrap();
    runtime.register_task(task("Fast", None, &["Main"]));
    runtime.register_statement_locations(7, vec![SourceLocation::new(7, 1, 4)]);
    let snapshot = runtime.metadata_snapshot();

    runtime.register_program(program("Later")).unwrap();
    runtime.register_task(task("Slow", None, &["Later"]));
    runtime.register_statement_locations(7, vec![SourceLocation::new(7, 8, 9)]);

    assert!(snapshot.programs().contains_key("Main"));
    assert!(!snapshot.programs().contains_key("Later"));
    assert_eq!(snapshot.tasks().len(), 1);
    assert_eq!(snapshot.task_thread_id(&"Fast".into()), Some(1));
    assert_eq!(snapshot.task_thread_id(&"Slow".into()), None);
    assert_eq!(snapshot.statement_locations(7).unwrap()[0].start, 1);
}

#[test]
fn runtime_core_virtual_file_labels_are_closed_and_unsigned() {
    assert_eq!(parse_virtual_file_label("file_0"), Some(0));
    assert_eq!(parse_virtual_file_label("file_4294967295"), Some(u32::MAX));
    for invalid in [
        "file_",
        "file_-1",
        "FILE_7",
        "path/file_7",
        "file_4294967296",
    ] {
        assert_eq!(parse_virtual_file_label(invalid), None, "{invalid}");
    }
}

#[test]
fn runtime_core_vm_debug_location_resolves_explicit_and_virtual_labels() {
    let mut runtime = Runtime::new();
    let source = "x := 1;\ny := 2;\n";
    let second = source.find("y := 2;").unwrap() as u32;
    runtime.register_source_text(7, source);
    runtime.register_statement_locations(7, vec![SourceLocation::new(7, second, second + 7)]);
    runtime.register_source_label(7, "src/main.st");

    for label in ["src/main.st", "file_7"] {
        let location = runtime
            .resolve_vm_debug_location(label, 2, 1)
            .expect("registered statement location");
        assert_eq!(location.file_id, 7);
        assert_eq!(location.start, second);
    }
    assert!(runtime
        .resolve_vm_debug_location("src/missing.st", 2, 1)
        .is_none());
}

#[test]
fn runtime_core_debug_evaluation_targets_requested_frame_without_reordering() {
    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("x", Value::DInt(1));
    let first = runtime.storage_mut().push_frame("First");
    runtime.storage_mut().set_local("x", Value::DInt(11));
    let second = runtime.storage_mut().push_frame("Second");
    runtime.storage_mut().set_local("x", Value::DInt(22));

    assert_eq!(
        runtime
            .evaluate_expression(&Expr::Name("x".into()), Some(first))
            .unwrap(),
        Value::DInt(11)
    );
    runtime
        .write_lvalue(&LValue::Name("x".into()), Value::DInt(17), Some(first))
        .unwrap();
    assert_eq!(
        runtime
            .read_lvalue(&LValue::Name("x".into()), Some(first))
            .unwrap(),
        Value::DInt(17)
    );
    assert_eq!(
        runtime
            .storage()
            .frames()
            .iter()
            .map(|frame| frame.id)
            .collect::<Vec<_>>(),
        vec![first, second]
    );
    assert_eq!(runtime.storage().get_global("x"), Some(&Value::DInt(1)));
}

#[test]
fn runtime_core_debug_evaluation_rejects_missing_frame_without_write() {
    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("x", Value::DInt(3));
    let missing = FrameId(u32::MAX);

    assert_eq!(
        runtime.evaluate_expression(&Expr::Name("x".into()), Some(missing)),
        Err(error::RuntimeError::InvalidFrame(u32::MAX))
    );
    assert_eq!(
        runtime.write_lvalue(&LValue::Name("x".into()), Value::DInt(9), Some(missing)),
        Err(error::RuntimeError::InvalidFrame(u32::MAX))
    );
    assert_eq!(runtime.storage().get_global("x"), Some(&Value::DInt(3)));
}

#[test]
fn runtime_core_var_access_reads_and_writes_direct_and_partial_bindings() {
    let mut runtime = Runtime::new();
    runtime
        .storage_mut()
        .set_global("Status", Value::Byte(0b0000_0100));
    let reference = runtime
        .storage()
        .ref_for_global("Status")
        .expect("status reference");
    runtime
        .access_map_mut()
        .bind("Whole".into(), reference.clone(), None);
    runtime
        .access_map_mut()
        .bind("Ready".into(), reference, Some(PartialAccess::Bit(2)));

    assert_eq!(runtime.read_access("Whole"), Some(Value::Byte(4)));
    assert_eq!(runtime.read_access("Ready"), Some(Value::Bool(true)));
    runtime.write_access("Ready", Value::Bool(false)).unwrap();
    assert_eq!(
        runtime.storage().get_global("Status"),
        Some(&Value::Byte(0))
    );
    runtime.write_access("Whole", Value::Byte(9)).unwrap();
    assert_eq!(runtime.read_access("Whole"), Some(Value::Byte(9)));
}

#[test]
fn runtime_core_var_access_failures_preserve_storage() {
    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("Status", Value::Byte(4));
    let reference = runtime
        .storage()
        .ref_for_global("Status")
        .expect("status reference");
    runtime
        .access_map_mut()
        .bind("InvalidBit".into(), reference, Some(PartialAccess::Bit(8)));
    runtime.access_map_mut().bind(
        "MissingTarget".into(),
        ValueRef {
            location: MemoryLocation::Global,
            offset: usize::MAX,
            path: RefPath::new(),
        },
        None,
    );

    assert_eq!(runtime.read_access("Unknown"), None);
    assert_eq!(
        runtime.write_access("Unknown", Value::Byte(1)),
        Err(error::RuntimeError::UndefinedVariable("Unknown".into()))
    );
    assert_eq!(runtime.read_access("InvalidBit"), None);
    assert!(matches!(
        runtime.write_access("InvalidBit", Value::Bool(false)),
        Err(error::RuntimeError::IndexOutOfBounds { .. })
    ));
    assert_eq!(runtime.read_access("MissingTarget"), None);
    assert_eq!(
        runtime.write_access("MissingTarget", Value::Byte(1)),
        Err(error::RuntimeError::NullReference)
    );
    assert_eq!(
        runtime.storage().get_global("Status"),
        Some(&Value::Byte(4))
    );
}

#[test]
fn runtime_core_fault_state_records_original_fault_and_can_be_cleared() {
    let mut runtime = Runtime::new();

    let reported = runtime.simulation_fault("sensor disconnected");

    assert_eq!(
        reported,
        error::RuntimeError::SimulationFault("sensor disconnected".into())
    );
    assert!(runtime.faulted());
    assert_eq!(
        runtime.last_fault(),
        Some(&error::RuntimeError::SimulationFault(
            "sensor disconnected".into()
        ))
    );
    runtime.clear_fault();
    assert!(!runtime.faulted());
    assert_eq!(runtime.last_fault(), None);
}

fn function(name: &str) -> FunctionDef {
    FunctionDef {
        name: name.into(),
        return_type: TypeId::DINT,
        params: Vec::new(),
        locals: Vec::new(),
        static_locals: Vec::new(),
        using: Vec::new(),
        body: Vec::new(),
    }
}

fn function_block(name: &str) -> FunctionBlockDef {
    FunctionBlockDef {
        name: name.into(),
        base: None,
        interfaces: Vec::new(),
        params: Vec::new(),
        vars: Vec::new(),
        temps: Vec::new(),
        using: Vec::new(),
        methods: Vec::new(),
        body: Vec::new(),
    }
}

fn class(name: &str) -> ClassDef {
    ClassDef {
        name: name.into(),
        base: None,
        interfaces: Vec::new(),
        vars: Vec::new(),
        using: Vec::new(),
        methods: Vec::new(),
    }
}

fn interface(name: &str) -> InterfaceDef {
    InterfaceDef {
        name: name.into(),
        base: None,
        using: Vec::new(),
        methods: Vec::new(),
    }
}

fn program(name: &str) -> ProgramDef {
    ProgramDef {
        name: name.into(),
        vars: Vec::new(),
        temps: Vec::new(),
        using: Vec::new(),
        body: Vec::new(),
    }
}

fn task(name: &str, single: Option<&str>, programs: &[&str]) -> TaskConfig {
    TaskConfig {
        name: name.into(),
        interval: Duration::from_millis(10),
        single: single.map(Into::into),
        priority: 1,
        programs: programs.iter().map(|program| (*program).into()).collect(),
        fb_instances: Vec::new(),
    }
}
