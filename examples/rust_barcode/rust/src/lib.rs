//! The Rust of the example `rust_barcode`: `GS1_LABEL` reads the text a barcode scanner sends for a
//! GS1-128 label into its fields, the GTIN (application identifier 01), the batch (10) and the expiry
//! date (17), and checks the GTIN's check digit. Text is where Structured Text is hard work and Rust
//! is not. A label the block cannot read sets `error` and never faults the scan.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

/// GS1's group separator, the FNC1 in a scanner's text: it ends a field of variable length.
const GS: u8 = 0x1D;

/// Reads a GS1-128 label's text into its GTIN, batch and expiry date.
#[trust_fb(name = "GS1_LABEL")]
#[budget("10us")]
pub struct Gs1Label {
    /// The scanner's text, with or without the symbology identifier `]C1`.
    #[input]
    pub code: Str<64>,
    /// The GTIN, 14 digits.
    #[output]
    pub gtin: Str<14>,
    /// The batch or lot number, up to 20 characters.
    #[output]
    pub batch: Str<20>,
    /// The expiry date; a day of 00 means the month's last day.
    #[output]
    pub expiry: Date,
    /// TRUE when the GTIN's last digit is its check digit.
    #[output]
    pub gtin_ok: bool,
    /// 0 when the label was read, otherwise what stopped it: 1 no text, 2 an application identifier
    /// this block does not read, 3 a field that ends early or runs too long, 4 a numeric field with
    /// another character, 5 an expiry date that is no date, 6 no GTIN.
    #[output]
    pub error: i16,
}

impl Body for Gs1Label {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        match parse(self.code.as_bytes()) {
            Ok(label) => {
                self.gtin.set(text(label.gtin));
                self.batch.set(text(label.batch));
                self.gtin_ok = check_digit_ok(label.gtin);
                self.expiry = match label.expiry {
                    // `parse` gives only days that exist, which `from_ymd` takes.
                    Some([year, month, day]) => {
                        Date::from_ymd(cx, year.into(), month.into(), day.into())?
                    }
                    None => Date::default(),
                };
                self.error = 0;
            }
            Err(problem) => {
                self.gtin.set("");
                self.batch.set("");
                self.expiry = Date::default();
                self.gtin_ok = false;
                self.error = problem as i16;
            }
        }
        Ok(())
    }
}

/// What stops a label from being read, as `GS1_LABEL.error` gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i16)]
enum Problem {
    Empty = 1,
    UnknownField = 2,
    Length = 3,
    NotDigits = 4,
    NoSuchDate = 5,
    NoGtin = 6,
}

/// A label's fields as its text holds them.
#[derive(Debug, PartialEq, Eq)]
struct Label<'t> {
    gtin: &'t [u8],
    batch: &'t [u8],
    expiry: Option<[u16; 3]>,
}

/// Reads the application identifiers of `text`, in any order: 01 (14 digits), 17 (6 digits,
/// YYMMDD) and 10 (up to 20 characters, ended by the group separator or the text's end).
fn parse(text: &[u8]) -> Result<Label<'_>, Problem> {
    // A scanner may send GS1-128's symbology identifier first.
    let mut rest = text.strip_prefix(b"]C1").unwrap_or(text);
    if rest.is_empty() {
        return Err(Problem::Empty);
    }
    let mut label = Label {
        gtin: &[],
        batch: &[],
        expiry: None,
    };
    while !rest.is_empty() {
        let (identifier, after) = rest.split_at_checked(2).ok_or(Problem::UnknownField)?;
        rest = match identifier {
            b"01" => {
                let (gtin, next) = fixed(after, 14)?;
                label.gtin = gtin;
                next
            }
            b"17" => {
                let (date, next) = fixed(after, 6)?;
                label.expiry = Some(date_of(date)?);
                next
            }
            b"10" => {
                let (batch, next) = variable(after, 20)?;
                label.batch = batch;
                next
            }
            _ => return Err(Problem::UnknownField),
        };
    }
    if label.gtin.is_empty() {
        return Err(Problem::NoGtin);
    }
    Ok(label)
}

/// The `length` digits of a fixed-length field, and the text after it. A group separator after a
/// fixed-length field is allowed and means nothing.
fn fixed(text: &[u8], length: usize) -> Result<(&[u8], &[u8]), Problem> {
    let (field, rest) = text.split_at_checked(length).ok_or(Problem::Length)?;
    if !field.iter().all(u8::is_ascii_digit) {
        return Err(Problem::NotDigits);
    }
    Ok((field, rest.strip_prefix(&[GS]).unwrap_or(rest)))
}

/// A field of variable length, up to `longest` characters, ended by the group separator or by the
/// text's end, and the text after it.
fn variable(text: &[u8], longest: usize) -> Result<(&[u8], &[u8]), Problem> {
    let end = text
        .iter()
        .position(|&byte| byte == GS)
        .unwrap_or(text.len());
    let (field, rest) = text.split_at_checked(end).ok_or(Problem::Length)?;
    if field.is_empty() || field.len() > longest {
        return Err(Problem::Length);
    }
    Ok((field, rest.strip_prefix(&[GS]).unwrap_or(rest)))
}

/// The year, month and day of YYMMDD digits, in the 2000s. A day of 00 is the month's last day,
/// as GS1 has it for an expiry date given by its month.
fn date_of(digits: &[u8]) -> Result<[u16; 3], Problem> {
    let pair = |at: usize| -> Option<u16> {
        let tens = u16::from(digits.get(at)?.checked_sub(b'0')?);
        let ones = u16::from(digits.get(at + 1)?.checked_sub(b'0')?);
        Some(tens * 10 + ones)
    };
    let (Some(year), Some(month), Some(day)) = (pair(0), pair(2), pair(4)) else {
        return Err(Problem::NotDigits);
    };
    let year = 2000 + year;
    let last = days_in(year, month).ok_or(Problem::NoSuchDate)?;
    match day {
        0 => Ok([year, month, last]),
        day if day <= last => Ok([year, month, day]),
        _ => Err(Problem::NoSuchDate),
    }
}

