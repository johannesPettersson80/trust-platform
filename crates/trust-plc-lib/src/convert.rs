//! Type conversions (IEC §6.6.2.5.2 to §6.6.2.5.7; specification 07 §2), with the runtime's
//! results and faults: a real is rounded to the nearest integer with ties to even (`TRUNC`
//! truncates), an integer outside its destination is an overflow, a bit string converts by its
//! bits, and a `TIME` converts to a `DWORD` of whole milliseconds.

use trust_plc_abi::module::fault;
use trust_plc_abi::{Cx, Fault, FaultCode};

use crate::arith::{finite, Real};

/// How a real becomes an integer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rounding {
    /// To the nearest integer, a tie to the even one: `REAL_TO_INT` (IEC §6.6.2.5.2, Table 22).
    Nearest,
    /// Toward zero: `TRUNC`.
    Truncate,
}

/// An integer type a conversion can produce.
pub trait IntTarget: Copy {
    /// Whether the type is signed.
    const SIGNED: bool;
    /// `value` in the type, or `None` outside it.
    fn from_i64(value: i64) -> Option<Self>;
    /// `value` in the type, or `None` outside it.
    fn from_u64(value: u64) -> Option<Self>;
}

macro_rules! int_target {
    ($($t:ty: $signed:expr),*) => {$(
        impl IntTarget for $t {
            const SIGNED: bool = $signed;
            fn from_i64(value: i64) -> Option<Self> {
                Self::try_from(value).ok()
            }
            fn from_u64(value: u64) -> Option<Self> {
                Self::try_from(value).ok()
            }
        }
    )*};
}

int_target!(i8: true, i16: true, i32: true, i64: true, u8: false, u16: false, u32: false,
    u64: false);

fn overflow(cx: &mut Cx, detail: i64) -> Fault {
    fault(cx, FaultCode::Overflow, detail, 0, 0)
}

/// `value` rounded to the nearest integer, a tie to the even one, as the runtime rounds a real to
/// an integer: `libm`'s `trunc` and `round` on every target.
#[must_use]
pub fn round_ties_to_even(value: f64) -> f64 {
    let truncated = libm::trunc(value);
    let fraction = value - truncated;
    // Exactly one half: the fraction of a binary64 is exact, so the comparison is exact.
    #[allow(clippy::float_cmp)]
    let tie = libm::fabs(fraction) == 0.5;
    if tie {
        // A binary64 half-integer has a magnitude below 2^52, so its truncated part is exact in an
        // i64.
        #[allow(clippy::cast_possible_truncation)]
        let even = (truncated as i64) & 1 == 0;
        if even {
            truncated
        } else {
            truncated + if fraction < 0.0 { -1.0 } else { 1.0 }
        }
    } else {
        libm::round(value)
    }
}

/// The real `value`, in binary64, as the integer type `T`: `REAL_TO_DINT`, `LREAL_TO_INT`, `TRUNC`.
///
/// # Errors
///
/// `Overflow` for a value that is not finite or whose integer is outside `T`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn real_to_int<T: IntTarget>(cx: &mut Cx, value: f64, rounding: Rounding) -> Result<T, Fault> {
    if !value.is_finite() {
        return Err(overflow(cx, 0));
    }
    let rounded = match rounding {
        Rounding::Nearest => round_ties_to_even(value),
        Rounding::Truncate => libm::trunc(value),
    };
    let converted = if T::SIGNED {
        if (-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&rounded) {
            T::from_i64(rounded as i64)
        } else {
            None
        }
    } else if (0.0..18_446_744_073_709_551_616.0).contains(&rounded) {
        T::from_u64(rounded as u64)
    } else {
        None
    };
    converted.ok_or_else(|| overflow(cx, rounded as i64))
}

/// The signed integer `value` as the integer type `T`: `DINT_TO_INT`, `INT_TO_UDINT`.
///
/// # Errors
///
/// `Overflow` outside `T`.
#[inline]
pub fn sint_to<T: IntTarget>(cx: &mut Cx, value: i64) -> Result<T, Fault> {
    T::from_i64(value).ok_or_else(|| overflow(cx, value))
}

/// The unsigned integer `value` as the integer type `T`: `UDINT_TO_INT`.
///
/// # Errors
///
/// `Overflow` outside `T`.
#[inline]
#[allow(clippy::cast_possible_wrap)]
pub fn uint_to<T: IntTarget>(cx: &mut Cx, value: u64) -> Result<T, Fault> {
    T::from_u64(value).ok_or_else(|| overflow(cx, value as i64))
}

/// A signed integer as a `REAL`: rounded to binary64 and then to binary32, as the runtime converts.
#[must_use]
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
pub fn sint_to_real32(value: i64) -> f32 {
    (value as f64) as f32
}

/// A signed integer as an `LREAL`.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn sint_to_real64(value: i64) -> f64 {
    value as f64
}

/// An unsigned integer as a `REAL`: rounded to binary64 and then to binary32.
#[must_use]
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
pub fn uint_to_real32(value: u64) -> f32 {
    (value as f64) as f32
}

/// An unsigned integer as an `LREAL`.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn uint_to_real64(value: u64) -> f64 {
    value as f64
}

/// A real as an `LREAL`: `REAL_TO_LREAL`, `LREAL_TO_LREAL`.
///
/// # Errors
///
/// `Overflow` for a value that is not finite.
#[inline]
pub fn real32_to_real<T: Real>(cx: &mut Cx, value: f32) -> Result<T, Fault> {
    finite(cx, T::narrow(f64::from(value)))
}

/// An `LREAL` as a real: `LREAL_TO_REAL` rounds to binary32.
///
/// # Errors
///
/// `Overflow` for a value that is not finite in the destination, such as an `LREAL` above the
/// largest `REAL`.
#[inline]
pub fn real64_to_real32<T: Real>(cx: &mut Cx, value: f64) -> Result<T, Fault> {
    finite(cx, T::narrow(value))
}

/// `DWORD_TO_REAL`: the bits of a binary32.
///
/// # Errors
///
/// `Overflow` for bits that are an infinity or NaN.
#[inline]
pub fn bits_to_real32(cx: &mut Cx, bits: u32) -> Result<f32, Fault> {
    finite(cx, f32::from_bits(bits))
}

/// `LWORD_TO_LREAL`: the bits of a binary64.
///
/// # Errors
///
/// `Overflow` for bits that are an infinity or NaN.
#[inline]
pub fn bits_to_real64(cx: &mut Cx, bits: u64) -> Result<f64, Fault> {
    finite(cx, f64::from_bits(bits))
}

/// `TIME_TO_DWORD`: the whole milliseconds of a duration (specification 07 §2.8).
///
/// # Errors
///
/// `Overflow` for a negative duration and for one of more than `u32::MAX` milliseconds.
#[inline]
pub fn time_to_dword(cx: &mut Cx, nanos: i64) -> Result<u32, Fault> {
    if nanos < 0 {
        return Err(overflow(cx, nanos));
    }
    u32::try_from(nanos / 1_000_000).map_err(|_| overflow(cx, nanos))
}

/// `DWORD_TO_TIME`: a duration of `millis` milliseconds, in nanoseconds.
#[must_use]
pub fn dword_to_time(millis: u32) -> i64 {
    i64::from(millis) * 1_000_000
}
