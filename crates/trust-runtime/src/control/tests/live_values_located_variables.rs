// Specification 44 §13.3 and specification 11 §6.9.2 (the CODESYS toolkit's B17): the scan's sample
// copies a variable bound to an input or a marker address in from that address before the tasks
// run, so a write or a force of the variable is also written where the sample reads it, and the
// program reads the value the write or the force gave it.

const LOCATED_SOURCE: &str = r#"
PROGRAM Main
VAR
    Level AT %IW0 : INT;
    LevelSeen : INT;
    Ready AT %IX2.1 : BOOL;
    ReadySeen : BOOL;
    Delay AT %ID8 : TIME;
    Count AT %MW4 : INT;
    CountSeen : INT;
END_VAR
LevelSeen := Level;
ReadySeen := Ready;
CountSeen := Count;
END_PROGRAM
"#;

fn located_fixture() -> (ControlState, crate::CompiledRuntime) {
    let state = control_test_state(LOCATED_SOURCE);
    let mut runtime = TestHarness::from_source(LOCATED_SOURCE)
        .expect("the located fixture compiles")
        .into_runtime();
    runtime.set_debug_control(state.debug.clone());
    (state, runtime)
}

fn located_request(state: &ControlState, request: serde_json::Value) {
    let response = handle_request_value(request.clone(), state, None);
    assert!(response.ok, "{request} is accepted: {:?}", response.error);
}

fn located_value(runtime: &crate::CompiledRuntime, name: &str) -> crate::value::Value {
    runtime.read_variable(name).expect("the variable reads")
}

fn located_image(runtime: &crate::CompiledRuntime, address: &str) -> crate::value::Value {
    let address = crate::io::IoAddress::parse(address).expect("the address parses");
    runtime.read_io(&address).expect("the process image reads")
}

/// The write_rejected events raised since the previous call.
fn located_rejections(state: &ControlState) -> usize {
    state
        .debug
        .drain_runtime_events()
        .iter()
        .filter(|event| matches!(event, crate::debug::RuntimeEvent::WriteRejected { .. }))
        .count()
}

/// Worked by hand: the input written 5 reads 5 in the program; forced to 9, the program reads 9,
/// and still 9 after the input is written 6; released, it reads the next input, 7. Catches the force
/// that the sample replaced with the input before the program ran (the toolkit's forced input: its
/// program read the input while `var.force` answered `forced`), and a force kept after its release.
#[test]
fn a_forced_variable_bound_to_an_input_is_what_the_program_reads_until_released() {
    use crate::value::Value;
    let (state, mut runtime) = located_fixture();
    let io_write = |id: u64, value: &str| {
        json!({"id": id, "type": "io.write", "params": {"address": "%IW0", "value": value}})
    };
    located_request(&state, io_write(1, "5"));
    runtime.execute_cycle().expect("the scan runs");
    assert_eq!(located_value(&runtime, "Main.LevelSeen"), Value::Int(5));

    located_request(
        &state,
        json!({"id": 2, "type": "var.force", "params": {"target": "global:Main.Level", "value": "9"}}),
    );
    runtime.execute_cycle().expect("the scan runs");
    assert_eq!(
        located_value(&runtime, "Main.LevelSeen"),
        Value::Int(9),
        "the program reads the forced value"
    );
    assert_eq!(located_value(&runtime, "Main.Level"), Value::Int(9));

    located_request(&state, io_write(3, "6"));
    runtime.execute_cycle().expect("the scan runs");
    assert_eq!(
        located_value(&runtime, "Main.LevelSeen"),
        Value::Int(9),
        "the force holds over a new input"
    );

    located_request(
        &state,
        json!({"id": 4, "type": "var.unforce", "params": {"target": "global:Main.Level"}}),
    );
    located_request(&state, io_write(5, "7"));
    runtime.execute_cycle().expect("the scan runs");
    assert_eq!(
        located_value(&runtime, "Main.LevelSeen"),
        Value::Int(7),
        "released, the program reads the input"
    );
    assert_eq!(located_rejections(&state), 0);
    assert!(!runtime.faulted());
}

