//! Leaf calls run in place only where the handler is not needed; debugging and non-leaf callees keep the handler.
#![cfg(feature = "register-ir")]
use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::value::{DateTimeValue, Duration, Value};
use trust_runtime_core::vm::register::RegisterExecutionConfiguration;
use trust_runtime_core::vm::{ExecutionServices, PreparationLimits, PreparedModule};

const LEAF_PARITY_SOURCE: &str = r#"
VAR_GLOBAL in_divisor : INT := INT#1; fb_divisor : INT := INT#1; END_VAR
FUNCTION Scale : INT
VAR_IN_OUT value : INT; END_VAR
VAR_INPUT increment : INT := INT#1; END_VAR
value := value + increment;
Scale := value / in_divisor;
END_FUNCTION
FUNCTION_BLOCK Edge
VAR_INPUT pulse : BOOL R_EDGE; END_VAR
VAR_OUTPUT seen : INT; END_VAR
IF pulse THEN seen := seen + INT#1; END_IF;
seen := seen / fb_divisor;
END_FUNCTION_BLOCK
FUNCTION Outer : INT
VAR local : INT := INT#2; END_VAR
Outer := Scale(value := local) + local;
END_FUNCTION
PROGRAM Main
VAR count : INT := INT#5; scaled : INT; edge : Edge; seen : INT; answer : INT; END_VAR
scaled := Scale(value := count);
edge(pulse := TRUE, seen => seen);
answer := Outer();
END_PROGRAM
"#;

/// Platform services; `true` attaches a debugger.
struct Attachment(bool);

impl ExecutionServices for Attachment {
    fn debug_active(&self) -> bool {
        self.0
    }
    fn deadline_exceeded(&self) -> bool {
        false
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
    }
}

/// One cycle's result, the fault latched after it, and the observed globals after it.
type CycleObservation = (
    Result<(), RuntimeError>,
    Option<RuntimeError>,
    Vec<Option<Value>>,
);

struct Observation {
    cycles: Vec<CycleObservation>,
    /// The fault that stopped the first failing cycle; the run continues after recovery.
    fault: Option<RuntimeError>,
    in_place_calls: u64,
    tier1_block_executions: u64,
}

/// Platform services for `observe`: `debugger` attaches a debugger, and tier-1 selects each block at its first hit.
struct ObservedArm {
    debugger: bool,
}

impl ExecutionServices for ObservedArm {
    fn register_configuration(&self) -> RegisterExecutionConfiguration {
        RegisterExecutionConfiguration {
            tier1_enabled: true,
            tier1_hot_threshold: 1,
            ..RegisterExecutionConfiguration::default()
        }
    }
    fn debug_active(&self) -> bool {
        self.debugger
    }
    fn deadline_exceeded(&self) -> bool {
        false
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
    }
}

/// Runs LEAF_PARITY_SOURCE for 24 cycles with leaf calls in place or through the pump.
/// `failing` is zeroed before cycle 21 so that the leaf fails at that cycle. The fault is then cleared and the
/// divisor restored, so cycles 22 to 24 run on the recovered state.
fn observe(debugger: bool, in_place: bool, failing: Option<&str>) -> Observation {
    let bytes = CompileSession::from_source(LEAF_PARITY_SOURCE)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap()
        .encode()
        .unwrap();
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let services = ObservedArm { debugger };
    let mut state = prepared.instantiate_with_services(0, &services).unwrap();
    state.set_register_profile_enabled(true);
    state.set_in_place_calls_enabled(in_place);
    let compile_attempts = state.tier1_snapshot().compile_attempts;
    let mut cycles: Vec<CycleObservation> = Vec::new();
    for tick in 1..=24i64 {
        if tick == 21
            && let Some(name) = failing
        {
            state.write_global(name, Value::Int(0)).unwrap();
        }
        let result = state.execute_cycle(Duration::from_millis(tick * 10));
        let Some(Value::Instance(main)) = state.storage().get_global("Main").cloned() else {
            panic!("program root")
        };
        let values: Vec<Option<Value>> = ["count", "scaled", "seen", "answer"]
            .iter()
            .map(|name| state.storage().get_instance_var(main, name).cloned())
            .collect();
        let failed = result.is_err();
        cycles.push((result, state.fault().cloned(), values));
        if failed {
            state.clear_fault();
            if let Some(name) = failing {
                state.write_global(name, Value::Int(1)).unwrap();
            }
        }
    }
    assert_eq!(
        state.tier1_snapshot().compile_attempts,
        compile_attempts,
        "RUN compiles nothing ({debugger}, {in_place}, {failing:?})"
    );
    Observation {
        fault: cycles
            .iter()
            .find_map(|(result, _, _)| result.clone().err()),
        cycles,
        in_place_calls: state.in_place_call_count(),
        tier1_block_executions: state.tier1_snapshot().block_executions,
    }
}

