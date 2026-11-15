//! A Rust POU's in-outs and references reach the variables Structured Text gives them
//! (specification 48 RU-29; specification 44 §7.3): a block's in-out keeps the reference of its
//! last call, a function's in-out is the caller's variable itself, a frame variable too, and a
//! reference is read with Structured Text's checks; a structure's every field, where it lies.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`. A toolchain without
//! cargo and the SDK's sources, as a release's bundle is until specification 48 §16, refuses the
//! build with that reason.

mod rust_pous_support;

use rust_pous_support::{built, built_under, custom_project, manifest};
use trust_runtime::error::RuntimeError;
use trust_runtime::native::{load, NoHooks};
use trust_runtime::stdlib::conversions::ConversionProfile;
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

/// Structures of the crate, and POUs that reach them through in-outs and references.
const STRUCTURES: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

/// A point of the plane, named otherwise in Structured Text.
#[trust_struct(name = \"S_POINT\")]
#[derive(Clone, Copy)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// A run between two points, its moves and its name.
#[trust_struct(name = \"S_RUN\")]
#[derive(Clone, Copy)]
pub struct Run {
    pub start: Point,
    pub finish: Point,
    pub moves: i16,
    pub label: Str<8>,
}

/// Moves the point it is given by (dx, dy) and returns its new x + y.
#[trust_function(name = \"SHIFT\")]
#[budget(\"1us\")]
pub fn shift(_cx: &mut Cx, point: InOut<'_, Point>, dx: f64, dy: f64) -> Result<f64, Fault> {
    let mut moved = point.get();
    moved.x += dx;
    moved.y += dy;
    point.set(moved);
    Ok(moved.x + moved.y)
}

/// Moves the first of three points by 1 along x, the second by 2 and the third by 3, and returns
/// the sum of their x.
#[trust_function(name = \"SPREAD\")]
#[budget(\"1us\")]
pub fn spread(_cx: &mut Cx, points: InOut<'_, Array<Point, 1, 3>>) -> Result<f64, Fault> {
    let mut all = points.get();
    let mut step = 0.0;
    let mut sum = 0.0;
    for point in all.as_mut_slice() {
        step += 1.0;
        point.x += step;
        sum += point.x;
    }
    points.set(all);
    Ok(sum)
}

/// The x of the point a reference names.
#[trust_function(name = \"X_OF\")]
#[budget(\"1us\")]
pub fn x_of(cx: &mut Cx, target: Ref<Point>) -> Result<f64, Fault> {
    Ok(target.get(cx)?.x)
}

/// Follows a run: each call moves its finish along x by the stride, puts the finish's y at twice
/// the start's, and counts the move.
#[trust_fb(name = \"FOLLOW\")]
#[budget(\"1us\")]
pub struct Follow {
    #[input]
    pub stride: f64,
    #[in_out]
    pub run: Ref<Run>,
}

impl Body for Follow {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        let mut run = self.run.get(cx)?;
        run.finish.x += self.stride;
        run.finish.y = run.start.y * 2.0;
        run.moves = checked::add(cx, run.moves, 1)?;
        self.run.set(cx, run)
    }
}

/// Counts its jumps, moves its point to x = 7 and faults.
#[trust_fb(name = \"JUMP\")]
#[budget(\"1us\")]
pub struct Jump {
    #[in_out]
    pub spot: Ref<Point>,
    #[output]
    pub jumps: i16,
}

impl Body for Jump {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.jumps = checked::add(cx, self.jumps, 1)?;
        let mut spot = self.spot.get(cx)?;
        spot.x = 7.0;
        self.spot.set(cx, spot)?;
        Err(rust_fault(cx, 3, 0, 0))
    }
}
";

