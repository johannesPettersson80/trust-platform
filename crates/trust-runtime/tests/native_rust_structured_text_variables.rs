//! A Rust POU's own variables of the project's Structured Text types (specification 48 RU-25,
//! RU-39): an input, an output and an array of a structure's or an enumeration's binding, and a held
//! block refused by rustc where a value lies.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

mod rust_pous_support;

use rust_pous_support::{build_error, built, custom_project, manifest};
use trust_runtime::native::{load, NoHooks};
use trust_runtime::value::{Duration, Value};

/// A crate whose block takes a POINT, gives a POINT and a MODE and keeps two POINTs.
const TRACER: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_fb(name = \"TRACER\")]
#[budget(\"2us\")]
pub struct Tracer {
    #[input] pub spot: st::POINT,
    #[output] pub last: st::POINT,
    #[output] pub mode: st::MODE,
    #[output] pub count: i32,
    trail: Array<st::POINT, 1, 2>,
}

impl Body for Tracer {
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        self.last = st::POINT { X: self.spot.X * 2.0, Y: self.spot.Y };
        self.mode = if self.spot.Y > 3.0 { st::MODE::RUN } else { st::MODE::IDLE };
        let kept = self.trail.iter().filter(|point| point.Y == 2.0).count();
        self.count = i32::try_from(kept).unwrap_or(-1);
        Ok(())
    }
}
";

/// The Structured Text of POINT and MODE and of a program that runs `body`.
fn types_and(body: &str) -> String {
    format!(
        "TYPE POINT :
STRUCT
    X : LREAL;
    Y : LREAL := 2.0;
END_STRUCT
END_TYPE

TYPE MODE : (IDLE, RUN := 5) := RUN;
END_TYPE

FUNCTION_BLOCK HYSTERESIS
VAR_INPUT
    IN : LREAL;
END_VAR
END_FUNCTION_BLOCK

{body}

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
"
    )
}

#[test]
fn a_rust_pou_holds_values_of_the_projects_structured_text_types() {
    // RU-25, RU-39: a Rust block's input, outputs and own array may be of the project's
    // Structured Text structure POINT and enumeration MODE, as their bindings: TRACER takes
    // (1.5, 4.0), gives LAST (3.0, 4.0) and MODE RUN, as Y > 3.0, and its TRAIL's two POINTs start
    // at POINT's initial values, Y 2.0 each, so COUNT is 2, at the task's first call at 10 ms.
    // Catches a variable of a Structured Text type refused, a structure carried through the
    // crate's places, and an array's elements not at their type's initial values.
    let root = custom_project(
        "rust-pous-structured-text-variables",
        &[
            (
                "Cargo.toml",
                &manifest("trust_st = { path = \"../build/trust_st\" }\n"),
            ),
            ("src/lib.rs", TRACER),
        ],
        &types_and(
            "PROGRAM Line
VAR
    tracer : TRACER;
    start : POINT := (X := 1.5, Y := 4.0);
    seen_x : LREAL;
    running : BOOL;
    count : DINT;
END_VAR
tracer(spot := start);
seen_x := tracer.last.X;
running := tracer.mode = RUN;
count := tracer.count;
END_PROGRAM",
        ),
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
    assert_eq!(program.value("MAIN.SEEN_X"), Some(Value::LReal(3.0)));
    assert_eq!(program.value("MAIN.RUNNING"), Some(Value::Bool(true)));
    assert_eq!(program.value("MAIN.COUNT"), Some(Value::DInt(2)));
    assert_eq!(program.value("MAIN.TRACER.LAST.Y"), Some(Value::LReal(4.0)));
}

/// A crate whose block keeps a reference to a held block.
const HELD_INPUT: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_fb(name = \"GUARD\")]
#[budget(\"2us\")]
pub struct Guard {
    r: Ref<st::HYSTERESIS>,
}

impl Body for Guard {
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        Ok(())
    }
}
";

#[test]
fn a_held_block_where_a_value_lies_is_refused_by_rustc() {
    // RU-39: a Structured Text function block a Rust POU holds is its own variable; a reference to
    // one is refused by rustc, which finds its binding no `trust_plc::Value`, with the rule (as an
    // input the native build refuses it first, NX-09). Catches a held block taken where a value
    // lies, which the bindings cannot read or write.
    let root = custom_project(
        "rust-pous-structured-text-held-input",
        &[
            (
                "Cargo.toml",
                &manifest("trust_st = { path = \"../build/trust_st\" }\n"),
            ),
            ("src/lib.rs", HELD_INPUT),
        ],
        &types_and(
            "PROGRAM Line
VAR
    guard : GUARD;
END_VAR
guard();
END_PROGRAM",
        ),
    );
    let (error, _) = build_error(&root);
    assert!(
        error.contains("is a Structured Text function block or class, which a Rust POU holds as its own variable"),
        "{error}"
    );
}
