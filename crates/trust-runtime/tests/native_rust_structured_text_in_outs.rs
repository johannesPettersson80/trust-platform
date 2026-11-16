//! A Rust POU passes its in-outs and outputs through a call of Structured Text (specification 48
//! RU-29, RU-39): an in-out as `InOut<'_, T>`, which the function's writes reach also when it
//! faults, and an output as `&mut T`, which takes the function's output when it returns.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

mod rust_pous_support;

use rust_pous_support::{built, custom_project, manifest};
use trust_runtime::error::RuntimeError;
use trust_runtime::native::{load, NoHooks};
use trust_runtime::value::{Duration, Value};

/// A crate whose function passes its in-out and an output of its own to Structured Text.
const IN_OUTS: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_function(name = \"ADD_TO\")]
#[budget(\"1us\")]
pub fn add_to(cx: &mut Cx, sum: InOut<'_, f64>, x: f64) -> Result<f64, Fault> {
    let mut doubled = 0.0;
    let total = st::ACCUMULATE(cx, sum, x, &mut doubled)?;
    Ok(total + doubled)
}
";

#[test]
fn in_outs_and_outputs_pass_through_a_call() {
    // RU-29, RU-38, RU-39: a call passes a function's in-out as RU-29's `InOut<'_, T>` and its
    // output as `&mut T`: ACCUMULATE adds x = 2.5 to the in-out total, 10.0 + 2.5 = 12.5, which the
    // program's variable then holds, writes its output doubled = 2.5 x 2 = 5.0 and returns 12.5, so
    // ADD_TO gives 12.5 + 5.0 = 17.5, at the task's first call at 10 ms. Catches an in-out that the
    // call reads but never writes back, and an output left in the generated cells.
    let root = custom_project(
        "rust-pous-structured-text-in-outs",
        &[
            (
                "Cargo.toml",
                &manifest("trust_st = { path = \"../build/trust_st\" }\n"),
            ),
            ("src/lib.rs", IN_OUTS),
        ],
        "FUNCTION ACCUMULATE : LREAL
VAR_IN_OUT
    total : LREAL;
END_VAR
VAR_INPUT
    x : LREAL;
END_VAR
VAR_OUTPUT
    doubled : LREAL;
END_VAR
total := total + x;
doubled := x * 2.0;
ACCUMULATE := total;
END_FUNCTION

PROGRAM Line
VAR
    sum : LREAL := 10.0;
    result : LREAL;
END_VAR
result := ADD_TO(sum := sum, x := 2.5);
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
",
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    for time in [0, 10] {
        program
            .scan(Duration::from_millis(time), &mut NoHooks)
            .expect("the scan runs");
    }
    assert_eq!(program.value("MAIN.SUM"), Some(Value::LReal(12.5)));
    assert_eq!(program.value("MAIN.RESULT"), Some(Value::LReal(17.5)));
}

/// A crate whose function passes its in-out to a Structured Text function that faults after
/// writing it.
const FAULTING_IN_OUT: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_function(name = \"BUMPED\")]
#[budget(\"1us\")]
pub fn bumped(cx: &mut Cx, count: InOut<'_, i32>, divisor: i32) -> Result<i32, Fault> {
    st::BUMP(cx, count, divisor)
}
";

#[test]
fn an_in_out_keeps_what_a_faulting_function_wrote() {
    // RU-29, RU-39: a write through an in-out is the caller's variable's at once, so a fault after
    // it leaves it written: BUMP adds 1 to the program's n, 5 + 1 = 6, then divides 100 by 0, whose
    // division by zero ends the scan at the task's first call at 10 ms with n at 6 and r, which
    // the call never assigns, at its -1. Catches an in-out written back only when the function
    // returns, which leaves n at 5.
    let root = custom_project(
        "rust-pous-structured-text-faulting-in-out",
        &[
            (
                "Cargo.toml",
                &manifest("trust_st = { path = \"../build/trust_st\" }\n"),
            ),
            ("src/lib.rs", FAULTING_IN_OUT),
        ],
        "FUNCTION BUMP : DINT
VAR_IN_OUT
    count : DINT;
END_VAR
VAR_INPUT
    divisor : DINT;
END_VAR
count := count + 1;
BUMP := 100 / divisor;
END_FUNCTION

PROGRAM Line
VAR
    n : DINT := 5;
    d : DINT;
    r : DINT := -1;
END_VAR
r := BUMPED(count := n, divisor := d);
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
",
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    program
        .scan(Duration::ZERO, &mut NoHooks)
        .expect("no task is due at the start");
    let error = program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect_err("the division by zero ends the scan");
    assert!(matches!(error, RuntimeError::DivisionByZero), "{error:?}");
    assert_eq!(program.value("MAIN.N"), Some(Value::DInt(6)));
    assert_eq!(program.value("MAIN.R"), Some(Value::DInt(-1)));
}
