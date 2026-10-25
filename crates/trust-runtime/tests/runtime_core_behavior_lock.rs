//! Behaviour locks of the runtime core: the watchdog and fault policy decisions, the watchdog's
//! safe-state contract, a fixture program's image and values, and the cycle boundary's order of
//! input reads, program runs and output writes.
//!
//! The tests that run a program need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::sync::{Arc, Mutex};

use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::io::{IoAddress, IoDriver, IoSafeState};
use trust_runtime::value::{Duration, Value};
use trust_runtime::watchdog::{
    FaultAction, FaultDecision, FaultPolicy, FaultSubsystem, WatchdogAction, WatchdogPolicy,
    WatchdogSubsystem,
};
use trust_runtime::{CompiledRuntime, RestartMode};

const FIXTURE_SOURCE: &str = include_str!("fixtures/runtime_core_behavior_lock/program.st");

fn runtime_with_fixture() -> CompiledRuntime {
    let mut runtime = TestHarness::from_source(FIXTURE_SOURCE)
        .expect("build runtime for behavior-lock fixture")
        .into_runtime();
    runtime
        .restart(RestartMode::Cold)
        .expect("cold restart behavior-lock runtime");
    runtime
}

fn main_var(runtime: &CompiledRuntime, name: &str) -> Value {
    runtime
        .read_variable(&format!("Main.{name}"))
        .unwrap_or_else(|error| panic!("missing Main.{name}: {error}"))
}

