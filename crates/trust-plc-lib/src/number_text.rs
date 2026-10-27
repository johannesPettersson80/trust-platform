//! Reals as text and text as numbers (specification 07 §2.8): a real is spelled as the runtime
//! spells it, the shortest digits that read back as the same binary64 value in plain decimal
//! notation with `.0` after a whole number, through the same `ryu`; a text is read as the runtime
//! reads it, its Unicode white space trimmed, single `_` between digits allowed, an integer with an
//! optional base (`16#FF`) and sign, a real by the kernel's reader, the runtime's own (specification
//! 44 §8.2).

use trust_plc_abi::module::{fault, read_real};
use trust_plc_abi::{Cx, Fault, FaultCode, StrView};

use crate::text::assign_bytes;

/// Room for the longest spelling of a binary64 value: a sign, `0.` and the zeros and digits of a
/// value near the smallest subnormal, at most 328 bytes, or a sign, 309 digits and `.0`.
pub const REAL_TEXT: usize = 340;

/// `REAL_TO_STRING`, `LREAL_TO_STRING`: the spelling of `value` into `dst`, cut at its capacity. A
/// `REAL` is spelled as the binary64 value it widens to.
pub fn write_real(dst: StrView<'_>, value: f64) {
    let mut text = [0u8; REAL_TEXT];
    let used = spell(value, &mut text);
    assign_bytes(dst, text.get(..used).unwrap_or(&[]));
}

/// The bytes of a spelling as they are written, past its room dropped.
struct Spelling<'o> {
    out: &'o mut [u8; REAL_TEXT],
    at: usize,
}

impl Spelling<'_> {
    fn push(&mut self, byte: u8) {
        if let Some(slot) = self.out.get_mut(self.at) {
            *slot = byte;
        }
        self.at += 1;
    }

    fn last(&self) -> Option<u8> {
        self.at
            .checked_sub(1)
            .and_then(|index| self.out.get(index))
            .copied()
    }
}

/// Writes the runtime's spelling of `value` into `out` and returns its length: binary64's
/// shortest round-trip digits, as Rust's `Display` spells them, then `.0` after a whole number.
fn spell(value: f64, out: &mut [u8; REAL_TEXT]) -> usize {
    let mut text = Spelling { out, at: 0 };
    if value.is_nan() {
        for byte in b"NaN" {
            text.push(*byte);
        }
        return text.at;
    }
    if value.is_infinite() {
        let spelled: &[u8] = if value.is_sign_negative() {
            b"-inf"
        } else {
            b"inf"
        };
        for byte in spelled {
            text.push(*byte);
        }
        return text.at;
    }
    let mut buffer = ryu::Buffer::new();
    let encoded = buffer.format_finite(value);
    let (negative, unsigned) = encoded
        .strip_prefix('-')
        .map_or((false, encoded), |digits| (true, digits));
    let (mantissa, exponent) = unsigned.split_once('e').unwrap_or((unsigned, "0"));
    // Ryu writes a signed decimal exponent within binary64's decimal range.
    let magnitude = exponent
        .trim_start_matches(['+', '-'])
        .bytes()
        .fold(0i32, |number, byte| number * 10 + i32::from(byte - b'0'));
    let power = if exponent.starts_with('-') {
        -magnitude
    } else {
        magnitude
    };
    let point = i32::try_from(mantissa.find('.').unwrap_or(mantissa.len())).unwrap_or(0);
    let decimal = point + power;
    // The digits ryu wrote, at most 17, without trailing zeros but at least one.
    let mut digits = [0u8; 24];
    let mut length = 0;
    for byte in mantissa.bytes().filter(|byte| *byte != b'.') {
        if let Some(slot) = digits.get_mut(length) {
            *slot = byte;
            length += 1;
        }
    }
    while length > 1 && digits.get(length - 1) == Some(&b'0') {
        length -= 1;
    }
    let last = length.saturating_sub(1);
    let length_i32 = i32::try_from(length).unwrap_or(0);
    if digits.get(last).is_some_and(|digit| digit & 1 == 0)
        && lower_decimal_halfway(
            value,
            digits.get(..length).unwrap_or(&[]),
            decimal - length_i32,
        )
    {
        // Ryu takes the even last digit at a decimal tie, where `Display` rounds the magnitude
        // up; an even digit is at most 8, so the step needs no carry.
        if let Some(digit) = digits.get_mut(last) {
            *digit += 1;
        }
    }
    if negative {
        text.push(b'-');
    }
    let start = usize::from(negative);
    let mut fraction = false;
    if decimal <= 0 {
        text.push(b'0');
        text.push(b'.');
        fraction = true;
        for _ in decimal..0 {
            text.push(b'0');
        }
        for byte in digits.iter().take(length) {
            text.push(*byte);
        }
    } else {
        let whole = usize::try_from(decimal).unwrap_or(0);
        for index in 0..whole {
            let digit = digits.get(index).filter(|_| index < length).copied();
            text.push(digit.unwrap_or(b'0'));
        }
        if whole < length {
            text.push(b'.');
            fraction = true;
            for byte in digits.iter().take(length).skip(whole) {
                text.push(*byte);
            }
        }
    }
    if fraction {
        while text.at > start && text.last() == Some(b'0') {
            text.at -= 1;
        }
        if text.at > start && text.last() == Some(b'.') {
            text.at -= 1;
        }
    } else {
        // A whole number gets `.0` (the runtime's `float_text::real`).
        text.push(b'.');
        text.push(b'0');
    }
    text.at.min(REAL_TEXT)
}

