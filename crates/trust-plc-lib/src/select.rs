//! The selection functions of IEC §6.6.2.5.10 (Table 30) with the runtime's rules: `MAX` and
//! `MIN` keep the first of equal values, and `LIMIT` raises to the minimum before it caps at the
//! maximum (specification 07 §6.1); an array's element by its index, a subrange's bounds and a
//! real's magnitude, which generated code checks with them.

use trust_plc_abi::module::fault;
use trust_plc_abi::{Cx, Fault, FaultCode, RefValue};

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

/// A reference to the element `index` of the array that `array` names, an array of `size`-byte
/// elements whose indices run from `lower` to `upper` (IEC §6.4.4.5): `NULL` stays `NULL`, which its
/// dereference refuses. A pointer may name fewer bytes than its array type spans: a string's bytes
/// taken as an array of bytes (`docs/IEC_DEVIATIONS.md` DEV-039) are those from the reference's byte
/// to the string's capacity, so an element past the bytes `array` spans is refused as one past the
/// bounds is. Such an element's reference names the string still, from the element's byte, so that
/// a write through it sets the string's length (`Areas::bytes_written`).
///
/// # Errors
///
/// `IndexOutOfBounds` with the index and the bounds of the elements `array` spans, for an index
/// outside `lower..=upper` or past those bytes.
#[inline]
pub fn element_reference(
    cx: &mut Cx,
    array: RefValue,
    index: i64,
    lower: i64,
    upper: i64,
    size: u32,
) -> Result<RefValue, Fault> {
    if array.kind == RefValue::NULL {
        return Ok(array);
    }
    let text = array.kind == RefValue::TEXT_BYTES;
    // A string's bytes follow its length's 2 bytes, and the reference names them from its byte.
    let bytes = if text {
        array
            .extent
            .saturating_sub(2)
            .saturating_sub(u32::from(array.char_index))
    } else {
        array.extent
    };
    let spanned = i64::from(bytes / size.max(1));
    let last = upper.min(lower.saturating_add(spanned).saturating_sub(1));
    let from_first = (lower..=last)
        .contains(&index)
        .then(|| u32::try_from(index - lower).ok())
        .flatten()
        .and_then(|position| position.checked_mul(size));
    let element = match from_first {
        Some(bytes) if text => u32::from(array.char_index)
            .checked_add(bytes)
            .and_then(|byte| u16::try_from(byte).ok())
            .map(|char_index| RefValue {
                char_index,
                ..array
            }),
        Some(bytes) => array.offset.checked_add(bytes).map(|offset| RefValue {
            offset,
            extent: size,
            ..array
        }),
        None => None,
    };
    element.ok_or_else(|| fault(cx, FaultCode::IndexOutOfBounds, index, lower, last))
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

/// `ABS` of a real: its sign bit cleared, as the runtime's `libm` clears it, so that `ABS(-0.0)` is
/// `0.0`.
#[inline]
pub fn fabs<T: Real>(value: T) -> T {
    value.magnitude()
}

/// The input `MUX` selects with `selector` among `count` inputs, from 0 (IEC §6.6.2.5.10, Table 30).
///
/// # Errors
///
/// `IndexOutOfBounds` with the selector and the bounds 0 to `count - 1` for a selector outside them.
#[inline]
pub fn mux_index(cx: &mut Cx, selector: i64, count: usize) -> Result<usize, Fault> {
    usize::try_from(selector)
        .ok()
        .filter(|index| *index < count)
        .ok_or_else(|| {
            let upper = i64::try_from(count).unwrap_or(i64::MAX) - 1;
            fault(cx, FaultCode::IndexOutOfBounds, selector, 0, upper)
        })
}
