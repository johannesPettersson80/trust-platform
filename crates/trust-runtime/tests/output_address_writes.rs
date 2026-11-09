//! Specification 44 §13.3: a write at an output address reaches the variable bound there, so that the program reads it and
//! each scan's publication carries it to the drivers until the program writes the variable itself. Before, only the output
//! image took the write, and the publication of the variable bound there replaced it after one scan: an experiment that
//! stepped the output of a loop whose program stops writing it in Manual never reached the plant (the CODESYS toolkit's S3).
use trust_runtime::debug::DebugControl;
use trust_runtime::harness::{CompileSession, SourceFile, TestHarness};
use trust_runtime::io::IoAddress;
use trust_runtime::value::{Duration, Value};
use trust_runtime::CompiledRuntime;

/// A program that writes its output `Power` (`%QL0`) and its lamp (`%QX8.2`) only in Auto (`%MX0.0`), as a CODESYS program
/// whose Manual simply stops writing them.
const PROGRAM: &str = r"
PROGRAM Main
VAR
    Auto : BOOL := FALSE;
    Power : LREAL := 75.0;
    Lamp : BOOL := FALSE;
    Scans : DINT := 0;
END_VAR
IF Auto THEN
    Power := 40.0;
    Lamp := FALSE;
END_IF;
Scans := Scans + 1;
END_PROGRAM

CONFIGURATION Plant
RESOURCE Res ON PLC
    TASK Cycle (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH Cycle : Main;
END_RESOURCE
VAR_CONFIG
    P1.Power AT %QL0 : LREAL;
    P1.Lamp AT %QX8.2 : BOOL;
    P1.Auto AT %MX0.0 : BOOL;
END_VAR
END_CONFIGURATION
";

fn runtime() -> CompiledRuntime {
    TestHarness::from_session(CompileSession::from_sources(vec![SourceFile::with_path(
        "src/Main.st",
        PROGRAM,
    )]))
    .unwrap_or_else(|error| panic!("the program compiles: {error}"))
    .into_runtime()
}

/// One scan, 100 ms of logical time after the last.
fn cycle(runtime: &mut CompiledRuntime) {
    let now = runtime.current_time();
    runtime.set_current_time(Duration::from_nanos(
        now.as_nanos()
            .saturating_add(Duration::from_millis(100).as_nanos()),
    ));
    runtime.execute_cycle().expect("the cycle runs");
}

fn power(runtime: &CompiledRuntime) -> (f64, f64) {
    let image = match runtime.read_io(&IoAddress::parse("%QL0").unwrap()).unwrap() {
        Value::LWord(bits) => f64::from_bits(bits),
        Value::LReal(value) => value,
        other => panic!("expected LREAL at %QL0, got {other:?}"),
    };
    let variable = match runtime.read_variable("P1.Power").unwrap() {
        Value::LReal(value) => value,
        other => panic!("expected LREAL in P1.Power, got {other:?}"),
    };
    (image, variable)
}

fn lamp(runtime: &CompiledRuntime) -> (Value, Value) {
    (
        runtime
            .read_io(&IoAddress::parse("%QX8.2").unwrap())
            .unwrap(),
        runtime.read_variable("P1.Lamp").unwrap(),
    )
}

/// Worked from the program: in Manual it writes neither output, so 85.0 written at `%QL0` is in both the output image and
/// `P1.Power` after the next scan and after the one after it, and TRUE written at `%QX8.2` is in the image and in `P1.Lamp`.
/// Once `%MX0.0` sets Auto, the program writes 40.0 and FALSE, which the outputs then carry. Wrong behaviour caught: a write
/// that only the output image took, which the publication of `P1.Power` (75.0) replaced at the end of the next scan.
#[test]
fn a_write_at_an_output_address_reaches_its_variable_and_holds_until_the_program_writes_it() {
    let mut runtime = runtime();
    let debug = DebugControl::new();
    runtime.set_debug_control(debug.clone());
    cycle(&mut runtime);
    assert_eq!(
        power(&runtime),
        (75.0, 75.0),
        "the starting value is published"
    );

    debug.enqueue_io_write(
        IoAddress::parse("%QL0").unwrap(),
        Value::LWord(85.0_f64.to_bits()),
    );
    debug.enqueue_io_write(IoAddress::parse("%QX8.2").unwrap(), Value::Bool(true));
    cycle(&mut runtime);
    assert_eq!(
        power(&runtime),
        (85.0, 85.0),
        "the output and its variable hold the write after the scan"
    );
    assert_eq!(lamp(&runtime), (Value::Bool(true), Value::Bool(true)));
    cycle(&mut runtime);
    assert_eq!(
        power(&runtime),
        (85.0, 85.0),
        "the write holds while the program does not write the output"
    );

    debug.enqueue_io_write(IoAddress::parse("%MX0.0").unwrap(), Value::Bool(true));
    cycle(&mut runtime);
    assert_eq!(
        power(&runtime),
        (40.0, 40.0),
        "in Auto the program's own value goes out"
    );
    assert_eq!(lamp(&runtime), (Value::Bool(false), Value::Bool(false)));
}
