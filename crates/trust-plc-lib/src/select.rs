//! The selection functions of IEC §6.6.2.5.10 (Table 30) with the runtime's rules: `MAX` and
//! `MIN` keep the first of equal values, and `LIMIT` raises to the minimum before it caps at the
//! maximum (specification 07 §6.1); an array's element by its index, a subrange's bounds and a
//! real's magnitude, which generated code checks with them.

use trust_plc_abi::module::fault;
use trust_plc_abi::{Cx, Fault, FaultCode};

use crate::arith::{Integer, Real};

/// `MAX(a, b)`: `b` only when it is greater than `a`. `MAX` of more inputs is this from the left.
#[inline]
pub fn max<T: PartialOrd + Copy>(a: T, b: T) -> T {
    if b > a {
        b
    } else {
        a
    }
}

/// `MIN(a, b)`: `b` only when it is less than `a`. `MIN` of more inputs is this from the left.
#[inline]
pub fn min<T: PartialOrd + Copy>(a: T, b: T) -> T {
    if b < a {
        b
    } else {
        a
    }
}

/// `LIMIT(MN, IN, MX)`, `MIN(MAX(IN, MN), MX)`: with `MN` above `MX` every input gives `MX`.
#[inline]
pub fn limit<T: PartialOrd + Copy>(mn: T, value: T, mx: T) -> T {
    let raised = if value < mn { mn } else { value };
    if raised > mx {
        mx
    } else {
        raised
    }
}

/// The element `index` of `array`, whose first index is `lower` (IEC §6.4.4.5).
///
/// # Errors
///
/// `IndexOutOfBounds` with the index and the bounds for an index outside the array.
#[inline]
pub fn at<'a, T, const N: usize>(
    cx: &mut Cx,
    array: &'a [T; N],
    index: i64,
    lower: i64,
) -> Result<&'a T, Fault> {
    let element = usize::try_from(index.wrapping_sub(lower))
        .ok()
        .and_then(|offset| array.get(offset));
    element.ok_or_else(|| {
        let upper = lower.saturating_add(i64::try_from(N).unwrap_or(i64::MAX) - 1);
        fault(cx, FaultCode::IndexOutOfBounds, index, lower, upper)
    })
}

/// `value` when it lies in the subrange `lower..upper` (IEC §6.4.4.4).
///
/// # Errors
///
/// `SubrangeViolation` with the value and the bounds otherwise.
#[inline]
pub fn subrange<T: Integer>(cx: &mut Cx, value: T, lower: i64, upper: i64) -> Result<T, Fault> {
    let wide = value.detail();
    if (lower..=upper).contains(&wide) {
        Ok(value)
    } else {
        Err(fault(cx, FaultCode::SubrangeViolation, wide, lower, upper))
    }
}

/// `ABS` of a real.
#[inline]
pub fn fabs<T: Real>(value: T) -> T {
    if value < T::ZERO {
        -value
    } else {
        value
    }
}
