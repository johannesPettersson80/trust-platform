//! The attributes of truST's Rust POUs (specification 48 §6.1).
//!
//! `#[trust_fb(name = "...")]` declares a struct a function block, `#[trust_function(name = "...")]`
//! a function, and `#[budget("...")]` gives either its budget. Each attribute reads its item with
//! `trust-plc-decl`, the reader the build's declarations writer uses too (RU-24), and reports a rule
//! the item breaks as a compile error at the part that breaks it. A function block gains:
//!
//! - `new()`, its starting values: the declared initial values and the types' defaults (RU-26);
//! - `call(&mut self, cx)`, its call as the kernel makes it: the body runs on a copy of the
//!   variables, which replaces them when the body returns `Ok`, so a fault leaves the block as it
//!   was (RU-28); a unit test calls it (RU-56);
//! - the call the program's generated code makes, with the instance's variables in declaration
//!   order, which it copies the same way (RU-28).
//!
//! Use them through `trust_plc`, whose prelude names them. Each attribute's example is a whole
//! program, written as a project writes it and called as a unit test calls it, and runs as a test
//! (RU-89).

#![forbid(unsafe_code)]

mod expand;
mod lower;
mod sequence;

use proc_macro::TokenStream;

/// Declares a struct a Rust function block (specification 48 §6.1): its named fields are the
/// block's variables, `#[input]` and `#[output]` ones, internal ones without either, `#[retain]`
/// and `#[init(...)]` ones, in declaration order. The struct implements `trust_plc::Body`, and
/// `#[budget("...")]` gives the longest one call may take. `implements = "I_RUN, I_RESET"` names
/// the Structured Text interfaces the block implements with its methods (RU-33).
///
/// ```
/// use trust_plc::prelude::*;
///
/// #[trust_fb(name = "FILTER_EMA")]
/// #[budget("15us")]
/// pub struct FilterEma {
///     #[input] pub x: f64,
///     #[input] #[init(0.1)] pub alpha: f64,
///     #[output] pub y: f64,
///     primed: bool,
/// }
///
/// impl Body for FilterEma {
///     fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
///         if self.primed {
///             self.y += self.alpha * (self.x - self.y);
///         } else {
///             self.y = self.x;
///             self.primed = true;
///         }
///         Ok(())
///     }
/// }
///
/// # fn main() -> Result<(), Fault> {
/// // A unit test calls the block as the kernel does (RU-56). The first call takes the input;
/// // each later one moves a tenth of the way to it: 10 + 0.1 × (20 − 10) = 11.
/// let mut cx = trust_plc::test::cx();
/// let mut filter = FilterEma::new();
/// filter.x = 10.0;
/// filter.call(&mut cx)?;
/// assert_eq!(filter.y, 10.0);
/// filter.x = 20.0;
/// filter.call(&mut cx)?;
/// assert_eq!(filter.y, 11.0);
/// # Ok(())
/// # }
/// ```
#[proc_macro_attribute]
pub fn trust_fb(args: TokenStream, item: TokenStream) -> TokenStream {
    expand::function_block(args.into(), item.into()).into()
}

/// Declares a struct a Rust program (specification 48 RU-35): `pub struct` with named fields, its
/// inputs (`#[input]`), outputs (`#[output]`), internal variables and retained ones (`#[retain]`),
/// no in-outs, `impl Body` and `#[budget("...")]`. A configuration runs it like any program.
///
/// ```
/// use trust_plc::prelude::*;
///
/// #[trust_program(name = "AXIS_CONTROLLER")]
/// #[budget("20us")]
/// pub struct AxisController {
///     #[input] pub target: f64,
///     #[output] pub command: f64,
///     #[retain] pub runs: u32,
///     position: f64,
/// }
///
/// impl Body for AxisController {
///     fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
///         self.runs = checked::add(cx, self.runs, 1)?;
///         self.position += (self.target - self.position) * 0.5;
///         self.command = self.position;
///         Ok(())
///     }
/// }
///
/// # fn main() -> Result<(), Fault> {
/// // A unit test runs a program's call as it runs a block's. Each run moves the position half
/// // the way to the target of 8: from 0 to 4, then to 6.
/// let mut cx = trust_plc::test::cx();
/// let mut axis = AxisController::new();
/// axis.target = 8.0;
/// axis.call(&mut cx)?;
/// axis.call(&mut cx)?;
/// assert_eq!((axis.command, axis.runs), (6.0, 2));
/// # Ok(())
/// # }
/// ```
#[proc_macro_attribute]
pub fn trust_program(args: TokenStream, item: TokenStream) -> TokenStream {
    expand::program(args.into(), item.into()).into()
}

