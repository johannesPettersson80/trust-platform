//! Rust POUs declared with the SDK's attributes start, compute and fault as specification 48 §6
//! gives (RU-22, RU-26, RU-28, RU-56). Every expected value is worked out by hand.

// The expected values are exact, worked out by hand and compared bit for bit.
#![allow(clippy::float_cmp)]

use trust_plc::prelude::*;
use trust_plc::{Declared, Placed, Stored};

/// Exponential moving average of one input: specification 48 RU-22's function block.
#[trust_fb(name = "FILTER_EMA")]
#[budget("15us")]
pub struct FilterEma {
    #[input]
    pub x: f64,
    #[input]
    #[init(0.1)]
    pub alpha: f64,
    #[output]
    pub y: f64,
    primed: bool,
}

impl Body for FilterEma {
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        if self.primed {
            self.y += self.alpha * (self.x - self.y);
        } else {
            self.y = self.x;
            self.primed = true;
        }
        Ok(())
    }
}

/// The larger of two inputs, limited to a maximum: specification 48 RU-22's function, its budget
/// written above its attribute.
///
/// # Errors
///
/// None: the result is one of the inputs.
#[budget("2us")]
#[trust_function(name = "MAX_LIMITED")]
pub fn max_limited(_cx: &mut Cx, a: i32, b: i32, limit: i32) -> Result<i32, Fault> {
    Ok(a.max(b).min(limit))
}

/// Counts the calls its level is above 100 in, with checked arithmetic: its count faults on
/// overflow as Structured Text's does (RU-15).
#[trust_fb(name = "HIGH_COUNTER")]
#[budget("1us")]
pub struct HighCounter {
    #[input]
    pub level: i16,
    #[output]
    pub high: bool,
    #[output]
    pub count: i16,
}

impl Body for HighCounter {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.high = self.level > 100;
        if self.high {
            self.count = checked::add(cx, self.count, 1)?;
        }
        Ok(())
    }
}

/// One variable of each kind of initial value.
#[trust_fb(name = "STARTS")]
#[budget("1us")]
pub struct Starts {
    #[init(true)]
    pub a: bool,
    #[init(-128)]
    pub b: i8,
    #[init(18_446_744_073_709_551_615)]
    pub c: u64,
    #[init(-2.5e-8)]
    pub d: f64,
    #[init(0.1)]
    pub e: f32,
    #[init(2)]
    pub f: f64,
    pub g: u32,
    pub h: bool,
}

impl Body for Starts {
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        Ok(())
    }
}

/// One variable of each date type, each with its initial value but the last.
#[trust_fb(name = "DATED_STARTS")]
#[budget("1us")]
pub struct DatedStarts {
    #[init("D#2024-02-29")]
    pub a: Date,
    #[init("LDATE#1969-12-31")]
    pub b: LDate,
    #[init("TOD#12:30:15.25")]
    pub c: Tod,
    #[init("LTOD#00:00:00.000000001")]
    pub d: LTod,
    #[init("DT#2024-02-29-12:30:15")]
    pub e: Dt,
    #[init("LDT#1970-01-01-00:00:01")]
    pub f: Ldt,
    pub g: Dt,
}

impl Body for DatedStarts {
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        Ok(())
    }
}

#[test]
fn a_function_block_starts_at_its_declared_dates() {
    // RU-26: a date variable starts at its literal's value as the areas store it, else at
    // 1970-01-01 at midnight. By hand: 2024-02-29 is day 19,782, 1,709,164,800,000 ms; 1969-12-31
    // is day -1, -86,400,000,000,000 ns; 12:30:15.25 is 45,015,250 ms. Catches a literal the
    // attribute turns into another day, another unit or the wrong sign.
    let starts = DatedStarts::new();
    assert_eq!(Stored::into_raw(starts.a), 1_709_164_800_000_i64);
    assert_eq!(Stored::into_raw(starts.b), -86_400_000_000_000_i64);
    assert_eq!(Stored::into_raw(starts.c), 45_015_250_i64);
    assert_eq!(Stored::into_raw(starts.d), 1_i64);
    assert_eq!(Stored::into_raw(starts.e), 1_709_209_815_000_i64);
    assert_eq!(Stored::into_raw(starts.f), 1_000_000_000_i64);
    assert_eq!(Stored::into_raw(starts.g), 0_i64);
}

