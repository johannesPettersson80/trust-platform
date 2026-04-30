use std::sync::{Arc, Mutex};

use trust_runtime::error::RuntimeError;
use trust_runtime::eval::expr::{Expr, LValue};
use trust_runtime::eval::stmt::Stmt;
use trust_runtime::execution_backend::ExecutionBackend;
use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::{IoAddress, IoDriver, IoSafeState};
use trust_runtime::task::{ProgramDef, TaskConfig};
use trust_runtime::value::Value;
use trust_runtime::watchdog::{
    FaultAction, FaultDecision, FaultPolicy, WatchdogAction, WatchdogPolicy,
};
use trust_runtime::{RestartMode, Runtime};

const FIXTURE_SOURCE: &str = include_str!("fixtures/runtime_core_behavior_lock/program.st");
const FIXTURE_PATH: &str = "fixtures/runtime_core_behavior_lock/program.st";

fn compile_fixture_session() -> CompileSession {
    CompileSession::from_sources(vec![SourceFile::with_path(FIXTURE_PATH, FIXTURE_SOURCE)])
}

fn fixture_bytecode() -> Vec<u8> {
    compile_fixture_session()
        .build_bytecode_bytes()
        .expect("build behavior-lock bytecode fixture")
}

fn runtime_with_fixture(bytes: &[u8]) -> Runtime {
    let mut runtime = compile_fixture_session()
        .build_runtime()
        .expect("build runtime for behavior-lock fixture");
    runtime
        .apply_bytecode_bytes(bytes, None)
        .expect("load behavior-lock bytecode fixture");
    runtime
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("select bytecode VM");
    runtime
        .restart(RestartMode::Cold)
        .expect("cold restart behavior-lock runtime");
    runtime
}

fn main_instance_id(runtime: &Runtime) -> trust_runtime::memory::InstanceId {
    match runtime.storage().get_global("Main") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected Main program instance, got {other:?}"),
    }
}

fn main_var(runtime: &Runtime, name: &str) -> Value {
    let main_id = main_instance_id(runtime);
    runtime
        .storage()
        .get_instance_var(main_id, name)
        .cloned()
        .unwrap_or_else(|| panic!("missing Main.{name}"))
}

fn struct_field<'a>(value: &'a Value, field: &str) -> &'a Value {
    let Value::Struct(payload) = value else {
        panic!("expected struct value, got {value:?}");
    };
    payload
        .fields()
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(field))
        .map(|(_, value)| value)
        .unwrap_or_else(|| panic!("missing struct field {field} in {payload:?}"))
}

fn assert_array(value: &Value, expected: &[Value]) {
    let Value::Array(array) = value else {
        panic!("expected array value, got {value:?}");
    };
    assert_eq!(array.elements(), expected);
}

fn set_input_bit(runtime: &mut Runtime, value: bool) {
    let address = IoAddress::parse("%IX0.0").expect("parse input address");
    runtime
        .io_mut()
        .write(&address, Value::Bool(value))
        .expect("write input image");
}

fn execute_fixture_cycle(runtime: &mut Runtime, input: bool) {
    set_input_bit(runtime, input);
    runtime.execute_cycle().expect("execute fixture cycle");
}

#[derive(Debug, Default)]
struct CycleTrace {
    events: Vec<String>,
    writes: Vec<(String, Vec<u8>)>,
}

struct BoundaryDriver {
    name: &'static str,
    first_input: u8,
    later_input: u8,
    reads: u32,
    trace: Arc<Mutex<CycleTrace>>,
}

impl BoundaryDriver {
    fn new(
        name: &'static str,
        first_input: u8,
        later_input: u8,
        trace: Arc<Mutex<CycleTrace>>,
    ) -> Self {
        Self {
            name,
            first_input,
            later_input,
            reads: 0,
            trace,
        }
    }
}

impl IoDriver for BoundaryDriver {
    fn read_inputs(&mut self, inputs: &mut [u8]) -> Result<(), trust_runtime::error::RuntimeError> {
        let input = if self.reads == 0 {
            self.first_input
        } else {
            self.later_input
        };
        if let Some(byte) = inputs.first_mut() {
            *byte = input;
        }
        self.reads += 1;
        self.trace
            .lock()
            .expect("cycle trace lock")
            .events
            .push(format!("{}:read", self.name));
        Ok(())
    }

    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), trust_runtime::error::RuntimeError> {
        let mut trace = self.trace.lock().expect("cycle trace lock");
        trace.events.push(format!("{}:write", self.name));
        trace.writes.push((self.name.to_string(), outputs.to_vec()));
        Ok(())
    }
}

