//! A Rust POU passes its in-outs, outputs and references through a call of Structured Text and a
//! held block (specification 48 RU-29, RU-39): an in-out as `InOut<'_, T>`, which the function's
//! writes reach also when it faults, an output as `&mut T`, which takes the function's output when
//! it returns, and a reference as `Ref<T>`, which names the variable itself.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

mod rust_pous_support;

use rust_pous_support::{built, custom_project, manifest};
use trust_runtime::error::RuntimeError;
use trust_runtime::native::{load, Build, Device, NoHooks};
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

/// A crate whose block passes the reference of its in-out to Structured Text functions and a held
/// watcher.
const REFERENCES: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_fb(name = \"BUMPER\")]
#[budget(\"2us\")]
pub struct Bumper {
    #[in_out] pub counter: Ref<i32>,
    #[output] pub after: i32,
    #[output] pub picked: i32,
    #[output] pub seen: i32,
    #[output] pub again: i32,
    watch: st::WATCH,
}

impl Body for Bumper {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.after = st::BUMP_AT(cx, self.counter, 5)?;
        let picked = st::PICK(cx, Ref::NULL, self.counter, true)?;
        self.picked = picked.get(cx)? * 10;
        self.watch.input.SOURCE = self.counter;
        self.watch.call(cx)?;
        self.seen = self.watch.output.SEEN;
        self.again = self.watch.output.LAST.get(cx)?;
        Ok(())
    }
}
";

#[test]
fn references_pass_through_calls_and_held_blocks() {
    // RU-25, RU-29, RU-39: a `REF_TO DINT` passes as `Ref<i32>`, naming the variable itself:
    // BUMP_AT adds 5 to the program's n through the block's in-out, 10 + 5 = 15, and gives 15;
    // PICK gives back its second reference, n's, read as 15 x 10 = 150; the held WATCH reads 15
    // through its input SOURCE and gives SOURCE back as LAST, read as 15, at the task's first call
    // at 10 ms. Catches a reference passed as a copy of the value (n stays 10), a result that names
    // nothing, and a held block's reference left in the generated cells.
    let root = custom_project(
        "rust-pous-structured-text-references",
        &[
            (
                "Cargo.toml",
                &manifest("trust_st = { path = \"../build/trust_st\" }\n"),
            ),
            ("src/lib.rs", REFERENCES),
        ],
        "FUNCTION BUMP_AT : DINT
VAR_INPUT
    TARGET : REF_TO DINT;
    AMOUNT : DINT;
END_VAR
TARGET^ := TARGET^ + AMOUNT;
BUMP_AT := TARGET^;
END_FUNCTION

FUNCTION PICK : REF_TO DINT
VAR_INPUT
    FIRST : REF_TO DINT;
    SECOND : REF_TO DINT;
    TAKE_SECOND : BOOL;
END_VAR
IF TAKE_SECOND THEN
    PICK := SECOND;
ELSE
    PICK := FIRST;
END_IF
END_FUNCTION

FUNCTION_BLOCK WATCH
VAR_INPUT
    SOURCE : REF_TO DINT;
END_VAR
VAR_OUTPUT
    SEEN : DINT;
    LAST : REF_TO DINT;
END_VAR
IF SOURCE <> NULL THEN
    SEEN := SOURCE^;
END_IF
LAST := SOURCE;
END_FUNCTION_BLOCK

PROGRAM Line
VAR
    n : DINT := 10;
    bumper : BUMPER;
END_VAR
bumper(counter := n);
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
    assert_eq!(program.value("MAIN.N"), Some(Value::DInt(15)));
    assert_eq!(program.value("MAIN.BUMPER.AFTER"), Some(Value::DInt(15)));
    assert_eq!(program.value("MAIN.BUMPER.PICKED"), Some(Value::DInt(150)));
    assert_eq!(program.value("MAIN.BUMPER.SEEN"), Some(Value::DInt(15)));
    assert_eq!(program.value("MAIN.BUMPER.AGAIN"), Some(Value::DInt(15)));
}

/// A crate whose blocks hold a Structured Text tally and give its in-out their own in-out, or
/// nothing.
const HELD_IN_OUTS: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_fb(name = \"COUNTER\")]
#[budget(\"2us\")]
pub struct Counter {
    #[in_out] pub sum: Ref<i32>,
    tally: st::TALLY,
}

impl Body for Counter {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.tally.input.TOTAL = self.sum;
        self.tally.input.ADD = 7;
        self.tally.call(cx)
    }
}

#[trust_fb(name = \"UNBOUND\")]
#[budget(\"2us\")]
pub struct Unbound {
    tally: st::TALLY,
}

impl Body for Unbound {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.tally.input.ADD = 1;
        self.tally.call(cx)
    }
}
";

/// A project of `HELD_IN_OUTS` whose program runs `body` each cycle.
fn held_in_outs(name: &str, body: &str) -> Option<(Build, Device)> {
    let root = custom_project(
        name,
        &[
            (
                "Cargo.toml",
                &manifest("trust_st = { path = \"../build/trust_st\" }\n"),
            ),
            ("src/lib.rs", HELD_IN_OUTS),
        ],
        &format!(
            "FUNCTION_BLOCK TALLY
VAR_IN_OUT
    TOTAL : DINT;
END_VAR
VAR_INPUT
    ADD : DINT;
END_VAR
TOTAL := TOTAL + ADD;
END_FUNCTION_BLOCK

PROGRAM Line
VAR
    n : DINT := 1;
    counter : COUNTER;
    unbound : UNBOUND;
END_VAR
{body}
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
    built(&root)
}

#[test]
fn a_held_blocks_in_out_is_a_reference_its_call_gives_it() {
    // RU-29, RU-39, specification 44 §7.3: a held block's in-out is a field of `input` of type
    // `Ref<T>`: COUNTER gives TALLY the reference of its own in-out, the program's n, so TALLY adds
    // 7 to n itself, 1 + 7 = 8, at the task's first call at 10 ms; a block whose in-out is left
    // NULL ends the scan with `NullReference` where it reads it. Catches an in-out refused, one
    // passed as a copy of its value (n stays 1), and a NULL read as zero.
    let Some((built, device)) = held_in_outs(
        "rust-pous-structured-text-held-in-out",
        "counter(sum := n);",
    ) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    for time in [0, 10] {
        program
            .scan(Duration::from_millis(time), &mut NoHooks)
            .expect("the scan runs");
    }
    assert_eq!(program.value("MAIN.N"), Some(Value::DInt(8)));
    assert_eq!(
        program.value("MAIN.COUNTER.TALLY.ADD"),
        Some(Value::DInt(7))
    );
    let Some((built, device)) =
        held_in_outs("rust-pous-structured-text-held-in-out-null", "unbound();")
    else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    program
        .scan(Duration::ZERO, &mut NoHooks)
        .expect("no task is due at the start");
    let error = program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect_err("the NULL ends the scan");
    assert_eq!(error, RuntimeError::NullReference);
}
