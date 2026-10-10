//! The text of numbers and Booleans in the sentences the engineer reads. The refusals of `experiment.ts` print numbers
//! with JavaScript's `String(value)`, so this module reproduces that text exactly.

use std::fmt::Write as _;

use super::plan::is_zero;

/// The text of a Boolean in a sentence (`String(value)` for a Boolean).
pub(super) fn js_bool(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

/// The text JavaScript's `String(value)` gives a finite number: the shortest digits that read back exactly, in
/// positional notation for decimal exponents from −6 to 20, and in exponential notation otherwise.
pub(crate) fn js_number(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    if is_zero(value) {
        return "0".to_string();
    }
    let sign = if value < 0.0 { "-" } else { "" };
    let scientific = format!("{:e}", value.abs());
    let Some((mantissa, exponent)) = scientific.split_once('e') else {
        return value.to_string();
    };
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let exponent: i32 = exponent.parse().unwrap_or(0);
    // ECMAScript's `n`: the decimal point sits after `n` digits of the `k` shortest digits.
    let k = i32::try_from(digits.len()).unwrap_or(0);
    let n = exponent + 1;
    let body = if k <= n && n <= 21 {
        format!(
            "{digits}{}",
            "0".repeat(usize::try_from(n - k).unwrap_or(0))
        )
    } else if 0 < n && n <= 21 {
        let (whole, fraction) = digits.split_at(usize::try_from(n).unwrap_or(0));
        format!("{whole}.{fraction}")
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat(usize::try_from(-n).unwrap_or(0)))
    } else {
        let (first, rest) = digits.split_at(1);
        let mantissa = if rest.is_empty() {
            first.to_string()
        } else {
            format!("{first}.{rest}")
        };
        let exponent_sign = if n - 1 < 0 { "-" } else { "+" };
        format!("{mantissa}e{exponent_sign}{}", (n - 1).abs())
    };
    format!("{sign}{body}")
}

/// `Number(value.toPrecision(precision))`: the value rounded to `precision` significant digits, an exact tie going
/// up in magnitude as JavaScript rounds it, and read back as a number. A zero or non-finite value is returned as it is.
pub(crate) fn js_to_precision(value: f64, precision: usize) -> f64 {
    let keep = i64::try_from(precision).unwrap_or(i64::MAX);
    round_magnitude(value, |digits, exponent| {
        round_half_up(digits, exponent, keep)
    })
}

/// `Number(value.toFixed(digits))`: the value rounded to `digits` places after the decimal point, an exact tie going
/// up in magnitude as JavaScript rounds it, and read back as a number. A zero or non-finite value is returned as it is.
pub(crate) fn js_to_fixed(value: f64, digits: usize) -> f64 {
    round_magnitude(value, |exact, exponent| {
        let places = i64::try_from(digits).unwrap_or(i64::MAX);
        round_half_up(exact, exponent, i64::from(exponent) + places + 1)
    })
}

/// `Math.round(value)`: the nearest integer, with an exact half going toward positive infinity, so `-2.5` gives `-2`.
pub(crate) fn js_round(value: f64) -> f64 {
    let floor = value.floor();
    if value - floor >= 0.5 {
        floor + 1.0
    } else {
        floor
    }
}

/// Rounds the magnitude of a value with `round` on its exact digits, then restores its sign.
fn round_magnitude(value: f64, round: impl Fn(&[u8], i32) -> Option<(Vec<u8>, i32)>) -> f64 {
    if !value.is_finite() || value == 0.0 {
        return if value.is_finite() { 0.0 } else { value };
    }
    let (exact, exponent) = exact_digits(value.abs());
    let magnitude =
        round(&exact, exponent).map_or(0.0, |(digits, rounded)| read_back(&digits, rounded));
    if value < 0.0 {
        -magnitude
    } else {
        magnitude
    }
}