/// Declares the `impl` block of a POU's sequence (specification 48 §15): its `async fn`, written as
/// straight-line Rust that awaits `s.next_cycle()`, `s.until(pred)` and `s.wait(t)`, or each or the
/// first of several with `s.all(..)` and `s.race(..)`, each limited by `.within(t)` if it must be,
/// becomes a state machine that each activation runs from its wait point to its next one (RU-71,
/// RU-72). The POU names the sequence, `sequence = "run"`, and has no
/// `impl Body` of its own; its last variable `_seq` holds where the sequence waits and its locals.
///
/// ```
/// use trust_plc::prelude::*;
///
/// #[trust_program(name = "PALLETIZER", sequence = "run")]
/// #[budget("80us")]
/// pub struct Palletizer {
///     #[input] pub part_present: bool,
///     #[output] pub close_gripper: bool,
/// }
///
/// #[trust_sequence]
/// impl Palletizer {
///     async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
///         loop {
///             s.until(|me: &Self| me.part_present).await;
///             self.close_gripper = true;
///             s.wait(Time::from_millis(s, 500)?).await;
///             self.close_gripper = false;
///         }
///     }
/// }
///
/// # fn main() -> Result<(), Fault> {
/// // A task every 100 ms (RU-56). A wait ends at a later activation than the one that began it:
/// // the part, there from 100 ms, closes the gripper at 100 ms, and the 500 ms hold that began
/// // then is over at 600 ms.
/// let mut cx = trust_plc::test::task(Time::from_nanos(100_000_000));
/// let mut palletizer = Palletizer::new();
/// palletizer.call(&mut cx)?;
/// assert!(!palletizer.close_gripper, "0 ms: no part");
/// palletizer.part_present = true;
/// trust_plc::test::advance(&mut cx);
/// palletizer.call(&mut cx)?;
/// assert!(palletizer.close_gripper, "100 ms: the part, the gripper closes");
/// for ms in [200, 300, 400, 500] {
///     trust_plc::test::advance(&mut cx);
///     palletizer.call(&mut cx)?;
///     assert!(palletizer.close_gripper, "{ms} ms: it holds the part");
/// }
/// trust_plc::test::advance(&mut cx);
/// palletizer.call(&mut cx)?;
/// assert!(!palletizer.close_gripper, "600 ms: 500 ms are over, it lets go");
/// # Ok(())
/// # }
/// ```
#[proc_macro_attribute]
pub fn trust_sequence(args: TokenStream, item: TokenStream) -> TokenStream {
    sequence::expand(&args.into(), &item.into()).into()
}

/// Declares a function a Rust function (specification 48 §6.1): `pub fn`, its context first, its
/// inputs after it, and a `Result` of its value and a fault, with `#[budget("...")]`.
///
/// ```
/// use trust_plc::prelude::*;
///
/// #[trust_function(name = "MAX_LIMITED")]
/// #[budget("2us")]
/// pub fn max_limited(_cx: &mut Cx, a: i32, b: i32, limit: i32) -> Result<i32, Fault> {
///     Ok(a.max(b).min(limit))
/// }
///
/// # fn main() {
/// // A unit test calls the function with a context, as the kernel does (RU-56): the larger
/// // input, at most the limit.
/// let mut cx = trust_plc::test::cx();
/// assert_eq!(max_limited(&mut cx, 3, 4, 5), Ok(4));
/// assert_eq!(max_limited(&mut cx, 3, 9, 5), Ok(5));
/// # }
/// ```
#[proc_macro_attribute]
pub fn trust_function(args: TokenStream, item: TokenStream) -> TokenStream {
    expand::function(args.into(), item.into()).into()
}