/// Strings with and without their initial values.
#[trust_fb(name = "TEXT_STARTS")]
#[budget("1us")]
pub struct TextStarts {
    #[init("pump 1")]
    pub a: Str<20>,
    #[init("é")]
    pub b: WStr<4>,
    pub c: Str,
}

impl Body for TextStarts {
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        Ok(())
    }
}

#[test]
fn a_function_block_starts_at_its_declared_texts() {
    // RU-26: a string starts at its declared text, else empty, with the capacity its type names
    // (`Str` alone is `Str<80>`). Catches a text the attribute loses or encodes in another form.
    let starts = TextStarts::new();
    assert_eq!(starts.a.as_str(), Some("pump 1"));
    assert_eq!(starts.a.capacity(), 20);
    assert_eq!(starts.b.as_units(), &[0x00E9]);
    assert!(starts.c.is_empty());
    assert_eq!(starts.c.capacity(), 80);
}

/// Arrays with and without their initial lists.
#[trust_fb(name = "ARRAY_STARTS")]
#[budget("1us")]
pub struct ArrayStarts {
    #[init([1, 2])]
    pub a: Array<i16, 1, 4>,
    pub b: Array<bool, 0, 2>,
    #[init(["x", "yz"])]
    pub c: Array<Str<4>, 1, 2>,
    #[init([[1, 2], [3]])]
    pub d: Array<Array<u8, 1, 2>, -1, 2>,
}

impl Body for ArrayStarts {
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        Ok(())
    }
}

#[test]
fn a_function_block_starts_at_its_declared_lists() {
    // RU-26: an array starts at its list's leading elements, the others at their type's default,
    // an array of arrays row by row. Catches a list placed from another index, the elements it
    // leaves out not at their default, and a row of an inner array lost.
    let starts = ArrayStarts::new();
    assert_eq!(starts.a.as_slice(), &[1, 2, 0, 0]);
    assert_eq!(starts.b.as_slice(), &[false, false]);
    assert_eq!(starts.c.get(1).and_then(Str::as_str), Some("x"));
    assert_eq!(starts.c.get(2).and_then(Str::as_str), Some("yz"));
    assert_eq!(starts.d.get(-1).map(Array::as_slice), Some(&[1_u8, 2][..]));
    assert_eq!(starts.d.get(0).map(Array::as_slice), Some(&[3_u8, 0][..]));
}

/// The modes of a pump, one below zero.
#[trust_enum(name = "MODE")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Off = 0,
    Auto = 1,
    Manual = 5,
    Back = -1,
}

/// Variables of the enumeration with and without their initial values.
#[trust_fb(name = "MODE_STARTS")]
#[budget("1us")]
pub struct ModeStarts {
    #[init(Mode::Manual)]
    pub a: Mode,
    pub b: Mode,
    #[init([Mode::Back])]
    pub c: Array<Mode, 1, 2>,
}

impl Body for ModeStarts {
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        Ok(())
    }
}

#[test]
fn an_enumeration_starts_at_its_variant_and_lies_as_its_value() {
    // RU-25, RU-26: a variable of an enumeration starts at the variant written, else at the first
    // variant, the type's default (IEC §6.4.4.2); it lies as its value within INT, and a value none
    // of its variants has reads as the first. Catches a variant stored as its position instead of
    // its value, a value below zero lost, and a stray value that reads as another variant.
    let starts = ModeStarts::new();
    assert_eq!((starts.a, starts.b), (Mode::Manual, Mode::Off));
    assert_eq!(starts.c.as_slice(), &[Mode::Back, Mode::Off]);
    assert_eq!(Stored::into_raw(Mode::Manual), 5_i16);
    assert_eq!(Stored::into_raw(Mode::Back), -1_i16);
    assert_eq!(<Mode as Stored>::from_raw(5), Mode::Manual);
    assert_eq!(<Mode as Stored>::from_raw(2), Mode::Off);
    let cell = core::cell::Cell::new(0_i16);
    Mode::Back.store(&cell);
    assert_eq!(cell.get(), -1);
    assert_eq!(<Mode as Placed>::load(&cell), Mode::Back);
}

