//! A Rust POU's variables of the IEC types beside Rust's own reach it and come back as the
//! program's areas store them (specification 48 RU-25, RU-26; specification 44 §6.2): bit strings,
//! characters, durations, dates and times of day, strings, arrays, enumerations and structures, each
//! with its declared starting value.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`. A toolchain without
//! cargo and the SDK's sources, as a release's bundle is until specification 48 §16, refuses the
//! build with that reason.

mod rust_pous_support;

use rust_pous_support::{built, custom_project, manifest};
use trust_runtime::native::{load, NoHooks};
use trust_runtime::value::{
    DateTimeValue, DateValue, Duration, LTimeOfDayValue, TimeOfDayValue, Value,
};

/// A block that computes with a duration, a bit string and a character.
const TYPED: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_fb(name = \"TYPED_BLOCK\")]
#[budget(\"1us\")]
pub struct TypedBlock {
    #[input]
    pub period: Time,
    #[input]
    pub flags: Byte,
    #[input]
    pub letter: Char,
    #[output]
    pub half: Time,
    #[output]
    pub low: Byte,
    #[output]
    pub next: Char,
    #[output]
    #[init(\"T#1s\")]
    pub total: Time,
}

impl Body for TypedBlock {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.half = Time::from_nanos(self.period.as_nanos() / 2);
        self.low = self.flags & Byte(0x0F);
        self.next = Char(self.letter.0 + 1);
        self.total = self.total.checked_add(cx, self.period)?;
        Ok(())
    }
}
";

/// A program that calls TYPED_BLOCK once per 10 ms with IEC literals.
const TYPED_PROGRAM: &str = "PROGRAM Line
VAR
    block : TYPED_BLOCK;
    half : TIME;
    low : BYTE;
    next : CHAR;
    total : TIME;