fn cycle_boundary_runtime() -> Runtime {
    let mut runtime = Runtime::new();
    runtime.io_mut().resize(1, 1, 0);
    runtime.storage_mut().set_global("in", Value::Bool(false));
    runtime
        .storage_mut()
        .set_global("out_a", Value::Bool(false));
    runtime
        .storage_mut()
        .set_global("out_b", Value::Bool(false));
    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(false));

    runtime
        .register_program(assign_program("P1", "out_a"))
        .unwrap();
    runtime
        .register_program(assign_program("P2", "out_b"))
        .unwrap();

    runtime
        .io_mut()
        .bind("in", IoAddress::parse("%IX0.0").expect("input address"));
    runtime
        .io_mut()
        .bind("out_a", IoAddress::parse("%QX0.0").expect("output address"));
    runtime
        .io_mut()
        .bind("out_b", IoAddress::parse("%QX0.1").expect("output address"));

    runtime.register_task(TaskConfig {
        name: "T".into(),
        interval: trust_runtime::value::Duration::ZERO,
        single: Some("trigger".into()),
        priority: 0,
        programs: vec!["P1".into(), "P2".into()],
        fb_instances: Vec::new(),
    });
    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(true));
    runtime
}

fn runtime_with_watchdog_safe_state(action: WatchdogAction) -> Runtime {
    let mut runtime = Runtime::new();
    runtime.io_mut().resize(0, 1, 0);
    let address = IoAddress::parse("%QX0.0").expect("safe-state output address");
    runtime
        .io_mut()
        .write(&address, Value::Bool(true))
        .expect("write initial output state");

    let mut safe_state = IoSafeState::default();
    safe_state.outputs.push((address, Value::Bool(false)));
    runtime.set_io_safe_state(safe_state);
    runtime.set_watchdog_policy(WatchdogPolicy {
        enabled: true,
        timeout: trust_runtime::value::Duration::from_nanos(1),
        action,
    });
    runtime
}

fn assign_program(name: &str, target: &str) -> ProgramDef {
    ProgramDef {
        name: name.into(),
        vars: Vec::new(),
        temps: Vec::new(),
        using: Vec::new(),
        body: vec![Stmt::Assign {
            target: LValue::Name(target.into()),
            value: Expr::Name("in".into()),
            location: None,
        }],
    }
}

#[test]
fn watchdog_and_fault_policy_decisions_are_stable() {
    assert_eq!(
        FaultDecision::from_watchdog(WatchdogAction::Halt),
        FaultDecision {
            action: FaultAction::Halt,
            apply_safe_state: true,
        }
    );
    assert_eq!(
        FaultDecision::from_watchdog(WatchdogAction::SafeHalt),
        FaultDecision {
            action: FaultAction::SafeHalt,
            apply_safe_state: true,
        }
    );
    assert_eq!(
        FaultDecision::from_watchdog(WatchdogAction::Restart),
        FaultDecision {
            action: FaultAction::Restart,
            apply_safe_state: false,
        }
    );

    assert_eq!(
        FaultDecision::from_fault_policy(FaultPolicy::Halt),
        FaultDecision {
            action: FaultAction::Halt,
            apply_safe_state: false,
        }
    );
    assert_eq!(
        FaultDecision::from_fault_policy(FaultPolicy::SafeHalt),
        FaultDecision {
            action: FaultAction::SafeHalt,
            apply_safe_state: true,
        }
    );
    assert_eq!(
        FaultDecision::from_fault_policy(FaultPolicy::Restart),
        FaultDecision {
            action: FaultAction::Restart,
            apply_safe_state: false,
        }
    );

    assert!(WatchdogAction::parse("warn").is_err());
    assert!(FaultPolicy::parse("degrade").is_err());
}

#[test]
fn watchdog_timeout_preserves_fault_snapshot_and_safe_state_contract() {
    for (action, expected_outputs) in [
        (WatchdogAction::Halt, vec![0x00]),
        (WatchdogAction::SafeHalt, vec![0x00]),
        (WatchdogAction::Restart, vec![0x01]),
    ] {
        let mut runtime = runtime_with_watchdog_safe_state(action);

        let err = runtime.watchdog_timeout();

        assert_eq!(err, RuntimeError::WatchdogTimeout);
        assert!(runtime.faulted(), "{action:?} must fault the runtime");
        assert_eq!(runtime.last_fault(), Some(&RuntimeError::WatchdogTimeout));
        assert_eq!(runtime.io().outputs(), expected_outputs.as_slice());
        assert_eq!(
            runtime
                .execute_cycle()
                .expect_err("faulted runtime must reject cycles"),
            RuntimeError::ResourceFaulted
        );
    }
}

