//! The IEC 61131-3 literal a value text spells (IEC 61131-3 Ed.3 §6.3.2 to §6.3.5, Tables 5 to 9,
//! and §6.4.4.2 for an enumeration's value), read as the language's own tokens read it
//! (`trust-syntax`): the syntax alone. [`super::typed`] admits a literal as a value of its
//! destination's declared type.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};

use trust_runtime_core::datetime::days_from_civil;

/// A literal of a value text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Literal {
    /// `TRUE` or `FALSE`, in any case (Table 5, feature 8).
    Bool(bool),
    /// An integer (Table 5, features 1 and 4 to 6): a decimal number, which may have a sign, or a
    /// number in base 2, 8 or 16 (`based`), which has none and is a bit-string literal. Single
    /// underscores between digits are not significant.
    Integer {
        /// The number.
        value: i128,
        /// Whether it is written in base 2, 8 or 16.
        based: bool,
    },
    /// A real number (Table 5, features 2 and 3): its decimal text, sign and exponent included,
    /// without underscores. Its destination reads the text once, so a `REAL` is rounded from the
    /// text and not through `LREAL`.
    Real(String),
    /// A duration (Table 8), in whole nanoseconds; `long` for `LT#` and `LTIME#`.
    Duration {
        /// Whether it is an `LTIME` literal.
        long: bool,
        /// The duration.
        nanos: i64,
    },
    /// A date (Table 9), in days since 1970-01-01; `long` for `LD#` and `LDATE#`.
    Date {
        /// Whether it is an `LDATE` literal.
        long: bool,
        /// The days since 1970-01-01.
        days: i64,
    },
    /// A time of day (Table 9), in nanoseconds since midnight; `long` for `LTOD#` and
    /// `LTIME_OF_DAY#`.
    TimeOfDay {
        /// Whether it is an `LTIME_OF_DAY` literal.
        long: bool,
        /// The nanoseconds since midnight.
        nanos: i64,
    },
    /// A date and time of day (Table 9); `long` for `LDT#` and `LDATE_AND_TIME#`.
    DateAndTime {
        /// Whether it is an `LDATE_AND_TIME` literal.
        long: bool,
        /// The days since 1970-01-01.
        days: i64,
        /// The nanoseconds since that day's midnight.
        nanos: i64,
    },
    /// A character string (Tables 6 and 7) with its two-character combinations decoded: `'…'`, or
    /// `"…"` when `wide`.
    Text {
        /// Whether it is a double-byte string, in double quotes.
        wide: bool,
        /// The characters.
        text: String,
    },
    /// A name, which an enumeration's value is (§6.4.4.2): `Red`.
    Name(String),
    /// A literal with the name of its type before `#` (Table 5 feature 9, Table 6 features 3 and 4,
    /// §6.4.4.2): `INT#-5`, `WSTRING#'OK'`, `Color#Red`.
    Typed {
        /// The type's name, as written.
        type_name: String,
        /// The literal after the `#`.
        literal: Box<Literal>,
    },
}

/// Why a text is no literal: no literal's syntax, or a literal whose value no type holds.
enum Bad {
    Syntax,
    Value(String),
}

/// The nanoseconds of a duration unit (Table 8).
const UNITS: [(&str, i128); 7] = [
    ("MS", 1_000_000),
    ("US", 1_000),
    ("NS", 1),
    ("D", 86_400_000_000_000),
    ("H", 3_600_000_000_000),
    ("M", 60_000_000_000),
    ("S", 1_000_000_000),
];

/// The scale of one duration segment: a segment counts in 10^-18 of a nanosecond, so its fraction
/// digits add up exactly and a total finer than a nanosecond is found.
const SCALE: i128 = 1_000_000_000_000_000_000;
/// The fraction digits a duration segment keeps exactly.
const SCALE_DIGITS: usize = 18;

/// The literal `text` spells, its surrounding whitespace ignored.
///
/// # Errors
///
/// The stable refusal `unsupported value '<text>'` for a text that is no literal, and a plain
/// reason for a literal whose value no type holds: a date that does not exist, a duration finer
/// than a nanosecond or longer than any duration.
pub fn parse_literal(text: &str) -> Result<Literal, String> {
    literal(text.trim()).map_err(|bad| match bad {
        Bad::Syntax => format!("unsupported value '{text}'"),
        Bad::Value(reason) => reason,
    })
}

fn literal(text: &str) -> Result<Literal, Bad> {
    if let Some((prefix, rest)) = typed_prefix(text) {
        return prefixed(prefix, rest);
    }
    plain(text)
}

