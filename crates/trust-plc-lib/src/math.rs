//! The numerical functions of IEC §6.6.2.5.6 (Table 23) with the runtime's rules: a real function
//! computes in binary64, also of a `REAL`, through the same `libm` the runtime uses, and a result
//! that is no finite number, an infinity or NaN, is an overflow; a `REAL` result is then rounded to
//! binary32 and must be finite there too (specification 07 §3).

use trust_plc_abi::module::fault;
use trust_plc_abi::{Cx, Fault, FaultCode};

/// A function of one real: `SQRT`, `LN`, `LOG`, `EXP`, `SIN`, `COS`, `TAN`, `ASIN`, `ACOS`, `ATAN`.
#[derive(Clone, Copy, Debug)]
pub enum Math {
    /// `SQRT`.
    Sqrt,
    /// `LN`.
    Ln,
    /// `LOG`, to base 10.
    Log,
    /// `EXP`.
    Exp,
    /// `SIN`.
    Sin,
    /// `COS`.
    Cos,
    /// `TAN`.
    Tan,
    /// `ASIN`.
    Asin,
    /// `ACOS`.
    Acos,
    /// `ATAN`.
    Atan,
}

impl Math {
    fn apply(self, value: f64) -> f64 {
        match self {
            Self::Sqrt => libm::sqrt(value),
            Self::Ln => libm::log(value),
            Self::Log => libm::log10(value),
            Self::Exp => libm::exp(value),
            Self::Sin => libm::sin(value),
            Self::Cos => libm::cos(value),
            Self::Tan => libm::tan(value),
            Self::Asin => libm::asin(value),
            Self::Acos => libm::acos(value),
            Self::Atan => libm::atan(value),
        }
    }
}

fn overflow(cx: &mut Cx) -> Fault {
    fault(cx, FaultCode::Overflow, 0, 0, 0)
}

/// A binary64 result as a `REAL`.
///
/// # Errors
///
/// `Overflow` for a result that is not finite in binary64 or in binary32.
fn narrow(cx: &mut Cx, result: f64) -> Result<f32, Fault> {
    if !result.is_finite() {
        return Err(overflow(cx));
    }
    #[allow(clippy::cast_possible_truncation)]
    let narrowed = result as f32;
    if narrowed.is_finite() {
        Ok(narrowed)
    } else {
        Err(overflow(cx))
    }
}

/// A binary64 result as an `LREAL`.
///
/// # Errors
///
/// `Overflow` for a result that is not finite.
fn finite(cx: &mut Cx, result: f64) -> Result<f64, Fault> {
    if result.is_finite() {
        Ok(result)
    } else {
        Err(overflow(cx))
    }
}

/// `function` of a `REAL`, computed in binary64.
///
/// # Errors
///
/// `Overflow` for a result that is not finite, as `SQRT` of a negative number gives.
pub fn real32(cx: &mut Cx, function: Math, value: f32) -> Result<f32, Fault> {
    narrow(cx, function.apply(f64::from(value)))
}

/// `function` of an `LREAL`.
///
/// # Errors
///
/// `Overflow` for a result that is not finite.
pub fn real64(cx: &mut Cx, function: Math, value: f64) -> Result<f64, Fault> {
    finite(cx, function.apply(value))
}

/// `EXPT(IN1, IN2)` of a `REAL` base: `IN1 ** IN2` in binary64, the exponent in binary64 as the
/// runtime converts it.
///
/// # Errors
///
/// `Overflow` for a result that is not finite.
pub fn expt32(cx: &mut Cx, base: f32, exponent: f64) -> Result<f32, Fault> {
    narrow(cx, libm::pow(f64::from(base), exponent))
}

/// `EXPT(IN1, IN2)` of an `LREAL` base.
///
/// # Errors
///
/// `Overflow` for a result that is not finite.
pub fn expt64(cx: &mut Cx, base: f64, exponent: f64) -> Result<f64, Fault> {
    finite(cx, libm::pow(base, exponent))
}

/// `ATAN2(Y, X)` of two `REAL`s, in binary32 as the runtime computes it.
#[must_use]
pub fn atan2_32(y: f32, x: f32) -> f32 {
    libm::atan2f(y, x)
}

/// `ATAN2(Y, X)` of two `LREAL`s.
#[must_use]
pub fn atan2_64(y: f64, x: f64) -> f64 {
    libm::atan2(y, x)
}
