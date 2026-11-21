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

/// RU-70's palletizer: a part arrives, the gripper closes within 2 s, it holds the part 500 ms
/// and lets it go, and the sequence waits for the next part.
#[trust_program(name = "PALLETIZER", sequence = "run")]
#[budget("80us")]
pub struct Palletizer {
    #[input]
    pub part_present: bool,
    #[input]
    pub gripper_closed: bool,
    #[output]
    pub close_gripper: bool,
    #[output]
    pub active_step: Str<16>,
}

#[trust_sequence]
impl Palletizer {
    async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
        loop {
            s.step(&mut self.active_step, "WAIT_PART");
            s.until(|me: &Self| me.part_present).await;
            s.step(&mut self.active_step, "GRIP");
            self.close_gripper = true;
            s.until(|me: &Self| me.gripper_closed)
                .within(Time::from_millis(s, 2_000)?)
                .await?;
            s.wait(Time::from_millis(s, 500)?).await;
            self.close_gripper = false;
        }
    }
}

/// The palletizer's step, its gripper output, and where its sequence waits since when.
fn palletizer(p: &Palletizer) -> (Option<&str>, bool, u16, i64) {
    (
        p.active_step.as_str(),
        p.close_gripper,
        p._seq.point,
        p._seq.began.as_nanos() / 1_000_000,
    )
}

#[test]
fn the_palletizer_runs_its_sequence_activation_by_activation() -> Result<(), Fault> {
    // RU-70 to RU-72, every 10 ms. At 0 ms the top names WAIT_PART and begins the wait for a part,
    // wait point 1. The part comes at 20 ms, a later activation: GRIP, the gripper commanded, and
    // the wait for it, point 2, begun at 20 ms. It closes at 40 ms, 20 ms into its 2 s: the wait of
    // 500 ms, point 3, begins at 40 ms and is over at 540 ms, when the gripper lets go and the
    // sequence waits for a part again, point 1 begun at 540 ms. The part is still there, but a wait
    // completes at a later activation, never at the one that began it: GRIP at 550 ms. Catches a
    // wait that completes at once, a wait of 500 ms that ends a cycle early or late, and a loop that
    // does not go round.
    let mut cx = trust_plc::test::task(Time::from_nanos(10_000_000));
    let mut p = Palletizer::new();
    p.call(&mut cx)?;
    assert_eq!(palletizer(&p), (Some("WAIT_PART"), false, 1, 0), "at 0 ms");
    trust_plc::test::advance(&mut cx);
    p.call(&mut cx)?;
    assert_eq!(
        palletizer(&p),
        (Some("WAIT_PART"), false, 1, 0),
        "at 10 ms, no part"
    );
    p.part_present = true;
    trust_plc::test::advance(&mut cx);
    p.call(&mut cx)?;
    assert_eq!(
        palletizer(&p),
        (Some("GRIP"), true, 2, 20),
        "at 20 ms, the part"
    );
    trust_plc::test::advance(&mut cx);
    p.call(&mut cx)?;
    assert_eq!(
        palletizer(&p),
        (Some("GRIP"), true, 2, 20),
        "at 30 ms, still open"
    );
    p.gripper_closed = true;
    trust_plc::test::advance(&mut cx);
    p.call(&mut cx)?;
    assert_eq!(
        palletizer(&p),
        (Some("GRIP"), true, 3, 40),
        "at 40 ms, closed"
    );
    for _ in 0..49 {
        trust_plc::test::advance(&mut cx);
        p.call(&mut cx)?;
    }
    assert_eq!(
        palletizer(&p),
        (Some("GRIP"), true, 3, 40),
        "at 530 ms, 490 ms held"
    );
    trust_plc::test::advance(&mut cx);
    p.call(&mut cx)?;
    assert_eq!(
        palletizer(&p),
        (Some("WAIT_PART"), false, 1, 540),
        "at 540 ms, 500 ms held"
    );
    trust_plc::test::advance(&mut cx);
    p.call(&mut cx)?;
    assert_eq!(
        palletizer(&p),
        (Some("GRIP"), true, 2, 550),
        "at 550 ms, the next part"
    );
    Ok(())
}