/// A literal without a type prefix: a truth value, a text, a name or a number.
fn plain(text: &str) -> Result<Literal, Bad> {
    if text.eq_ignore_ascii_case("TRUE") {
        return Ok(Literal::Bool(true));
    }
    if text.eq_ignore_ascii_case("FALSE") {
        return Ok(Literal::Bool(false));
    }
    if text.starts_with('\'') {
        return quoted(text, false);
    }
    if text.starts_with('"') {
        return quoted(text, true);
    }
    if is_identifier(text) {
        return Ok(Literal::Name(text.to_string()));
    }
    number(text)
}

/// The type prefix `name#` at the start of `text` and the rest; a based number (`16#FF`) has none,
/// because a prefix begins with a letter or an underscore.
fn typed_prefix(text: &str) -> Option<(&str, &str)> {
    let (prefix, rest) = text.split_once('#')?;
    is_identifier(prefix).then_some((prefix, rest))
}

/// Whether `text` is an identifier: a letter or an underscore, then letters, digits and
/// underscores (the `Ident` token).
fn is_identifier(text: &str) -> bool {
    let mut characters = text.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && characters.all(|next| next.is_ascii_alphanumeric() || next == '_')
}

/// The literal after the type prefix `prefix`: a duration, a date or a time of day by its own
/// prefix, and otherwise a typed literal of a number, a truth value, a text or a name.
fn prefixed(prefix: &str, rest: &str) -> Result<Literal, Bad> {
    let upper = prefix.to_ascii_uppercase();
    match upper.as_str() {
        "T" | "TIME" => duration(rest, false),
        "LT" | "LTIME" => duration(rest, true),
        "D" | "DATE" => date(rest, false),
        "LD" | "LDATE" => date(rest, true),
        "TOD" | "TIME_OF_DAY" => time_of_day(rest, false),
        "LTOD" | "LTIME_OF_DAY" => time_of_day(rest, true),
        "DT" | "DATE_AND_TIME" => date_and_time(rest, false),
        "LDT" | "LDATE_AND_TIME" => date_and_time(rest, true),
        _ => {
            // One prefix only: `INT#INT#5` is no literal; a based number after it is (`INT#16#7F`).
            if typed_prefix(rest).is_some() {
                return Err(Bad::Syntax);
            }
            Ok(Literal::Typed {
                type_name: prefix.to_string(),
                literal: Box::new(plain(rest)?),
            })
        }
    }
}

/// A decimal or based integer, or a real (Table 5): a sign is allowed before a decimal number only.
fn number(text: &str) -> Result<Literal, Bad> {
    let (negative, unsigned) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    if let Some((base, digits)) = unsigned.split_once('#') {
        if unsigned.len() != text.len() {
            return Err(Bad::Syntax);
        }
        let radix = match base {
            "2" => 2,
            "8" => 8,
            "16" => 16,
            _ => return Err(Bad::Syntax),
        };
        let value = digits_value(digits, radix, text)?;
        return Ok(Literal::Integer { value, based: true });
    }
    if let Some((whole, fraction)) = unsigned.split_once('.') {
        let (fraction, exponent) = match fraction.find(['e', 'E']) {
            Some(at) => (&fraction[..at], Some(&fraction[at + 1..])),
            None => (fraction, None),
        };
        if !is_digits(whole) || !is_digits(fraction) {
            return Err(Bad::Syntax);
        }
        let mut real = String::new();
        if negative {
            real.push('-');
        }
        real.extend(whole.chars().filter(|c| *c != '_'));
        real.push('.');
        real.extend(fraction.chars().filter(|c| *c != '_'));
        if let Some(exponent) = exponent {
            let digits = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
            if !is_digits(digits) {
                return Err(Bad::Syntax);
            }
            real.push('E');
            real.extend(exponent.chars().filter(|c| *c != '_'));
        }
        return Ok(Literal::Real(real));
    }
    if !is_digits(unsigned) {
        return Err(Bad::Syntax);
    }
    let magnitude = digits_value(unsigned, 10, text)?;
    Ok(Literal::Integer {
        value: if negative { -magnitude } else { magnitude },
        based: false,
    })
}

/// Whether `text` is decimal digits with single underscores between them (`[0-9]([0-9]|_[0-9])*`).
fn is_digits(text: &str) -> bool {
    in_radix(text, 10)
}

/// Whether `text` is digits of `radix` with single underscores between them.
fn in_radix(text: &str, radix: u32) -> bool {
    let bytes = text.as_bytes();
    !bytes.is_empty()
        && char::from(bytes[0]).is_digit(radix)
        && bytes.windows(2).all(|pair| match pair {
            [b'_', next] => char::from(*next).is_digit(radix),
            [_, b'_'] => true,
            [_, next] => char::from(*next).is_digit(radix),
            _ => false,
        })
        && bytes.last().is_some_and(|last| *last != b'_')
}