END_VAR
block(period := T#500ms, flags := 16#AB, letter := 'Y');
half := block.half;
low := block.low;
next := block.next;
total := block.total;
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_rust_pou_computes_with_durations_bit_strings_and_characters() {
    // Catches a TIME, BYTE or CHAR that reaches the Rust POU as another value, or comes back so:
    // RU-25's `Time`, `Byte` and `Char` hold what the areas store (specification 44 §6.2). Three
    // calls at 0, 10 and 20 ms: half of T#500ms is T#250ms, 16#AB AND 16#0F is 16#0B, 'Y' + 1 is
    // 'Z', and the total starts at its declared T#1s (RU-26) and grows by T#500ms at each call:
    // T#2.5s.
    let root = custom_project(
        "rust-pous-typed",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", TYPED)],
        TYPED_PROGRAM,
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    // The task is due at its start (specification 10 §4.3): three activations, at 0, 10 and
    // 20 ms.
    for time in [0, 10, 20] {
        program
            .scan(Duration::from_millis(time), &mut NoHooks)
            .expect("the scan runs");
    }
    assert_eq!(
        program.value("MAIN.HALF"),
        Some(Value::Time(Duration::from_millis(250)))
    );
    assert_eq!(program.value("MAIN.LOW"), Some(Value::Byte(0x0B)));
    assert_eq!(program.value("MAIN.NEXT"), Some(Value::Char(b'Z')));
    assert_eq!(
        program.value("MAIN.TOTAL"),
        Some(Value::Time(Duration::from_millis(2_500)))
    );
}

/// A block that moves a moment by a duration and splits it into its day and its time of day.
const DATED: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_fb(name = \"STAMP\")]
#[budget(\"1us\")]
pub struct Stamp {
    #[input]
    pub shift: Time,
    #[output]
    #[init(\"DT#2024-02-29-22:30:15\")]
    pub moment: Dt,
    #[output]
    pub day: Date,
    #[output]
    pub clock: Tod,
    #[output]
    pub weekday: i16,
    #[output]
    #[init(\"LTOD#12:00:00.000000001\")]
    pub precise: LTod,
    #[output]
    #[init(\"D#2024-02-29\")]
    pub leap: Date,
}

impl Body for Stamp {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.moment = self.moment.checked_add(cx, self.shift)?;
        self.day = self.moment.date(cx)?;
        self.clock = self.moment.tod();
        self.weekday = self.day.day_of_week();
        Ok(())
    }
}
";

/// A program that calls the block once per 10 ms and compares its day in Structured Text.
const DATED_PROGRAM: &str = "PROGRAM Line
VAR
    stamp : STAMP;
    moment : DT;
    day : DATE;
    clock : TOD;
    weekday : INT;
    precise : LTOD;
    leap : DATE;
    sameDay : BOOL;
END_VAR
stamp(shift := T#1h);
moment := stamp.moment;
day := stamp.day;
clock := stamp.clock;
weekday := stamp.weekday;
precise := stamp.precise;
leap := stamp.leap;
sameDay := stamp.day = D#2024-03-01;
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_rust_pou_keeps_dates_and_times_of_day_as_structured_text_does() {
    // RU-25, RU-26: a DT, a DATE, a TOD and an LTOD reach the Rust POU and come back as the areas
    // store them, and a date's declared value is its starting value. By hand: three calls at 0,
    // 10 and 20 ms move DT#2024-02-29-22:30:15 by T#1h each, past midnight and the leap day, to
    // DT#2024-03-01-01:30:15, 1,709,256,615,000 ms; its day 2024-03-01 is 1,709,251,200,000 ms, a
    // Friday (5), and its time of day 5,415,000 ms. The LTOD keeps its declared 12:00:00 and one
    // nanosecond, 43,200,000,000,001 ns, and the DATE its declared 2024-02-29,
    // 1,709,164,800,000 ms. Catches a date read in another unit, a day not carried past midnight
    // or the end of February, a starting value the Rust POU and its declaration disagree on, and
    // a DATE that Structured Text does not take for the same day.
    let root = custom_project(
        "rust-pous-dated",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", DATED)],
        DATED_PROGRAM,
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    // The task is due at its start (specification 10 §4.3): three activations, at 0, 10 and
    // 20 ms.
    for time in [0, 10, 20] {
        program
            .scan(Duration::from_millis(time), &mut NoHooks)
            .expect("the scan runs");
    }
    assert_eq!(
        program.value("MAIN.MOMENT"),
        Some(Value::Dt(DateTimeValue::new(1_709_256_615_000)))
    );
    assert_eq!(
        program.value("MAIN.DAY"),
        Some(Value::Date(DateValue::new(1_709_251_200_000)))
    );
    assert_eq!(
        program.value("MAIN.CLOCK"),
        Some(Value::Tod(TimeOfDayValue::new(5_415_000)))
    );
    assert_eq!(program.value("MAIN.WEEKDAY"), Some(Value::Int(5)));
    assert_eq!(
        program.value("MAIN.PRECISE"),
        Some(Value::LTod(LTimeOfDayValue::new(43_200_000_000_001)))
    );
    assert_eq!(
        program.value("MAIN.LEAP"),
        Some(Value::Date(DateValue::new(1_709_164_800_000)))
    );
    assert_eq!(program.value("MAIN.SAMEDAY"), Some(Value::Bool(true)));
}

/// A block that keeps a name and its length, and a function that shouts a text.
const TEXTED: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_fb(name = \"LABEL\")]
#[budget(\"1us\")]
pub struct Label {
    #[input]
    pub name: Str<12>,
    #[output]
    #[init(\"none\")]
    pub last: Str<12>,
    #[output]
    pub wide: WStr<4>,
    #[output]
    pub length: i16,
}

impl Body for Label {
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        self.last = self.name;
        self.wide.set(self.name.as_str().unwrap_or(\"?\"));
        self.length = i16::try_from(self.name.len()).unwrap_or(-1);
        Ok(())
    }
}

#[trust_function(name = \"SHOUT\")]
#[budget(\"1us\")]
pub fn shout(_cx: &mut Cx, text: Str<8>) -> Result<Str<8>, Fault> {
    let mut upper = [0_u8; 8];
    for (slot, byte) in upper.iter_mut().zip(text.as_bytes()) {
        *slot = byte.to_ascii_uppercase();
    }
    let shouted = upper.get(..text.len()).and_then(|bytes| core::str::from_utf8(bytes).ok());
    Ok(Str::from_text(shouted.unwrap_or(\"\")))
}
";

/// A program that names a valve with a character outside ASCII, reads an instance never called,
/// and shouts.
const TEXTED_PROGRAM: &str = "PROGRAM Line
VAR
    label : LABEL;
    idle : LABEL;
    last : STRING[12];
    wide : WSTRING[4];
    length : INT;
    loud : STRING[8];
    idleLast : STRING[12];
END_VAR
label(name := 'valve 2$E9');
last := label.last;
wide := label.wide;
length := label.length;
loud := SHOUT(text := 'ab-c');
idleLast := idle.last;
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_rust_pou_keeps_strings_as_structured_text_does() {
    // RU-25, RU-26: a STRING and a WSTRING reach the Rust POU and come back as they lie in their
    // cells, an input, an output and a function's argument and result alike, and a string's
    // declared text is its starting value. By hand: 'valve 2' and é ($E9, U+00E9, C3 A9 in UTF-8)
    // are 9 bytes; the WSTRING[4] keeps the first four characters, "valv"; SHOUT gives 'AB-C'; the
    // instance never called keeps its declared 'none'. Catches a text cut or lengthened on its
    // way, a length read in characters, a character outside ASCII broken, a function's string
    // result lost, and a starting text the Rust POU and its declaration disagree on.
    let root = custom_project(
        "rust-pous-texted",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", TEXTED)],
        TEXTED_PROGRAM,
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    // The task is due at its start (specification 10 §4.3): three activations, at 0, 10 and
    // 20 ms.
    for time in [0, 10, 20] {
        program
            .scan(Duration::from_millis(time), &mut NoHooks)
            .expect("the scan runs");
    }
    assert_eq!(
        program.value("MAIN.LAST"),
        Some(Value::String("valve 2é".into()))
    );
    assert_eq!(
        program.value("MAIN.WIDE"),
        Some(Value::WString("valv".into()))
    );
    assert_eq!(program.value("MAIN.LENGTH"), Some(Value::Int(9)));
    assert_eq!(
        program.value("MAIN.LOUD"),
        Some(Value::String("AB-C".into()))
    );
    assert_eq!(
        program.value("MAIN.IDLELAST"),
        Some(Value::String("none".into()))
    );
}

/// A block that reads an array of samples into a scaled array, the largest sample, a cell of a
/// grid and a flag, and a function that totals an array.
const LISTED: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_fb(name = \"STATS\")]
#[budget(\"1us\")]
pub struct Stats {
    #[input]
    pub samples: Array<i16, 1, 4>,
    #[output]
    #[init([10, 20])]
    pub scaled: Array<i32, 0, 3>,
    #[output]
    pub largest: i16,
    #[output]
    #[init([[1, 2], [3, 4]])]
    pub grid: Array<Array<u8, 1, 2>, 1, 2>,
    #[output]
    #[init([\"a\", \"bc\"])]
    pub names: Array<Str<4>, 1, 2>,
    #[output]
    pub flags: Array<bool, 1, 3>,
}

impl Body for Stats {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.largest = self.samples.iter().copied().max().unwrap_or(0);
        for (slot, sample) in self.scaled.as_mut_slice().iter_mut().zip(self.samples.iter()) {
            *slot = checked::mul(cx, i32::from(*sample), 2)?;
        }
        if let Some(cell) = self.grid.get_mut(2).and_then(|row| row.get_mut(1)) {
            *cell = 9;
        }
        if let Some(flag) = self.flags.get_mut(3) {
            *flag = self.largest > 5;
        }
        Ok(())
    }
}

#[trust_function(name = \"TOTAL\")]
#[budget(\"1us\")]
pub fn total(cx: &mut Cx, values: Array<i16, 1, 4>) -> Result<i32, Fault> {
    values
        .iter()
        .try_fold(0_i32, |sum, value| checked::add(cx, sum, i32::from(*value)))
}
";

/// A program that gives the block its samples, reads its arrays and totals the samples.
const LISTED_PROGRAM: &str = "PROGRAM Line
VAR
    stats : STATS;
    idle : STATS;
    samples : ARRAY[1..4] OF INT := [3, 7, -2, 5];
    scaled : ARRAY[0..2] OF DINT;
    largest : INT;
    grid : ARRAY[1..2] OF ARRAY[1..2] OF USINT;
    names : ARRAY[1..2] OF STRING[4];
    flags : ARRAY[1..3] OF BOOL;
    sum : DINT;
    idleScaled : ARRAY[0..2] OF DINT;
END_VAR
stats(samples := samples);
scaled := stats.scaled;
largest := stats.largest;
grid := stats.grid;
names := stats.names;
flags := stats.flags;
sum := TOTAL(values := samples);
idleScaled := idle.scaled;
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
";

/// The elements of an array value, an array of arrays row by row.
fn flat(value: Value) -> Vec<Value> {
    match value {
        Value::Array(array) => array.elements().iter().cloned().flat_map(flat).collect(),
        other => vec![other],
    }
}

/// The elements of the array value `value` a program read, an array of arrays row by row.
fn elements(value: Option<Value>) -> Vec<Value> {
    flat(value.expect("the array is a variable"))
}

#[test]
fn a_rust_pou_keeps_arrays_as_structured_text_does() {
    // RU-25, RU-26: an array reaches the Rust POU and comes back element by element, indexed from
    // its lower bound, an input, an output, an array of arrays, an array of strings and a
    // function's argument alike, and an array's declared list is its starting value. By hand: the
    // samples [3, 7, -2, 5] give the largest 7; the scaled ARRAY[0..2] takes the first three
    // doubled, [6, 14, -4]; GRID[2][1] becomes 9, [[1, 2], [9, 4]]; the names keep their declared
    // ['a', 'bc']; FLAGS[3] is TRUE since 7 > 5; TOTAL gives 3 + 7 - 2 + 5 = 13; the instance never
    // called keeps its declared [10, 20, 0]. Catches an element moved by an index counted from 0,
    // an array cut or lengthened on its way, a row of an inner array lost, a function's array
    // argument read from another place, and a starting list the Rust POU and its declaration
    // disagree on.
    let root = custom_project(
        "rust-pous-listed",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", LISTED)],
        LISTED_PROGRAM,
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    // The task is due at its start (specification 10 §4.3): three activations, at 0, 10 and
    // 20 ms.
    for time in [0, 10, 20] {
        program
            .scan(Duration::from_millis(time), &mut NoHooks)
            .expect("the scan runs");
    }
    assert_eq!(
        elements(program.value("MAIN.SCALED")),
        [Value::DInt(6), Value::DInt(14), Value::DInt(-4)]
    );
    assert_eq!(program.value("MAIN.LARGEST"), Some(Value::Int(7)));
    assert_eq!(
        elements(program.value("MAIN.GRID")),
        [
            Value::USInt(1),
            Value::USInt(2),
            Value::USInt(9),
            Value::USInt(4)
        ]
    );
    assert_eq!(
        elements(program.value("MAIN.NAMES")),
        [Value::String("a".into()), Value::String("bc".into())]
    );
    assert_eq!(
        elements(program.value("MAIN.FLAGS")),
        [Value::Bool(false), Value::Bool(false), Value::Bool(true)]
    );
    assert_eq!(program.value("MAIN.SUM"), Some(Value::DInt(13)));
    assert_eq!(
        elements(program.value("MAIN.IDLESCALED")),
        [Value::DInt(10), Value::DInt(20), Value::DInt(0)]
    );
}

/// An enumeration, a block that runs a pump by its mode and keeps its last two modes, and a
/// function that gives the next mode.
const MODED: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_enum(name = \"MODE\")]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Off = 0,
    Auto = 1,
    Manual = 5,
}

#[trust_fb(name = \"PUMP\")]
#[budget(\"1us\")]
pub struct Pump {
    #[input]
    pub mode: Mode,
    #[input]
    pub level: i16,
    #[output]
    pub running: bool,
    #[output]
    #[init(Mode::Auto)]
    pub last: Mode,
    #[output]
    pub history: Array<Mode, 1, 2>,
}

impl Body for Pump {
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        self.running = match self.mode {
            Mode::Off => false,
            Mode::Auto => self.level < 50,
            Mode::Manual => true,
        };
        self.history = Array::from_items([self.last, self.mode]);
        self.last = self.mode;
        Ok(())
    }
}

#[trust_function(name = \"NEXT_MODE\")]
#[budget(\"1us\")]
pub fn next_mode(_cx: &mut Cx, mode: Mode) -> Result<Mode, Fault> {
    Ok(match mode {
        Mode::Off => Mode::Auto,
        Mode::Auto => Mode::Manual,
        Mode::Manual => Mode::Off,
    })
}
";

/// A program that runs the pump in its manual mode, asks for the next mode and compares the
/// pump's mode in Structured Text.
const MODED_PROGRAM: &str = "PROGRAM Line
VAR
    pump : PUMP;
    idle : PUMP;
    mode : MODE := Manual;
    running : BOOL;
    last : MODE;
    history : ARRAY[1..2] OF MODE;
    next : MODE;
    isManual : BOOL;
    idleLast : MODE;
END_VAR
pump(mode := mode, level := 80);
running := pump.running;
last := pump.last;
history := pump.history;
next := NEXT_MODE(mode := pump.last);
isManual := pump.last = MODE#Manual;
idleLast := idle.last;
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
";

/// The variant and the value of an enumerated value a program read.
fn variant(value: &Value) -> (String, i64) {
    match value {
        Value::Enum(value) => (value.variant_name().to_string(), value.numeric_value()),
        other => panic!("{other:?} is no enumerated value"),
    }
}

#[test]
fn a_rust_pou_keeps_enumerations_as_structured_text_does() {
    // RU-25, RU-26: a variable of an enumeration reaches the Rust POU and comes back as its value,
    // an input, an output, an array's element and a function's argument and result alike, and its
    // declared variant is its starting value. By hand: the pump runs in Manual (5) whatever its
    // level; its history after three calls is [Manual, Manual]; the next mode after Manual is Off
    // (0); Structured Text takes the pump's mode for MODE#Manual; the instance never called keeps
    // its declared Auto (1). Catches a variant read as its position instead of its value, an
    // enumeration's array element lost, and a starting variant the Rust POU and its declaration
    // disagree on.
    let root = custom_project(
        "rust-pous-moded",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", MODED)],
        MODED_PROGRAM,
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    // The task is due at its start (specification 10 §4.3): three activations, at 0, 10 and
    // 20 ms.
    for time in [0, 10, 20] {
        program
            .scan(Duration::from_millis(time), &mut NoHooks)
            .expect("the scan runs");
    }
    let read = |path: &str| {
        program
            .value(path)
            .unwrap_or_else(|| panic!("{path} is a variable"))
    };
    assert_eq!(read("MAIN.RUNNING"), Value::Bool(true));
    assert_eq!(variant(&read("MAIN.LAST")), ("Manual".into(), 5));
    let history: Vec<(String, i64)> = flat(read("MAIN.HISTORY")).iter().map(variant).collect();
    assert_eq!(history, [("Manual".into(), 5), ("Manual".into(), 5)]);
    assert_eq!(variant(&read("MAIN.NEXT")), ("Off".into(), 0));
    assert_eq!(read("MAIN.ISMANUAL"), Value::Bool(true));
    assert_eq!(variant(&read("MAIN.IDLELAST")), ("Auto".into(), 1));
}

/// An enumeration, two structures, one inside the other, a block that moves a point by a delta and
/// keeps the last segment and a trail of points, and a function that gives a segment's midpoint.
const SHAPED: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

#[trust_enum(name = \"SIDE\")]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left = 0,
    Right = 1,
}

#[trust_struct(name = \"POINT\")]
#[derive(Clone, Copy)]
pub struct Point {
    pub x: f64,
    #[init(2.0)]
    pub y: f64,
}

#[trust_struct(name = \"SEGMENT\")]
#[derive(Clone, Copy)]
pub struct Segment {
    pub origin: Point,
    pub target: Point,
    #[init(Side::Right)]
    pub side: Side,
    #[init(\"seg\")]
    pub name: Str<8>,
}

#[trust_fb(name = \"TRACER\")]
#[budget(\"1us\")]
pub struct Tracer {
    #[input]
    pub delta: Point,
    #[output]
    pub pos: Point,
    #[output]
    pub last: Segment,
    #[output]
    pub trail: Array<Point, 1, 3>,
}

impl Body for Tracer {
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        let next = Point {
            x: self.pos.x + self.delta.x,
            y: self.pos.y + self.delta.y,
        };
        let side = if next.x >= 0.0 { Side::Right } else { Side::Left };
        self.last = Segment {
            origin: self.pos,
            target: next,
            side,
            name: self.last.name,
        };
        let old = self.trail;
        if let [_, second, third] = old.as_slice() {
            self.trail = Array::from_items([*second, *third, next]);
        }
        self.pos = next;
        Ok(())
    }
}

#[trust_function(name = \"MIDPOINT\")]
#[budget(\"1us\")]
pub fn midpoint(_cx: &mut Cx, segment: Segment) -> Result<Point, Fault> {
    Ok(Point {
        x: (segment.origin.x + segment.target.x) / 2.0,
        y: (segment.origin.y + segment.target.y) / 2.0,
    })
}
";

/// A program that moves the tracer three times and reads its structures' fields.
const SHAPED_PROGRAM: &str = "PROGRAM Line
VAR
    tracer : TRACER;
    idle : TRACER;
    delta : POINT := (x := 1.0, y := -0.5);
    middle : POINT;
    posX : LREAL;
    posY : LREAL;
    originX : LREAL;
    targetY : LREAL;
    sideRight : BOOL;
    name : STRING[8];
    trail1X : LREAL;
    trail3Y : LREAL;
    middleX : LREAL;
    middleY : LREAL;
    idleY : LREAL;
END_VAR
tracer(delta := delta);
posX := tracer.pos.x;
posY := tracer.pos.y;
originX := tracer.last.origin.x;
targetY := tracer.last.target.y;
sideRight := tracer.last.side = SIDE#Right;
name := tracer.last.name;
trail1X := tracer.trail[1].x;
trail3Y := tracer.trail[3].y;
middle := MIDPOINT(segment := tracer.last);
middleX := middle.x;
middleY := middle.y;
idleY := idle.pos.y;
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_rust_pou_keeps_structures_as_structured_text_does() {
    // RU-25, RU-28: a structure of the crate reaches the Rust POU and comes back field by field, a
    // structure inside it, its enumeration and string fields, an array of it, an input, an output
    // and a function's argument and result alike; it starts at its fields' initial values. By
    // hand: the tracer starts at (0, 2) and moves by (1, -0.5) at each of three calls, to (1, 1.5),
    // (2, 1) and (3, 0.5); its last segment runs from (2, 1) to (3, 0.5) on the Right, named 'seg'
    // as declared; its trail is [(1, 1.5), (2, 1), (3, 0.5)]; the segment's midpoint is
    // (2.5, 0.75); the instance never called stays at its declared y, 2.0. Catches a field read
    // from another field's cell, a structure inside a structure or an array lost, and a starting
    // value the Rust POU and its declaration disagree on.
    let root = custom_project(
        "rust-pous-shaped",
        &[("Cargo.toml", &manifest("")), ("src/lib.rs", SHAPED)],
        SHAPED_PROGRAM,
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    // The task is due at its start (specification 10 §4.3): three activations, at 0, 10 and
    // 20 ms.
    for time in [0, 10, 20] {
        program
            .scan(Duration::from_millis(time), &mut NoHooks)
            .expect("the scan runs");
    }
    for (path, expected) in [
        ("MAIN.POSX", 3.0),
        ("MAIN.POSY", 0.5),
        ("MAIN.ORIGINX", 2.0),
        ("MAIN.TARGETY", 0.5),
        ("MAIN.TRAIL1X", 1.0),
        ("MAIN.TRAIL3Y", 0.5),
        ("MAIN.MIDDLEX", 2.5),
        ("MAIN.MIDDLEY", 0.75),
        ("MAIN.IDLEY", 2.0),
    ] {
        assert_eq!(program.value(path), Some(Value::LReal(expected)), "{path}");
    }
    assert_eq!(program.value("MAIN.SIDERIGHT"), Some(Value::Bool(true)));
    assert_eq!(
        program.value("MAIN.NAME"),
        Some(Value::String("seg".into()))
    );
}