#[test]
fn stable_bytecode_fixture_loads_on_runtime_core_path() {
    let bytes = fixture_bytecode();
    assert!(!bytes.is_empty(), "bytecode fixture must not be empty");

    let runtime = runtime_with_fixture(&bytes);

    assert_eq!(runtime.execution_backend(), ExecutionBackend::BytecodeVm);
    assert!(
        !runtime.faulted(),
        "fresh fixture runtime must not be faulted"
    );
    assert!(runtime.last_fault().is_none(), "fresh fixture has no fault");
}

#[test]
fn vm_fixture_execution_image_status_and_values_are_stable() {
    let bytes = fixture_bytecode();
    let mut first = runtime_with_fixture(&bytes);
    let mut second = runtime_with_fixture(&bytes);

    execute_fixture_cycle(&mut first, true);
    execute_fixture_cycle(&mut second, true);

    assert_eq!(first.io().outputs(), second.io().outputs());
    assert_eq!(first.io().outputs(), &[0x01, 0x00, 0x34, 0x12]);
    assert_eq!(first.cycle_counter(), 1);
    assert_eq!(second.cycle_counter(), 1);
    assert!(
        !first.faulted(),
        "first run faulted: {:?}",
        first.last_fault()
    );
    assert!(
        !second.faulted(),
        "second run faulted: {:?}",
        second.last_fault()
    );
    assert_eq!(first.last_fault(), second.last_fault());

    let phase = main_var(&first, "phase");
    let Value::Enum(phase) = phase else {
        panic!("expected enum phase, got {phase:?}");
    };
    assert_eq!(phase.type_name().as_str(), "Phase");
    assert_eq!(phase.variant_name().as_str(), "RUNNING");
    assert_eq!(phase.numeric_value(), 1);

    assert_array(
        &main_var(&first, "samples"),
        &[Value::DInt(7), Value::DInt(9), Value::DInt(12)],
    );

    let payload = main_var(&first, "payload");
    assert_eq!(struct_field(&payload, "count"), &Value::DInt(12));
    assert_eq!(struct_field(&payload, "flag"), &Value::Bool(true));

    match main_var(&first, "ref_count") {
        Value::Reference(Some(_)) => {}
        other => panic!("expected live reference, got {other:?}"),
    }

    match main_var(&first, "fb") {
        Value::Instance(id) => {
            let out = first
                .storage()
                .get_instance_var(id, "OUT")
                .unwrap_or_else(|| panic!("missing Bump.OUT for instance {id:?}"));
            assert_eq!(out, &Value::DInt(13));
        }
        other => panic!("expected FB instance, got {other:?}"),
    }

    assert_eq!(main_var(&first, "retained_count"), Value::DInt(18));

    first.restart(RestartMode::Warm).expect("warm restart");
    assert_eq!(main_var(&first, "retained_count"), Value::DInt(18));

    first.restart(RestartMode::Cold).expect("cold restart");
    assert_eq!(main_var(&first, "retained_count"), Value::DInt(5));
}

#[test]
fn cycle_boundary_latches_inputs_once_and_commits_outputs_after_ready_programs() {
    let trace = Arc::new(Mutex::new(CycleTrace::default()));
    let mut runtime = cycle_boundary_runtime();
    runtime.add_io_driver(
        "driver",
        Box::new(BoundaryDriver::new("driver", 0x01, 0x00, trace.clone())),
    );

    runtime.execute_cycle().expect("execute boundary cycle");

    let trace = trace.lock().expect("cycle trace lock");
    assert_eq!(trace.events, ["driver:read", "driver:write"]);
    assert_eq!(trace.writes, [("driver".to_string(), vec![0x03])]);
    assert_eq!(runtime.io().outputs(), &[0x03]);
    assert_eq!(
        runtime.storage().get_global("out_a"),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        runtime.storage().get_global("out_b"),
        Some(&Value::Bool(true))
    );
}

#[test]
fn cycle_boundary_reads_every_driver_before_any_driver_writes_outputs() {
    let trace = Arc::new(Mutex::new(CycleTrace::default()));
    let mut runtime = cycle_boundary_runtime();
    runtime.add_io_driver(
        "first",
        Box::new(BoundaryDriver::new("first", 0x01, 0x00, trace.clone())),
    );
    runtime.add_io_driver(
        "second",
        Box::new(BoundaryDriver::new("second", 0x01, 0x00, trace.clone())),
    );

    runtime.execute_cycle().expect("execute multi-driver cycle");

    let trace = trace.lock().expect("cycle trace lock");
    assert_eq!(
        trace.events,
        ["first:read", "second:read", "first:write", "second:write"]
    );
    assert_eq!(
        trace.writes,
        [
            ("first".to_string(), vec![0x03]),
            ("second".to_string(), vec![0x03])
        ]
    );
}
