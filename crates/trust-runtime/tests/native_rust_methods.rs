//! Structured Text calls a Rust function block's methods as its own (specification 48 RU-32): the
//! method runs on the instance as the block's body does, its inputs, in-outs, outputs and result
//! reach the caller, and a task counts its budget (RU-47).
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`. A toolchain without
//! cargo and the SDK's sources, as a release's bundle is until specification 48 §16, refuses the
//! build with that reason.

mod rust_pous_support;

use rust_pous_support::{built, custom_project, manifest};
use trust_runtime::error::RuntimeError;
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
