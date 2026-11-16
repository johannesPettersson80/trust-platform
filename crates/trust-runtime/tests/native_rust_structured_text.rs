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

/// A crate whose functions pass a TIME and a BYTE to Structured Text and back.
const TYPED: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_function(name = \"TICK\")]
#[budget(\"1us\")]
pub fn tick(cx: &mut Cx, start: Time) -> Result<Time, Fault> {
    st::LATER(cx, start, Time::from_nanos(10_000_000))
}

#[trust_function(name = \"NIBBLE\")]
#[budget(\"1us\")]
pub fn nibble(cx: &mut Cx, value: Byte) -> Result<Byte, Fault> {
    st::LOW_NIBBLE(cx, value)
}
";

#[test]
fn a_call_passes_rust_types_of_structured_text_types() {
    // RU-25, RU-39: a binding takes and gives the Rust type a Rust POU writes for each
    // elementary type, `Time` for TIME and `Byte` for BYTE: LATER(T#1s, 10 ms) is T#1s + T#10ms =
    // T#1010ms, and LOW_NIBBLE(16#A7) is 16#A7 AND 16#0F = 16#07, at the task's first call at 10
    // ms. Catches a value passed as its cell's raw form in the wrong unit, and a result left in
    // the generated code's form.
    let root = custom_project(
        "rust-pous-structured-text-types",
        &[
            (
                "Cargo.toml",
                &manifest("trust_st = { path = \"../build/trust_st\" }\n"),
            ),
            ("src/lib.rs", TYPED),
        ],
        "FUNCTION LATER : TIME
VAR_INPUT
    start : TIME;
    delta : TIME;
END_VAR
LATER := start + delta;
END_FUNCTION

FUNCTION LOW_NIBBLE : BYTE
VAR_INPUT
    value : BYTE;
END_VAR
LOW_NIBBLE := value AND 16#0F;
END_FUNCTION

PROGRAM Line
VAR
    next : TIME;
    low : BYTE;
END_VAR
next := TICK(start := T#1s);
low := NIBBLE(value := 16#A7);
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
    assert_eq!(
        program.value("MAIN.NEXT"),
        Some(Value::Time(Duration::from_millis(1_010)))
    );
    assert_eq!(program.value("MAIN.LOW"), Some(Value::Byte(0x07)));
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
    // RU-38 as built so far (checklist G186): a call passes elementary, string and one-dimensional
    // array inputs and results; a function with a two-dimensional array input is refused at the
    // call, naming it. Catches a binding that passes a matrix's cells as a row of rows.
    let declarations = "FUNCTION SCALE : LREAL
VAR_INPUT
    x : LREAL;
    factors : ARRAY[1..2, 1..2] OF LREAL;
END_VAR
SCALE := x;
END_FUNCTION
";
    let error = refusal("rust-pous-structured-text-matrix", "SCALE", declarations);
    assert!(
        error.contains("st::SCALE at rust/src/lib.rs:9: SCALE has the parameter factors of type")
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
    // a field holds (RU-39), is refused where no field holds it. Catches a call that reaches the
    // compiler as an unknown item, a Rust POU called through `st::`, and a block called as a
    // function.
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
            "st::HOLD at rust/src/lib.rs:9 names the Structured Text function block HOLD, which \
             a Rust function block or program holds as a field, `name: st::HOLD`, and no field \
             holds (specification 48 RU-38)",
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

/// A Rust function block that holds a Structured Text block of a TIME and a BYTE.
const STRETCHER: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_fb(name = \"STRETCHER\")]
#[budget(\"2us\")]
pub struct Stretcher {
    #[input] pub pt: Time,
    #[output] pub doubled: Time,
    #[output] pub low: Byte,
    stretch: st::STRETCH,
}

impl Body for Stretcher {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.stretch.input.PT = self.pt;
        self.stretch.input.BITS = Byte(0xA7);
        self.stretch.call(cx)?;
        self.doubled = self.stretch.output.DOUBLED;
        self.low = self.stretch.output.LOW;
        Ok(())
    }
}
";

#[test]
fn a_held_block_passes_rust_types_of_structured_text_types() {
    // RU-25, RU-39: a held block's inputs and outputs are the Rust types a Rust POU writes for
    // them, `Time` for TIME and `Byte` for BYTE: STRETCH gives PT + PT = T#250ms + T#250ms =
    // T#500ms and 16#A7 AND 16#0F = 16#07 at the task's first call at 10 ms, and keeps PT, the
    // input the Rust block gave it. Catches an input or an output passed as its cell's raw form.
    let root = custom_project(
        "rust-pous-structured-text-held-types",
        &[
            (
                "Cargo.toml",
                &manifest("trust_st = { path = \"../build/trust_st\" }\n"),
            ),
            ("src/lib.rs", STRETCHER),
        ],
        "FUNCTION_BLOCK STRETCH
VAR_INPUT
    PT : TIME;
    BITS : BYTE;
END_VAR
VAR_OUTPUT
    DOUBLED : TIME;
    LOW : BYTE;
END_VAR
DOUBLED := PT + PT;
LOW := BITS AND 16#0F;
END_FUNCTION_BLOCK

PROGRAM Line
VAR
    s : STRETCHER;
    doubled : TIME;
    low : BYTE;
END_VAR
s(pt := T#250ms);
doubled := s.doubled;
low := s.low;
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
    assert_eq!(
        program.value("MAIN.DOUBLED"),
        Some(Value::Time(Duration::from_millis(500)))
    );
    assert_eq!(program.value("MAIN.LOW"), Some(Value::Byte(0x07)));
    assert_eq!(
        program.value("MAIN.S.STRETCH.PT"),
        Some(Value::Time(Duration::from_millis(250)))
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

/// A crate whose function greets through Structured Text and whose block holds a tagger.
const TEXTS: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_function(name = \"WELCOME\")]
#[budget(\"1us\")]
pub fn welcome(cx: &mut Cx, who: Str<10>) -> Result<Str<20>, Fault> {
    st::GREET(cx, who)
}

#[trust_fb(name = \"LABELLER\")]
#[budget(\"2us\")]
pub struct Labeller {
    #[input] pub label: Str<8>,
    #[output] pub tagged: Str<12>,
    tagger: st::TAGGER,
}

impl Body for Labeller {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.tagger.input.LABEL = self.label;
        self.tagger.call(cx)?;
        self.tagged = self.tagger.output.TAGGED;
        Ok(())
    }
}
";

#[test]
fn strings_pass_through_calls_and_held_blocks() {
    // RU-25, RU-38, RU-39: a call passes a STRING[10] input and gives a STRING[20] result as the
    // Rust POU's `Str<10>` and `Str<20>`, and a held block's STRING inputs and outputs are `Str<8>`
    // and `Str<12>`: GREET('Ann') = 'Hello ' + 'Ann' = 'Hello Ann', and TAGGER('pump') = '#pump',
    // at the task's first call at 10 ms; the instance keeps its input. Catches a string passed as
    // its cells' bytes without its length, and a result left in the generated code's cell.
    let root = custom_project(
        "rust-pous-structured-text-strings",
        &[
            (
                "Cargo.toml",
                &manifest("trust_st = { path = \"../build/trust_st\" }\n"),
            ),
            ("src/lib.rs", TEXTS),
        ],
        "FUNCTION GREET : STRING[20]
VAR_INPUT
    who : STRING[10];
END_VAR
GREET := CONCAT('Hello ', who);
END_FUNCTION

FUNCTION_BLOCK TAGGER
VAR_INPUT
    LABEL : STRING[8];
END_VAR
VAR_OUTPUT
    TAGGED : STRING[12];
END_VAR
TAGGED := CONCAT('#', LABEL);
END_FUNCTION_BLOCK

PROGRAM Line
VAR
    message : STRING[20];
    labeller : LABELLER;
    tagged : STRING[12];
END_VAR
message := WELCOME(who := 'Ann');
labeller(label := 'pump');
tagged := labeller.tagged;
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
    assert_eq!(
        program.value("MAIN.MESSAGE"),
        Some(Value::String("Hello Ann".into()))
    );
    assert_eq!(
        program.value("MAIN.TAGGED"),
        Some(Value::String("#pump".into()))
    );
    assert_eq!(
        program.value("MAIN.LABELLER.TAGGER.LABEL"),
        Some(Value::String("pump".into()))
    );
}

/// A crate whose functions pass arrays to Structured Text and back, and whose block holds one.
const ARRAYS: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_function(name = \"TOTAL3\")]
#[budget(\"1us\")]
pub fn total3(cx: &mut Cx, values: Array<f64, 1, 3>) -> Result<f64, Fault> {
    st::SUM3(cx, values)
}

#[trust_function(name = \"STEPS\")]
#[budget(\"1us\")]
pub fn steps(cx: &mut Cx, rise: i16) -> Result<Array<i16, 0, 3>, Fault> {
    st::RAMP(cx, rise)
}

#[trust_fb(name = \"PAIR_COUNTER\")]
#[budget(\"2us\")]
pub struct PairCounter {
    #[input] pub a: i32,
    #[input] pub b: i32,
    #[output] pub total: i32,
    accum: st::ACCUM,
}

impl Body for PairCounter {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.accum.input.SAMPLES = Array::from_items([self.a, self.b]);
        self.accum.call(cx)?;
        self.total = self.accum.output.TOTAL;
        Ok(())
    }
}
";

/// The elements of the array value `value` a program read.
fn items(value: Option<Value>) -> Vec<Value> {
    match value.expect("the array is a variable") {
        Value::Array(array) => array.elements().to_vec(),
        other => vec![other],
    }
}

#[test]
fn arrays_pass_through_calls_and_held_blocks() {
    // RU-25, RU-38, RU-39: a call passes an ARRAY[1..3] OF LREAL input and gives an
    // ARRAY[0..2] OF INT result as the Rust POU's `Array<f64, 1, 3>` and `Array<i16, 0, 3>`, and a
    // held block's ARRAY[1..2] OF DINT input is `Array<i32, 1, 2>`: SUM3([1.0, 2.0, 3.5]) = 6.5,
    // RAMP(10) = [0, 10, 20], and ACCUM's total after one call with 4 and 5 is 0 + 4 + 5 = 9, at the
    // task's first call at 10 ms; the instance keeps its samples. Catches elements that change
    // places, a lower bound taken for the first index, and a result left in the generated cells.
    let root = custom_project(
        "rust-pous-structured-text-arrays",
        &[
            (
                "Cargo.toml",
                &manifest("trust_st = { path = \"../build/trust_st\" }\n"),
            ),
            ("src/lib.rs", ARRAYS),
        ],
        "FUNCTION SUM3 : LREAL
VAR_INPUT
    values : ARRAY[1..3] OF LREAL;
END_VAR
SUM3 := values[1] + values[2] + values[3];
END_FUNCTION

FUNCTION RAMP : ARRAY[0..2] OF INT
VAR_INPUT
    rise : INT;
END_VAR
VAR
    r : ARRAY[0..2] OF INT;
END_VAR
r[0] := 0;
r[1] := rise;
r[2] := rise * 2;
RAMP := r;
END_FUNCTION

FUNCTION_BLOCK ACCUM
VAR_INPUT
    SAMPLES : ARRAY[1..2] OF DINT;
END_VAR
VAR_OUTPUT
    TOTAL : DINT;
END_VAR
TOTAL := TOTAL + SAMPLES[1] + SAMPLES[2];
END_FUNCTION_BLOCK

PROGRAM Line
VAR
    values : ARRAY[1..3] OF LREAL := [1.0, 2.0, 3.5];
    sum : LREAL;
    ramp : ARRAY[0..2] OF INT;
    counter : PAIR_COUNTER;
    total : DINT;
END_VAR
sum := TOTAL3(values := values);
ramp := STEPS(rise := 10);
counter(a := 4, b := 5);
total := counter.total;
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
    assert_eq!(program.value("MAIN.SUM"), Some(Value::LReal(6.5)));
    assert_eq!(
        items(program.value("MAIN.RAMP")),
        [Value::Int(0), Value::Int(10), Value::Int(20)]
    );
    assert_eq!(program.value("MAIN.TOTAL"), Some(Value::DInt(9)));
    assert_eq!(
        items(program.value("MAIN.COUNTER.ACCUM.SAMPLES")),
        [Value::DInt(4), Value::DInt(5)]
    );
}
