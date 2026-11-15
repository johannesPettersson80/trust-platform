//! A Rust POU's variables of the IEC types beside Rust's own reach it and come back as the
//! program's areas store them (specification 48 RU-25, RU-26; specification 44 §6.2): bit strings,
//! characters, durations, dates and times of day, and strings, each with its declared starting value.
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
    // calls at 10, 20 and 30 ms: half of T#500ms is T#250ms, 16#AB AND 16#0F is 16#0B, 'Y' + 1 is
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
    for time in [0, 10, 20, 30] {
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
    // store them, and a date's declared value is its starting value. By hand: three calls at 10,
    // 20 and 30 ms move DT#2024-02-29-22:30:15 by T#1h each, past midnight and the leap day, to
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
    for time in [0, 10, 20, 30] {
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
    for time in [0, 10, 20, 30] {
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
