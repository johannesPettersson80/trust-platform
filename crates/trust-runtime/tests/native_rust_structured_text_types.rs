//! A Rust POU passes values of each type a binding carries through a call of Structured Text and
//! a held block (specification 48 RU-25, RU-38, RU-39): the elementary types, strings, arrays and
//! the project's structures.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

mod rust_pous_support;

use rust_pous_support::{built, custom_project, manifest};
use trust_runtime::native::{load, NoHooks};
use trust_runtime::value::{Duration, Value};

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

/// A crate whose functions pass the Structured Text structure POINT and whose block holds a
/// tracker of it.
const POINTS: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_function(name = \"MANHATTAN\")]
#[budget(\"1us\")]
pub fn manhattan(cx: &mut Cx, x: f64, y: f64) -> Result<f64, Fault> {
    st::NORM1(cx, st::POINT { X: x, Y: y })
}

#[trust_function(name = \"SWAPPED\")]
#[budget(\"1us\")]
pub fn swapped(cx: &mut Cx, x: f64, y: f64) -> Result<f64, Fault> {
    let mirrored = st::MIRROR(cx, st::POINT { X: x, Y: y })?;
    Ok(mirrored.X * 10.0 + mirrored.Y)
}

#[trust_fb(name = \"FOLLOWER\")]
#[budget(\"2us\")]
pub struct Follower {
    #[input] pub x: f64,
    #[output] pub last_y: f64,
    #[output] pub moves: i32,
    tracker: st::TRACKER,
}

impl Body for Follower {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.tracker.input.AT_POINT = st::POINT { X: self.x, Y: self.x * 2.0 };
        self.tracker.call(cx)?;
        self.last_y = self.tracker.output.LAST.Y;
        self.moves = self.tracker.output.MOVES;
        Ok(())
    }
}
";

