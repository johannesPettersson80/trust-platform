//! Rust POUs declared with the SDK's attributes start, compute and fault as specification 48 §6
//! gives (RU-22, RU-26, RU-28, RU-56). Every expected value is worked out by hand.

// The expected values are exact, worked out by hand and compared bit for bit.
#![allow(clippy::float_cmp)]

use trust_plc::prelude::*;

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
