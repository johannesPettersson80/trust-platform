//! A Rust POU calls Structured Text (specification 48 §8, RU-38 to RU-42).
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

mod rust_pous_support;

use std::path::PathBuf;

use rust_pous_support::{build_error, built, custom_project, manifest};
use trust_runtime::error::{ExecutionLimitKind, RuntimeError};
use trust_runtime::native::{load, NoHooks};
use trust_runtime::value::{Duration, Value};

/// A crate whose function TWICE calls `st::{call}` at line 9 of its root.
fn caller(call: &str) -> String {
    format!(
        "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_function(name = \"TWICE\")]
#[budget(\"1us\")]
pub fn twice(cx: &mut Cx, x: f64) -> Result<f64, Fault> {{
    st::{call}(cx, x, 2.0)
}}
"
    )
}

/// A program that calls the Rust function TWICE, after the Structured Text `declarations`.
fn program(declarations: &str) -> String {
    format!(
        "{declarations}
PROGRAM Line
VAR
    doubled : LREAL;
END_VAR
doubled := TWICE(x := 1.5);
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
"
    )
}

/// The Structured Text function SCALE, which multiplies `x` by `factor`.
const SCALE: &str = "FUNCTION SCALE : LREAL
VAR_INPUT
    x : LREAL;
    factor : LREAL;
END_VAR
SCALE := x * factor;
END_FUNCTION
";

/// A project whose crate calls `st::{call}` and whose Structured Text is `declarations` with the
/// program.
fn project(name: &str, call: &str, declarations: &str) -> PathBuf {
    let crate_root = caller(call);
    custom_project(
        name,
        &[
            (
                "Cargo.toml",
                &manifest("trust_st = { path = \"../build/trust_st\" }\n"),
            ),
            ("src/lib.rs", &crate_root),
        ],
        &program(declarations),
    )
}

/// The error of building [`project`].
fn refusal(name: &str, call: &str, declarations: &str) -> String {
    build_error(&project(name, call, declarations)).0
}

#[test]
fn a_rust_function_calls_a_structured_text_function() {
    // RU-38, RU-39: the Rust function TWICE returns `st::SCALE(cx, x, 2.0)`, the Structured Text
    // function SCALE := x * factor; the program's call TWICE(x := 1.5) gives 1.5 × 2.0 = 3.0 at the
    // task's first call at 10 ms (specification 10 §4.3). The build writes layer 1 into the
    // project's build/trust_st, where the crate's manifest names it. Catches a binding that passes
    // the arguments in another order or loses the result, and layer 1 written where the crate does
    // not find it.
    let root = project("rust-pous-structured-text", "SCALE", SCALE);
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
    assert_eq!(program.value("MAIN.DOUBLED"), Some(Value::LReal(3.0)));
    let library =
        std::fs::read_to_string(root.join("build/trust_st/src/lib.rs")).expect("layer 1's library");
    assert!(library.contains("pub fn SCALE(cx: &mut trust_plc::Cx, x: f64, factor: f64)"));
    // The folder keeps itself out of version control.
    assert_eq!(
        std::fs::read_to_string(root.join("build/trust_st/.gitignore")).ok(),
        Some("*\n".to_string())
    );
}

