//! The operators that can fault (specification 05 §3, specification 10 §10; specification 44
//! §6.5): integer arithmetic checked in its type, real arithmetic checked for a finite result.

use core::ops::{Add, Div, Mul, Neg, Sub};

use trust_plc_abi::module::fault;
use trust_plc_abi::{Cx, Fault, FaultCode};

/// An integer type of IEC 61131-3: `SINT` to `ULINT`, and the bit strings as unsigned integers.
pub trait Integer: Copy + PartialEq + PartialOrd {
    /// The zero of the type.
    const ZERO: Self;
    /// `self + other`, or `None` outside the type.
    fn checked_add_(self, other: Self) -> Option<Self>;
    /// `self - other`, or `None` outside the type.
    fn checked_sub_(self, other: Self) -> Option<Self>;
    /// `self * other`, or `None` outside the type.
    fn checked_mul_(self, other: Self) -> Option<Self>;
    /// `self / other` truncated toward zero, or `None` outside the type; `other` is not zero.
    fn checked_div_(self, other: Self) -> Option<Self>;
    /// `self MOD other` with the dividend's sign, `0` for a divisor of -1; `other` is not zero.
    #[must_use]
    fn iec_rem(self, other: Self) -> Self;
    /// `self ** exponent` for a non-negative exponent, or `None` outside the type.
    fn checked_pow_(self, exponent: u32) -> Option<Self>;
    /// The value as a fault's detail: exact for every signed value, wrapped above `i64::MAX`.
    fn detail(self) -> i64;
    /// The value as an exponent.
    ///
    /// # Errors
    ///
    /// `Err(true)` below zero, `Err(false)` above `u32::MAX`.
    fn exponent(self) -> Result<u32, bool>;
}

/// A signed integer type, which negation and `ABS` can overflow.
pub trait Signed: Integer {
    /// `-self`, or `None` for the type's minimum.
    fn checked_neg_(self) -> Option<Self>;
    /// `|self|`, or `None` for the type's minimum.
    fn checked_abs_(self) -> Option<Self>;
}

macro_rules! integer {
    ($($t:ty: $signed:tt),*) => {$(
        impl Integer for $t {
            const ZERO: Self = 0;
            fn checked_add_(self, other: Self) -> Option<Self> {
                self.checked_add(other)
            }
            fn checked_sub_(self, other: Self) -> Option<Self> {
                self.checked_sub(other)
            }
            fn checked_mul_(self, other: Self) -> Option<Self> {
                self.checked_mul(other)
            }
            fn checked_div_(self, other: Self) -> Option<Self> {
                self.checked_div(other)
            }
            fn iec_rem(self, other: Self) -> Self {
                integer!(@rem $signed self other)
            }
            fn checked_pow_(self, exponent: u32) -> Option<Self> {
                self.checked_pow(exponent)
            }
            #[allow(clippy::cast_possible_wrap, clippy::cast_lossless)]
            fn detail(self) -> i64 {
                self as i64
            }
            fn exponent(self) -> Result<u32, bool> {
                integer!(@exponent $signed self)
            }
        }
        integer!(@signed $signed $t);
    )*};
    (@rem signed $a:ident $b:ident) => {
        // Rust's checked remainder refuses MIN % -1, whose IEC result is 0.
        if $b == -1 { 0 } else { $a.checked_rem($b).unwrap_or(0) }
    };
    (@rem unsigned $a:ident $b:ident) => {
        $a.checked_rem($b).unwrap_or(0)
    };
    (@exponent signed $a:ident) => {
        if $a < 0 { Err(true) } else { u32::try_from($a).map_err(|_| false) }
    };
    (@exponent unsigned $a:ident) => {
        u32::try_from($a).map_err(|_| false)
    };
    (@signed signed $t:ty) => {
        impl Signed for $t {
            fn checked_neg_(self) -> Option<Self> {
                self.checked_neg()
            }
            fn checked_abs_(self) -> Option<Self> {
                self.checked_abs()
            }
        }
    };
    (@signed unsigned $t:ty) => {};
}

integer!(i8: signed, i16: signed, i32: signed, i64: signed, u8: unsigned, u16: unsigned,
    u32: unsigned, u64: unsigned);

fn overflow<T: Integer>(cx: &mut Cx, a: T, b: T) -> Fault {
    fault(cx, FaultCode::Overflow, a.detail(), b.detail(), 0)
}

/// `a + b`.
///
/// # Errors
///
/// `Overflow` outside the type.
#[inline]
pub fn add<T: Integer>(cx: &mut Cx, a: T, b: T) -> Result<T, Fault> {
    a.checked_add_(b).ok_or_else(|| overflow(cx, a, b))
}

/// `a - b`.
///
/// # Errors
///
/// `Overflow` outside the type.
#[inline]
pub fn sub<T: Integer>(cx: &mut Cx, a: T, b: T) -> Result<T, Fault> {
    a.checked_sub_(b).ok_or_else(|| overflow(cx, a, b))
}

/// `a * b`.
///
/// # Errors
///
/// `Overflow` outside the type.
#[inline]
pub fn mul<T: Integer>(cx: &mut Cx, a: T, b: T) -> Result<T, Fault> {
    a.checked_mul_(b).ok_or_else(|| overflow(cx, a, b))
}

/// `a / b`, truncated toward zero (IEC §6.6.2.5.8).
///
/// # Errors
///
/// `DivisionByZero` for a divisor of zero; `Overflow` for the minimum divided by -1.
#[inline]
pub fn div<T: Integer>(cx: &mut Cx, a: T, b: T) -> Result<T, Fault> {
    if b == T::ZERO {
        return Err(fault(cx, FaultCode::DivisionByZero, a.detail(), 0, 0));
    }
    a.checked_div_(b).ok_or_else(|| overflow(cx, a, b))
}