/// The days of `month` in `year`, `None` for no month.
fn days_in(year: u16, month: u16) -> Option<u16> {
    let leap = year.checked_rem(4) == Some(0)
        && (year.checked_rem(100) != Some(0) || year.checked_rem(400) == Some(0));
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 if leap => Some(29),
        2 => Some(28),
        _ => None,
    }
}

/// Whether the last digit of `gtin` is its GS1 check digit: the digits before it, weighted 3 and 1
/// from the right, add up to a sum whose next multiple of ten the check digit completes.
fn check_digit_ok(gtin: &[u8]) -> bool {
    let Some((check, data)) = gtin.split_last() else {
        return false;
    };
    let sum: u32 = data
        .iter()
        .rev()
        .enumerate()
        .map(|(place, digit)| {
            let weight = if place & 1 == 0 { 3 } else { 1 };
            u32::from(digit.wrapping_sub(b'0')) * weight
        })
        .sum();
    let expected = (10 - sum.checked_rem(10).unwrap_or(0)).checked_rem(10);
    expected == Some(u32::from(check.wrapping_sub(b'0')))
}

/// The text of a field that `parse` checked; a byte that is no UTF-8 gives the empty text.
fn text(bytes: &[u8]) -> &str {
    core::str::from_utf8(bytes).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// GS1's example GTIN, whose check digit is 3: the weighted sum of its first 13 digits is 47.
    const GTIN: &str = "09501101530003";

    #[test]
    fn a_label_s_fields_are_read_in_any_order() {
        // The three application identifiers with and without `]C1`, and with the batch before the
        // date, which its group separator ends. Catches a reader that takes one order only, or that
        // keeps the separator in the batch.
        for code in [
            "01095011015300031726123110ABC123",
            "]C101095011015300031726123110ABC123",
            "010950110153000310ABC123\u{1d}17261231",
        ] {
            assert_eq!(
                parse(code.as_bytes()),
                Ok(Label {
                    gtin: GTIN.as_bytes(),
                    batch: b"ABC123",
                    expiry: Some([2026, 12, 31]),
                }),
                "{code}"
            );
        }
    }

    #[test]
    fn the_check_digit_is_the_one_gs1_gives() {
        // 0x3 + 9x1 + 5x3 + 0x1 + 1x3 + 1x1 + 0x3 + 1x1 + 5x3 + 3x1 + 0x3 + 0x1 + 0x3 = 47, and 50 - 47
        // = 3. Catches weights taken from the left, and a check digit taken as data.
        assert!(check_digit_ok(GTIN.as_bytes()));
        assert!(!check_digit_ok(b"09501101530004"));
        assert!(!check_digit_ok(b""));
    }

    #[test]
    fn a_day_of_00_is_the_month_s_last() {
        // GS1: an expiry date given by its month has the day 00. February 2028 is a leap month, 2026's
        // is not. Catches 00 taken as a day, and a February of 29 days in every year.
        assert_eq!(date_of(b"280200"), Ok([2028, 2, 29]));
        assert_eq!(date_of(b"260200"), Ok([2026, 2, 28]));
        assert_eq!(date_of(b"250600"), Ok([2025, 6, 30]));
        assert_eq!(date_of(b"261331"), Err(Problem::NoSuchDate));
        assert_eq!(date_of(b"260431"), Err(Problem::NoSuchDate));
    }

    #[test]
    fn a_label_that_cannot_be_read_names_why() {
        // Each problem with its own code. Catches a reader that accepts a short GTIN, an unknown
        // identifier, a letter in a date, a batch of more than 20 characters or a label without a
        // GTIN.
        for (code, problem) in [
            ("", Problem::Empty),
            ("]C1", Problem::Empty),
            ("0109501101530", Problem::Length),
            ("990000", Problem::UnknownField),
            ("01095011015300031726AB31", Problem::NotDigits),
            ("010950110153000317261301", Problem::NoSuchDate),
            ("10ABCDEFGHIJKLMNOPQRSTU", Problem::Length),
            ("10ABC123", Problem::NoGtin),
        ] {
            assert_eq!(parse(code.as_bytes()), Err(problem), "{code}");
        }
    }

    #[test]
    fn the_block_gives_the_fields_and_an_error_for_a_bad_label() {
        // The block's body, as a scan calls it: the read label's fields with error 0, then a label
        // with a wrong check digit, read but not ok, and an empty one, error 1 and empty fields.
        // Catches outputs kept from the label before.
        let mut cx = trust_plc::test::cx();
        let mut block = Gs1Label::new();
        block.code.set("01095011015300031726123110ABC123");
        block.body(&mut cx).expect("the call");
        assert_eq!(
            (block.gtin.as_str(), block.batch.as_str()),
            (Some(GTIN), Some("ABC123"))
        );
        assert_eq!(block.expiry.ymd(), [2026, 12, 31]);
        assert!(block.gtin_ok);
        assert_eq!(block.error, 0);

        block.code.set("0109501101530004");
        block.body(&mut cx).expect("the call");
        assert_eq!(block.gtin.as_str(), Some("09501101530004"));
        assert!(!block.gtin_ok);
        assert_eq!(block.error, 0);

        block.code.set("");
        block.body(&mut cx).expect("the call");
        assert_eq!(
            (block.gtin.as_str(), block.batch.as_str()),
            (Some(""), Some(""))
        );
        assert_eq!(block.expiry, Date::default());
        assert_eq!(block.error, 1);
    }
}
