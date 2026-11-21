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
//! Use them through `trust_plc`, whose prelude names them.

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
/// ```ignore
/// #[trust_fb(name = "FILTER_EMA")]
/// #[budget("15us")]
/// pub struct FilterEma {
///     #[input] pub x: f64,
///     #[input] #[init(0.1)] pub alpha: f64,
///     #[output] pub y: f64,
///     primed: bool,
/// }
/// ```
#[proc_macro_attribute]
pub fn trust_fb(args: TokenStream, item: TokenStream) -> TokenStream {
    expand::function_block(args.into(), item.into()).into()
}

/// Declares a struct a Rust program (specification 48 RU-35): `pub struct` with named fields, its
/// inputs (`#[input]`), outputs (`#[output]`), internal variables and retained ones (`#[retain]`),
/// no in-outs, `impl Body` and `#[budget("...")]`. A configuration runs it like any program.
///
/// ```ignore
/// #[trust_program(name = "AXIS_CONTROLLER")]
/// #[budget("20us")]
/// pub struct AxisController {
///     #[input] pub target: f64,
///     #[output] pub command: f64,
///     #[retain] pub runs: u32,
/// }
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
/// ```ignore
/// #[trust_program(name = "PALLETIZER", sequence = "run")]
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
/// ```
#[proc_macro_attribute]
pub fn trust_sequence(args: TokenStream, item: TokenStream) -> TokenStream {
    sequence::expand(&args.into(), &item.into()).into()
}

/// Declares a function a Rust function (specification 48 §6.1): `pub fn`, its context first, its
/// inputs after it, and a `Result` of its value and a fault, with `#[budget("...")]`.
///
/// ```ignore
/// #[trust_function(name = "MAX_LIMITED")]
/// #[budget("2us")]
/// pub fn max_limited(_cx: &mut Cx, a: i32, b: i32, limit: i32) -> Result<i32, Fault> {
///     Ok(a.max(b).min(limit))
/// }
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
/// ```ignore
/// impl Axis {
///     #[trust_method]
///     #[budget("2us")]
///     pub fn jog(&mut self, cx: &mut Cx, step: f64, reached: &mut bool) -> Result<f64, Fault> {
///         self.position = checked::fadd(cx, self.position, step)?;
///         *reached = self.position >= self.target;
///         Ok(self.position)
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn trust_method(args: TokenStream, item: TokenStream) -> TokenStream {
    expand::method(args.into(), item.into()).into()
}

/// Declares an enum a Rust enumeration (specification 48 RU-25): its variants hold no data, each
/// has its value within `INT`, and it derives `Clone` and `Copy`. The generated declarations
/// declare it as a closed enumeration of Structured Text with these names and values.
///
/// ```ignore
/// #[trust_enum(name = "MODE")]
/// #[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// pub enum Mode {
///     Off = 0,
///     Auto = 1,
///     Manual = 5,
/// }
/// ```
#[proc_macro_attribute]
pub fn trust_enum(args: TokenStream, item: TokenStream) -> TokenStream {
    expand::enumeration(args.into(), item.into()).into()
}

/// Declares a struct a Rust structure (specification 48 RU-25): its named `pub` fields are of
/// RU-25's types, each with `#[init(...)]` or its type's default, and it derives `Clone` and `Copy`.
/// The generated declarations declare it as a structure of Structured Text with these fields.
///
/// ```ignore
/// #[trust_struct(name = "POINT")]
/// #[derive(Clone, Copy, Debug, PartialEq)]
/// pub struct Point {
///     pub x: f64,
///     #[init(2.0)]
///     pub y: f64,
/// }
/// ```
#[proc_macro_attribute]
pub fn trust_struct(args: TokenStream, item: TokenStream) -> TokenStream {
    expand::structure(args.into(), item.into()).into()
}

/// The budget of a Rust POU (specification 48 RU-46): the longest one call may take on the slowest
/// target of its project, `500ns`, `15us`, `1.5ms` or `2s`. The POU's attribute reads it, below or
/// above that attribute.
#[proc_macro_attribute]
pub fn budget(args: TokenStream, item: TokenStream) -> TokenStream {
    expand::budget(&args.into(), item.into()).into()
}

#[cfg(test)]
mod tests;