/// Declares a function of a Rust function block's `impl` block a method of the block
/// (specification 48 RU-32): `pub fn`, `&mut self` first, its context second, then its inputs by
/// value, its in-outs as `InOut<T>` and its outputs as `&mut T`, and a `Result` of its value and a
/// fault, with `#[budget("...")]`. `#[trust_method(private)]`, `(protected)` and `(internal)` give
/// its access specifier; a method without one is public.
///
/// ```
/// use trust_plc::prelude::*;
///
/// #[trust_fb(name = "AXIS")]
/// #[budget("2us")]
/// pub struct Axis {
///     #[input] pub target: f64,
///     #[output] pub position: f64,
/// }
///
/// impl Body for Axis {
///     fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
///         Ok(())
///     }
/// }
///
/// impl Axis {
///     #[trust_method]
///     #[budget("2us")]
///     pub fn jog(&mut self, cx: &mut Cx, distance: f64, reached: &mut bool) -> Result<f64, Fault> {
///         self.position = checked::fadd(cx, self.position, distance)?;
///         *reached = self.position >= self.target;
///         Ok(self.position)
///     }
/// }
///
/// # fn main() -> Result<(), Fault> {
/// // Two jogs of 1.5 towards a target of 2: the first stops short at 1.5, the second passes it.
/// let mut cx = trust_plc::test::cx();
/// let mut axis = Axis::new();
/// axis.target = 2.0;
/// let mut reached = false;
/// assert_eq!(axis.jog(&mut cx, 1.5, &mut reached)?, 1.5);
/// assert!(!reached);
/// assert_eq!(axis.jog(&mut cx, 1.5, &mut reached)?, 3.0);
/// assert!(reached);
/// # Ok(())
/// # }
/// ```
#[proc_macro_attribute]
pub fn trust_method(args: TokenStream, item: TokenStream) -> TokenStream {
    expand::method(args.into(), item.into()).into()
}

/// Declares an enum a Rust enumeration (specification 48 RU-25): its variants hold no data, each
/// has its value within `INT`, and it derives `Clone` and `Copy`. The generated declarations
/// declare it as a closed enumeration of Structured Text with these names and values.
///
/// ```
/// use trust_plc::prelude::*;
///
/// #[trust_enum(name = "MODE")]
/// #[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// pub enum Mode {
///     Off = 0,
///     Auto = 1,
///     Manual = 5,
/// }
///
/// #[trust_fb(name = "PUMP")]
/// #[budget("1us")]
/// pub struct Pump {
///     #[input] #[init(Mode::Auto)] pub mode: Mode,
///     #[input] pub demand: bool,
///     #[output] pub running: bool,
/// }
///
/// impl Body for Pump {
///     fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
///         self.running = match self.mode {
///             Mode::Off => false,
///             Mode::Auto => self.demand,
///             Mode::Manual => true,
///         };
///         Ok(())
///     }
/// }
///
/// # fn main() -> Result<(), Fault> {
/// // The pump starts in Auto, its `#[init]` (RU-26), and runs on demand; in Manual it runs
/// // without one.
/// let mut cx = trust_plc::test::cx();
/// let mut pump = Pump::new();
/// assert_eq!(pump.mode, Mode::Auto);
/// pump.call(&mut cx)?;
/// assert!(!pump.running);
/// pump.mode = Mode::Manual;
/// pump.call(&mut cx)?;
/// assert!(pump.running);
/// # Ok(())
/// # }
/// ```
#[proc_macro_attribute]
pub fn trust_enum(args: TokenStream, item: TokenStream) -> TokenStream {
    expand::enumeration(args.into(), item.into()).into()
}