/// Whether `value` is exactly `(digits + 1/2) * 10^decimal_exponent`: the decimal tie that ryu
/// breaks to the even digit (the runtime's `is_lower_decimal_halfway`).
fn lower_decimal_halfway(value: f64, digits: &[u8], decimal_exponent: i32) -> bool {
    let bits = value.to_bits() & !(1u64 << 63);
    let exponent = i32::try_from(bits >> 52).unwrap_or(0);
    let mut significand = bits & ((1u64 << 52) - 1);
    let binary_exponent = if exponent == 0 {
        -1074
    } else {
        significand |= 1u64 << 52;
        exponent - 1023 - 52
    };
    if significand == 0 {
        return false;
    }
    let twos = significand.trailing_zeros();
    if binary_exponent + 1 - decimal_exponent + i32::try_from(twos).unwrap_or(0) != 0 {
        return false;
    }
    let Some(decimal) = digits.iter().try_fold(0u64, |number, digit| {
        number
            .checked_mul(10)?
            .checked_add(u64::from(*digit - b'0'))
    }) else {
        return false;
    };
    let Some(twice_plus_one) = decimal.checked_mul(2).and_then(|n| n.checked_add(1)) else {
        return false;
    };
    let odd_binary = significand >> twos;
    let (mut scaled, expected) = if decimal_exponent >= 0 {
        (twice_plus_one, odd_binary)
    } else {
        (odd_binary, twice_plus_one)
    };
    for _ in 0..decimal_exponent.unsigned_abs() {
        let Some(next) = scaled.checked_mul(5) else {
            return false;
        };
        scaled = next;
    }
    scaled == expected
}

fn type_mismatch(cx: &mut Cx) -> Fault {
    fault(cx, FaultCode::TypeMismatch, 0, 0, 0)
}

/// The bytes of `text` copied to the front of `scratch`, which holds its capacity.
fn copied<'s>(text: StrView<'_>, scratch: &'s mut [u8]) -> &'s mut [u8] {
    let length = text.len().min(scratch.len());
    let Some(bytes) = scratch.get_mut(..length) else {
        return &mut [];
    };
    for (index, slot) in bytes.iter_mut().enumerate() {
        *slot = text.byte(index);
    }
    bytes
}

/// `STRING_TO_DINT` and the others: the integer `text` spells (specification 07 §2.8), in 128
/// bits; the caller converts it to the destination's type, an overflow outside it. `scratch`
/// holds the text's capacity.
///
/// # Errors
///
/// `TypeMismatch` for a text that spells no integer or one outside 128 bits.
pub fn parse_int(cx: &mut Cx, text: StrView<'_>, scratch: &mut [u8]) -> Result<i128, Fault> {
    let bytes = copied(text, scratch);
    core::str::from_utf8(bytes)
        .ok()
        .and_then(parse_int_text)
        .ok_or_else(|| type_mismatch(cx))
}

