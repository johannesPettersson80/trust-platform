//! The standard's arithmetic for Rust POUs (specification 48 §5.3, RU-15).
//!
//! Structured Text faults on an integer result outside its type, on a division or a `MOD` by zero
//! and on a real result that is not finite (specification 44 §6.5); Rust's own operators wrap or
//! panic instead. These are the program library's operations, the ones the generated Structured
//! Text calls: each returns the fault and records its detail in the context, so that a Rust POU and
//! the Structured Text beside it give the same result and the same fault for the same values:
//!
//! - [`add`], [`sub`], [`mul`], [`pow`], [`neg`] and [`abs`] of integers: `Overflow` for a result
//!   outside the type;
//! - [`div`]: `DivisionByZero`, and `Overflow` for the minimum divided by -1; [`rem`]:
//!   `ModuloByZero`, with the dividend's sign as `MOD` has it;
//! - [`fadd`], [`fsub`], [`fmul`], [`fpow`] and [`finite`] of reals: `Overflow` for a result that is
//!   not finite; [`fdiv`]: `DivisionByZero` for a zero divisor, as for integers, and `Overflow` for
//!   a result that is not finite; [`fneg`] cannot fault.

pub use trust_plc_lib::{
    abs, add, div, fadd, fdiv, finite, fmul, fneg, fpow, fsub, mul, neg, pow, rem, sub, Integer,
    Real, Signed,
};
