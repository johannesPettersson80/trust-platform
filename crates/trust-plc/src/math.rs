//! The elementary functions for Rust POUs (specification 48 §5.5, RU-17).
//!
//! Each is the program library's own, computed with the pure-Rust `libm` that every module links,
//! never a target's library, so that a Rust POU computes the same result on every device
//! (specification 44 NX-04), and the same as Structured Text's `SQRT`, `SIN` and the rest
//! (IEC 61131-3 Table 28): a `REAL` is computed in binary64 and rounded once.

use trust_plc_abi::{Cx, Fault};
use trust_plc_lib::Math;

pub use trust_plc_lib::{atan2_32, atan2_64, expt32, expt64};

/// A real type of IEC 61131-3 that the elementary functions take: `REAL` (`f32`) or `LREAL`
/// (`f64`).
pub trait Elementary: Copy + sealed::Sealed {
    /// `function` of `value`, as the program library computes it for the type.
    ///
    /// # Errors
    ///
    /// `Overflow` for a result that is not finite.
    #[doc(hidden)]
    fn elementary(cx: &mut Cx, function: Math, value: Self) -> Result<Self, Fault>;
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for f32 {}
    impl Sealed for f64 {}
}

impl Elementary for f32 {
    fn elementary(cx: &mut Cx, function: Math, value: f32) -> Result<f32, Fault> {
        trust_plc_lib::real32(cx, function, value)
    }
}

impl Elementary for f64 {
    fn elementary(cx: &mut Cx, function: Math, value: f64) -> Result<f64, Fault> {
        trust_plc_lib::real64(cx, function, value)
    }
}

macro_rules! elementary {
    ($($(#[$doc:meta])* $name:ident => $function:ident;)*) => {$(
        $(#[$doc])*
        ///
        /// # Errors
        ///
        /// `Overflow` for a result that is not finite, as for an argument outside the function's
        /// domain.
        pub fn $name<T: Elementary>(cx: &mut Cx, value: T) -> Result<T, Fault> {
            T::elementary(cx, Math::$function, value)
        }
    )*};
}

elementary! {
    /// The square root, Structured Text's `SQRT`.
    sqrt => Sqrt;
    /// The natural logarithm, `LN`.
    ln => Ln;
    /// The logarithm to base 10, `LOG`.
    log => Log;
    /// The natural exponential, `EXP`.
    exp => Exp;
    /// The sine of an angle in radians, `SIN`.
    sin => Sin;
    /// The cosine, `COS`.
    cos => Cos;
    /// The tangent, `TAN`.
    tan => Tan;
    /// The principal arc sine, `ASIN`.
    asin => Asin;
    /// The principal arc cosine, `ACOS`.
    acos => Acos;
    /// The principal arc tangent, `ATAN`.
    atan => Atan;
}