/// The runtime's `parse_int_text`: `None` where it gives a type mismatch.
fn parse_int_text(text: &str) -> Option<i128> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some((base_text, digits)) = trimmed.split_once('#') {
        // The optional sign belongs to the digits, never to the base (specification 07 §2.8).
        if !separators_ok(base_text, 10)
            || !base_text
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'_')
        {
            return None;
        }
        let base = digits_value(base_text, 10).and_then(|base| u32::try_from(base).ok())?;
        if !(2..=36).contains(&base) || !separators_ok(digits, base) {
            return None;
        }
        let (negative, unsigned) = match digits.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, digits.strip_prefix('+').unwrap_or(digits)),
        };
        if unsigned.starts_with(['+', '-']) {
            return None;
        }
        let value = digits_value(unsigned, base)?;
        return Some(if negative { -value } else { value });
    }
    if !separators_ok(trimmed, 10) {
        return None;
    }
    match trimmed.strip_prefix('-') {
        Some(rest) => negative_digits_value(rest),
        None => digits_value(trimmed.strip_prefix('+').unwrap_or(trimmed), 10),
    }
}

/// `STRING_TO_LREAL` and `STRING_TO_REAL` before the narrowing: the real `text` spells, its
/// separators taken out here and read by the kernel with the runtime's reader. `scratch` holds the
/// text's capacity.
///
/// # Errors
///
/// `TypeMismatch` for a text that spells no real, `Overflow` for one that is not finite.
pub fn parse_real(cx: &mut Cx, text: StrView<'_>, scratch: &mut [u8]) -> Result<f64, Fault> {
    let bytes = copied(text, scratch);
    let Ok(whole) = core::str::from_utf8(bytes) else {
        return Err(type_mismatch(cx));
    };
    let trimmed = whole.trim();
    if trimmed.is_empty() || !separators_ok(trimmed, 10) {
        return Err(type_mismatch(cx));
    }
    let start = whole.len() - whole.trim_start().len();
    let end = start + trimmed.len();
    // The separators taken out in place.
    let mut kept = start;
    for index in start..end {
        let Some(&byte) = bytes.get(index) else {
            break;
        };
        if byte != b'_' {
            if let Some(slot) = bytes.get_mut(kept) {
                *slot = byte;
            }
            kept += 1;
        }
    }
    let cleaned = bytes.get(start..kept).unwrap_or(&[]);
    let value = read_real(cx, cleaned)?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(fault(cx, FaultCode::Overflow, 0, 0, 0))
    }
}

/// Whether every `_` of `text` stands between two digits of `radix` (IEC §6.3.2).
fn separators_ok(text: &str, radix: u32) -> bool {
    let mut previous: Option<char> = None;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '_' {
            let after = chars
                .peek()
                .is_some_and(|next| digit(*next, radix).is_some());
            let before = previous.is_some_and(|prev| digit(prev, radix).is_some());
            if !(before && after) {
                return false;
            }
        }
        previous = Some(ch);
    }
    true
}

/// The value of the digits of `text` in `radix`, its separators skipped: `None` for no digit, a
/// character that is no digit, and a value above `i128::MAX`.
fn digits_value(text: &str, radix: u32) -> Option<i128> {
    let mut value: Option<i128> = None;
    for character in text.chars().filter(|character| *character != '_') {
        let next = i128::from(digit(character, radix)?);
        value = Some(
            value
                .unwrap_or(0)
                .checked_mul(i128::from(radix))?
                .checked_add(next)?,
        );
    }
    value
}

/// The negative value of the decimal digits of `text`, its separators skipped, down to
/// `i128::MIN`.
fn negative_digits_value(text: &str) -> Option<i128> {
    let mut value: Option<i128> = None;
    for character in text.chars().filter(|character| *character != '_') {
        let next = i128::from(digit(character, 10)?);
        value = Some(value.unwrap_or(0).checked_mul(10)?.checked_sub(next)?);
    }
    value
}

/// The value of `character` as a digit of `radix`, as `char::to_digit` gives it, without its
/// assertion on the radix, so that no module can panic (specification 44 §8.6).
fn digit(character: char, radix: u32) -> Option<u32> {
    let value = match character {
        '0'..='9' => u32::from(character) - u32::from('0'),
        'a'..='z' => u32::from(character) - u32::from('a') + 10,
        'A'..='Z' => u32::from(character) - u32::from('A') + 10,
        _ => return None,
    };
    (value < radix).then_some(value)
}