/// The exact decimal digits of a finite, non-zero value, and the exponent `e` of its leading digit, so that the value
/// is `d₀.d₁d₂… × 10^e`. Every finite double has at most 767 significant decimal digits, so the 800 digits written
/// after the point of the scientific form are exact.
fn exact_digits(value: f64) -> (Vec<u8>, i32) {
    let text = format!("{value:.800e}");
    let (mantissa, exponent) = text.split_once('e').unwrap_or((text.as_str(), "0"));
    let digits = mantissa
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(|byte| byte - b'0')
        .collect();
    (digits, exponent.parse().unwrap_or(0))
}

/// Keeps the first `keep` significant digits of exact digits, rounded half up on the next digit. The result is
/// `None` when a `keep` below zero leaves nothing; with `keep` of zero the value rounds to one unit of the
/// exponent's place or to zero. A carry out of the leading digit moves the exponent up.
fn round_half_up(digits: &[u8], exponent: i32, keep: i64) -> Option<(Vec<u8>, i32)> {
    let keep = usize::try_from(keep).ok()?;
    if keep == 0 {
        return digits
            .first()
            .is_some_and(|&digit| digit >= 5)
            .then_some((vec![1], exponent + 1));
    }
    let mut kept: Vec<u8> = digits.iter().take(keep).copied().collect();
    kept.resize(keep, 0);
    let mut exponent = exponent;
    if digits.get(keep).is_some_and(|&digit| digit >= 5) {
        let mut index = keep;
        loop {
            if index == 0 {
                kept.insert(0, 1);
                kept.truncate(keep);
                exponent += 1;
                break;
            }
            index -= 1;
            if kept[index] == 9 {
                kept[index] = 0;
            } else {
                kept[index] += 1;
                break;
            }
        }
    }
    Some((kept, exponent))
}

/// The number read from significant digits and their exponent, as JavaScript reads the decimal text.
fn read_back(digits: &[u8], exponent: i32) -> f64 {
    let mut text = String::new();
    for (index, digit) in digits.iter().enumerate() {
        if index == 1 {
            text.push('.');
        }
        text.push(char::from(b'0' + digit));
    }
    let _ = write!(text, "e{exponent}");
    text.parse().unwrap_or(f64::NAN)
}

#[cfg(test)]
mod tests {
    use super::{js_number, js_to_fixed, js_to_precision};

    #[test]
    fn js_number_text_matches_string_of_a_number() {
        assert_eq!(js_number(110.0), "110");
        assert_eq!(js_number(-2.5), "-2.5");
        assert_eq!(js_number(0.5), "0.5");
        assert_eq!(js_number(25.05), "25.05");
        assert_eq!(js_number(1.2345), "1.2345");
        assert_eq!(js_number(0.0), "0");
        assert_eq!(js_number(-0.0), "0");
        assert_eq!(js_number(0.1 + 0.2), "0.30000000000000004");
        assert_eq!(js_number(0.000_001), "0.000001");
        assert_eq!(js_number(1e-7), "1e-7");
        assert_eq!(js_number(1e21), "1e+21");
        assert_eq!(
            js_number(123_456_789_012_345_680_000.0),
            "123456789012345680000"
        );
        assert_eq!(js_number(1.5e300), "1.5e+300");
    }

    #[test]
    fn js_rounding_sends_an_exact_tie_up_where_rust_rounds_it_to_even() {
        // 2.125 and 12.5 are exact in binary, so each is a tie: toPrecision rounds it up in magnitude.
        assert_eq!(js_number(js_to_precision(2.125, 3)), "2.13");
        assert_eq!(js_number(js_to_precision(-2.125, 3)), "-2.13");
        assert_eq!(js_number(js_to_precision(12.5, 2)), "13");
        assert_eq!(js_number(js_to_precision(99.96, 3)), "100");
        assert_eq!(js_number(js_to_precision(123_456.0, 3)), "123000");
        // toFixed: 12.3125 = 12 + 5/16 is a tie at the fourth decimal.
        assert_eq!(js_number(js_to_fixed(12.3125, 3)), "12.313");
        assert_eq!(js_number(js_to_fixed(0.0006, 3)), "0.001");
        assert_eq!(js_number(js_to_fixed(0.0004, 3)), "0");
        assert_eq!(js_number(js_to_fixed(9.9996, 3)), "10");
        assert_eq!(js_number(js_to_precision(0.0, 3)), "0");
    }
}
