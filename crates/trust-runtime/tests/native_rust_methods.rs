//! Structured Text calls a Rust function block's methods as its own (specification 48 RU-32): the
//! method runs on the instance as the block's body does, its inputs, in-outs, outputs and result
//! reach the caller, and a task counts its budget (RU-47). A Rust block implements Structured
//! Text interfaces, and a call through one reaches its method (RU-33).
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`. A toolchain without
//! cargo and the SDK's sources, as a release's bundle is until specification 48 §16, refuses the
//! build with that reason.

mod rust_pous_support;

use rust_pous_support::{build_error, built, custom_project, manifest};
use trust_runtime::error::RuntimeError;
use trust_runtime::native::rust_pous::DECLARATIONS_FILE;
use trust_runtime::native::{load, NoHooks};
use trust_runtime::value::{Duration, Value};

/// A block AXIS whose method `jog` moves it and `double` doubles the variable it is given.
const AXIS: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

/// An axis that moves by the distances its method `jog` is given.
#[trust_fb(name = \"AXIS\")]
#[budget(\"1us\")]
pub struct Axis {
    #[input]
    pub target: f64,
    #[output]
    pub position: f64,
    moves: i16,
}

impl Body for Axis {
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        Ok(())
    }
}

impl Axis {
    /// Moves by `distance` and counts the move; `reached` tells whether the target is reached; the
    /// result is the new position. A position beyond LREAL faults.
    #[trust_method]
    #[budget(\"6ms\")]
    pub fn jog(&mut self, cx: &mut Cx, distance: f64, reached: &mut bool) -> Result<f64, Fault> {
        self.moves = checked::add(cx, self.moves, 1)?;
        self.position = checked::fadd(cx, self.position, distance)?;
        *reached = self.position >= self.target;
        Ok(self.position)
    }

    /// Doubles the variable it is given.
    #[trust_method]
    #[budget(\"1us\")]
    pub fn double(&mut self, cx: &mut Cx, value: InOut<'_, i16>) -> Result<(), Fault> {
        value.set(checked::mul(cx, value.get(), 2)?);
        Ok(())
    }
}
";

/// The task that runs `Line` every 10 ms.
const TASK: &str = "CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn structured_text_calls_a_rust_blocks_methods() {
    // RU-32, one scan at 10 ms: AXIS with the target 5.0; `jog(2.5)` moves it to 2.5, not reached;
    // the second `jog(2.5)` to 5.0, reached; the instance keeps the position 5.0 and 2 moves.
    // `double` doubles the program's 21 to 42 through its in-out. RU-47: the task calls `jog`
    // twice, 2 × 6 ms = 12 ms, more than its interval. Catches a method that runs on a copy it
    // throws away, an output or a result that does not reach the caller, an in-out that is not the
    // caller's variable, and a method's budget the task does not count.
    let program = format!(
        "PROGRAM Line
VAR
    axis : AXIS;
    first : LREAL;
    second : LREAL;
    first_reached : BOOL := TRUE;
    second_reached : BOOL;
    count : INT := 21;
END_VAR
axis(target := 5.0);
first := axis.jog(distance := 2.5, reached => first_reached);
second := axis.jog(distance := 2.5, reached => second_reached);
axis.double(value := count);
END_PROGRAM

{TASK}"
    );
    let root = custom_project(
        "rust-pous-methods",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", AXIS)],
        &program,
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    assert!(
        built
            .warnings
            .iter()
            .any(|warning| warning.contains("AXIS.JOG 2 × 6 ms") && warning.contains("RU-47")),
        "{:?}",
        built.warnings
    );
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect("the scan runs");
    let expected = [
        ("MAIN.FIRST", Value::LReal(2.5)),
        ("MAIN.FIRST_REACHED", Value::Bool(false)),
        ("MAIN.SECOND", Value::LReal(5.0)),
        ("MAIN.SECOND_REACHED", Value::Bool(true)),
        ("MAIN.AXIS.POSITION", Value::LReal(5.0)),
        ("MAIN.AXIS.MOVES", Value::Int(2)),
        ("MAIN.COUNT", Value::Int(42)),
    ];
    for (name, value) in expected {
        assert_eq!(program.value(name), Some(value), "{name}");
    }
}

#[test]
fn a_methods_fault_leaves_the_instance_as_it_was() {
    // RU-32 and RU-28: the second `jog(1.0E308)` takes the position beyond LREAL, `Overflow`; the
    // fault ends the scan, the instance keeps the first move's position 1.0E308 and its 1 move,
    // although the method counted a second before it faulted, and the statement after the call
    // never runs. Catches a method whose fault keeps what it changed.
    let program = format!(
        "PROGRAM Line
VAR
    axis : AXIS;
    moved : LREAL;
    reached : BOOL;
    after : BOOL;
END_VAR
axis(target := 5.0);
moved := axis.jog(distance := 1.0E308, reached => reached);
moved := axis.jog(distance := 1.0E308, reached => reached);
after := TRUE;
END_PROGRAM

{TASK}"
    );
    let root = custom_project(
        "rust-pous-method-fault",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", AXIS)],
        &program,
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    let error = program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect_err("the overflow ends the scan");
    assert_eq!(error, RuntimeError::Overflow);
    assert_eq!(
        program.value("MAIN.AXIS.POSITION"),
        Some(Value::LReal(1.0e308))
    );
    assert_eq!(program.value("MAIN.AXIS.MOVES"), Some(Value::Int(1)));
    assert_eq!(program.value("MAIN.AFTER"), Some(Value::Bool(false)));
}

/// A crate with the block COUNTER, which implements I_COUNTER, and the methods `methods`.
fn counter(methods: &str) -> String {
    format!(
        "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

/// A counter that counts by the amounts its method `count_by` is given.
#[trust_fb(name = \"COUNTER\", implements = \"I_COUNTER\")]
#[budget(\"1us\")]
pub struct Counter {{
    #[output]
    pub count: i32,
}}

impl Body for Counter {{
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {{
        Ok(())
    }}
}}
{methods}"
    )
}

/// COUNTER's method `count_by`, its input `amount` of the Rust type `ty`, with a budget of 4 ms.
fn count_by(ty: &str) -> String {
    format!(
        "
impl Counter {{
    /// Counts by `amount`; the result is the new count.
    #[trust_method]
    #[budget(\"4ms\")]
    pub fn count_by(&mut self, cx: &mut Cx, amount: {ty}) -> Result<i32, Fault> {{
        self.count = checked::add(cx, self.count, i32::from(amount))?;
        Ok(self.count)
    }}
}}
"
    )
}

/// The interface I_COUNTER, and ST_COUNTER, a Structured Text block that implements it and counts
/// by ten times the amount.
const I_COUNTER: &str = "INTERFACE I_COUNTER
METHOD count_by : DINT
VAR_INPUT
    amount : DINT;
END_VAR
END_METHOD
END_INTERFACE

FUNCTION_BLOCK ST_COUNTER IMPLEMENTS I_COUNTER
VAR_OUTPUT
    count : DINT;
END_VAR
METHOD PUBLIC count_by : DINT
VAR_INPUT
    amount : DINT;
END_VAR
count := count + 10 * amount;
count_by := count;
END_METHOD
END_FUNCTION_BLOCK
";

#[test]
fn a_call_through_an_interface_reaches_a_rust_blocks_method() {
    // RU-33, one scan at 10 ms: the interface variable holds the Rust COUNTER for `count_by(2)`
    // and `count_by(3)`, which count it to 2 and 5, then the Structured Text ST_COUNTER for
    // `count_by(3)`, 10 × 3 = 30. RU-47: each of the three calls through I_COUNTER can reach
    // COUNTER's `count_by`, so the task counts 3 × 4 ms = 12 ms, more than its interval. Catches a
    // dispatcher without the Rust block's arm, an arm that runs another type's method, a Rust
    // method whose count is lost, and an interface call whose Rust work the admission does not
    // count.
    let program = format!(
        "{I_COUNTER}
PROGRAM Line
VAR
    in_rust : COUNTER;
    in_st : ST_COUNTER;
    counter : I_COUNTER;
    first : DINT;
    second : DINT;
    third : DINT;
END_VAR
counter := in_rust;
first := counter.count_by(amount := 2);
second := counter.count_by(amount := 3);
counter := in_st;
third := counter.count_by(amount := 3);
END_PROGRAM

{TASK}"
    );
    let root = custom_project(
        "rust-pous-interfaces",
        &[
            ("Cargo.toml", &manifest("")),
            ("src/lib.rs", &counter(&count_by("i32"))),
        ],
        &program,
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    assert!(
        built
            .warnings
            .iter()
            .any(|warning| warning.contains("COUNTER.COUNT_BY 3 × 4 ms")
                && warning.contains("RU-47")),
        "{:?}",
        built.warnings
    );
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect("the scan runs");
    let expected = [
        ("MAIN.FIRST", Value::DInt(2)),
        ("MAIN.SECOND", Value::DInt(5)),
        ("MAIN.IN_RUST.COUNT", Value::DInt(5)),
        ("MAIN.THIRD", Value::DInt(30)),
        ("MAIN.IN_ST.COUNT", Value::DInt(30)),
    ];
    for (name, value) in expected {
        assert_eq!(program.value(name), Some(value), "{name}");
    }
}

#[test]
fn a_rust_block_must_have_each_method_of_its_interfaces() {
    // RU-33: the build checks the Rust block against each prototype of the interfaces it
    // implements, as it checks a Structured Text block (IEC §6.6.6), before anything compiles, so
    // whatever the toolchain: without `count_by`, and with a `count_by` whose input is INT where
    // the prototype's is DINT. Catches a Rust block whose declaration claims an interface its
    // methods do not give, which a call through the interface would reach.
    let program = format!(
        "{I_COUNTER}
PROGRAM Line
VAR
    in_rust : COUNTER;
END_VAR
END_PROGRAM

{TASK}"
    );
    let cases = [
        (
            "rust-pous-interface-missing",
            counter(""),
            "type 'COUNTER' must implement method 'count_by' from interface 'I_COUNTER'",
        ),
        (
            "rust-pous-interface-signature",
            counter(&count_by("i16")),
            "method 'count_by' does not match interface 'I_COUNTER' signature",
        ),
    ];
    for (name, crate_root, message) in cases {
        let root = custom_project(
            name,
            &[("Cargo.toml", &manifest("")), ("src/lib.rs", &crate_root)],
            &program,
        );
        let (error, _) = build_error(&root);
        assert!(
            error.contains(message) && error.contains(DECLARATIONS_FILE),
            "{name}: {error}"
        );
    }
}
