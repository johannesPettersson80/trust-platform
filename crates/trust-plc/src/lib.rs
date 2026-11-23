//! The SDK of truST's Rust POUs (specification 48).
//!
//! A Rust function, function block or program of a truST project is compiled into the program's
//! module with the generated Structured Text, and keeps the scan's rules (specification 48 §5): it
//! returns faults instead of panicking, allocates nothing and runs without the standard library.
//! This crate gives it the attributes that declare it (`#[trust_fb]`, `#[trust_function]`,
//! `#[trust_method]`, `#[budget]` and `#[bench_case]`, §6.1 and §10; `#[trust_program]`, §7),
//! its own context of a call ([`Cx`]: the task's logical time, the wall clock and its faults,
//! RU-11), and what it needs from the two crates every module already links: the faults of the
//! kernel interface (`trust-plc-abi`), and the standard's arithmetic and mathematics of the
//! program library (`trust-plc-lib`). A Rust POU therefore computes and faults as the Structured
//! Text beside it does: both call the same functions.
//!
//! The crate holds no `unsafe` code; the `unsafe` of a module stays in the interface crate
//! (specification 44 §6.9).
//!
//! # Example
//!
//! A function block that counts the calls in which its level is above 100. Its count uses the
//! checked arithmetic of Structured Text, so it faults on overflow instead of wrapping (RU-15), and
//! a call that faults leaves the block as it was (RU-28). A unit test calls it as the kernel does
//! (RU-56), and `cargo test` runs it with the project's other tests:
//!
//! ```
//! use trust_plc::prelude::*;
//!
//! #[trust_fb(name = "HIGH_COUNTER")]
//! #[budget("1us")]
//! pub struct HighCounter {
//!     #[input] pub level: i16,
//!     #[output] pub high: bool,
//!     #[output] pub count: i16,
//! }
//!
//! impl Body for HighCounter {
//!     fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
//!         self.high = self.level > 100;
//!         if self.high {
//!             self.count = checked::add(cx, self.count, 1)?;
//!         }
//!         Ok(())
//!     }
//! }
//!
//! # fn main() -> Result<(), Fault> {
//! let mut cx = trust_plc::test::cx();
//! let mut counter = HighCounter::new();
//! counter.level = 120;
//! counter.call(&mut cx)?;
//! counter.call(&mut cx)?;
//! assert_eq!((counter.high, counter.count), (true, 2));
//!
//! // At the largest INT the next count overflows: the call faults and changes nothing, so `high`
//! // stays as it was although the body had set it.
//! counter.count = i16::MAX;
//! counter.high = false;
//! let fault = counter.call(&mut cx).expect_err("the count overflows");
//! assert_eq!(fault.raw(), FaultCode::Overflow as u32);
//! assert_eq!((counter.high, counter.count), (false, i16::MAX));
//! # Ok(())
//! # }
//! ```

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod arrays;
pub mod checked;
mod context;
pub mod dates;
pub mod math;
mod places;
pub mod prelude;
mod refs;
pub mod seq;
#[cfg(test)]
mod seq_tests;
pub mod strings;
pub mod test;
pub mod types;

pub use arrays::Array;
pub use context::Cx;
pub use dates::{Date, Dt, LDate, LTod, Ldt, Tod};
pub use places::Place;
pub use refs::{InOut, Ref};
pub use strings::{Str, WStr};
pub use trust_plc_abi::{Fault, FaultCode};
pub use trust_plc_macros::{
    bench_case, budget, trust_enum, trust_fb, trust_function, trust_method, trust_program,
    trust_sequence, trust_struct,
};
pub use types::{
    declared_default, Byte, Char, Declared, Dword, LTime, Lword, Placed, Stored, Time, Value,
    WChar, Word,
};

/// What the generated code calls around a Rust POU (specification 48 RU-11, RU-29). A Rust POU has
/// no use for it: it never holds the kernel interface's context or a reference's raw form.
#[doc(hidden)]
pub mod __private {
    pub use crate::context::{enter, leave};
    pub use crate::places::{PlaceOf, PlaceVia};
    pub use crate::refs::{reference, reference_to, reference_value, reference_via};
    pub use crate::types::assert_value;
}

/// A Rust POU's own fault (specification 48 RU-14): the number the POU chooses and two numbers of
/// detail, which the kernel reports with the Structured Text statement that called the POU. It ends
/// the scan as a fault of Structured Text does; return it from the POU: `return
/// Err(rust_fault(cx, 7, level, limit))`.
pub fn rust_fault(cx: &mut Cx, number: i64, first: i64, second: i64) -> Fault {
    trust_plc_abi::module::fault(cx.raw(), FaultCode::RustFault, number, first, second)
}

/// The body of a Rust function block (specification 48 §6.1): what one call runs on the block's
/// variables. `#[trust_fb]` gives the block the calls that run it as the kernel does (RU-28).
#[diagnostic::on_unimplemented(
    message = "`{Self}` is a Rust function block: implement `trust_plc::Body` for it",
    label = "its body is missing"
)]
pub trait Body {
    /// Runs one call on the block's variables.
    ///
    /// # Errors
    ///
    /// A fault, which ends the scan as a fault of Structured Text does (specification 48 RU-14).
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault>;
}

#[cfg(test)]
mod tests;
