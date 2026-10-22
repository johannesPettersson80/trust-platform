//! Texts (IEC §6.6.2.5.11, Table 34; specification 02 §6 and 07 §8): a `STRING` holds UTF-8, a
//! text longer than a variable's capacity is cut there at the last whole character, a string
//! function's result is at most 65,535 bytes, `LEN` counts characters (DEV-017), and two strings
//! compare by their bytes.

use core::cmp::Ordering;

use trust_plc_abi::module::fault;
use trust_plc_abi::{Cx, Fault, FaultCode, StrView};

/// The longest text a string function may produce, in UTF-8 bytes (IEC §6.4.2.2, the implementer's
/// maximum; specification 07 §8.1).
pub const MAX_TEXT: usize = 65_535;

/// One part of a `CONCAT`: a literal's bytes or a string.
#[derive(Clone, Copy)]
pub enum Piece<'a> {
    /// The UTF-8 bytes of a literal.
    Bytes(&'a [u8]),
    /// A string's current value.
    Str(StrView<'a>),
}

impl Piece<'_> {
    fn len(&self) -> usize {
        match self {
            Piece::Bytes(bytes) => bytes.len(),
            Piece::Str(view) => view.len(),
        }
    }

    fn byte(&self, index: usize) -> u8 {
        match self {
            Piece::Bytes(bytes) => bytes.get(index).copied().unwrap_or(0),
            Piece::Str(view) => view.byte(index),
        }
    }
}

/// Whether `byte` begins a character, that is, is no UTF-8 continuation byte.
fn starts_character(byte: u8) -> bool {
    byte & 0b1100_0000 != 0b1000_0000
}

/// Writes `parts` one after the other into `dst`, from byte `start`, up to the last whole
/// character that fits its capacity, sets its length, and returns it.
fn write_parts(dst: StrView<'_>, start: usize, parts: &[Piece<'_>]) -> usize {
    let capacity = dst.capacity();
    let mut at = start.min(capacity);
    'parts: for part in parts {
        let len = part.len();
        let mut index = 0;
        while index < len {
            // The bytes of the character that starts at `index`.
            let mut end = index + 1;
            while end < len && !starts_character(part.byte(end)) {
                end += 1;
            }
            if at + (end - index) > capacity {
                break 'parts;
            }
            for offset in index..end {
                if let Some(cell) = dst.data.get(at) {
                    cell.set(part.byte(offset));
                }
                at += 1;
            }
            index = end;
        }
    }
    // The capacity is at most 65,535 (specification 44 §7.6).
    dst.len.set(u16::try_from(at).unwrap_or(u16::MAX));
    at
}

/// `dst := 'literal'`: the literal's bytes, cut at `dst`'s capacity.
pub fn assign_bytes(dst: StrView<'_>, bytes: &[u8]) {
    write_parts(dst, 0, &[Piece::Bytes(bytes)]);
}

/// `dst := src`, cut at `dst`'s capacity. `dst` and `src` may be the same string.
pub fn assign(dst: StrView<'_>, src: StrView<'_>) {
    write_parts(dst, 0, &[Piece::Str(src)]);
}

/// `dst := CONCAT(parts…)`: the parts one after the other, cut at `dst`'s capacity. `dst` must not
/// be one of the parts; generated code concatenates into a temporary when it may be.
///
/// # Errors
///
/// `StringCapacityExceeded` for a result longer than [`MAX_TEXT`], before anything is written.
pub fn concat(cx: &mut Cx, dst: StrView<'_>, parts: &[Piece<'_>]) -> Result<(), Fault> {
    let total: usize = parts.iter().map(Piece::len).sum();
    if total > MAX_TEXT {
        return Err(fault(
            cx,
            FaultCode::StringCapacityExceeded,
            i64::try_from(total).unwrap_or(i64::MAX),
            0,
            0,
        ));
    }
    write_parts(dst, 0, parts);
    Ok(())
}

/// The decimal digits of `value` into `dst`, cut at its capacity: `DINT_TO_STRING`.
pub fn write_int(dst: StrView<'_>, value: i64) {
    let mut digits = [0u8; 20];
    let magnitude = value.unsigned_abs();
    let used = decimal(&mut digits, magnitude);
    let sign: &[u8] = if value < 0 { b"-" } else { b"" };
    let digits = digits.get(digits.len() - used..).unwrap_or(&[]);
    write_parts(dst, 0, &[Piece::Bytes(sign), Piece::Bytes(digits)]);
}

/// The decimal digits of `value` into `dst`, cut at its capacity: `UDINT_TO_STRING`.
pub fn write_uint(dst: StrView<'_>, value: u64) {
    let mut digits = [0u8; 20];
    let used = decimal(&mut digits, value);
    let digits = digits.get(digits.len() - used..).unwrap_or(&[]);
    write_parts(dst, 0, &[Piece::Bytes(digits)]);
}

/// Writes the decimal digits of `value` at the end of `digits` and returns how many.
fn decimal(digits: &mut [u8; 20], mut value: u64) -> usize {
    let mut used = 0;
    loop {
        let index = digits.len() - 1 - used;
        if let Some(digit) = digits.get_mut(index) {
            // `value % 10` is below 10.
            *digit = b'0' + u8::try_from(value % 10).unwrap_or(0);
        }
        used += 1;
        value /= 10;
        if value == 0 || used == digits.len() {
            return used;
        }
    }
}

/// `LEN(text)`: its characters (DEV-017).
#[must_use]
pub fn len(text: StrView<'_>) -> usize {
    (0..text.len())
        .filter(|&index| starts_character(text.byte(index)))
        .count()
}

/// The order of two strings by their bytes, a prefix first: `=`, `<`, `>` of strings.
#[must_use]
pub fn compare(a: StrView<'_>, b: StrView<'_>) -> Ordering {
    let common = a.len().min(b.len());
    for index in 0..common {
        match a.byte(index).cmp(&b.byte(index)) {
            Ordering::Equal => {}
            unequal => return unequal,
        }
    }
    a.len().cmp(&b.len())
}
