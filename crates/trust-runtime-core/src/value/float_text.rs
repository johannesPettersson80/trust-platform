//! Lossless floating text with compact tables and the existing decimal spelling.
use alloc::string::String;
use lexical_parse_float::FromLexical;

pub(crate) fn parse(text: &str) -> Result<f64, crate::error::RuntimeError> {
    f64::from_lexical(text.as_bytes()).map_err(|_| crate::error::RuntimeError::TypeMismatch)
}

/// Match binary64 Display: shortest round-tripping digits, expanded decimal
/// notation, no redundant fractional zeros, and a preserved negative zero.
pub(crate) fn display(value: f64) -> String {
    let mut text = String::new();
    display_into(value, &mut text);
    text
}

/// [`display`], appended to `out` (C1-12).
pub(crate) fn display_into(value: f64, out: &mut String) {
    if value.is_nan() {
        out.push_str("NaN");
        return;
    }
    if value.is_infinite() {
        out.push_str(if value.is_sign_negative() {
            "-inf"
        } else {
            "inf"
        });
        return;
    }
    let mut buffer = ryu::Buffer::new();
    let encoded = buffer.format_finite(value);
    let (negative, unsigned) = encoded
        .strip_prefix('-')
        .map_or((false, encoded), |digits| (true, digits));
    let (mantissa, exponent) = unsigned.split_once('e').unwrap_or((unsigned, "0"));
    // Ryu emits a signed decimal exponent bounded by binary64's decimal range.
    // Read that fixed library representation without invoking float parsing.
    let magnitude = exponent
        .trim_start_matches(['+', '-'])
        .bytes()
        .fold(0i32, |number, byte| number * 10 + i32::from(byte - b'0'));
    let power = if exponent.starts_with('-') {
        -magnitude
    } else {
        magnitude
    };
    let decimal = mantissa.find('.').unwrap_or(mantissa.len()) as i32 + power;
    let mut digits = [0u8; 24];
    let mut length = 0;
    for byte in mantissa.bytes().filter(|byte| *byte != b'.') {
        digits[length] = byte;
        length += 1;
    }
    while length > 1 && digits[length - 1] == b'0' {
        length -= 1;
    }
    if digits[length - 1] & 1 == 0
        && is_lower_decimal_halfway(value, &digits[..length], decimal - length as i32)
    {
        // Ryu chooses an even final digit at a decimal tie; Rust Display's
        // shortest formatter rounds the magnitude upward. An even ASCII digit
        // is at most '8', so this exact tie adjustment cannot need a carry.
        digits[length - 1] += 1;
    }
    let capacity = if decimal <= 0 {
        2 + (-decimal) as usize + length
    } else {
        (decimal as usize).max(length) + usize::from((decimal as usize) < length)
    };
    // Reserve the actual decimal spelling plus sign and the optional `.0` suffix.
    out.reserve(capacity + usize::from(negative) + 2);
    let start = out.len();
    if negative {
        out.push('-');
    }
    if decimal <= 0 {
        out.push_str("0.");
        for _ in decimal..0 {
            out.push('0');
        }
        for byte in &digits[..length] {
            out.push(char::from(*byte));
        }
    } else {
        for index in 0..decimal as usize {
            out.push(char::from(
                *digits
                    .get(index)
                    .filter(|_| index < length)
                    .unwrap_or(&b'0'),
            ));
        }
        if (decimal as usize) < length {
            out.push('.');
            for byte in &digits[decimal as usize..length] {
                out.push(char::from(*byte));
            }
        }
    }
    if out[start..].contains('.') {
        while out[start..].ends_with('0') {
            out.pop();
        }
        if out[start..].ends_with('.') {
            out.pop();
        }
    }
}

// Test v == (d + 1/2) * 10^e exactly. Factor powers of two from the
// binary significand; the remaining comparison uses only odd integers and
// checked powers of five. Overflow cannot equal the bounded opposite operand.
fn is_lower_decimal_halfway(value: f64, digits: &[u8], decimal_exponent: i32) -> bool {
    let bits = value.to_bits() & !(1u64 << 63);
    let exponent = (bits >> 52) as i32;
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
    if binary_exponent + 1 - decimal_exponent + twos as i32 != 0 {
        return false;
    }
    let Some(decimal) = digits.iter().try_fold(0u64, |number, digit| {
        number
            .checked_mul(10)?
            .checked_add(u64::from(*digit - b'0'))
    }) else {
        return false;
    };
    let Some(twice_decimal_plus_one) = decimal.checked_mul(2).and_then(|n| n.checked_add(1)) else {
        return false;
    };
    let odd_binary = significand >> twos;
    let (mut scaled, expected) = if decimal_exponent >= 0 {
        (twice_decimal_plus_one, odd_binary)
    } else {
        (odd_binary, twice_decimal_plus_one)
    };
    for _ in 0..decimal_exponent.unsigned_abs() {
        let Some(next) = scaled.checked_mul(5) else {
            return false;
        };
        scaled = next;
    }
    scaled == expected
}

pub(crate) fn real(value: f64) -> String {
    let mut text = String::new();
    real_into(value, &mut text);
    text
}

/// [`real`], appended to `out` (C1-12).
pub(crate) fn real_into(value: f64, out: &mut String) {
    let start = out.len();
    display_into(value, out);
    let text = &out[start..];
    if !text.contains('.') && !text.contains('e') && !text.contains('E') {
        out.push_str(".0");
    }
}

#[cfg(test)]
mod tests;
