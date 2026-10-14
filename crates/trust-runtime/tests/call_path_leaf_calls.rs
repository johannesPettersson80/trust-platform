//! Leaf calls run in place only where the handler is not needed; debugging and non-leaf callees keep the handler.
#![cfg(feature = "register-ir")]
use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::value::{DateTimeValue, Duration, Value};
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

/// One cycle's result and the observed globals after it.
type CycleObservation = (Result<(), RuntimeError>, Vec<Option<Value>>);

struct Observation {
    cycles: Vec<CycleObservation>,
    fault: Option<RuntimeError>,
    in_place_calls: u64,
}

/// Runs LEAF_PARITY_SOURCE for 24 cycles with leaf calls in place or through the pump.
/// `failing` is zeroed before cycle 21 so that the leaf fails at that cycle.
fn observe(debugger: bool, in_place: bool, failing: Option<&str>) -> Observation {
    let bytes = CompileSession::from_source(LEAF_PARITY_SOURCE)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap()
        .encode()
        .unwrap();
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let services = Attachment(debugger);
    let mut state = prepared.instantiate_with_services(0, &services).unwrap();
    state.set_register_profile_enabled(true);
    state.set_tier1_enabled(true);
    state.set_in_place_calls_enabled(in_place);
    let mut cycles = Vec::new();
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
        let values = ["count", "scaled", "seen", "answer"]
            .iter()
            .map(|name| state.storage().get_instance_var(main, name).cloned())
            .collect();
        let failed = result.is_err();
        cycles.push((result, values));
        if failed {
            break;
        }
    }
    Observation {
        cycles,
        fault: state.fault().cloned(),
        in_place_calls: state.in_place_call_count(),
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
        let pumped = observe(false, false, failing);
        let debugged = observe(true, true, failing);
        assert_eq!(
            debugged.in_place_calls, 0,
            "a debugger attached keeps every leaf call on the handler ({failing:?})"
        );
        assert_eq!(debugged.cycles, pumped.cycles, "results ({failing:?})");
        assert_eq!(debugged.fault, pumped.fault, "fault ({failing:?})");
        assert_eq!(debugged.cycles, in_place.cycles, "results ({failing:?})");
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
