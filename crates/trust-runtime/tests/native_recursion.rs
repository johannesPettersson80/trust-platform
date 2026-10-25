//! Recursion in a compiled program (specification 44 §6.4, §8.5): IEC 61131-3 leaves the recursive
//! call of POUs to the implementer (§6.6.1.1), and truST runs it within a call depth that the task
//! stack holds.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::sync::atomic::{AtomicUsize, Ordering};

use trust_runtime::error::RuntimeError;
use trust_runtime::harness::SourceFile;
use trust_runtime::native::{build, load, Build, Device, NoHooks, Options, Target, Toolchain};
use trust_runtime::value::{Duration, Value};

/// The task stack the tests give the kernel, less than a test thread's 2 MiB.
const TASK_STACK: u64 = 1 << 20;

/// `source` compiled for this computer, and the device it runs on.
fn compiled(source: &str) -> (Build, Device) {
    static BUILDS: AtomicUsize = AtomicUsize::new(0);
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let host = Target::this_cpu().expect("a computer target");
    let work = std::env::temp_dir().join(format!(
        "trust-native-recursion-{}-{}",
        std::process::id(),
        BUILDS.fetch_add(1, Ordering::Relaxed)
    ));
    let built = build(
        &[SourceFile::new(source)],
        &Options {
            target: host,
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the program compiles");
    let _ = std::fs::remove_dir_all(&work);
    let device = Device {
        target: host,
        program_memory: 1 << 20,
        task_stack: TASK_STACK,
        max_statements: 1_000_000,
        trusted_keys: &[],
    };
    (built, device)
}

/// `SumTo(n)` adds 1 to `n` by calling itself, and counts its calls in the variable `calls` names.
/// It calls itself by its qualified name, as the runtime's own recursion test does: inside a function
/// its bare name is its result variable, and the runtime takes a bare call for a call on that
/// variable (a defect of the runtime's lane, recorded in the native execution checklist).
const SUM_TO: &str = "
NAMESPACE Demo
FUNCTION SumTo : DINT
VAR_INPUT
    n : DINT;
END_VAR
VAR_IN_OUT
    calls : DINT;
END_VAR
calls := calls + 1;
IF n <= 0 THEN
    SumTo := 0;
ELSE
    SumTo := n + Demo.SumTo(n := n - 1, calls := calls);
END_IF;
END_FUNCTION
END_NAMESPACE

PROGRAM Adder
VAR
    Calls : DINT;
    Result : DINT;
END_VAR
Result := Demo.SumTo(n := 10, calls := Calls);
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Adder;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_recursive_function_runs_within_its_call_depth() {
    // Specification 44 §6.4: a function that calls itself runs, its call of itself bounded by the
    // call depth. SumTo(10) = 10 + 9 + ... + 1 + 0 = 55 in 11 calls, one for each n from 10 to 0,
    // and the image carries a call level, the frame of the cycle SumTo makes (§8.5). Catches a
    // recursion refused or miscomputed, and a recursive program with no call level, whose depth
    // the stack would not bound.
    let (built, device) = compiled(SUM_TO);
    assert!(
        built.figure.call_level > 0,
        "a recursion has a call level: {:?}",
        built.figure
    );
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect("the scan runs");
    assert_eq!(program.value("MAIN.RESULT"), Some(Value::DInt(55)));
    assert_eq!(program.value("MAIN.CALLS"), Some(Value::DInt(11)));
}

/// `Forever` calls itself without end, by its qualified name, and counts its calls in the variable
/// `calls` names.
const FOREVER: &str = "
NAMESPACE Demo
FUNCTION Forever : DINT
VAR_INPUT
    n : DINT;
END_VAR
VAR_IN_OUT
    calls : DINT;
END_VAR
calls := calls + 1;
Forever := Demo.Forever(n := n + 1, calls := calls);
END_FUNCTION
END_NAMESPACE

PROGRAM Endless
VAR
    Calls : DINT;
    Result : DINT;
END_VAR
Result := Demo.Forever(n := 0, calls := Calls);
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Endless;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_recursion_that_does_not_end_ends_the_scan_at_the_call_depth() {
    // Specification 44 §6.4 and §8.5: the kernel gives the activation the depth (task stack −
    // bound) / call level, here worked out from the image's figure; the program's call of
    // `Forever` and as many recursive calls as the depth holds run, each counting in `Calls`, and
    // the next recursive call ends the scan with the error of a call beyond the call levels,
    // `call depth exceeded`, instead of running the task stack over. Catches a recursion
    // that overflows the stack (the native image had no bound before 23 October 2026), a depth
    // not worked out from the figure, and an entry into the recursion counted as a turn.
    let (built, device) = compiled(FOREVER);
    let figure = built.figure;
    assert!(figure.call_level > 0, "{figure:?}");
    let depth = (TASK_STACK - figure.stack) / figure.call_level;
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    let error = program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect_err("the recursion ends the scan");
    assert_eq!(error, RuntimeError::call_depth_exceeded());
    assert_eq!(error.to_string(), "call depth exceeded");
    let calls = i64::try_from(depth + 1).expect("a depth of a test's task stack");
    assert_eq!(
        program.value("MAIN.CALLS"),
        Some(Value::DInt(i32::try_from(calls).expect("fits a DINT")))
    );
}

/// A shape scaled three times: each `Scaled` asks the shape it wraps for its area through the
/// interface, so `Scaled.Area` calls itself through the interface's dispatcher, on another
/// instance at each turn.
const DECORATORS: &str = "
INTERFACE IShape
    METHOD Area : REAL
    END_METHOD
END_INTERFACE

FUNCTION_BLOCK Square IMPLEMENTS IShape
VAR
    side : REAL;
END_VAR
METHOD PUBLIC Area : REAL
Area := side * side;
END_METHOD
METHOD PUBLIC Size
VAR_INPUT
    s : REAL;
END_VAR
side := s;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Scaled IMPLEMENTS IShape
VAR
    inner : IShape;
    factor : REAL;
END_VAR
METHOD PUBLIC Area : REAL
Area := factor * inner.Area();
END_METHOD
METHOD PUBLIC Wrap
VAR_INPUT
    shape : IShape;
    f : REAL;
END_VAR
inner := shape;
factor := f;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Shapes
VAR
    sq : Square;
    a : Scaled;
    b : Scaled;
    c : Scaled;
    isq : IShape;
    ia : IShape;
    ib : IShape;
    Result : REAL;
END_VAR
sq.Size(s := 2.0);
isq := sq;
a.Wrap(shape := isq, f := 2.0);
ia := a;
b.Wrap(shape := ia, f := 3.0);
ib := b;
c.Wrap(shape := ib, f := 5.0);
Result := c.Area();
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Shapes;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_recursion_through_an_interface_runs_on_each_instance() {
    // Specification 44 §6.4: `Scaled.Area` calls the interface's dispatcher, which calls the
    // method of the instance's class, `Scaled.Area` again for `b` and `a` and `Square.Area` for
    // `sq`: a cycle through a dispatcher, counted at `Scaled.Area`'s call. The area is
    // 2 * 2 = 4 scaled by 2, 3 and 5: 120. Catches a cycle through a dispatcher left without a
    // call level, as a cycle once counted per class instance, and a dispatch that stops at the
    // first decorator.
    let (built, device) = compiled(DECORATORS);
    assert!(built.figure.call_level > 0, "{:?}", built.figure);
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect("the scan runs");
    assert_eq!(program.value("MAIN.RESULT"), Some(Value::Real(120.0)));
}

/// A counter in a 10 ms task, which calls no POU of its own.
const COUNTER: &str = "
PROGRAM Counter
VAR
    Count : UDINT;
END_VAR
Count := Count + 1;
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Counter;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_program_without_a_recursion_has_no_call_level() {
    // Specification 44 §8.5: the call level is 0 for a program without a cycle, whose activations
    // make no recursive call. Catches a call level for every program, which would lower nothing
    // but report a recursion that is not there.
    let (built, _) = compiled(COUNTER);
    assert_eq!(built.figure.call_level, 0, "{:?}", built.figure);
}