#[test]
fn an_in_out_of_an_enumeration_reads_and_writes_its_cell() {
    // RU-29: an in-out of an enumeration is the caller's INT cell, which the generated code passes:
    // it reads the variant of the value there and writes the variant's value at once, an array of
    // them each element's cell. Catches an enumeration without a place of its own, which an in-out
    // of it needs, and a variant written as its position.
    let cell = core::cell::Cell::new(5_i16);
    let mode: InOut<'_, Mode> = InOut::new(&cell);
    assert_eq!(mode.get(), Mode::Manual);
    mode.set(Mode::Back);
    assert_eq!(cell.get(), -1);
    let cells = [core::cell::Cell::new(1_i16), core::cell::Cell::new(0)];
    let modes: InOut<'_, Array<Mode, 0, 2>> = InOut::new(&cells);
    assert_eq!(modes.get().as_slice(), &[Mode::Auto, Mode::Off]);
    modes.set(Array::from_items([Mode::Off, Mode::Manual]));
    assert_eq!([cells[0].get(), cells[1].get()], [0, 5]);
}

/// A structure whose fields start at a number, a variant, a text and a list.
#[trust_struct(name = "SAMPLE")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub x: f64,
    #[init(2.5)]
    pub y: f64,
    #[init(Mode::Auto)]
    pub mode: Mode,
    #[init("ok")]
    pub note: Str<4>,
    #[init([1, 2])]
    pub counts: Array<i16, 1, 3>,
}

/// A block with a variable of the structure and an array of it.
#[trust_fb(name = "SAMPLE_STARTS")]
#[budget("1us")]
pub struct SampleStarts {
    pub one: Sample,
    pub many: Array<Sample, 0, 2>,
}

impl Body for SampleStarts {
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        Ok(())
    }
}

#[test]
fn a_structure_starts_at_its_fields_initial_values() {
    // RU-25, RU-26: a structure's default is its fields' initial values and their types' defaults,
    // a nested enumeration's, string's and array's too, and a variable of it, an array's element
    // too, starts at that default. Catches a field's initial value lost and an array of structures
    // started at zeros.
    let expected = Sample {
        x: 0.0,
        y: 2.5,
        mode: Mode::Auto,
        note: Str::from_text("ok"),
        counts: Array::from_items([1, 2, 0]),
    };
    assert_eq!(<Sample as Declared>::DEFAULT, expected);
    let starts = SampleStarts::new();
    assert_eq!(starts.one, expected);
    assert_eq!(starts.many.as_slice(), &[expected, expected]);
}

#[test]
fn a_function_block_starts_at_its_declared_values() {
    // Catches a start that ignores `#[init]` or a literal turned into another value: RU-26 starts a
    // variable at its declared value, else at its type's default (IEC §6.4.2, Table 10).
    let filter = FilterEma::new();
    assert_eq!((filter.x, filter.alpha, filter.y), (0.0, 0.1, 0.0));
    let starts = Starts::new();
    assert!(starts.a && !starts.h);
    assert_eq!((starts.b, starts.c, starts.g), (-128, u64::MAX, 0));
    assert_eq!((starts.d, starts.e, starts.f), (-2.5e-8, 0.1_f32, 2.0));
}