/// Worked by hand: `Ready AT %IX2.1` forced TRUE is read TRUE by the program, and the input byte
/// `%IB2` holds bit 1 alone, 2. Catches a bit written as a whole byte or at another bit, and the
/// forced value that the sample replaced (main: FALSE).
#[test]
fn a_forced_bit_input_is_written_at_its_bit() {
    use crate::value::Value;
    let (state, mut runtime) = located_fixture();
    located_request(
        &state,
        json!({"id": 1, "type": "var.force", "params": {"target": "global:Main.Ready", "value": "TRUE"}}),
    );
    runtime.execute_cycle().expect("the scan runs");
    assert_eq!(located_value(&runtime, "Main.ReadySeen"), Value::Bool(true));
    assert_eq!(located_image(&runtime, "%IB2"), Value::Byte(2));
    assert!(!runtime.faulted());
}

/// Specification 11 (an accepted `set`) and 44 §13.3: a write of a variable bound to a marker
/// address is written at the address too, so the marker keeps it from scan to scan; an input keeps
/// it until a driver's next input read, and this runtime has no driver. Worked by hand: `Count`
/// written 4 and `Level` written 3 read 4 and 3 in the program in this scan and the next, and
/// `%MW4` holds 4. Catches the write that the sample replaced with the address's old value before
/// the program ran (main: 0 for both).
#[test]
fn a_written_variable_bound_to_a_marker_or_an_undriven_input_keeps_its_value() {
    use crate::value::Value;
    let (state, mut runtime) = located_fixture();
    located_request(
        &state,
        json!({"id": 1, "type": "set", "params": {"target": "global:Main.Count", "value": "4"}}),
    );
    located_request(
        &state,
        json!({"id": 2, "type": "set", "params": {"target": "global:Main.Level", "value": "3"}}),
    );
    for scan in 1..=2 {
        runtime.execute_cycle().expect("the scan runs");
        assert_eq!(
            located_value(&runtime, "Main.CountSeen"),
            Value::Int(4),
            "scan {scan}: the program reads the written marker"
        );
        assert_eq!(
            located_value(&runtime, "Main.LevelSeen"),
            Value::Int(3),
            "scan {scan}: the program reads the written input"
        );
    }
    assert_eq!(located_image(&runtime, "%MW4"), Value::Word(4));
    assert_eq!(located_rejections(&state), 0);
}

/// Specification 44 §13.3: a `TIME` bound in whole milliseconds (`AT %ID8`) that its address
/// cannot hold, a negative duration, is refused at the boundary with one write_rejected event, and
/// the variable takes back its address's value, 0 ms; a duration it holds, 1,500 ms, reaches the
/// variable and its address. Catches a refused write that still changed the variable, and the write
/// that the sample silently replaced (main: no event, and 0 ms after the second write).
#[test]
fn a_time_that_its_millisecond_input_cannot_hold_is_refused_and_one_it_holds_is_kept() {
    use crate::value::{Duration, Value};
    let (state, mut runtime) = located_fixture();
    let set = |id: u64, value: &str| {
        json!({"id": id, "type": "set", "params": {"target": "global:Main.Delay", "value": value}})
    };
    located_request(&state, set(1, "T#-5ms"));
    runtime.execute_cycle().expect("a refused write does not fault the scan");
    assert_eq!(located_rejections(&state), 1, "one write_rejected event");
    assert_eq!(
        located_value(&runtime, "Main.Delay"),
        Value::Time(Duration::ZERO)
    );

    located_request(&state, set(2, "T#1500ms"));
    runtime.execute_cycle().expect("the scan runs");
    assert_eq!(located_rejections(&state), 0);
    assert_eq!(
        located_value(&runtime, "Main.Delay"),
        Value::Time(Duration::from_millis(1500))
    );
    assert_eq!(located_image(&runtime, "%ID8"), Value::DWord(1500));
    assert!(!runtime.faulted());
}