/// The value of the digits `text` in `radix`, of the integer literal `literal`.
///
/// # Errors
///
/// [`Bad::Syntax`] for text that is no such digits, and the reason of a number beyond `i128`,
/// which no integer type holds.
fn digits_value(text: &str, radix: u32, literal: &str) -> Result<i128, Bad> {
    if !in_radix(text, radix) {
        return Err(Bad::Syntax);
    }
    let mut value: i128 = 0;
    for digit in text.chars().filter(|c| *c != '_') {
        let digit = i128::from(digit.to_digit(radix).ok_or(Bad::Syntax)?);
        value = value
            .checked_mul(i128::from(radix))
            .and_then(|value| value.checked_add(digit))
            .ok_or_else(|| {
                Bad::Value(format!(
                    "'{literal}' is outside the range of every integer type"
                ))
            })?;
    }
    Ok(value)
}

/// A duration after its prefix (Table 8): an optional sign, then numbers each followed by its unit,
/// `d`, `h`, `m`, `s`, `ms`, `us` or `ns` in any case, separated by an optional underscore; a
/// number may have a fraction. The total is a whole number of nanoseconds within `i64`.
fn duration(text: &str, long: bool) -> Result<Literal, Bad> {
    let (negative, mut rest) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    if rest.is_empty() {
        return Err(Bad::Syntax);
    }
    let mut total: i128 = 0;
    while !rest.is_empty() {
        let digits = rest
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .ok_or(Bad::Syntax)?;
        let (whole, fraction) = rest[..digits]
            .split_once('.')
            .unwrap_or((&rest[..digits], ""));
        let plain_digits = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
        if whole.is_empty()
            || !plain_digits(whole)
            || !plain_digits(fraction)
            || (rest[..digits].contains('.') && fraction.is_empty())
            || fraction.len() > SCALE_DIGITS
        {
            return Err(Bad::Syntax);
        }
        rest = &rest[digits..];
        let upper = rest.get(..2).map(str::to_ascii_uppercase);
        let (unit, nanos) = UNITS
            .iter()
            .find(|(unit, _)| match unit.len() {
                2 => upper.as_deref() == Some(*unit),
                _ => rest
                    .get(..1)
                    .is_some_and(|first| first.eq_ignore_ascii_case(unit)),
            })
            .ok_or(Bad::Syntax)?;
        rest = &rest[unit.len()..];
        let out_of_range = || Bad::Value(format!("'{text}' is longer than any duration"));
        let whole: i128 = whole.parse().map_err(|_| out_of_range())?;
        let mut scaled_fraction: i128 = 0;
        for digit in fraction.bytes() {
            scaled_fraction = scaled_fraction * 10 + i128::from(digit - b'0');
        }
        for _ in fraction.len()..SCALE_DIGITS {
            scaled_fraction *= 10;
        }
        let segment = whole
            .checked_mul(SCALE)
            .and_then(|scaled| scaled.checked_add(scaled_fraction))
            .and_then(|scaled| scaled.checked_mul(*nanos))
            .ok_or_else(out_of_range)?;
        total = total.checked_add(segment).ok_or_else(out_of_range)?;
        if let Some(after) = rest.strip_prefix('_') {
            if after.is_empty() {
                return Err(Bad::Syntax);
            }
            rest = after;
        }
    }
    if total % SCALE != 0 {
        return Err(Bad::Value(format!(
            "'{text}' is finer than the nanosecond a duration holds"
        )));
    }
    let magnitude = total / SCALE;
    let nanos = i64::try_from(if negative { -magnitude } else { magnitude })
        .map_err(|_| Bad::Value(format!("'{text}' is longer than any duration")))?;
    Ok(Literal::Duration { long, nanos })
}

/// The number of the `count` decimal digits of `text` from `start`, which must be digits.
fn fixed(text: &str, start: usize, count: usize) -> Result<i64, Bad> {
    let part = text.get(start..start + count).ok_or(Bad::Syntax)?;
    if !part.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Bad::Syntax);
    }
    part.parse().map_err(|_| Bad::Syntax)
}

/// The days since 1970-01-01 of the date `yyyy-mm-dd` at the start of `text`, which must exist.
fn days(text: &str, whole: &str) -> Result<i64, Bad> {
    if text.get(4..5) != Some("-") || text.get(7..8) != Some("-") {
        return Err(Bad::Syntax);
    }
    let (year, month, day) = (fixed(text, 0, 4)?, fixed(text, 5, 2)?, fixed(text, 8, 2)?);
    days_from_civil(year, month, day).map_err(|_| Bad::Value(format!("'{whole}' is no date")))
}

