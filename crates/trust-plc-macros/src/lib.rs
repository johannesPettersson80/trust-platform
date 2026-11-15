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

use proc_macro::TokenStream;

/// Declares a struct a Rust function block (specification 48 §6.1): its named fields are the
/// block's variables, `#[input]` and `#[output]` ones, internal ones without either, `#[retain]`
/// and `#[init(...)]` ones, in declaration order. The struct implements `trust_plc::Body`, and
/// `#[budget("...")]` gives the longest one call may take.
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

/// The budget of a Rust POU (specification 48 RU-46): the longest one call may take on the slowest
/// target of its project, `500ns`, `15us`, `1.5ms` or `2s`. The POU's attribute reads it, below or
/// above that attribute.
#[proc_macro_attribute]
pub fn budget(args: TokenStream, item: TokenStream) -> TokenStream {
    expand::budget(&args.into(), item.into()).into()
}

#[cfg(test)]
mod tests;