#[test]
fn a_function_blocks_call_runs_its_body_on_its_variables() -> Result<(), Fault> {
    // Catches a call that loses the variables between calls: the first call primes the filter with
    // its input, each later one moves it by alpha times the difference (1 + 0.5 × (3 − 1) = 2, then
    // 2 + 0.5 × (3 − 2) = 2.5).
    let mut cx = trust_plc::test::cx();
    let mut filter = FilterEma::new();
    filter.x = 1.0;
    filter.alpha = 0.5;
    filter.call(&mut cx)?;
    assert_eq!(filter.y, 1.0);
    filter.x = 3.0;
    filter.call(&mut cx)?;
    assert_eq!(filter.y, 2.0);
    filter.call(&mut cx)?;
    assert_eq!(filter.y, 2.5);
    Ok(())
}

#[test]
#[allow(clippy::used_underscore_items)]
fn the_generated_codes_call_copies_the_variables_in_and_back() -> Result<(), Fault> {
    // Catches a call of the generated code that misses a variable or puts it in another's place:
    // the instance's variables in declaration order, written back after the body (RU-28): 4, then
    // 4 + 0.25 × (8 − 4) = 5.
    let mut cx = trust_plc::test::cx();
    let (mut x, mut alpha, mut y, mut primed) = (4.0, 0.25, 0.0, false);
    FilterEma::__trust_call(&mut cx, &mut x, &mut alpha, &mut y, &mut primed)?;
    assert_eq!((x, alpha, y, primed), (4.0, 0.25, 4.0, true));
    x = 8.0;
    FilterEma::__trust_call(&mut cx, &mut x, &mut alpha, &mut y, &mut primed)?;
    assert_eq!((x, alpha, y, primed), (8.0, 0.25, 5.0, true));
    Ok(())
}

#[test]
#[allow(clippy::used_underscore_items)]
fn a_fault_leaves_the_block_as_it_was() {
    // Catches a call that keeps what the body changed before its fault: RU-28 leaves the variables
    // as the area held them. The body sets `high`, then its count overflows (RU-15: `Overflow`).
    let mut cx = trust_plc::test::cx();
    let mut counter = HighCounter::new();
    counter.level = 200;
    counter.count = i16::MAX;
    let fault = counter.call(&mut cx).expect_err("the count overflows");
    assert_eq!(fault.raw(), FaultCode::Overflow as u32);
    assert_eq!((counter.high, counter.count), (false, i16::MAX));

    let (mut level, mut high, mut count) = (200, false, i16::MAX);
    let fault = HighCounter::__trust_call(&mut cx, &mut level, &mut high, &mut count)
        .expect_err("the count overflows");
    assert_eq!(fault.raw(), FaultCode::Overflow as u32);
    assert_eq!((level, high, count), (200, false, i16::MAX));
}

#[test]
fn a_function_is_called_as_it_is_written() {
    // Catches an attribute that changes the function: MAX_LIMITED(3, 7, 5) is min(max(3, 7), 5) = 5,
    // and MAX_LIMITED(3, 4, 5) is 4.
    let mut cx = trust_plc::test::cx();
    assert_eq!(max_limited(&mut cx, 3, 7, 5), Ok(5));
    assert_eq!(max_limited(&mut cx, 3, 4, 5), Ok(4));
}

/// A program that moves its position half the way to its target and counts its runs.
#[trust_program(name = "AXIS")]
#[budget("2us")]
pub struct Axis {
    #[input]
    pub target: f64,
    #[output]
    pub command: f64,
    #[retain]
    pub runs: u32,
    position: f64,
}

impl Body for Axis {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.runs = checked::add(cx, self.runs, 1)?;
        self.position += (self.target - self.position) * 0.5;
        self.command = self.position;
        Ok(())
    }
}

#[test]
fn a_programs_call_runs_its_body_as_a_blocks_does() -> Result<(), Fault> {
    // RU-35, RU-56: a unit test runs a Rust program's call as it runs a block's: from its starting
    // values, a target of 8 moves the position to 4 and then 6, and counts 2 runs. Catches a
    // program whose attribute gives it no call of the kernel's form.
    let mut cx = trust_plc::test::cx();
    let mut axis = Axis::new();
    axis.target = 8.0;
    axis.call(&mut cx)?;
    axis.call(&mut cx)?;
    assert_eq!((axis.command, axis.runs), (6.0, 2));
    Ok(())
}