#[test]
fn in_outs_and_references_of_structures_reach_the_variables_structured_text_gives() {
    // RU-29: three calls at 10, 20 and 30 ms. SHIFT moves the program's spot from (1, 2) by
    // (1.5, -0.5) each time, to (5.5, 0.5), and returns 5.5 + 0.5 = 6; X_OF reads its x through
    // REF(spot) after this scan's SHIFT: 5.5. FOLLOW's in-out is the program's trip: its finish
    // moves by 2 along x to 6, its finish's y is twice its start's 4, so 8, its moves count from 10
    // to 13 and its label stays 'A-B'. SPREAD moves the path's three points by 1, 2 and 3 along x at each
    // call, to 3, 6 and 9, and returns 18. NUDGE shifts a structure of its own frame twice by 1
    // along x, from 10 to 12. Catches a structure's in-out that works on a copy, a field read or
    // written at another field's place, a nested structure, a string field or an array of
    // structures lost, and a reference or a frame variable of a structure that cannot be reached.
    let program = format!(
        "FUNCTION NUDGE : LREAL
VAR_INPUT
    start : LREAL;
END_VAR
VAR
    p : S_POINT;
    ignored : LREAL;
END_VAR
p.x := start;
ignored := SHIFT(point := p, dx := 1.0, dy := 0.0);
ignored := SHIFT(point := p, dx := 1.0, dy := 0.0);
NUDGE := p.x;
END_FUNCTION

PROGRAM Line
VAR
    spot : S_POINT := (x := 1.0, y := 2.0);
    trip : S_RUN := (moves := 10, label := 'A-B');
    path : ARRAY[1..3] OF S_POINT;
    follower : FOLLOW;
    travel : LREAL;
    total : LREAL;
    first : LREAL;
    second : LREAL;
    third : LREAL;
    thirdY : LREAL;
    reached : LREAL;
    nudged : LREAL;
END_VAR
travel := SHIFT(point := spot, dx := 1.5, dy := -0.5);
reached := X_OF(target := REF(spot));
trip.start.y := 4.0;
follower(stride := 2.0, run := trip);
total := SPREAD(points := path);
first := path[1].x;
second := path[2].x;
third := path[3].x;
thirdY := path[3].y;
nudged := NUDGE(start := 10.0);
END_PROGRAM

{TASK}"
    );
    let root = custom_project(
        "rust-pous-structure-references",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", STRUCTURES)],
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
    let lreal = |path: &str| program.value(path);
    assert_eq!(lreal("MAIN.SPOT.X"), Some(Value::LReal(5.5)));
    assert_eq!(lreal("MAIN.SPOT.Y"), Some(Value::LReal(0.5)));
    assert_eq!(lreal("MAIN.TRAVEL"), Some(Value::LReal(6.0)));
    assert_eq!(lreal("MAIN.REACHED"), Some(Value::LReal(5.5)));
    assert_eq!(lreal("MAIN.TRIP.FINISH.X"), Some(Value::LReal(6.0)));
    assert_eq!(lreal("MAIN.TRIP.FINISH.Y"), Some(Value::LReal(8.0)));
    assert_eq!(lreal("MAIN.TRIP.START.X"), Some(Value::LReal(0.0)));
    assert_eq!(program.value("MAIN.TRIP.MOVES"), Some(Value::Int(13)));
    assert_eq!(
        program.value("MAIN.TRIP.LABEL"),
        Some(Value::String("A-B".into()))
    );
    assert_eq!(lreal("MAIN.FIRST"), Some(Value::LReal(3.0)));
    assert_eq!(lreal("MAIN.SECOND"), Some(Value::LReal(6.0)));
    assert_eq!(lreal("MAIN.THIRD"), Some(Value::LReal(9.0)));
    assert_eq!(lreal("MAIN.THIRDY"), Some(Value::LReal(0.0)));
    assert_eq!(lreal("MAIN.TOTAL"), Some(Value::LReal(18.0)));
    assert_eq!(lreal("MAIN.NUDGED"), Some(Value::LReal(12.0)));
}

#[test]
fn a_write_through_a_structures_reference_is_made_before_the_fault() {
    // RU-29: JUMP writes its point through its in-out, x = 7, then faults; the write is made at
    // once, so the spot holds (7, 2) after the fault, while the block's own variable stays as it
    // was, 0 jumps (RU-28), and the statement after the call never runs. Catches a structure
    // written back only when the body returns `Ok`, and a fault that keeps the block's variables.
    let program = format!(
        "PROGRAM Line
VAR
    spot : S_POINT := (x := 1.0, y := 2.0);
    jumper : JUMP;
    after : BOOL;
END_VAR
jumper(spot := spot);
after := TRUE;
END_PROGRAM

{TASK}"
    );
    let root = custom_project(
        "rust-pous-structure-fault",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", STRUCTURES)],
        &program,
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    let error = program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect_err("the fault ends the scan");
    assert_eq!(
        error,
        RuntimeError::RustFault {
            number: 3,
            first: 0,
            second: 0
        }
    );
    assert_eq!(program.value("MAIN.SPOT.X"), Some(Value::LReal(7.0)));
    assert_eq!(program.value("MAIN.SPOT.Y"), Some(Value::LReal(2.0)));
    assert_eq!(program.value("MAIN.JUMPER.JUMPS"), Some(Value::Int(0)));
    assert_eq!(program.value("MAIN.AFTER"), Some(Value::Bool(false)));
}

/// A function whose string in-out takes the caller's string of any capacity.
const TEXTS: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

/// Gives the length of the text it reads, at most 6 bytes, and writes 'XYZ' into it.
#[trust_function(name = \"MARK\")]
#[budget(\"1us\")]
pub fn mark(_cx: &mut Cx, text: InOut<'_, Str<6>>) -> Result<i16, Fault> {
    let read = text.get();
    text.set(Str::from_text(\"XYZ\"));
    Ok(i16::try_from(read.len()).unwrap_or(i16::MAX))
}
";

#[test]
fn a_functions_string_in_out_is_the_callers_string_of_any_capacity() {
    // RU-29 and docs/IEC_DEVIATIONS.md DEV-035, under the CODESYS profile, which binds a string
    // in-out across capacities: one call at 10 ms. MARK's in-out is a STRING[6]: the caller's
    // STRING[20] holding 'abcdefghij' reads as an assignment to STRING[6] does, 'abcdef', 6 bytes,
    // and takes 'XYZ'; the caller's STRING[2] holding 'ab' reads its 2 bytes and takes 'XY', the
    // assignment's cut at its capacity. Catches a string in-out that works on a copy, reads past the
    // in-out's capacity or writes past the caller's.
    let program = format!(
        "PROGRAM Line
VAR
    long : STRING[20] := 'abcdefghij';
    short : STRING[2] := 'ab';
    longRead : INT;
    shortRead : INT;
END_VAR
longRead := MARK(text := long);
shortRead := MARK(text := short);
END_PROGRAM

{TASK}"
    );
    let root = custom_project(
        "rust-pous-string-in-outs",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", TEXTS)],
        &program,
    );
    let Some((built, device)) = built_under(&root, ConversionProfile::Codesys) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    for time in [0, 10] {
        program
            .scan(Duration::from_millis(time), &mut NoHooks)
            .expect("the scan runs");
    }
    assert_eq!(program.value("MAIN.LONGREAD"), Some(Value::Int(6)));
    assert_eq!(
        program.value("MAIN.LONG"),
        Some(Value::String("XYZ".into()))
    );
    assert_eq!(program.value("MAIN.SHORTREAD"), Some(Value::Int(2)));
    assert_eq!(
        program.value("MAIN.SHORT"),
        Some(Value::String("XY".into()))
    );
}