/// A date after its prefix (Table 9): `yyyy-mm-dd`.
fn date(text: &str, long: bool) -> Result<Literal, Bad> {
    if text.len() != 10 {
        return Err(Bad::Syntax);
    }
    Ok(Literal::Date {
        long,
        days: days(text, text)?,
    })
}

/// The nanoseconds since midnight of `hh:mm:ss`, with an optional fraction of a second whose digits
/// may be separated by single underscores; a fraction finer than a nanosecond is refused.
fn clock(text: &str) -> Result<i64, Bad> {
    if text.get(2..3) != Some(":") || text.get(5..6) != Some(":") || text.len() < 8 {
        return Err(Bad::Syntax);
    }
    let (hours, minutes, seconds) = (fixed(text, 0, 2)?, fixed(text, 3, 2)?, fixed(text, 6, 2)?);
    let fraction = match &text[8..] {
        "" => "",
        rest => rest.strip_prefix('.').ok_or(Bad::Syntax)?,
    };
    if !fraction.is_empty() && !is_digits(fraction) {
        return Err(Bad::Syntax);
    }
    if hours > 23 || minutes > 59 || seconds > 59 {
        return Err(Bad::Value(format!("'{text}' is no time of day")));
    }
    let digits: String = fraction.chars().filter(|c| *c != '_').collect();
    if digits.len() > 9 && digits[9..].bytes().any(|byte| byte != b'0') {
        return Err(Bad::Value(format!(
            "'{text}' is finer than the nanosecond a time of day holds"
        )));
    }
    let mut nanos: i64 = 0;
    for position in 0..9 {
        let digit = digits
            .as_bytes()
            .get(position)
            .map_or(0, |byte| byte - b'0');
        nanos = nanos * 10 + i64::from(digit);
    }
    Ok(((hours * 60 + minutes) * 60 + seconds) * 1_000_000_000 + nanos)
}

/// A time of day after its prefix (Table 9): `hh:mm:ss` with an optional fraction.
fn time_of_day(text: &str, long: bool) -> Result<Literal, Bad> {
    Ok(Literal::TimeOfDay {
        long,
        nanos: clock(text)?,
    })
}

/// A date and time of day after its prefix (Table 9): `yyyy-mm-dd-hh:mm:ss` with an optional
/// fraction.
fn date_and_time(text: &str, long: bool) -> Result<Literal, Bad> {
    if text.get(10..11) != Some("-") {
        return Err(Bad::Syntax);
    }
    Ok(Literal::DateAndTime {
        long,
        days: days(&text[..10], text)?,
        nanos: clock(&text[11..])?,
    })
}

/// A character string in quotes (Tables 6 and 7): `'…'` with `$hh` codes, or, when `wide`, `"…"`
/// with `$hhhh` codes; `$$`, `$L`, `$N`, `$P`, `$R` and `$T` in any case, and the string's own
/// quote as `$'` or `$"`.
fn quoted(text: &str, wide: bool) -> Result<Literal, Bad> {
    let quote = if wide { '"' } else { '\'' };
    let inner = text
        .strip_prefix(quote)
        .and_then(|rest| rest.strip_suffix(quote))
        .ok_or(Bad::Syntax)?;
    let mut decoded = String::new();
    let mut characters = inner.chars();
    while let Some(character) = characters.next() {
        match character {
            '$' => decoded.push(escape(&mut characters, wide).ok_or(Bad::Syntax)?),
            '\r' | '\n' => return Err(Bad::Syntax),
            _ if character == quote => return Err(Bad::Syntax),
            _ => decoded.push(character),
        }
    }
    Ok(Literal::Text {
        wide,
        text: decoded,
    })
}

/// The character a `$` combination after it stands for (Table 7), or the character of its code:
/// two hexadecimal digits in a single-byte string, four in a double-byte one.
fn escape(characters: &mut core::str::Chars<'_>, wide: bool) -> Option<char> {
    let first = characters.next()?;
    let named = match first.to_ascii_uppercase() {
        '$' => Some('$'),
        'L' | 'N' => Some('\n'),
        'P' => Some('\u{000C}'),
        'R' => Some('\r'),
        'T' => Some('\t'),
        '\'' if !wide => Some('\''),
        '"' if wide => Some('"'),
        _ => None,
    };
    if named.is_some() {
        return named;
    }
    let digits = if wide { 4 } else { 2 };
    let mut code = first.to_digit(16)?;
    for _ in 1..digits {
        code = code * 16 + characters.next()?.to_digit(16)?;
    }
    char::from_u32(code)
}

#[cfg(test)]
mod tests;