/// The `len` bytes of the output image from `%QB0`.
fn output_bytes(runtime: &CompiledRuntime, len: usize) -> Vec<u8> {
    (0..len)
        .map(|byte| {
            let address = IoAddress::parse(&format!("%QB{byte}")).expect("an output byte");
            match runtime.read_io(&address) {
                Ok(Value::Byte(value)) => value,
                other => panic!("%QB{byte}: {other:?}"),
            }
        })
        .collect()
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

fn set_input_bit(runtime: &mut CompiledRuntime, value: bool) {
    let address = IoAddress::parse("%IX0.0").expect("parse input address");
    runtime
        .write_io(&address, Value::Bool(value))
        .expect("write input image");
}

fn execute_fixture_cycle(runtime: &mut CompiledRuntime, input: bool) {
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

/// `in` copied to `out_a` by `P1` and to `out_b` by `P2`, both in one event task that a rising edge
/// of `trigger` starts; `trigger` is set, so the next scan runs both.
const CYCLE_BOUNDARY: &str = r#"
CONFIGURATION C
VAR_GLOBAL
    in AT %IX0.0 : BOOL;
    out_a AT %QX0.0 : BOOL;
    out_b AT %QX0.1 : BOOL;
    trigger : BOOL;
END_VAR
RESOURCE R ON PLC
    TASK T (SINGLE := trigger, PRIORITY := 0);
    PROGRAM I1 WITH T : P1;
    PROGRAM I2 WITH T : P2;
END_RESOURCE
END_CONFIGURATION

PROGRAM P1
out_a := in;
END_PROGRAM

PROGRAM P2
out_b := in;
END_PROGRAM
"#;

fn cycle_boundary_runtime() -> CompiledRuntime {
    let mut runtime = TestHarness::from_source(CYCLE_BOUNDARY)
        .expect("install declared test runtime")
        .into_runtime();
    runtime
        .write_variable("trigger", &Value::Bool(true))
        .expect("checked event trigger");
    runtime
}

/// A program with one output bit, so the output image has the byte the safe state writes.
const ONE_OUTPUT: &str = "
VAR_GLOBAL
    lamp AT %QX0.0 : BOOL;
END_VAR

PROGRAM Main
END_PROGRAM
";

fn runtime_with_watchdog_safe_state(action: WatchdogAction) -> CompiledRuntime {
    let address = IoAddress::parse("%QX0.0").expect("safe-state output address");
    let mut runtime = TestHarness::from_source(ONE_OUTPUT)
        .expect("install declared test runtime")
        .into_runtime();
    runtime
        .write_io(&address, Value::Bool(true))
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

    for (policy, expected) in [
        (
            FaultPolicy::Halt,
            FaultDecision {
                action: FaultAction::Halt,
                apply_safe_state: false,
            },
        ),
        (
            FaultPolicy::SafeHalt,
            FaultDecision {
                action: FaultAction::SafeHalt,
                apply_safe_state: true,
            },
        ),
        (
            FaultPolicy::Restart,
            FaultDecision {
                action: FaultAction::Restart,
                apply_safe_state: false,
            },
        ),
    ] {
        let mut faults = FaultSubsystem::new();
        faults.set_policy(policy);
        assert_eq!(faults.decision(), expected);
        assert_eq!(faults.policy(), policy);
        assert!(!faults.is_faulted());
        assert_eq!(faults.last_fault(), None);

        faults.record(RuntimeError::WatchdogTimeout);
        assert_eq!(faults.decision(), expected);
        assert_eq!(faults.policy(), policy);
        assert!(faults.is_faulted());
        assert_eq!(faults.last_fault(), Some(&RuntimeError::WatchdogTimeout));
    }

    assert!(WatchdogAction::parse("warn").is_err());
    assert!(FaultPolicy::parse("degrade").is_err());
}

#[test]
fn watchdog_and_fault_policy_state_updates_are_stable() {
    for (input, expected) in [
        (
            WatchdogPolicy {
                enabled: false,
                timeout: Duration::from_nanos(-1),
                action: WatchdogAction::Halt,
            },
            WatchdogPolicy {
                enabled: false,
                timeout: Duration::from_nanos(-1),
                action: WatchdogAction::Halt,
            },
        ),
        (
            WatchdogPolicy {
                enabled: true,
                timeout: Duration::ZERO,
                action: WatchdogAction::Restart,
            },
            WatchdogPolicy {
                enabled: true,
                timeout: Duration::from_millis(1),
                action: WatchdogAction::Restart,
            },
        ),
        (
            WatchdogPolicy {
                enabled: true,
                timeout: Duration::from_millis(7),
                action: WatchdogAction::SafeHalt,
            },
            WatchdogPolicy {
                enabled: true,
                timeout: Duration::from_millis(7),
                action: WatchdogAction::SafeHalt,
            },
        ),
    ] {
        let mut watchdog = WatchdogSubsystem::new();
        watchdog.set_policy(input);
        assert_eq!(watchdog.policy(), expected);
        assert_eq!(
            watchdog.decision(),
            FaultDecision::from_watchdog(expected.action)
        );
    }

    let mut faults = FaultSubsystem::new();
    faults.set_policy(FaultPolicy::SafeHalt);
    faults.record(RuntimeError::WatchdogTimeout);
    faults.set_policy(FaultPolicy::Restart);
    assert_eq!(faults.policy(), FaultPolicy::Restart);
    assert!(faults.is_faulted());
    assert_eq!(faults.last_fault(), Some(&RuntimeError::WatchdogTimeout));

    faults.record(RuntimeError::DivisionByZero);
    assert_eq!(faults.policy(), FaultPolicy::Restart);
    assert!(faults.is_faulted());
    assert_eq!(faults.last_fault(), Some(&RuntimeError::DivisionByZero));
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
        assert_eq!(runtime.last_fault(), Some(RuntimeError::WatchdogTimeout));
        assert_eq!(output_bytes(&runtime, 1), expected_outputs);
        assert_eq!(
            runtime
                .execute_cycle()
                .expect_err("faulted runtime must reject cycles"),
            RuntimeError::ResourceFaulted
        );
    }
}

#[test]
fn vm_fixture_execution_image_status_and_values_are_stable() {
    let mut first = runtime_with_fixture();
    let mut second = runtime_with_fixture();

    execute_fixture_cycle(&mut first, true);
    execute_fixture_cycle(&mut second, true);

    assert_eq!(output_bytes(&first, 4), output_bytes(&second, 4));
    assert_eq!(output_bytes(&first, 4), [0x01, 0x00, 0x34, 0x12]);
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
    assert!(
        first.last_fault().is_none(),
        "first run must not retain a hidden fault"
    );
    assert!(
        second.last_fault().is_none(),
        "second run must not retain a hidden fault"
    );

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

    assert_eq!(main_var(&first, "fb.OUT"), Value::DInt(13));

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
    assert_eq!(output_bytes(&runtime, 1), [0x03]);
    assert_eq!(runtime.read_variable("out_a"), Ok(Value::Bool(true)));
    assert_eq!(runtime.read_variable("out_b"), Ok(Value::Bool(true)));
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