#[test]
fn structured_text_that_rust_calls_spends_the_scans_budget() {
    // RU-39, specification 44 §8.4: the statements of the Structured Text that a Rust POU calls
    // count against the scan's budget as any statement does: a loop of a million turns in SUM,
    // which TWICE calls, ends the scan with the work limit of the device's 1,000 statements, as
    // the same loop in a program does. Catches Structured Text run on a context whose statements
    // the kernel never counts.
    let declarations = "FUNCTION SUM : LREAL
VAR_INPUT
    x : LREAL;
    factor : LREAL;
END_VAR
VAR
    i : DINT;
END_VAR
SUM := 0.0;
FOR i := 1 TO 1000000 DO
    SUM := SUM + x * factor;
END_FOR;
END_FUNCTION
";
    let root = project("rust-pous-structured-text-budget", "SUM", declarations);
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
        .expect_err("the loop ends the scan");
    assert!(
        matches!(
            error,
            RuntimeError::ExecutionLimit {
                kind: ExecutionLimitKind::Work,
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn a_call_that_passes_what_rust_does_not_pass_yet_is_refused() {
    // RU-38 as built so far (checklist G186): a call passes elementary inputs and an elementary
    // result; a function with a STRING input is refused at the call, naming it. Catches a binding
    // that passes a string's cells as a number.
    let declarations = "FUNCTION SCALE : LREAL
VAR_INPUT
    x : LREAL;
    unit : STRING;
END_VAR
SCALE := x;
END_FUNCTION
";
    let error = refusal("rust-pous-structured-text-string", "SCALE", declarations);
    assert!(
        error.contains("st::SCALE at rust/src/lib.rs:9: SCALE has the parameter unit of type")
            && error.contains("which a call from Rust does not pass yet"),
        "{error}"
    );
}

#[test]
fn structured_text_that_rust_calls_cannot_call_rust() {
    // RU-40: the crate's TWICE calls the Structured Text SCALE, which calls HALF, which calls
    // TWICE back: layer 1 would call the crate it is compiled before. The build refuses it,
    // naming the call and every POU on the way. Catches a cycle found only by the compiler, and a
    // chain without the POU between.
    let declarations = "FUNCTION SCALE : LREAL
VAR_INPUT
    x : LREAL;
    factor : LREAL;
END_VAR
SCALE := HALF(x := x) * factor;
END_FUNCTION

FUNCTION HALF : LREAL
VAR_INPUT
    x : LREAL;
END_VAR
HALF := TWICE(x := x) / 4.0;
END_FUNCTION
";
    let error = refusal("rust-pous-structured-text-cycle", "SCALE", declarations);
    assert!(
        error.contains(
            "st::SCALE at rust/src/lib.rs:9 calls SCALE, which calls HALF, which calls TWICE \
             (Rust): Structured Text that Rust calls is compiled before the crate, so it cannot \
             call Rust (specification 48 RU-40)"
        ),
        "{error}"
    );
}

#[test]
fn a_call_of_what_is_no_structured_text_function_is_refused() {
    // RU-38: `st::` names the project's Structured Text functions and function blocks. A name of
    // none of them, and the name of a Rust POU, are refused at the call; a function block, which
    // Rust holds (RU-39), is refused until the build compiles it for Rust. Catches a call that
    // reaches the compiler as an unknown item, and a Rust POU called through `st::`.
    let block = "FUNCTION_BLOCK HOLD
VAR_INPUT
    x : LREAL;
END_VAR
END_FUNCTION_BLOCK
";
    for (call, declarations, expected) in [
        (
            "NOPE",
            SCALE,
            "st::NOPE at rust/src/lib.rs:9 names no Structured Text function or function block \
             of the project (specification 48 RU-38)",
        ),
        (
            "TWICE",
            SCALE,
            "st::TWICE at rust/src/lib.rs:9 names a Rust POU, which the crate calls as Rust \
             (specification 48 RU-38)",
        ),
        (
            "HOLD",
            block,
            "st::HOLD at rust/src/lib.rs:9 names a Structured Text function block, which the \
             build does not compile for Rust yet (specification 48 RU-38, milestone R3)",
        ),
    ] {
        let error = refusal("rust-pous-structured-text-names", call, declarations);
        assert!(error.contains(expected), "{call}: {error}");
    }
}

#[test]
fn a_structure_in_place_beside_a_call_of_structured_text_is_refused() {
    // RU-29 beside RU-39 as built so far (checklist G186): the place of a crate's structure that an
    // in-out passes is the generated structure's `trust_plc::Place`, which lies in layer 1 once the
    // crate calls Structured Text, where layer 3 cannot implement it; the build refuses the two
    // together, naming both, until milestone R3 compiles them. Catches the compiler's orphan error
    // in place of the reason.
    let crate_root = format!(
        "{}
/// A point of the plane.
#[trust_struct(name = \"S_POINT\")]
#[derive(Clone, Copy)]
pub struct Point {{
    pub x: f64,
    pub y: f64,
}}

/// Moves the point by (dx, dy) and returns its new x + y.
#[trust_function(name = \"SHIFT\")]
#[budget(\"1us\")]
pub fn shift(_cx: &mut Cx, point: InOut<'_, Point>, dx: f64, dy: f64) -> Result<f64, Fault> {{
    let mut moved = point.get();
    moved.x += dx;
    moved.y += dy;
    point.set(moved);
    Ok(moved.x + moved.y)
}}
",
        caller("SCALE")
    );
    let root = custom_project(
        "rust-pous-structured-text-places",
        &[
            (
                "Cargo.toml",
                &manifest("trust_st = { path = \"../build/trust_st\" }\n"),
            ),
            ("src/lib.rs", &crate_root),
        ],
        &format!(
            "{SCALE}
PROGRAM Line
VAR
    doubled : LREAL;
    point : S_POINT;
    sum : LREAL;
END_VAR
doubled := TWICE(x := 1.5);
sum := SHIFT(point := point, dx := 1.0, dy := 2.0);
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
"
        ),
    );
    let error = build_error(&root).0;
    assert!(
        error.contains(
            "the crate calls Structured Text (st::SCALE at rust/src/lib.rs:9) and passes its \
             structure S_POINT as an in-out or a reference, which the build does not compile \
             together yet (specification 48 RU-29, RU-39, milestone R3)"
        ),
        "{error}"
    );
}
