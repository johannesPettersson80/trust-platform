//! Several instances of one program type and the connections of a program's inputs and outputs
//! in the configuration (IEC 61131-3 §6.8.2, Table 62 6a, 8a, 8b, 9a, 9b; §6.5.6.2;
//! specification 18 §2.4). Every expected value is worked out by hand: the tasks run as
//! specification 10 §4.3 releases them, a 10 ms task first at 10 ms, a 20 ms task at 20 and 40.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};
use trust_runtime::RestartMode;

/// Runs `harness` to `steps` samples 10 ms apart, the first at 10 ms.
fn run(harness: &mut TestHarness, steps: u32) {
    for _ in 0..steps {
        harness.advance_time(Duration::from_millis(10));
        let result = harness.cycle();
        assert!(result.errors.is_empty(), "{:?}", result.errors);
    }
}

/// The program of `source`, compiled.
fn compiled(source: &str) -> TestHarness {
    TestHarness::from_source(source).unwrap_or_else(|error| panic!("must compile: {error}"))
}

/// The build's refusal of `source`.
fn refusal(source: &str) -> String {
    match TestHarness::from_source(source) {
        Ok(_) => panic!("the build must refuse the program"),
        Err(error) => error.to_string(),
    }
}

/// Two instances of one program type, each in its own task, each with its own variables and
/// `VAR_CONFIG` values (IEC Figure 28's `STATION_1.P1.COUNT := 1; STATION_2.P1.COUNT := 100`).
/// At 10, 20, 30 and 40 ms `A` runs four times from 100 by 1: 104; `B` runs at 20 and 40 from 0
/// by 10: 20. Catches the second instance refused, both instances sharing one storage (both
/// would read 124), a `VAR_CONFIG` value given to every instance of the type, and an instance run
/// by the other's task.
#[test]
fn two_instances_of_one_program_type_each_keep_their_own_variables() {
    let mut harness = compiled(
        r#"
PROGRAM Counter
VAR
    count : INT;
    increment : INT := 1;
END_VAR
count := count + increment;
END_PROGRAM

CONFIGURATION Cell
RESOURCE Station ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    TASK Slow (INTERVAL := T#20ms, PRIORITY := 2);
    PROGRAM A WITH Fast : Counter;
    PROGRAM B WITH Slow : Counter;
END_RESOURCE
VAR_CONFIG
    A.count : INT := 100;
    B.increment : INT := 10;
END_VAR
END_CONFIGURATION
"#,
    );
    run(&mut harness, 4);
    assert_eq!(harness.get_output("A.count"), Some(Value::Int(104)));
    assert_eq!(harness.get_output("A.increment"), Some(Value::Int(1)));
    assert_eq!(harness.get_output("B.count"), Some(Value::Int(20)));
    assert_eq!(harness.get_output("B.increment"), Some(Value::Int(10)));
}

/// Each instance reads and writes its own addresses through its connections (Table 62 8a, 9a):
/// `First` doubles `%IW0` = 5 into `%QW0`, `Second` doubles `%IW2` = 7 into `%QW2`. Catches the
/// connections dropped (both outputs 0, as the front end dropped them before), and an instance
/// bound at the other's addresses.
#[test]
fn each_instance_reads_and_writes_the_addresses_it_is_connected_to() {
    let mut harness = compiled(
        r#"
PROGRAM Doubler
VAR_INPUT
    inp : INT;
END_VAR
VAR_OUTPUT
    outp : INT;
END_VAR
outp := inp * 2;
END_PROGRAM

CONFIGURATION Cell
RESOURCE Station ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM First WITH Fast : Doubler(inp := %IW0, outp => %QW0);
    PROGRAM Second WITH Fast : Doubler(inp := %IW2, outp => %QW2);
END_RESOURCE
END_CONFIGURATION
"#,
    );
    harness
        .set_direct_input("%IW0", Value::Word(5))
        .expect("an input address");
    harness
        .set_direct_input("%IW2", Value::Word(7))
        .expect("an input address");
    run(&mut harness, 1);
    let output = |address: &str| {
        harness
            .get_direct_output(address)
            .unwrap_or_else(|error| panic!("{address}: {error}"))
    };
    assert_eq!(output("%QW0"), Value::Word(10));
    assert_eq!(output("%QW2"), Value::Word(14));
}

/// An input connected to a global takes its value at each activation before the body, an input
/// connected to a constant that constant, and an output connected to a global gives it its value
/// after the body, widened from `DINT` to `LINT` as an assignment widens it (Table 62 8b, 9b;
/// Table 11). Activation 1: 21 × 2 = 42; `Src` set to 5, activation 2: 10. Catches connections
/// dropped (`Result` 0), made once only (activation 2 would compute 21 × 2 again), and the output
/// copied before the body (`Result` one activation behind: 0, then 42).
#[test]
fn connections_to_globals_and_constants_are_made_around_each_activation() {
    let mut harness = compiled(
        r#"
PROGRAM Scale
VAR_INPUT
    inp : INT;
    factor : INT;
END_VAR
VAR_OUTPUT
    outp : DINT;
END_VAR
outp := INT_TO_DINT(inp * factor);
END_PROGRAM

CONFIGURATION Cell
VAR_GLOBAL
    Src : INT := 21;
    Result : LINT;
END_VAR
RESOURCE Station ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P WITH Fast : Scale(inp := Src, factor := 2, outp => Result);
END_RESOURCE
END_CONFIGURATION
"#,
    );
    run(&mut harness, 1);
    assert_eq!(harness.get_output("Result"), Some(Value::LInt(42)));
    assert_eq!(harness.get_output("P.inp"), Some(Value::Int(21)));
    assert_eq!(harness.get_output("P.factor"), Some(Value::Int(2)));
    harness.set_input("Src", Value::Int(5));
    run(&mut harness, 1);
    assert_eq!(harness.get_output("Result"), Some(Value::LInt(10)));
}

/// A connection that names no input or output of the program, an output connected with `:=`, an
/// input with `=>`, an input connected to an output address and an output connected to a global
/// it does not convert to implicitly are refused at build with the reason (Table 62; Table 11).
/// Catches each being compiled into something else or dropped without a word.
#[test]
fn connections_the_program_does_not_take_are_refused() {
    let program = r#"
PROGRAM Doubler
VAR_INPUT
    inp : INT;
END_VAR
VAR_OUTPUT
    outp : DINT;
END_VAR
outp := INT_TO_DINT(inp) * 2;
END_PROGRAM
"#;
    let configured = |connections: &str| {
        format!(
            "{program}
CONFIGURATION Cell
VAR_GLOBAL
    Small : INT;
END_VAR
RESOURCE Station ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P WITH Fast : Doubler({connections});
END_RESOURCE
END_CONFIGURATION
"
        )
    };
    let missing = refusal(&configured("missing := 1"));
    assert!(
        missing.contains("missing := of program instance P names no VAR_INPUT of program Doubler"),
        "{missing}"
    );
    let output_as_input = refusal(&configured("outp := 1"));
    assert!(
        output_as_input.contains("outp := of program instance P names no VAR_INPUT"),
        "{output_as_input}"
    );
    let input_as_output = refusal(&configured("inp => Small"));
    assert!(
        input_as_output.contains("inp => of program instance P names no VAR_OUTPUT"),
        "{input_as_output}"
    );
    let output_address = refusal(&configured("inp := %QW0"));
    assert!(
        output_address.contains("P.inp is connected to %QW0: an input reads an input or a marker"),
        "{output_address}"
    );
    let narrowed = refusal(&configured("outp => Small"));
    assert!(
        narrowed.contains("P.outp is connected to a value of another type"),
        "{narrowed}"
    );
}

/// `RETAIN` and `NON_RETAIN` on two instances of one program type apply to each instance (IEC
/// §6.5.6.2): after one activation both values are 4; a warm restart keeps `First`'s 4 and starts
/// `Second` at its initial 3. Catches the qualifier merged per type (both kept, both lost, or the
/// program refused as it was before).
#[test]
fn retain_qualifiers_apply_to_each_instance() {
    let mut harness = compiled(
        r#"
PROGRAM Main
VAR
    value : INT := 3;
END_VAR
value := value + 1;
END_PROGRAM

CONFIGURATION Cell
RESOURCE Station ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM RETAIN First WITH Fast : Main;
    PROGRAM NON_RETAIN Second WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
"#,
    );
    run(&mut harness, 1);
    assert_eq!(harness.get_output("First.value"), Some(Value::Int(4)));
    assert_eq!(harness.get_output("Second.value"), Some(Value::Int(4)));
    harness.restart(RestartMode::Warm).expect("warm restart");
    assert_eq!(harness.get_output("First.value"), Some(Value::Int(4)));
    assert_eq!(harness.get_output("Second.value"), Some(Value::Int(3)));
}

/// A program's own access path names the variable of the instance that runs the body (IEC
/// §6.5.2.2, Table 47): `One` reads its 7, `Two` its 70; the tools name each instance's path
/// `<instance>.<name>`. Catches the path bound to the first instance (`Two.y` 8).
#[test]
fn a_programs_access_path_names_the_variable_of_its_own_instance() {
    let mut harness = compiled(
        r#"
PROGRAM Main
VAR
    x : INT;
    y : INT;
END_VAR
VAR_ACCESS
    Ax : x : INT READ_ONLY;
END_VAR
y := Ax + 1;
END_PROGRAM

CONFIGURATION Cell
RESOURCE Station ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM One WITH Fast : Main;
    PROGRAM Two WITH Fast : Main;
END_RESOURCE
VAR_CONFIG
    One.x : INT := 7;
    Two.x : INT := 70;
END_VAR
END_CONFIGURATION
"#,
    );
    run(&mut harness, 1);
    assert_eq!(harness.get_output("One.y"), Some(Value::Int(8)));
    assert_eq!(harness.get_output("Two.y"), Some(Value::Int(71)));
    assert_eq!(harness.get_access("One.Ax"), Some(Value::Int(7)));
    assert_eq!(harness.get_access("Two.Ax"), Some(Value::Int(70)));
}
