//! Execute standard blocks and port/fault boundaries through admitted STBC 2.0.
use std::cell::Cell;
use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::memory::InstanceId;
use trust_runtime_core::value::{DateTimeValue, Duration, Value};
use trust_runtime_core::vm::{ExecutionServices, PreparationLimits, PreparedModule, RuntimeState};

fn prepare(source: &str) -> PreparedModule {
    let module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("accepted ST source");
    PreparedModule::from_bytes(&module.encode().unwrap(), PreparationLimits::default()).unwrap()
}
fn instance(value: Option<&Value>) -> InstanceId {
    match value {
        Some(Value::Instance(id)) => *id,
        other => panic!("instance: {other:?}"),
    }
}
fn member(state: &RuntimeState<'_>, block: &str, field: &str) -> Value {
    let main = instance(state.storage().get_global("Main"));
    let owner = instance(state.storage().get_instance_var(main, block));
    state
        .storage()
        .get_instance_var(owner, field)
        .unwrap()
        .clone()
}

#[test]
fn off_delay_pulse_and_both_edge_kinds_execute_in_source_free_state() {
    let prepared = prepare(
        r#"
VAR_GLOBAL signal : BOOL; END_VAR
FUNCTION_BLOCK FallingCapture
VAR_INPUT pulse : BOOL F_EDGE; END_VAR
VAR_OUTPUT count : INT; END_VAR
IF pulse THEN count := count + INT#1; END_IF;
END_FUNCTION_BLOCK
PROGRAM Main
VAR off_delay : TOF; pulse_timer : TP; rising : R_TRIG; falling : F_TRIG; captured : FallingCapture; END_VAR
off_delay(IN := signal, PT := T#25ms);
pulse_timer(IN := signal, PT := T#25ms);
rising(CLK := signal);
falling(CLK := signal);
captured(pulse := signal);
END_PROGRAM
"#,
    );
    let mut state = prepared.instantiate(0).unwrap();
    // Preserve the hosted blocks' sampled-delta contract, including timer ET.
    for (time, signal, off_q, off_et, pulse_q, pulse_et, rise, fall, count) in [
        (0, true, true, 0, true, 0, true, false, 0),
        (10, true, true, 0, true, 10, false, false, 0),
        (20, false, true, 10, true, 20, false, true, 1),
        (30, false, true, 20, false, 0, false, false, 1),
        (40, false, false, 25, false, 0, false, false, 1),
    ] {
        state.write_global("signal", Value::Bool(signal)).unwrap();
        state.execute_cycle(Duration::from_millis(time)).unwrap();
        for (block, q) in [
            ("off_delay", off_q),
            ("pulse_timer", pulse_q),
            ("rising", rise),
            ("falling", fall),
        ] {
            assert_eq!(
                member(&state, block, "Q"),
                Value::Bool(q),
                "{block} at {time}"
            );
        }
        assert_eq!(
            member(&state, "off_delay", "ET"),
            Value::Time(Duration::from_millis(off_et))
        );
        assert_eq!(
            member(&state, "pulse_timer", "ET"),
            Value::Time(Duration::from_millis(pulse_et))
        );
        assert_eq!(member(&state, "captured", "count"), Value::Int(count));
    }
}

#[test]
fn counters_and_bistable_priority_execute_in_source_free_state() {
    let prepared = prepare(
        r#"
VAR_GLOBAL up_input : BOOL; down_input : BOOL; reset_input : BOOL; load_input : BOOL; set_input : BOOL; END_VAR
PROGRAM Main
VAR upward : CTU; downward : CTD; combined : CTUD; set_dominant : SR; reset_dominant : RS; END_VAR
upward(CU := up_input, R := reset_input, PV := INT#2);
downward(CD := down_input, LD := load_input, PV := INT#2);
combined(CU := up_input, CD := down_input, R := reset_input, LD := load_input, PV := INT#2);
set_dominant(S1 := set_input, R := reset_input);
reset_dominant(S := set_input, R1 := reset_input);
END_PROGRAM
"#,
    );
    let mut state = prepared.instantiate(0).unwrap();
    for (time, up, down, reset, load, set, up_cv, down_cv, both_cv, sr, rs) in [
        (0, false, false, false, true, true, 0, 2, 2, true, true),
        (10, true, false, false, false, false, 1, 2, 3, true, true),
        (20, true, true, false, false, false, 1, 1, 2, true, true),
        (30, false, false, false, false, false, 1, 1, 2, true, true),
        (40, true, true, true, false, true, 0, 0, 0, true, false),
        (50, false, false, true, false, false, 0, 0, 0, false, false),
    ] {
        for (name, value) in [
            ("up_input", up),
            ("down_input", down),
            ("reset_input", reset),
            ("load_input", load),
            ("set_input", set),
        ] {
            state.write_global(name, Value::Bool(value)).unwrap();
        }
        state.execute_cycle(Duration::from_millis(time)).unwrap();
        for (block, cv) in [
            ("upward", up_cv),
            ("downward", down_cv),
            ("combined", both_cv),
        ] {
            assert_eq!(
                member(&state, block, "CV"),
                Value::Int(cv),
                "{block} at {time}"
            );
        }
        assert_eq!(member(&state, "upward", "Q"), Value::Bool(up_cv >= 2));
        assert_eq!(member(&state, "downward", "Q"), Value::Bool(down_cv <= 0));
        assert_eq!(member(&state, "combined", "QU"), Value::Bool(both_cv >= 2));
        assert_eq!(member(&state, "combined", "QD"), Value::Bool(both_cv <= 0));
        assert_eq!(member(&state, "set_dominant", "Q1"), Value::Bool(sr));
        assert_eq!(member(&state, "reset_dominant", "Q1"), Value::Bool(rs));
    }
}

#[test]
fn hierarchical_inputs_are_sampled_and_outputs_published_through_declared_bindings() {
    let prepared = prepare("PROGRAM Main VAR inp AT %IX1.2.3 : BOOL; out AT %QX1.2.4 : BOOL; END_VAR out := inp; END_PROGRAM");
    let mut state = prepared.instantiate(0).unwrap();
    let input = IoAddress::parse("%IX1.2.3").unwrap();
    let output = IoAddress::parse("%QX1.2.4").unwrap();
    let unknown = IoAddress::parse("%IX9.2.3").unwrap();
    assert!(matches!(
        state.set_hierarchical_input(&unknown, Value::Bool(true)),
        Err(RuntimeError::InvalidIoAddress(_))
    ));
    for (time, value) in [(0, false), (10, true), (20, false)] {
        state
            .set_hierarchical_input(&input, Value::Bool(value))
            .unwrap();
        state.execute_cycle(Duration::from_millis(time)).unwrap();
        assert_eq!(
            state.read_hierarchical_output(&output),
            Some(&Value::Bool(value))
        );
    }
}

struct Deadline {
    armed: Cell<bool>,
    polls: Cell<usize>,
}
impl ExecutionServices for Deadline {
    fn deadline_exceeded(&self) -> bool {
        if !self.armed.get() {
            return false;
        }
        self.polls.set(self.polls.get() + 1);
        self.polls.get() >= 32
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
    }
}

#[test]
fn physical_deadline_faults_mid_cycle_and_withholds_outputs() {
    let prepared = prepare(
        r#"
VAR_GLOBAL reached : DINT; END_VAR
PROGRAM Main
VAR out AT %QX0.0 : BOOL; index : DINT; END_VAR
out := TRUE;
reached := DINT#1;
FOR index := DINT#0 TO DINT#1000 DO reached := reached + DINT#1; END_FOR;
END_PROGRAM
"#,
    );
    let clock = Deadline {
        armed: Cell::new(false),
        polls: Cell::new(0),
    };
    let mut state = prepared.instantiate_with_services(0, &clock).unwrap();
    clock.armed.set(true);
    let fault = state.execute_cycle(Duration::from_millis(10)).unwrap_err();
    assert_eq!(
        fault,
        trust_runtime_core::vm::VmTrap::DeadlineExceeded.into_runtime_error()
    );
    let Some(Value::DInt(reached)) = state.storage().get_global("reached") else {
        panic!("progress counter")
    };
    assert!(
        *reached > 1 && *reached < 1002,
        "deadline must interrupt running ST, not reject entry"
    );
    assert_eq!(
        clock.polls.get(),
        32,
        "the first expired clock sample stops execution"
    );
    assert_eq!(state.outputs(), [0]);
    assert_eq!(state.fault(), Some(&fault));
}
