//! A Rust POU calls Structured Text (specification 48 §8, RU-38 to RU-42).
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

mod rust_pous_support;

use rust_pous_support::{build_error, custom_project, manifest};

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

/// The error of building a project whose crate calls `st::{call}` and whose Structured Text is
/// `declarations` with the program.
fn refusal(name: &str, call: &str, declarations: &str) -> String {
    let crate_root = caller(call);
    let root = custom_project(
        name,
        &[
            (
                "Cargo.toml",
                &manifest("trust_st = { path = \"../build/trust_st\" }\n"),
            ),
            ("src/lib.rs", &crate_root),
        ],
        &program(declarations),
    );
    build_error(&root).0
}

#[test]
fn a_call_of_structured_text_from_rust_is_refused_until_the_layers_are_built() {
    // RU-38 to RU-42 as built so far (checklist G186): the build generates the Structured Text the
    // crate calls and checks what it reaches, then refuses the call, naming it, until it compiles
    // layer 1 (milestone R3). Catches a crate whose `st::` would fail in the compiler instead,
    // without saying why.
    let error = refusal("rust-pous-structured-text", "SCALE", SCALE);
    assert!(
        error.contains(
            "the crate calls Structured Text (st::SCALE at rust/src/lib.rs:9), which the build \
             does not compile yet (specification 48 §8, milestone R3)"
        ),
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
