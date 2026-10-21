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
        // The pulse ends on this FALSE sample: ET stops at PT and resets on the next one.
        (30, false, true, 20, false, 25, false, false, 1),
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

/// TON on the engine, whose block reaches IN, PT, Q, ET and its elapsed-time baseline at the places
/// preparation found for them (spec 34 PERF-05, checklist P1-4 and P1-5). The expected values are
/// spec 08 §5 and the timer decision of 12 July 2026 in `docs/IEC_DECISIONS.md` worked by hand: the
/// first call adds no time, each later call adds the clock's advance while IN is TRUE, ET stops at
/// PT, where Q turns TRUE, and IN FALSE clears Q and ET. Catches a member read or written at another
/// member's place (ET taken from PT, Q written over IN), a baseline kept at a wrong place, and a
/// call that binds IN or PT to another member.
#[test]
fn on_delay_executes_in_source_free_state() {
    let prepared = prepare(
        r#"
VAR_GLOBAL signal : BOOL; END_VAR
PROGRAM Main
VAR on_delay : TON; END_VAR
on_delay(IN := signal, PT := T#25ms);
END_PROGRAM
"#,
    );
    let mut state = prepared.instantiate(0).unwrap();
    for (time, signal, q, et) in [
        (0, true, false, 0),
        (10, true, false, 10),
        (20, true, false, 20),
        (30, true, true, 25),
        (40, true, true, 25),
        (50, false, false, 0),
    ] {
        state.write_global("signal", Value::Bool(signal)).unwrap();
        state.execute_cycle(Duration::from_millis(time)).unwrap();
        assert_eq!(
            member(&state, "on_delay", "Q"),
            Value::Bool(q),
            "Q at {time}"
        );
        assert_eq!(
            member(&state, "on_delay", "ET"),
            Value::Time(Duration::from_millis(et)),
            "ET at {time}"
        );
        assert_eq!(
            member(&state, "on_delay", "IN"),
            Value::Bool(signal),
            "IN at {time}"
        );
        assert_eq!(
            member(&state, "on_delay", "PT"),
            Value::Time(Duration::from_millis(25)),
            "PT at {time}"
        );
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

/// A deadline that passes at the `at`-th clock sample after it is armed.
struct Expiring {
    armed: Cell<bool>,
    polls: Cell<usize>,
    at: usize,
}
impl ExecutionServices for Expiring {
    fn deadline_exceeded(&self) -> bool {
        if !self.armed.get() {
            return false;
        }
        self.polls.set(self.polls.get() + 1);
        self.polls.get() >= self.at
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
    }
}

/// Inputs sampled into their variables' leaves (spec 34 PERF-05) are sampled all or none: a
/// synchronization refused at its deadline writes back what it wrote (spec 34 §6.2, "Process
/// images"). The image holds 5 and 7 and the variables start at 0. For a deadline that passes at
/// each of the scan's clock samples in turn, a refused scan leaves both variables at 0 or both at 5
/// and 7, and a completed one at 5 and 7. Every sample is the one that passes for one of the scans,
/// so the check before the second write is among them. Catches a refusal that leaves the first
/// input written, and a write that reaches another variable.
#[test]
fn a_refused_input_synchronization_leaves_no_input_written() {
    let prepared = prepare(
        "PROGRAM Main VAR first AT %ID0 : DINT; second AT %ID4 : DINT; seen : DINT; END_VAR
         seen := first + second; END_PROGRAM",
    );
    let mut refused_before = false;
    let mut refused_after = false;
    for at in 1..=40 {
        let clock = Expiring {
            armed: Cell::new(false),
            polls: Cell::new(0),
            at,
        };
        let mut state = prepared.instantiate_with_services(0, &clock).unwrap();
        state.inputs_mut()[0..4].copy_from_slice(&5i32.to_le_bytes());
        state.inputs_mut()[4..8].copy_from_slice(&7i32.to_le_bytes());
        clock.armed.set(true);
        let result = state.execute_cycle(Duration::from_millis(10));
        let main = instance(state.storage().get_global("Main"));
        let pair = (
            state.storage().get_instance_var(main, "first").cloned(),
            state.storage().get_instance_var(main, "second").cloned(),
        );
        let written = (Some(Value::DInt(5)), Some(Value::DInt(7)));
        let kept = (Some(Value::DInt(0)), Some(Value::DInt(0)));
        match result {
            Ok(()) => assert_eq!(pair, written, "a completed scan at sample {at}"),
            Err(_) if pair == kept => refused_before = true,
            Err(_) if pair == written => refused_after = true,
            Err(error) => panic!("refused at sample {at} ({error}) with {pair:?}"),
        }
    }
    assert!(
        refused_before,
        "some scan was refused before its inputs were kept"
    );
    assert!(
        refused_after,
        "some scan was refused after its inputs were kept"
    );
}

/// Scalar bindings whose image bytes are their variables' bytes are sampled and published by
/// copying them (spec 34 PERF-05; checklist P1-6). Worked out from the process image's encoding
/// (little-endian, two's complement, one bit for a BOOL; `io_image::read_flat_image`): the first
/// scan's `%IB0` is 2#1111_0111, whose bit 3 is clear, so `flag` is FALSE; the second's is
/// 2#0000_1000, so `flag` is TRUE and `%QB0` holds bit 5 alone, 16#20; 16#FE is SINT -2, 16#34 16#12
/// WORD 16#1234, 16#FF 16#FF INT -1, 16#FE 16#FF 16#FF 16#FF DINT -2 and 16#FD and seven 16#FF LINT
/// -3, and each comes back in the same bytes of `%Q`. Catches a byte order or sign error, a BOOL
/// read from its whole byte or written over its neighbours, and a copy wider than its binding (the
/// bytes between the bindings stay 0).
#[test]
fn scalar_bindings_are_sampled_and_published_in_their_bytes() {
    let prepared = prepare(
        "PROGRAM Main VAR
            flag AT %IX0.3 : BOOL; small AT %IB1 : SINT; bits AT %IW2 : WORD;
            signed AT %IW4 : INT; long AT %ID8 : DINT; wide AT %IL16 : LINT;
            flagOut AT %QX0.5 : BOOL; smallOut AT %QB1 : SINT; bitsOut AT %QW2 : WORD;
            signedOut AT %QW4 : INT; longOut AT %QD8 : DINT; wideOut AT %QL16 : LINT;
         END_VAR
         flagOut := flag; smallOut := small; bitsOut := bits;
         signedOut := signed; longOut := long; wideOut := wide;
         END_PROGRAM",
    );
    let mut state = prepared.instantiate(0).unwrap();
    let image: [u8; 24] = [
        0xF7, 0xFE, 0x34, 0x12, 0xFF, 0xFF, 0, 0, 0xFE, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0, 0xFD, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    ];
    state.inputs_mut()[..24].copy_from_slice(&image);
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    let main = instance(state.storage().get_global("Main"));
    let read = |state: &RuntimeState<'_>, name: &str| {
        state.storage().get_instance_var(main, name).cloned()
    };
    assert_eq!(read(&state, "flag"), Some(Value::Bool(false)));
    state.inputs_mut()[0] = 0x08;
    state.execute_cycle(Duration::from_millis(20)).unwrap();
    assert_eq!(read(&state, "flag"), Some(Value::Bool(true)));
    assert_eq!(read(&state, "small"), Some(Value::SInt(-2)));
    assert_eq!(read(&state, "bits"), Some(Value::Word(0x1234)));
    assert_eq!(read(&state, "signed"), Some(Value::Int(-1)));
    assert_eq!(read(&state, "long"), Some(Value::DInt(-2)));
    assert_eq!(read(&state, "wide"), Some(Value::LInt(-3)));
    let expected: [u8; 24] = [
        0x20, 0xFE, 0x34, 0x12, 0xFF, 0xFF, 0, 0, 0xFE, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0, 0xFD, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    ];
    assert_eq!(&state.outputs()[..24], &expected);
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
