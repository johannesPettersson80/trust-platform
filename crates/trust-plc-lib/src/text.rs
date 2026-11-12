//! Texts (IEC §6.6.2.5.11, Table 34; specification 02 §6 and 07 §8): a `STRING` holds UTF-8, a
//! text longer than a variable's capacity is cut there at the last whole character, a string
//! function's result is at most 65,535 bytes, `LEN` counts characters (DEV-017), and two strings
//! compare by their bytes.

use core::cmp::Ordering;
use core::num::NonZeroU64;

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
    /// The bytes of a string from the first to before the second offset, which lie at character
    /// boundaries.
    Range(StrView<'a>, usize, usize),
}

impl Piece<'_> {
    pub(crate) fn len(&self) -> usize {
        match self {
            Piece::Bytes(bytes) => bytes.len(),
            Piece::Str(view) => view.len(),
            Piece::Range(_, start, end) => end.saturating_sub(*start),
        }
    }

    fn byte(&self, index: usize) -> u8 {
        match self {
            Piece::Bytes(bytes) => bytes.get(index).copied().unwrap_or(0),
            Piece::Str(view) => view.byte(index),
            Piece::Range(view, start, _) => view.byte(start + index),
        }
    }
}

/// Whether `byte` begins a character, that is, is no UTF-8 continuation byte.
pub(crate) fn starts_character(byte: u8) -> bool {
    byte & 0b1100_0000 != 0b1000_0000
}

/// Writes `parts` one after the other into `dst`, from byte `start`, up to the last whole
/// character that fits its capacity, sets its length, and returns it.
pub(crate) fn write_parts(dst: StrView<'_>, start: usize, parts: &[Piece<'_>]) -> usize {
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
    dst.set_len(at);
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

/// `TRUE` or `FALSE` into `dst`, cut at its capacity: `BOOL_TO_STRING` under the CODESYS
/// conversion rules (specification 07 §2.7, `docs/IEC_DEVIATIONS.md` DEV-036).
pub fn write_bool(dst: StrView<'_>, value: bool) {
    let text: &[u8] = if value { b"TRUE" } else { b"FALSE" };
    write_parts(dst, 0, &[Piece::Bytes(text)]);
}

/// The room of a `TIME`'s text: `T#-106751d23h47m16s854ms`, the longest a `TIME` of 64-bit
/// nanoseconds gives.
pub const TIME_TEXT: usize = 24;
/// The room of an `LTIME`'s text: `LTIME#-106751d23h47m16s854ms775us808ns`.
pub const LTIME_TEXT: usize = 38;

/// The units of a duration's text from days down, each its nanoseconds and its one or two letters: a
/// `TIME`'s text ends with milliseconds, an `LTIME`'s with nanoseconds. The table holds no pointers,
/// so it stays in a module's read-only data, which has no relocation (specification 44 §5.3), and
/// its units are not zero by type, so dividing by them cannot panic (§8.6).
const DURATION_UNITS: [(NonZeroU64, [u8; 2], usize); 7] = [
    (nanoseconds(86_400_000_000_000), *b"d ", 1),
    (nanoseconds(3_600_000_000_000), *b"h ", 1),
    (nanoseconds(60_000_000_000), *b"m ", 1),
    (nanoseconds(1_000_000_000), *b"s ", 1),
    (nanoseconds(1_000_000), *b"ms", 2),
    (nanoseconds(1_000), *b"us", 2),
    (nanoseconds(1), *b"ns", 2),
];

/// A unit of `nanos` nanoseconds, which is not zero.
const fn nanoseconds(nanos: u64) -> NonZeroU64 {
    match NonZeroU64::new(nanos) {
        Some(unit) => unit,
        None => NonZeroU64::MIN,
    }
}

/// A duration of `nanos` as text into `dst`, cut at its capacity: `TO_STRING` and `TIME_TO_STRING`
/// of a `TIME`, and of an `LTIME` (`long`), under the CODESYS conversion rules (specification 07
/// §2.7, `docs/IEC_DEVIATIONS.md` DEV-036), as measured on a WAGO PFC200: the prefix `T#`
/// (`LTIME#`), then each unit that is not zero from days down to milliseconds (nanoseconds), so
/// `T#1s500ms`, `T#2d` and `LTIME#1h2m3s4ms5us6ns`, and `T#0ms` (`LTIME#0ns`) for no time. A
/// `TIME`'s part below a millisecond is left out, as the vendor's `TIME` counts milliseconds; a
/// negative duration, which the vendor's unsigned types cannot hold, has its sign after the `#`, as
/// an IEC literal does.
pub fn write_duration(dst: StrView<'_>, nanos: i64, long: bool) {
    let prefix: &[u8] = if long { b"LTIME#" } else { b"T#" };
    let zero: &[u8] = if long { b"0ns" } else { b"0ms" };
    let units = DURATION_UNITS
        .get(..if long { DURATION_UNITS.len() } else { 5 })
        .unwrap_or(&[]);
    let smallest = units.last().map_or(NonZeroU64::MIN, |(unit, _, _)| *unit);
    let magnitude = nanos.unsigned_abs() / smallest * smallest.get();
    let mut text = [0u8; LTIME_TEXT];
    let mut at = 0;
    let mut push = |bytes: &[u8]| {
        for byte in bytes {
            if let Some(cell) = text.get_mut(at) {
                *cell = *byte;
            }
            at += 1;
        }
    };
    push(prefix);
    if magnitude == 0 {
        push(zero);
    } else {
        if nanos < 0 {
            push(b"-");
        }
        let mut rest = magnitude;
        for &(unit, letters, used_letters) in units {
            let count = rest / unit;
            rest %= unit;
            if count > 0 {
                let mut digits = [0u8; 20];
                let used = decimal(&mut digits, count);
                push(digits.get(digits.len() - used..).unwrap_or(&[]));
                push(letters.get(..used_letters).unwrap_or(&[]));
            }
        }
    }
    assign_bytes(dst, text.get(..at).unwrap_or(&[]));
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