/// `a MOD b`, with the sign of `a`; `MIN MOD -1` is 0.
///
/// # Errors
///
/// `ModuloByZero` for a divisor of zero.
#[inline]
pub fn rem<T: Integer>(cx: &mut Cx, a: T, b: T) -> Result<T, Fault> {
    if b == T::ZERO {
        return Err(fault(cx, FaultCode::ModuloByZero, a.detail(), 0, 0));
    }
    Ok(a.iec_rem(b))
}

/// `a ** b` for integers.
///
/// # Errors
///
/// `TypeMismatch` for a negative exponent; `Overflow` for an exponent above `u32::MAX` and for a
/// result outside the type.
#[inline]
pub fn pow<T: Integer>(cx: &mut Cx, a: T, b: T) -> Result<T, Fault> {
    match b.exponent() {
        Ok(exponent) => a.checked_pow_(exponent).ok_or_else(|| overflow(cx, a, b)),
        Err(true) => Err(fault(
            cx,
            FaultCode::TypeMismatch,
            a.detail(),
            b.detail(),
            0,
        )),
        Err(false) => Err(overflow(cx, a, b)),
    }
}

/// `-a`.
///
/// # Errors
///
/// `Overflow` for the type's minimum.
#[inline]
pub fn neg<T: Signed>(cx: &mut Cx, a: T) -> Result<T, Fault> {
    a.checked_neg_().ok_or_else(|| overflow(cx, a, T::ZERO))
}

/// `ABS(a)` of a signed integer.
///
/// # Errors
///
/// `Overflow` for the type's minimum.
#[inline]
pub fn abs<T: Signed>(cx: &mut Cx, a: T) -> Result<T, Fault> {
    a.checked_abs_().ok_or_else(|| overflow(cx, a, T::ZERO))
}

/// `REAL` or `LREAL`.
pub trait Real:
    Copy
    + PartialEq
    + PartialOrd
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + Neg<Output = Self>
{
    /// Zero.
    const ZERO: Self;
    /// Whether the value is neither infinite nor NaN.
    fn is_finite_(self) -> bool;
    /// The value in binary64, exactly.
    fn wide(self) -> f64;
    /// The binary64 `value` rounded to the type.
    #[must_use]
    fn narrow(value: f64) -> Self;
    /// The value with its sign bit cleared, as `libm`'s `fabs` gives it: `-0.0` becomes `0.0`.
    #[must_use]
    fn magnitude(self) -> Self;
}

impl Real for f32 {
    const ZERO: Self = 0.0;
    fn is_finite_(self) -> bool {
        self.is_finite()
    }
    fn magnitude(self) -> Self {
        libm::fabsf(self)
    }
    fn wide(self) -> f64 {
        f64::from(self)
    }
    #[allow(clippy::cast_possible_truncation)]
    fn narrow(value: f64) -> Self {
        value as f32
    }
}

impl Real for f64 {
    const ZERO: Self = 0.0;
    fn is_finite_(self) -> bool {
        self.is_finite()
    }
    fn magnitude(self) -> Self {
        libm::fabs(self)
    }
    fn wide(self) -> f64 {
        self
    }
    fn narrow(value: f64) -> Self {
        value
    }
}

/// `value` when it is finite.
///
/// # Errors
///
/// `Overflow` for an infinite or NaN result, as the runtime reports a real result outside the type.
#[inline]
pub fn finite<T: Real>(cx: &mut Cx, value: T) -> Result<T, Fault> {
    if value.is_finite_() {
        Ok(value)
    } else {
        Err(real_overflow(cx))
    }
}

#[cold]
fn real_overflow(cx: &mut Cx) -> Fault {
    fault(cx, FaultCode::Overflow, 0, 0, 0)
}

/// `a + b` of reals. Computed in the type: the binary64 sum of two binary32 values rounds to the
/// same binary32 as the type's own sum, so the result is the runtime's.
///
/// # Errors
///
/// `Overflow` for a result that is not finite.
#[inline]
pub fn fadd<T: Real>(cx: &mut Cx, a: T, b: T) -> Result<T, Fault> {
    finite(cx, a + b)
}

/// `a - b` of reals.
///
/// # Errors
///
/// `Overflow` for a result that is not finite.
#[inline]
pub fn fsub<T: Real>(cx: &mut Cx, a: T, b: T) -> Result<T, Fault> {
    finite(cx, a - b)
}

/// `a * b` of reals.
///
/// # Errors
///
/// `Overflow` for a result that is not finite.
#[inline]
pub fn fmul<T: Real>(cx: &mut Cx, a: T, b: T) -> Result<T, Fault> {
    finite(cx, a * b)
}

/// `a / b` of reals.
///
/// # Errors
///
/// `DivisionByZero` for a divisor of zero; `Overflow` for a result that is not finite.
#[inline]
pub fn fdiv<T: Real>(cx: &mut Cx, a: T, b: T) -> Result<T, Fault> {
    if b == T::ZERO {
        return Err(fault(cx, FaultCode::DivisionByZero, 0, 0, 0));
    }
    finite(cx, a / b)
}

/// `-a` of a real.
#[inline]
pub fn fneg<T: Real>(a: T) -> T {
    -a
}

/// `a ** b` of reals, `EXPT`: computed in binary64 by the shared `libm` and rounded to the type.
///
/// # Errors
///
/// `Overflow` for a result that is not finite, in binary64 or in the type.
#[inline]
pub fn fpow<T: Real>(cx: &mut Cx, a: T, b: T) -> Result<T, Fault> {
    let wide = finite(cx, libm::pow(a.wide(), b.wide()))?;
    finite(cx, T::narrow(wide))
}
