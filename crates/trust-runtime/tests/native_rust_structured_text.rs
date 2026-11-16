//! A Rust POU calls Structured Text (specification 48 §8, RU-38 to RU-42).
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

mod rust_pous_support;

use std::path::PathBuf;

use rust_pous_support::{build_error, built, custom_project, manifest, sources};
use trust_runtime::error::{ExecutionLimitKind, RuntimeError};
use trust_runtime::native::{build_project, load, NoHooks, Options, Target, Toolchain};
use trust_runtime::stdlib::conversions::ConversionProfile;
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
    // RU-38 as built so far (checklist G186): a function with an input of a reference to a
    // function block is refused at the call, naming it, until the bindings pass one. Catches a
    // binding that passes an instance's reference as the address it holds.
    let declarations = "FUNCTION SCALE : LREAL
VAR_INPUT
    x : LREAL;
    target : REF_TO TON;
END_VAR
SCALE := x;
END_FUNCTION
";
    let error = refusal("rust-pous-structured-text-reference", "SCALE", declarations);
    assert!(
        error.contains("st::SCALE at rust/src/lib.rs:9: SCALE has the parameter target of type")
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
    // RU-40: `st::` names the project's Structured Text functions, function blocks and data
    // types. A name of none of them, and the name of a Rust POU, are refused at the call; a
    // function block, which a field holds (RU-39), is refused where no field holds it; a structure
    // with a variable the bindings do not pass yet is refused, naming the variable. Catches a call
    // that reaches the compiler as an unknown item, a Rust POU called through `st::`, a block
    // called as a function, and a binding the compiler finds missing.
    let block = "FUNCTION_BLOCK HOLD
VAR_INPUT
    x : LREAL;
END_VAR
END_FUNCTION_BLOCK
";
    let holder = "TYPE HOLDER :
STRUCT
    R : REF_TO TON;
END_STRUCT
END_TYPE
";
    for (call, declarations, expected) in [
        (
            "NOPE",
            SCALE,
            "st::NOPE at rust/src/lib.rs:9 names no Structured Text function, function block, \
             structure, enumeration or type with named values of the project (specification 48 \
             RU-40)",
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
            "st::HOLD at rust/src/lib.rs:9 names the Structured Text function block HOLD, which \
             a Rust function block or program holds as a field, `name: st::HOLD`, and no field \
             holds (specification 48 RU-38)",
        ),
        (
            "HOLDER",
            holder,
            "st::HOLDER at rust/src/lib.rs:9 names the structure HOLDER, which has the variable R, \
             of a type the bindings do not pass yet (specification 48 RU-39, milestone R3)",
        ),
    ] {
        let error = refusal("rust-pous-structured-text-names", call, declarations);
        assert!(error.contains(expected), "{call}: {error}");
    }
}

#[test]
fn a_crate_that_calls_structured_text_passes_its_structures_in_place() {
    // RU-29 beside RU-39: the crate calls SCALE and passes its structure S_POINT, which lies in
    // layer 1 then, to SHIFT as an in-out; layer 3's own `Places` reads and writes it there. SHIFT
    // moves the point (0, 0) by (1, 2) and returns 1 + 2 = 3, and TWICE(1.5) = 3.0, at the task's
    // first call at 10 ms. Catches a structure's place that only compiles beside the generated
    // structure, which the orphan rule refuses once layer 1 holds it.
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
    assert_eq!(program.value("MAIN.SUM"), Some(Value::LReal(3.0)));
    assert_eq!(program.value("MAIN.POINT.X"), Some(Value::LReal(1.0)));
    assert_eq!(program.value("MAIN.POINT.Y"), Some(Value::LReal(2.0)));
}

/// RU-38's example: a Rust function block that holds the Structured Text block HYSTERESIS.
const GUARD: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

/// Raises its alarm above the high level and drops it below the low one.
#[trust_fb(name = \"TANK_GUARD\")]
#[budget(\"2us\")]
pub struct TankGuard {
    #[input] pub level: f64,
    #[output] pub alarm: bool,
    hysteresis: st::HYSTERESIS,
}

impl Body for TankGuard {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.hysteresis.input.IN = self.level;
        self.hysteresis.input.HIGH = 10.0;
        self.hysteresis.input.LOW = 8.0;
        self.hysteresis.call(cx)?;
        self.alarm = self.hysteresis.output.Q;
        Ok(())
    }
}
";