/// Declares a struct a Rust structure (specification 48 RU-25): its named `pub` fields are of
/// RU-25's types, each with `#[init(...)]` or its type's default, and it derives `Clone` and `Copy`.
/// The generated declarations declare it as a structure of Structured Text with these fields.
///
/// ```
/// use trust_plc::prelude::*;
///
/// #[trust_struct(name = "POINT")]
/// #[derive(Clone, Copy, Debug, PartialEq)]
/// pub struct Point {
///     pub x: f64,
///     #[init(2.0)]
///     pub y: f64,
/// }
///
/// #[trust_fb(name = "MIDPOINT")]
/// #[budget("1us")]
/// pub struct Midpoint {
///     #[input] pub a: Point,
///     #[input] pub b: Point,
///     #[output] pub mid: Point,
/// }
///
/// impl Body for Midpoint {
///     fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
///         self.mid = Point { x: (self.a.x + self.b.x) / 2.0, y: (self.a.y + self.b.y) / 2.0 };
///         Ok(())
///     }
/// }
///
/// # fn main() -> Result<(), Fault> {
/// // Every point starts at its fields' initial values, x at 0 and y at 2 (RU-26). Halfway from
/// // (0, 2) to (4, 6) is (2, 4).
/// let mut midpoint = Midpoint::new();
/// assert_eq!(midpoint.a, Point { x: 0.0, y: 2.0 });
/// midpoint.b = Point { x: 4.0, y: 6.0 };
/// midpoint.call(&mut trust_plc::test::cx())?;
/// assert_eq!(midpoint.mid, Point { x: 2.0, y: 4.0 });
/// # Ok(())
/// # }
/// ```
#[proc_macro_attribute]
pub fn trust_struct(args: TokenStream, item: TokenStream) -> TokenStream {
    expand::structure(args.into(), item.into()).into()
}

/// The budget of a Rust POU (specification 48 RU-46): the longest one call may take on the slowest
/// target of its project, `500ns`, `15us`, `1.5ms` or `2s`. The POU's attribute reads it, below or
/// above that attribute.
///
/// ```
/// use trust_plc::prelude::*;
///
/// #[budget("500ns")]
/// #[trust_function(name = "CLAMP")]
/// pub fn clamp(_cx: &mut Cx, x: f64, low: f64, high: f64) -> Result<f64, Fault> {
///     Ok(x.max(low).min(high))
/// }
///
/// # fn main() {
/// // The budget, here above the attribute, bounds the call on a target and changes nothing in it:
/// // 7.5 limited to 0 to 5 is 5.
/// assert_eq!(clamp(&mut trust_plc::test::cx(), 7.5, 0.0, 5.0), Ok(5.0));
/// # }
/// ```
#[proc_macro_attribute]
pub fn budget(args: TokenStream, item: TokenStream) -> TokenStream {
    expand::budget(&args.into(), item.into()).into()
}

/// A case with which `trust-dev rust bench` measures a Rust POU or method (specification 48
/// RU-48): values for its inputs and in-outs, each written as `#[init(...)]` writes it, the others
/// at their declared initial values; one attribute for each case. The POU's attribute reads it,
/// below or above that attribute. It is not called `bench`: that is Rust's own benchmark attribute,
/// which an attribute of the prelude's glob import cannot shadow.
///
/// ```
/// use trust_plc::prelude::*;
///
/// #[trust_fb(name = "FILTER_EMA")]
/// #[budget("20us")]
/// #[bench_case(x = 1.0e6, alpha = 0.001)]
/// #[bench_case(x = -1.0e6)]
/// pub struct FilterEma {
///     #[input] pub x: f64,
///     #[input] #[init(0.1)] pub alpha: f64,
///     #[output] pub y: f64,
/// }
///
/// impl Body for FilterEma {
///     fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
///         self.y += self.alpha * (self.x - self.y);
///         Ok(())
///     }
/// }
///
/// # fn main() -> Result<(), Fault> {
/// // The cases are for `trust-dev rust bench` and leave the block's own calls as they are:
/// // 0 + 0.1 × (10 − 0) = 1.
/// let mut filter = FilterEma::new();
/// filter.x = 10.0;
/// filter.call(&mut trust_plc::test::cx())?;
/// assert_eq!(filter.y, 1.0);
/// # Ok(())
/// # }
/// ```
#[proc_macro_attribute]
pub fn bench_case(args: TokenStream, item: TokenStream) -> TokenStream {
    expand::bench_case(&args.into(), item.into()).into()
}

#[cfg(test)]
mod tests;