#[test]
fn a_wait_that_outlasts_its_within_faults_with_its_point_and_times() -> Result<(), Fault> {
    // RU-72 and Appendix A: the wait for the gripper, point 2, begins at 10 ms. At 2010 ms it has
    // lasted exactly its 2 s, still in time; at 2020 ms it has lasted 2010 ms, and the activation
    // faults with RustFault and the numbers [2, 2000, 2010]. The faulted call leaves the instance
    // as it was (RU-28): still waiting at point 2 since 10 ms, the gripper still commanded. Catches
    // a limit that faults at the limit, numbers of another wait point or time, and a fault that
    // keeps half of the activation's changes.
    let mut raw = trust_plc_abi::Cx::new(&trust_plc_abi::kernel::WITHOUT_DEBUGGER);
    raw.interval_ns = 10_000_000;
    let mut cx = trust_plc::__private::enter(&raw);
    let mut p = Palletizer::new();
    p.call(&mut cx)?;
    p.part_present = true;
    trust_plc::test::advance(&mut cx);
    p.call(&mut cx)?;
    assert_eq!(palletizer(&p), (Some("GRIP"), true, 2, 10), "at 10 ms");
    for _ in 0..200 {
        trust_plc::test::advance(&mut cx);
        p.call(&mut cx)?;
    }
    assert_eq!(
        palletizer(&p),
        (Some("GRIP"), true, 2, 10),
        "at 2010 ms, exactly 2 s"
    );
    trust_plc::test::advance(&mut cx);
    let fault = p.call(&mut cx).expect_err("at 2020 ms, 2010 ms waited");
    let _ = trust_plc::__private::leave(cx, &mut raw);
    assert_eq!(fault.raw(), FaultCode::RustFault as u32);
    assert_eq!(raw.fault.detail, [2, 2000, 2010]);
    assert_eq!(
        palletizer(&p),
        (Some("GRIP"), true, 2, 10),
        "after the fault"
    );
    Ok(())
}

/// A counter whose sequence counts up to `limit`, a cycle a step, telling even from odd with a
/// wait in each branch, then counts `value` down a cycle a step, counts the round and ends.
#[trust_fb(name = "COUNTER_SEQUENCE", sequence = "count")]
#[budget("20us")]
pub struct CounterSequence {
    #[input]
    pub limit: i16,
    #[output]
    pub value: i16,
    #[output]
    pub even: bool,
    #[output]
    pub rounds: i16,
}

#[trust_sequence]
impl CounterSequence {
    async fn count(&mut self, s: &mut Seq) -> Result<(), Fault> {
        if self.limit <= 0 {
            return Ok(());
        }
        let mut n: i16 = 0;
        loop {
            n = checked::add(s, n, 1)?;
            self.value = n;
            if n >= self.limit {
                break;
            }
            if n & 1 == 0 {
                self.even = true;
                s.next_cycle().await;
            } else {
                self.even = false;
                s.next_cycle().await;
            }
        }
        while self.value > 0 {
            self.value = checked::sub(s, self.value, 1)?;
            s.next_cycle().await;
        }
        self.rounds = checked::add(s, self.rounds, 1)?;
        Ok(())
    }
}

/// The counter's outputs, where its sequence waits, and its local `n`.
fn counter(c: &CounterSequence) -> (i16, bool, i16, u16, i16) {
    (c.value, c.even, c.rounds, c._seq.point, c._seq.n)
}

#[test]
fn a_sequence_keeps_its_locals_and_runs_its_loops_branches_and_end() -> Result<(), Fault> {
    // RU-71 and RU-72 with `limit` 3. The wait points are numbered in the body's order: 1 in the
    // even branch, 2 in the odd one, 3 in the count down. Activation 1 counts n to 1, odd, and
    // waits at 2; 2 counts to 2, even, at 1; 3 counts to 3 = limit and leaves the loop, then counts
    // `value` down to 2 and waits at 3; 4 and 5 count it to 1 and 0; 6 leaves the `while`, counts
    // the round and ends, so the sequence is at its top again (point 0); 7 starts again from n = 0.
    // The local n keeps its value from one activation to the next in `_seq.n`. Catches a local
    // that starts again at each activation, a `break` that does not leave its loop, a branch that
    // resumes at the other branch's wait point, and a sequence that stays at its end.
    let mut cx = trust_plc::test::cx();
    let mut c = CounterSequence::new();
    c.limit = 3;
    let expected = [
        (1, false, 0, 2, 1),
        (2, true, 0, 1, 2),
        (2, true, 0, 3, 3),
        (1, true, 0, 3, 3),
        (0, true, 0, 3, 3),
        (0, true, 1, 0, 3),
        (1, false, 1, 2, 1),
    ];
    for (activation, expected) in expected.into_iter().enumerate() {
        c.call(&mut cx)?;
        assert_eq!(counter(&c), expected, "activation {}", activation + 1);
    }
    Ok(())
}

#[test]
fn a_sequence_that_returns_starts_again_at_its_top() -> Result<(), Fault> {
    // RU-71: with `limit` 0 the body returns at its first statement, at every activation, and the
    // sequence stays at its top without counting a round. Catches a `return` that leaves the
    // sequence waiting where it last waited.
    let mut cx = trust_plc::test::cx();
    let mut c = CounterSequence::new();
    c.call(&mut cx)?;
    c.call(&mut cx)?;
    assert_eq!(counter(&c), (0, false, 0, 0, 0));
    Ok(())
}