#[test]
fn structures_pass_through_calls_and_held_blocks() {
    // RU-25, RU-38, RU-39: a structure of the project's Structured Text has a binding, `st::POINT`,
    // which a call passes in and gives back and a held block's input and output hold: NORM1((-3,
    // 4)) = |-3| + |4| = 7, MIRROR((1, 2)) = (2, 1), so SWAPPED gives 2 x 10 + 1 = 21, and TRACKER
    // keeps (1.5, 3.0), the last point FOLLOWER gave it, after one move, at the task's first call at
    // 10 ms. Catches fields that change places, a structure passed as its cells' bytes, and a
    // held block's structure left in the generated cells.
    let root = custom_project(
        "rust-pous-structured-text-structures",
        &[
            (
                "Cargo.toml",
                &manifest("trust_st = { path = \"../build/trust_st\" }\n"),
            ),
            ("src/lib.rs", POINTS),
        ],
        "TYPE POINT :
STRUCT
    X : LREAL;
    Y : LREAL;
END_STRUCT
END_TYPE

FUNCTION NORM1 : LREAL
VAR_INPUT
    P : POINT;
END_VAR
NORM1 := ABS(P.X) + ABS(P.Y);
END_FUNCTION

FUNCTION MIRROR : POINT
VAR_INPUT
    P : POINT;
END_VAR
VAR
    m : POINT;
END_VAR
m.X := P.Y;
m.Y := P.X;
MIRROR := m;
END_FUNCTION

FUNCTION_BLOCK TRACKER
VAR_INPUT
    AT_POINT : POINT;
END_VAR
VAR_OUTPUT
    LAST : POINT;
    MOVES : DINT;
END_VAR
LAST := AT_POINT;
MOVES := MOVES + 1;
END_FUNCTION_BLOCK

PROGRAM Line
VAR
    norm : LREAL;
    swap : LREAL;
    follower : FOLLOWER;
    last_y : LREAL;
END_VAR
norm := MANHATTAN(x := -3.0, y := 4.0);
swap := SWAPPED(x := 1.0, y := 2.0);
follower(x := 1.5);
last_y := follower.last_y;
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
    assert_eq!(program.value("MAIN.NORM"), Some(Value::LReal(7.0)));
    assert_eq!(program.value("MAIN.SWAP"), Some(Value::LReal(21.0)));
    assert_eq!(program.value("MAIN.LAST_Y"), Some(Value::LReal(3.0)));
    assert_eq!(
        program.value("MAIN.FOLLOWER.TRACKER.LAST.X"),
        Some(Value::LReal(1.5))
    );
    assert_eq!(
        program.value("MAIN.FOLLOWER.TRACKER.MOVES"),
        Some(Value::DInt(1))
    );
}

/// A crate whose block reads the default of the Structured Text structure LIMITS and passes it.
const DEFAULTS: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_fb(name = \"READER\")]
#[budget(\"2us\")]
pub struct Reader {
    #[output] pub low: f64,
    #[output] pub tag: Str<8>,
    #[output] pub step2: i16,
    #[output] pub wait: Time,
    #[output] pub mask: Byte,
    #[output] pub corner_y: f64,
    #[output] pub same: bool,
    #[output] pub width: f64,
}

impl Body for Reader {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        let limits = st::LIMITS::default();
        self.low = limits.LOW;
        self.tag = limits.TAG;
        self.step2 = limits.STEPS.get(2).copied().unwrap_or(0);
        self.wait = limits.WAIT;
        self.mask = limits.MASK;
        self.corner_y = limits.AT_CORNER.Y;
        self.same = limits == trust_plc::declared_default::<st::LIMITS>();
        self.width = st::WIDTH(cx, limits)?;
        Ok(())
    }
}
";

#[test]
fn a_structure_binding_starts_at_its_declared_initial_values() {
    // RU-39: the binding of a Structured Text structure has, as its `Default` and its `Declared`
    // default, the value a variable of the type starts at: LOW -5.0, TAG 'ok', STEPS [3, 4],
    // WAIT T#250ms and MASK 16#0F as LIMITS declares them, AT_CORNER.Y -1.0 as CORNER declares it
    // (IEC §6.4.4.6.2), and WIDTH(LIMITS) = HIGH - LOW = 20.0 - (-5.0) = 25.0, at the task's first
    // call at 10 ms. Catches a default at the field types' zeros (0.0, '', 0, T#0s, 16#00) and a
    // `Default` other than the `Declared` default.
    let root = custom_project(
        "rust-pous-structured-text-defaults",
        &[
            (
                "Cargo.toml",
                &manifest("trust_st = { path = \"../build/trust_st\" }\n"),
            ),
            ("src/lib.rs", DEFAULTS),
        ],
        "TYPE CORNER :
STRUCT
    X : LREAL := 0.5;
    Y : LREAL := -1.0;
END_STRUCT
END_TYPE

TYPE LIMITS :
STRUCT
    LOW : LREAL := -5.0;
    HIGH : LREAL := 20.0;
    TAG : STRING[8] := 'ok';
    STEPS : ARRAY[1..2] OF INT := [3, 4];
    WAIT : TIME := T#250ms;
    MASK : BYTE := 16#0F;
    AT_CORNER : CORNER;
END_STRUCT
END_TYPE

FUNCTION WIDTH : LREAL
VAR_INPUT
    L : LIMITS;
END_VAR
WIDTH := L.HIGH - L.LOW;
END_FUNCTION

PROGRAM Line
VAR
    reader : READER;
END_VAR
reader();
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
    let reader = |name: &str| program.value(&format!("MAIN.READER.{name}"));
    assert_eq!(reader("LOW"), Some(Value::LReal(-5.0)));
    assert_eq!(reader("TAG"), Some(Value::String("ok".into())));
    assert_eq!(reader("STEP2"), Some(Value::Int(4)));
    assert_eq!(
        reader("WAIT"),
        Some(Value::Time(Duration::from_millis(250)))
    );
    assert_eq!(reader("MASK"), Some(Value::Byte(0x0F)));
    assert_eq!(reader("CORNER_Y"), Some(Value::LReal(-1.0)));
    assert_eq!(reader("SAME"), Some(Value::Bool(true)));
    assert_eq!(reader("WIDTH"), Some(Value::LReal(25.0)));
}

/// A crate whose functions pass matrices to Structured Text and back, and whose block holds a grid.
const MATRICES: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_function(name = \"CROSSED\")]
#[budget(\"1us\")]
pub fn crossed(cx: &mut Cx) -> Result<f64, Fault> {
    let m = Array::from_items([Array::from_items([1.5, 2.0]), Array::from_items([3.0, 4.25])]);
    st::CROSS(cx, m)
}

#[trust_function(name = \"MARKED\")]
#[budget(\"1us\")]
pub fn marked(cx: &mut Cx) -> Result<i32, Fault> {
    let r = st::MARKS(cx)?;
    let at = |i: i64, j: i64| r.get(i).and_then(|row| row.get(j)).copied().unwrap_or(0);
    Ok(i32::from(at(1, 2)) * 10 + i32::from(at(0, 0)))
}

#[trust_fb(name = \"GRIDDER\")]
#[budget(\"2us\")]
pub struct Gridder {
    #[output] pub lit: i32,
    #[output] pub corner: bool,
    grid: st::GRID,
}

impl Body for Gridder {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.grid.input.CELLS = Array::from_items([
            Array::from_items([false, false, true]),
            Array::from_items([true, false, false]),
        ]);
        self.grid.call(cx)?;
        self.lit = self.grid.output.LIT;
        self.corner = self
            .grid
            .output
            .FLIPPED
            .get(3)
            .and_then(|row| row.get(1))
            .copied()
            .unwrap_or(false);
        Ok(())
    }
}
";

#[test]
fn two_dimensional_arrays_pass_through_calls_and_held_blocks() {
    // RU-25, RU-39: an array of two dimensions passes as the array of arrays of them,
    // `Array<Array<T, A2, N2>, A1, N1>`, whose elements lie as Structured Text's do: CROSS(M) =
    // M[1, 2] + 10 x M[2, 1] = 2.0 + 10 x 3.0 = 32.0 (23.0 for a transposed matrix); MARKS gives
    // [0..1, 0..2] with 1 at [0, 0] and 7 at [1, 2], so MARKED = 7 x 10 + 1 = 71; GRID counts the
    // two TRUE cells of [1..2, 1..3] and writes FLIPPED[3, 1] = CELLS[1, 3] = TRUE, at the task's
    // first call at 10 ms. Catches a matrix passed as its transpose, a result of rows in the wrong
    // order, and a held block's matrix left in the generated cells.
    let root = custom_project(
        "rust-pous-structured-text-matrices",
        &[
            (
                "Cargo.toml",
                &manifest("trust_st = { path = \"../build/trust_st\" }\n"),
            ),
            ("src/lib.rs", MATRICES),
        ],
        "FUNCTION CROSS : LREAL
VAR_INPUT
    M : ARRAY[1..2, 1..2] OF LREAL;
END_VAR
CROSS := M[1, 2] + 10.0 * M[2, 1];
END_FUNCTION

FUNCTION MARKS : ARRAY[0..1, 0..2] OF INT
VAR
    r : ARRAY[0..1, 0..2] OF INT;
END_VAR
r[0, 0] := 1;
r[1, 2] := 7;
MARKS := r;
END_FUNCTION

FUNCTION_BLOCK GRID
VAR_INPUT
    CELLS : ARRAY[1..2, 1..3] OF BOOL;
END_VAR
VAR_OUTPUT
    LIT : DINT;
    FLIPPED : ARRAY[1..3, 1..2] OF BOOL;
END_VAR
VAR
    i : INT;
    j : INT;
END_VAR
LIT := 0;
FOR i := 1 TO 2 DO
    FOR j := 1 TO 3 DO
        FLIPPED[j, i] := CELLS[i, j];
        IF CELLS[i, j] THEN
            LIT := LIT + 1;
        END_IF;
    END_FOR;
END_FOR;
END_FUNCTION_BLOCK

PROGRAM Line
VAR
    cross_value : LREAL;
    marked_value : DINT;
    gridder : GRIDDER;
END_VAR
cross_value := CROSSED();
marked_value := MARKED();
gridder();
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
    assert_eq!(program.value("MAIN.CROSS_VALUE"), Some(Value::LReal(32.0)));
    assert_eq!(program.value("MAIN.MARKED_VALUE"), Some(Value::DInt(71)));
    assert_eq!(program.value("MAIN.GRIDDER.LIT"), Some(Value::DInt(2)));
    assert_eq!(
        program.value("MAIN.GRIDDER.CORNER"),
        Some(Value::Bool(true))
    );
}
