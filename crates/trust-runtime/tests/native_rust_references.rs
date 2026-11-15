//! A Rust POU's in-outs and references reach the variables Structured Text gives them
//! (specification 48 RU-29; specification 44 §7.3): a block's in-out keeps the reference of its
//! last call, a function's in-out is the caller's variable itself, a frame variable too, and a
//! reference is read with Structured Text's checks.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`. A toolchain without
//! cargo and the SDK's sources, as a release's bundle is until specification 48 §16, refuses the
//! build with that reason.

mod rust_pous_support;

use rust_pous_support::{built, custom_project, manifest};
use trust_runtime::error::RuntimeError;
use trust_runtime::native::{load, NoHooks};
use trust_runtime::value::{Duration, Value};

/// A block that fills the level it is given, a function that counts up its in-out, and one that
/// reads what a reference names.
const REFERENCES: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

/// Adds its rate to the level it is given, at each call.
#[trust_fb(name = \"FILL\")]
#[budget(\"1us\")]
pub struct Fill {
    #[input]
    pub rate: f64,
    #[in_out]
    pub level: Ref<f64>,
}

impl Body for Fill {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        let level = self.level.get(cx)?;
        self.level.set(cx, level + self.rate)
    }
}

/// Counts up the variable it is given and returns the new count.
#[trust_function(name = \"BUMP\")]
#[budget(\"1us\")]
pub fn bump(cx: &mut Cx, count: InOut<'_, i16>) -> Result<i16, Fault> {
    let next = checked::add(cx, count.get(), 1)?;
    count.set(next);
    Ok(next)
}

/// The value a reference names, and one.
#[trust_function(name = \"NEXT_OF\")]
#[budget(\"1us\")]
pub fn next_of(cx: &mut Cx, source: Ref<i16>) -> Result<i16, Fault> {
    let value = source.get(cx)?;
    checked::add(cx, value, 1)
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
fn in_outs_and_references_reach_the_variables_structured_text_gives() {
    // RU-29: three calls at 10, 20 and 30 ms (the task is not due at 0, specification 10 §4.3).
    // FILL's in-out is the program's tank: 1.0 + 3 × 2.5 = 8.5. BUMP counts the program's counter
    // up to 3 and returns it, and counts TWICE's own local, a frame variable, from 40: the second
    // call sees the first's 41 and returns 42. NEXT_OF reads the counter through REF(counter)
    // after this scan's BUMP: 3 + 1 = 4. Catches an in-out that works on a copy, a block that loses
    // its reference, a function in-out that cannot name a frame variable, and a reference read at
    // the wrong place.
    let program = format!(
        "FUNCTION TWICE : INT
VAR_INPUT
    start : INT;
END_VAR
VAR
    local : INT;
    first : INT;
END_VAR
local := start;
first := BUMP(count := local);
TWICE := BUMP(count := local);
END_FUNCTION

PROGRAM Line
VAR
    filler : FILL;
    tank : LREAL := 1.0;
    counter : INT;
    bumped : INT;
    doubled : INT;
    next : INT;
END_VAR
filler(rate := 2.5, level := tank);
bumped := BUMP(count := counter);
doubled := TWICE(start := 40);
next := NEXT_OF(source := REF(counter));
END_PROGRAM

{TASK}"
    );
    let root = custom_project(
        "rust-pous-references",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", REFERENCES)],
        &program,
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    for time in [0, 10, 20, 30] {
        program
            .scan(Duration::from_millis(time), &mut NoHooks)
            .expect("the scan runs");
    }
    assert_eq!(program.value("MAIN.TANK"), Some(Value::LReal(8.5)));
    assert_eq!(program.value("MAIN.COUNTER"), Some(Value::Int(3)));
    assert_eq!(program.value("MAIN.BUMPED"), Some(Value::Int(3)));
    assert_eq!(program.value("MAIN.DOUBLED"), Some(Value::Int(42)));
    assert_eq!(program.value("MAIN.NEXT"), Some(Value::Int(4)));
}

#[test]
fn a_null_reference_ends_the_scan_as_structured_texts_does() {
    // RU-29 and specification 44 §6.5: reading through NULL ends the scan with `NullReference`,
    // and the statement after the call never runs. Catches a reference read without its check.
    let program = format!(
        "PROGRAM Line
VAR
    nothing : REF_TO INT;
    next : INT;
    after : BOOL;
END_VAR
next := NEXT_OF(source := nothing);
after := TRUE;
END_PROGRAM

{TASK}"
    );
    let root = custom_project(
        "rust-pous-null-reference",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", REFERENCES)],
        &program,
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    let error = program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect_err("the NULL ends the scan");
    assert_eq!(error, RuntimeError::NullReference);
    assert_eq!(program.value("MAIN.AFTER"), Some(Value::Bool(false)));
}