/// A press that closes once both guards have reported closed, each a pulse that need not last, and
/// at least 30 ms have passed, all within 100 ms; it presses for one activation and starts again.
#[trust_program(name = "PRESS", sequence = "run")]
#[budget("40us")]
pub struct Press {
    #[input]
    pub left_closed: bool,
    #[input]
    pub right_closed: bool,
    #[output]
    pub pressing: bool,
    #[output]
    pub presses: i16,
}

#[trust_sequence]
impl Press {
    async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
        loop {
            s.all(
                s.until(|me: &Self| me.left_closed),
                s.until(|me: &Self| me.right_closed),
                s.wait(Time::from_millis(s, 30)?),
            )
            .within(Time::from_millis(s, 100)?)
            .await?;
            self.pressing = true;
            self.presses = checked::add(s, self.presses, 1)?;
            s.next_cycle().await;
            self.pressing = false;
        }
    }
}

/// The press's output, its count, and where its sequence waits since when with which members of
/// its `s.all` complete.
fn press(p: &Press) -> (bool, i16, u16, i64, u16) {
    (
        p.pressing,
        p.presses,
        p._seq.point,
        p._seq.began.as_nanos() / 1_000_000,
        p._seq.done.0,
    )
}

#[test]
fn an_all_completes_when_its_last_member_has_and_keeps_those_that_did() -> Result<(), Fault> {
    // RU-72 and IEC §6.7.5 Table 61 features 4a and 4b, every 10 ms. At 0 ms the s.all, point 1,
    // begins with no member complete. The left guard pulses at 10 ms: member 1 completes, done
    // 0b001, and stays complete at 20 ms when the pulse is gone. At 30 ms 30 ms have passed:
    // member 3, done 0b101. The right guard closes at 40 ms, the last member: the press closes,
    // counts 1 and waits a cycle, point 2 begun at 40 ms, done 0 again. At 50 ms it opens and the
    // s.all begins anew. Catches members that must hold at one activation together (the level
    // `until(a && b)`), a member that completes at the activation that began the wait, and a
    // `done` that leaks into the next wait.
    let mut cx = trust_plc::test::task(Time::from_nanos(10_000_000));
    let mut p = Press::new();
    p.call(&mut cx)?;
    assert_eq!(press(&p), (false, 0, 1, 0, 0b000), "at 0 ms");
    p.left_closed = true;
    trust_plc::test::advance(&mut cx);
    p.call(&mut cx)?;
    assert_eq!(
        press(&p),
        (false, 0, 1, 0, 0b001),
        "at 10 ms, the left pulse"
    );
    p.left_closed = false;
    trust_plc::test::advance(&mut cx);
    p.call(&mut cx)?;
    assert_eq!(
        press(&p),
        (false, 0, 1, 0, 0b001),
        "at 20 ms, the pulse gone"
    );
    trust_plc::test::advance(&mut cx);
    p.call(&mut cx)?;
    assert_eq!(press(&p), (false, 0, 1, 0, 0b101), "at 30 ms, 30 ms passed");
    p.right_closed = true;
    trust_plc::test::advance(&mut cx);
    p.call(&mut cx)?;
    assert_eq!(
        press(&p),
        (true, 1, 2, 40, 0b000),
        "at 40 ms, the right guard"
    );
    trust_plc::test::advance(&mut cx);
    p.call(&mut cx)?;
    assert_eq!(press(&p), (false, 1, 1, 50, 0b000), "at 50 ms, anew");
    Ok(())
}

#[test]
fn an_all_that_would_complete_too_late_faults_first() -> Result<(), Fault> {
    // RU-72: `.within(t)` limits the whole s.all and is checked before its members. The left
    // guard pulses at 10 ms (done 0b001), 30 ms pass at 30 ms (0b101), and at 100 ms the wait has
    // lasted exactly its 100 ms, still in time. The right guard closes at 110 ms, which would
    // complete it, but 110 ms is too late: RustFault with [1, 100, 110], and the instance as it
    // was before that activation (RU-28), done 0b101 and the press open. Catches a limit checked
    // after the members, and one that faults at exactly its time.
    let mut raw = trust_plc_abi::Cx::new(&trust_plc_abi::kernel::WITHOUT_DEBUGGER);
    raw.interval_ns = 10_000_000;
    let mut cx = trust_plc::__private::enter(&raw);
    let mut p = Press::new();
    p.call(&mut cx)?;
    p.left_closed = true;
    trust_plc::test::advance(&mut cx);
    p.call(&mut cx)?;
    p.left_closed = false;
    for _ in 0..9 {
        trust_plc::test::advance(&mut cx);
        p.call(&mut cx)?;
    }
    assert_eq!(
        press(&p),
        (false, 0, 1, 0, 0b101),
        "at 100 ms, exactly 100 ms"
    );
    p.right_closed = true;
    trust_plc::test::advance(&mut cx);
    let fault = p.call(&mut cx).expect_err("at 110 ms, too late");
    let _ = trust_plc::__private::leave(cx, &mut raw);
    assert_eq!(fault.raw(), FaultCode::RustFault as u32);
    assert_eq!(raw.fault.detail, [1, 100, 110]);
    assert_eq!(press(&p), (false, 0, 1, 0, 0b101), "after the fault");
    Ok(())
}

