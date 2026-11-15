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

use crate::{Cx, Fault};

pub use trust_plc_lib::{fneg, Integer, Real, Signed};

/// The program library's operations that record their fault in the context, each taking a Rust
/// POU's context and calling the library's with the kernel interface's one inside it.
macro_rules! checked {
    ($($(#[$doc:meta])* $name:ident<$bound:ident>($($argument:ident),+);)*) => {$(
        $(#[$doc])*
        #[inline]
        pub fn $name<T: $bound>(cx: &mut Cx, $($argument: T),+) -> Result<T, Fault> {
            trust_plc_lib::$name(cx.raw(), $($argument),+)
        }
    )*};
}

checked! {
    /// `a + b` of integers, Structured Text's `+`.
    ///
    /// # Errors
    ///
    /// `Overflow` for a result outside the type.
    add<Integer>(a, b);
    /// `a - b` of integers.
    ///
    /// # Errors
    ///
    /// `Overflow` for a result outside the type.
    sub<Integer>(a, b);
    /// `a * b` of integers.
    ///
    /// # Errors
    ///
    /// `Overflow` for a result outside the type.
    mul<Integer>(a, b);
    /// `a / b` of integers, truncated toward zero (IEC §6.6.2.5.8).
    ///
    /// # Errors
    ///
    /// `DivisionByZero` for a divisor of zero; `Overflow` for the minimum divided by -1.
    div<Integer>(a, b);
    /// `a MOD b`, with the sign of `a`; `MIN MOD -1` is 0.
    ///
    /// # Errors
    ///
    /// `ModuloByZero` for a divisor of zero.
    rem<Integer>(a, b);
    /// `a ** b` of integers.
    ///
    /// # Errors
    ///
    /// `TypeMismatch` for a negative exponent; `Overflow` for an exponent above `u32::MAX` and for
    /// a result outside the type.
    pow<Integer>(a, b);
    /// `-a` of a signed integer.
    ///
    /// # Errors
    ///
    /// `Overflow` for the type's minimum.
    neg<Signed>(a);
    /// `ABS(a)` of a signed integer.
    ///
    /// # Errors
    ///
    /// `Overflow` for the type's minimum.
    abs<Signed>(a);
    /// `value` when it is finite, as Structured Text checks a real result.
    ///
    /// # Errors
    ///
    /// `Overflow` for an infinite or NaN value.
    finite<Real>(value);
    /// `a + b` of reals.
    ///
    /// # Errors
    ///
    /// `Overflow` for a result that is not finite.
    fadd<Real>(a, b);
    /// `a - b` of reals.
    ///
    /// # Errors
    ///
    /// `Overflow` for a result that is not finite.
    fsub<Real>(a, b);
    /// `a * b` of reals.
    ///
    /// # Errors
    ///
    /// `Overflow` for a result that is not finite.
    fmul<Real>(a, b);
    /// `a / b` of reals.
    ///
    /// # Errors
    ///
    /// `DivisionByZero` for a divisor of zero; `Overflow` for a result that is not finite.
    fdiv<Real>(a, b);
    /// `a ** b` of reals, computed in binary64 and rounded once.
    ///
    /// # Errors
    ///
    /// `Overflow` for a result that is not finite.
    fpow<Real>(a, b);
}