/// Spec 34 §4.3 HOST-04 "Work done in place": calls made while a debugger is attached keep the handler.
#[test]
fn call_path_leaf_calls_stay_on_the_handler_while_a_debugger_is_attached() {
    for failing in [None, Some("in_divisor")] {
        let in_place = observe(false, true, failing);
        assert!(
            in_place.in_place_calls > 0,
            "without a debugger the leaf calls run in place ({failing:?})"
        );
        assert!(
            in_place.tier1_block_executions > 0,
            "tier-1 runs the leaf blocks in place ({failing:?})"
        );
        let pumped = observe(false, false, failing);
        assert!(
            pumped.tier1_block_executions > 0,
            "tier-1 runs the leaf blocks through the pump ({failing:?})"
        );
        let debugged = observe(true, true, failing);
        assert_eq!(
            debugged.in_place_calls, 0,
            "a debugger attached keeps every leaf call on the handler ({failing:?})"
        );
        assert_eq!(
            debugged.tier1_block_executions, 0,
            "a debugger attached keeps every block on the stack path ({failing:?})"
        );
        assert_eq!(debugged.cycles, pumped.cycles, "results ({failing:?})");
        assert_eq!(debugged.fault, pumped.fault, "fault ({failing:?})");
        assert_eq!(debugged.cycles, in_place.cycles, "results ({failing:?})");
        // The fault stops only its own cycle; the run then goes on and cycles 22 to 24 succeed.
        assert_eq!(
            in_place.fault.is_some(),
            failing.is_some(),
            "fault ({failing:?})"
        );
        assert_eq!(in_place.cycles.len(), 24, "all cycles run ({failing:?})");
        assert!(
            in_place.cycles[21..]
                .iter()
                .all(|(result, fault, _)| result.is_ok() && fault.is_none()),
            "cycles 22 to 24 succeed after clear_fault ({failing:?})"
        );
    }
}

/// Smallest construction allowance that admits the whole observation with leaf calls in place or through the pump.
fn minimum_construction_bytes(in_place: bool) -> usize {
    let bytes = CompileSession::from_source(LEAF_PARITY_SOURCE)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap()
        .encode()
        .unwrap();
    let services = Attachment(false);
    let admitted = |max_construction_bytes: usize| {
        let limits = PreparationLimits {
            max_construction_bytes,
            ..PreparationLimits::default()
        };
        let Ok(prepared) = PreparedModule::from_bytes(&bytes, limits) else {
            return false;
        };
        let Ok(mut state) = prepared.instantiate_with_services(0, &services) else {
            return false;
        };
        state.set_register_profile_enabled(true);
        state.set_tier1_enabled(true);
        state.set_in_place_calls_enabled(in_place);
        (1..=24i64).all(|tick| {
            state
                .execute_cycle(Duration::from_millis(tick * 10))
                .is_ok()
        })
    };
    let (mut low, mut high) = (1usize, PreparationLimits::default().max_construction_bytes);
    assert!(admitted(high), "the default allowance admits the program");
    while low < high {
        let middle = low + (high - low) / 2;
        if admitted(middle) {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

/// Spec 34 §4.3 HOST-04 "Work done in place": a leaf call reserves no handler storage, so that storage is
/// neither charged nor a point of refusal.
#[test]
fn call_path_in_place_leaf_calls_are_never_refused_for_handler_storage() {
    let in_place = minimum_construction_bytes(true);
    let pumped = minimum_construction_bytes(false);
    assert!(
        in_place <= pumped,
        "in-place minimum {in_place} must not exceed the pump minimum {pumped}"
    );
}

/// Spec 34 §4.3 HOST-04 "Work done in place" and "Calls": a call to a non-leaf callee keeps the handler, so
/// only the leaf calls made from inside it run in place.
#[test]
fn call_path_non_leaf_callees_keep_the_handler_and_only_leaf_calls_run_in_place() {
    let source = r#"
FUNCTION Leaf : INT
VAR_INPUT x : INT; END_VAR
Leaf := x + INT#1;
END_FUNCTION
FUNCTION Middle : INT
VAR_INPUT x : INT; END_VAR
Middle := Leaf(x := x) + INT#1;
END_FUNCTION
PROGRAM Main
VAR total : INT; END_VAR
total := Middle(x := INT#1) + Middle(x := INT#2) + Middle(x := INT#3);
END_PROGRAM
"#;
    let bytes = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap()
        .encode()
        .unwrap();
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let services = Attachment(false);
    let mut state = prepared.instantiate_with_services(0, &services).unwrap();
    state.set_register_profile_enabled(true);
    state.set_in_place_calls_enabled(true);
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    let Some(Value::Instance(main)) = state.storage().get_global("Main") else {
        panic!("program root")
    };
    assert_eq!(
        state.storage().get_instance_var(*main, "total"),
        Some(&Value::Int(12))
    );
    assert_eq!(
        state.in_place_call_count(),
        3,
        "three Leaf calls run in place; the three Middle calls keep the handler"
    );
}

/// Spec 34 §4.3 HOST-04 "Work done in place" and TIME-08: with register profiling and tier-1 selection on, in-place
/// and pumped leaf calls admit at the same minimum work over every cycle of the run.
#[test]
fn call_path_in_place_leaf_calls_charge_the_pump_work_over_all_cycles_with_tier1() {
    let bytes = CompileSession::from_source(LEAF_PARITY_SOURCE)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap()
        .encode()
        .unwrap();
    let services = Attachment(false);
    let admitted = |max_work: usize, in_place: bool| {
        let limits = PreparationLimits {
            max_work,
            ..PreparationLimits::default()
        };
        let Ok(prepared) = PreparedModule::from_bytes(&bytes, limits) else {
            return false;
        };
        let Ok(mut state) = prepared.instantiate_with_services(0, &services) else {
            return false;
        };
        state.set_register_profile_enabled(true);
        state.set_tier1_enabled(true);
        state.set_in_place_calls_enabled(in_place);
        (1..=24i64).all(|tick| {
            state
                .execute_cycle(Duration::from_millis(tick * 10))
                .is_ok()
        })
    };
    let minimum = |in_place: bool| {
        let (mut low, mut high) = (1usize, PreparationLimits::default().max_work);
        assert!(admitted(high, in_place));
        while low < high {
            let middle = low + (high - low) / 2;
            if admitted(middle, in_place) {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        low
    };
    assert_eq!(minimum(true), minimum(false));
}