/// A feeder that waits for a part, a stop or 50 ms, whichever comes first: it counts a part fed
/// and a timeout missed, and keeps the number of the member that ended the wait.
#[trust_fb(name = "FEEDER", sequence = "feed")]
#[budget("40us")]
pub struct Feeder {
    #[input]
    pub part: bool,
    #[input]
    pub stop: bool,
    #[output]
    pub fed: i16,
    #[output]
    pub missed: i16,
    #[output]
    pub ended: u16,
}

#[trust_sequence]
impl Feeder {
    async fn feed(&mut self, s: &mut Seq) -> Result<(), Fault> {
        loop {
            let won: u16 = s
                .race(
                    s.until(|me: &Self| me.part),
                    s.until(|me: &Self| me.stop),
                    s.wait(Time::from_millis(s, 50)?),
                )
                .await;
            self.ended = won;
            if won == 1 {
                self.fed = checked::add(s, self.fed, 1)?;
            } else if won == 3 {
                self.missed = checked::add(s, self.missed, 1)?;
            }
            s.next_cycle().await;
        }
    }
}

/// The feeder's counts, the member that ended its last race, and where its sequence waits since
/// when, with its local `won`.
fn feeder(f: &Feeder) -> (i16, i16, u16, u16, i64, u16) {
    (
        f.fed,
        f.missed,
        f.ended,
        f._seq.point,
        f._seq.began.as_nanos() / 1_000_000,
        f._seq.won,
    )
}

#[test]
fn a_race_completes_with_its_first_member_in_order_that_completes() -> Result<(), Fault> {
    // RU-72 and IEC §6.7.5 Table 61 feature 2a, every 10 ms. At 0 ms the race, point 1, begins.
    // At 20 ms a part and a stop come together: both complete, and the first in order wins, 1, a
    // part fed; the cycle's wait, point 2, begins at 20 ms. At 30 ms the race begins anew; the part
    // is still there but a wait never completes at the activation that began it. Part and stop go;
    // at 80 ms 50 ms have passed since 30 ms: member 3 wins, a miss, point 2 at 80 ms. The race
    // begins at 90 ms, and the stop at 100 ms ends it, 2, neither fed nor missed. Catches a race
    // that takes the last member that completed, one that completes at once, a timeout counted
    // from the wrong activation, and a value that is not the member's number from 1.
    let mut cx = trust_plc::test::task(Time::from_nanos(10_000_000));
    let mut f = Feeder::new();
    f.call(&mut cx)?;
    assert_eq!(feeder(&f), (0, 0, 0, 1, 0, 0), "at 0 ms");
    trust_plc::test::advance(&mut cx);
    f.call(&mut cx)?;
    assert_eq!(feeder(&f), (0, 0, 0, 1, 0, 0), "at 10 ms, nothing");
    f.part = true;
    f.stop = true;
    trust_plc::test::advance(&mut cx);
    f.call(&mut cx)?;
    assert_eq!(feeder(&f), (1, 0, 1, 2, 20, 1), "at 20 ms, part and stop");
    trust_plc::test::advance(&mut cx);
    f.call(&mut cx)?;
    assert_eq!(feeder(&f), (1, 0, 1, 1, 30, 1), "at 30 ms, anew");
    f.part = false;
    f.stop = false;
    for _ in 0..4 {
        trust_plc::test::advance(&mut cx);
        f.call(&mut cx)?;
    }
    assert_eq!(feeder(&f), (1, 0, 1, 1, 30, 1), "at 70 ms, 40 ms waited");
    trust_plc::test::advance(&mut cx);
    f.call(&mut cx)?;
    assert_eq!(feeder(&f), (1, 1, 3, 2, 80, 3), "at 80 ms, 50 ms waited");
    trust_plc::test::advance(&mut cx);
    f.call(&mut cx)?;
    assert_eq!(feeder(&f), (1, 1, 3, 1, 90, 3), "at 90 ms, anew");
    f.stop = true;
    trust_plc::test::advance(&mut cx);
    f.call(&mut cx)?;
    assert_eq!(feeder(&f), (1, 1, 2, 2, 100, 2), "at 100 ms, the stop");
    Ok(())
}
