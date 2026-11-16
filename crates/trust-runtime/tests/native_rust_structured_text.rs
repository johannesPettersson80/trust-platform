//! A Rust POU calls Structured Text (specification 48 §8, RU-38 to RU-42).
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

mod rust_pous_support;

use rust_pous_support::{build_error, custom_project, manifest};

/// A crate whose function calls the Structured Text function SCALE at line 9 of its root.
const CALLER: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_function(name = \"TWICE\")]
#[budget(\"1us\")]
pub fn twice(cx: &mut Cx, x: f64) -> Result<f64, Fault> {
    st::SCALE(cx, x, 2.0)
}
";

#[test]
fn a_call_of_structured_text_from_rust_is_refused_until_the_layers_are_built() {
    // RU-38 to RU-42 as built so far (checklist G186): the build reads what the crate calls under
    // `st::` and refuses it, naming the call, until it builds layer 1 (milestone R3). Catches a
    // crate whose `st::` would fail in the compiler instead, without saying why.
    let root = custom_project(
        "rust-pous-structured-text",
        &[
            (
                "Cargo.toml",
                &manifest("trust_st = { path = \"../build/trust_st\" }\n"),
            ),
            ("src/lib.rs", CALLER),
        ],
        "FUNCTION SCALE : LREAL
VAR_INPUT
    x : LREAL;
    factor : LREAL;
END_VAR
SCALE := x * factor;
END_FUNCTION

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
",
    );
    let (error, _) = build_error(&root);
    assert!(
        error.contains(
            "the crate calls Structured Text (st::SCALE at rust/src/lib.rs:9), which the build \
             does not compile yet (specification 48 §8, milestone R3)"
        ),
        "{error}"
    );
}