#[test]
fn a_rust_block_holds_and_calls_a_structured_text_block() {
    // RU-38, RU-39: TANK_GUARD holds the Structured Text block HYSTERESIS as its field, which lies
    // inside TANK_GUARD's instance in the area, and calls it each call with IN := level, HIGH :=
    // 10, LOW := 8. The levels 5, 12, 9 and 7 at the task's calls at 10, 20, 30 and 40 ms give Q
    // FALSE (5 < 10), TRUE (12 > 10), TRUE (9 lies between 8 and 10: the block keeps its state)
    // and FALSE (7 < 8). The instance's own variables are the program's, IN the last level and Q
    // the alarm. Catches a held block that keeps no state from call to call, inputs that do not
    // reach it, and an instance outside the area.
    let root = custom_project(
        "rust-pous-structured-text-held",
        &[
            (
                "Cargo.toml",
                &manifest("trust_st = { path = \"../build/trust_st\" }\n"),
            ),
            ("src/lib.rs", GUARD),
        ],
        "FUNCTION_BLOCK HYSTERESIS
VAR_INPUT
    IN : LREAL;
    HIGH : LREAL;
    LOW : LREAL;
END_VAR
VAR_OUTPUT
    Q : BOOL;
END_VAR
IF IN > HIGH THEN
    Q := TRUE;
ELSIF IN < LOW THEN
    Q := FALSE;
END_IF;
END_FUNCTION_BLOCK

PROGRAM Line
VAR
    guard : TANK_GUARD;
    levels : ARRAY[1..4] OF LREAL := [5.0, 12.0, 9.0, 7.0];
    turn : INT;
    alarm : BOOL;
END_VAR
turn := turn + 1;
guard(level := levels[turn]);
alarm := guard.alarm;
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
    for (time, expected) in [(10, false), (20, true), (30, true), (40, false)] {
        program
            .scan(Duration::from_millis(time), &mut NoHooks)
            .expect("the scan runs");
        assert_eq!(
            program.value("MAIN.ALARM"),
            Some(Value::Bool(expected)),
            "at {time} ms"
        );
    }
    assert_eq!(
        program.value("MAIN.GUARD.HYSTERESIS.IN"),
        Some(Value::LReal(7.0))
    );
    assert_eq!(
        program.value("MAIN.GUARD.HYSTERESIS.Q"),
        Some(Value::Bool(false))
    );
}

/// A crate whose functions call SCALE and ADDRESS_BYTES.
const IMAGES: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_function(name = \"TWICE\")]
#[budget(\"1us\")]
pub fn twice(cx: &mut Cx, x: f64) -> Result<f64, Fault> {
    st::SCALE(cx, x, 2.0)
}

#[trust_function(name = \"WIDTH\")]
#[budget(\"1us\")]
pub fn width(cx: &mut Cx) -> Result<i32, Fault> {
    st::ADDRESS_BYTES(cx)
}
";

/// The program that calls TWICE and WIDTH, after SCALE and ADDRESS_BYTES.
const IMAGES_PROGRAM: &str = "FUNCTION SCALE : LREAL
VAR_INPUT
    x : LREAL;
    factor : LREAL;
END_VAR
SCALE := x * factor;
END_FUNCTION

FUNCTION ADDRESS_BYTES : DINT
VAR
    r : REF_TO INT;
END_VAR
ADDRESS_BYTES := SIZEOF(r);
END_FUNCTION

PROGRAM Line
VAR
    doubled : LREAL;
    bytes : DINT;
END_VAR
doubled := TWICE(x := 1.5);
bytes := WIDTH();
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn layer_one_is_one_text_for_every_image() {
    // RU-39: layer 1 is the same text for every target and image: a debug image's build and a
    // release image's build of one project write the same library, which holds both images'
    // modules, the debug one chosen by the cargo feature `hooks`, and SIZEOF of an address chosen
    // by the target's pointer width, 8 bytes on this 64-bit computer. Each image runs: TWICE(1.5) =
    // 3.0 and WIDTH() = 8 at the task's first call at 10 ms. Catches a layer 1 that changes with
    // the image or the target, which a build of another one or the editor's command would rewrite
    // under a running build.
    let root = custom_project(
        "rust-pous-structured-text-images",
        &[
            (
                "Cargo.toml",
                &manifest("trust_st = { path = \"../build/trust_st\" }\n"),
            ),
            ("src/lib.rs", IMAGES),
        ],
        IMAGES_PROGRAM,
    );
    let library = || {
        std::fs::read_to_string(root.join("build/trust_st/src/lib.rs")).expect("layer 1's library")
    };
    let Some((debug, device)) = built(&root) else {
        return;
    };
    let after_debug = library();
    let toolchain = Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain");
    let release = build_project(
        &root,
        &sources(&root),
        &[],
        &Options {
            target: Target::this_cpu().expect("a computer target"),
            hooks: false,
        },
        ConversionProfile::Iec,
        &toolchain,
        &root.join("build/native"),
    )
    .expect("the release image builds");
    assert_eq!(library(), after_debug, "one layer 1 for both images");
    assert!(after_debug.contains("#[cfg(feature = \"hooks\")]"));
    assert!(after_debug.contains("#[cfg(not(feature = \"hooks\"))]"));
    for image in [&debug.image, &release.image] {
        let mut program = load(image, &device).expect("the image loads");
        program.cold_start(Duration::ZERO).expect("the cold start");
        for time in [0, 10] {
            program
                .scan(Duration::from_millis(time), &mut NoHooks)
                .expect("the scan runs");
        }
        assert_eq!(program.value("MAIN.DOUBLED"), Some(Value::LReal(3.0)));
        assert_eq!(program.value("MAIN.BYTES"), Some(Value::DInt(8)));
    }
}
