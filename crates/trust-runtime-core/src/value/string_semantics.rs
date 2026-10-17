use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use crate::error::RuntimeError;

use super::Value;

/// The capacity of a `STRING` or `WSTRING` declared without a length (`docs/IEC_DECISIONS.md`
/// 2026-10-17). An STBC string type whose `max_length` is 0 has this capacity.
pub const DEFAULT_STRING_CAPACITY: u16 = 80;

/// The implementer maximum of IEC 61131-3 Ed.3 §6.4.2.2: the longest text a string function may
/// produce, in UTF-8 bytes for `STRING` and in UTF-16 code units for `WSTRING`.
pub const MAX_STRING_CAPACITY: usize = 65_535;

/// Characters (Unicode scalar values): the count of `LEN`, of indexing and of the positions of
/// the string functions (DEV-017).
pub fn string_element_count(text: &str) -> usize {
    text.chars().count()
}

/// The capacity of an STBC `STRING` or `WSTRING` type: its `max_length`, or
/// [`DEFAULT_STRING_CAPACITY`] for 0, which earlier producers wrote for a declaration without a
/// length.
#[must_use]
pub const fn string_capacity(max_length: u16) -> u16 {
    if max_length == 0 {
        DEFAULT_STRING_CAPACITY
    } else {
        max_length
    }
}

/// Where a text is cut to fit `capacity`: UTF-8 bytes for `STRING`, UTF-16 code units for a
/// `wide` `WSTRING`. Returns the byte length of the longest prefix of whole characters that
/// fits, or `None` when the whole text fits. A cut never splits a character.
#[must_use]
pub fn string_capacity_cut(text: &str, wide: bool, capacity: usize) -> Option<usize> {
    // A character never has more UTF-16 code units than UTF-8 bytes, so a text whose bytes fit
    // fits in either unit.
    if text.len() <= capacity {
        return None;
    }
    if !wide {
        let mut end = capacity;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        return Some(end);
    }
    let mut units = 0usize;
    for (index, character) in text.char_indices() {
        units += character.len_utf16();
        if units > capacity {
            return Some(index);
        }
    }
    None
}

/// Refuse a string function result longer than [`MAX_STRING_CAPACITY`] (IEC 61131-3 Ed.3
/// §6.6.2.5.11, Table 34).
///
/// # Errors
///
/// [`RuntimeError::StringCapacityExceeded`] when the text is longer.
pub fn check_string_result(text: &str, wide: bool) -> Result<(), RuntimeError> {
    match string_capacity_cut(text, wide, MAX_STRING_CAPACITY) {
        Some(_) => Err(RuntimeError::StringCapacityExceeded),
        None => Ok(()),
    }
}

pub fn string_element_position(text: &str, index: i64) -> Result<usize, RuntimeError> {
    if index < 1 {
        return Err(RuntimeError::IndexOutOfBounds {
            index,
            lower: 1,
            upper: i64::MAX,
        });
    }
    let upper = i64::try_from(string_element_count(text)).map_err(|_| RuntimeError::Overflow)?;
    if index > upper {
        return Err(RuntimeError::IndexOutOfBounds {
            index,
            lower: 1,
            upper,
        });
    }
    // `index` is in 1..=upper here, so the zero-based position fits `usize` on every target.
    Ok((index - 1) as usize)
}

pub fn read_string_element(text: &str, index: i64, wide: bool) -> Result<Value, RuntimeError> {
    let position = string_element_position(text, index)?;
    let ch = text
        .chars()
        .nth(position)
        .ok_or(RuntimeError::IndexOutOfBounds {
            index,
            lower: 1,
            upper: i64::try_from(string_element_count(text)).map_err(|_| RuntimeError::Overflow)?,
        })?;
    if wide {
        let code = u16::try_from(ch as u32).map_err(|_| RuntimeError::Overflow)?;
        Ok(Value::WChar(code))
    } else {
        let code = u8::try_from(ch as u32).map_err(|_| RuntimeError::Overflow)?;
        Ok(Value::Char(code))
    }
}

/// Replace one element, then cut the text to its declared `capacity` when the replacement made
/// it longer (`docs/IEC_DECISIONS.md` 2026-10-17, the `REPLACE` and assignment of IEC 61131-3
/// Ed.3 §6.4.2.2 Example 1 d)). `capacity` counts UTF-8 bytes for `STRING` and UTF-16 code units
/// for `WSTRING`; a cut never splits a character.
pub fn write_string_element(
    text: &str,
    index: i64,
    value: Value,
    wide: bool,
    capacity: usize,
) -> Result<String, RuntimeError> {
    let position = string_element_position(text, index)?;
    let mut chars: Vec<char> = text.chars().collect();
    chars[position] = value_to_char(value, wide)?;
    let mut updated: String = chars.into_iter().collect();
    if let Some(end) = string_capacity_cut(&updated, wide, capacity) {
        updated.truncate(end);
    }
    Ok(updated)
}

pub fn string_left(text: &str, count: i64) -> String {
    let chars: Vec<char> = text.chars().collect();
    let take = if count <= 0 {
        0
    } else {
        count.min(chars.len() as i64) as usize
    };
    chars.into_iter().take(take).collect()
}

pub fn string_right(text: &str, count: i64) -> String {
    let chars: Vec<char> = text.chars().collect();
    let take = if count <= 0 {
        0
    } else {
        count.min(chars.len() as i64) as usize
    };
    let start = chars.len().saturating_sub(take);
    chars.into_iter().skip(start).collect()
}

// Zero-based index of the element at one-based `position`, or `None` when that
// element is past the end. Positions below one clamp to the first element. The
// conversion is checked and a value that does not fit `usize` is past the end, so
// a 32-bit build gives the same answer as a 64-bit one.
fn start_index(position: i64, element_count: usize) -> Option<usize> {
    let start = if position <= 1 { 0 } else { position - 1 };
    usize::try_from(start)
        .ok()
        .filter(|&start| start < element_count)
}

pub fn string_mid(text: &str, length: i64, position: i64) -> String {
    let chars: Vec<char> = text.chars().collect();
    let Some(start) = start_index(position, chars.len()) else {
        return String::new();
    };
    if length <= 0 {
        return String::new();
    }
    let end = (start as i64)
        .saturating_add(length)
        .min(chars.len() as i64) as usize;
    chars.into_iter().skip(start).take(end - start).collect()
}

pub fn string_insert(input: &str, insert: &str, position: i64) -> String {
    let chars: Vec<char> = input.chars().collect();
    let idx = if position <= 0 {
        0
    } else if position >= chars.len() as i64 {
        chars.len()
    } else {
        position as usize
    };
    let mut result = String::new();
    result.extend(chars.iter().take(idx));
    result.push_str(insert);
    result.extend(chars.iter().skip(idx));
    result
}

pub fn string_delete(input: &str, length: i64, position: i64) -> String {
    if length <= 0 {
        return input.to_string();
    }
    let chars: Vec<char> = input.chars().collect();
    let Some(start) = start_index(position, chars.len()) else {
        return input.to_string();
    };
    let end = (start as i64)
        .saturating_add(length)
        .min(chars.len() as i64) as usize;
    let mut result = String::new();
    result.extend(chars.iter().take(start));
    result.extend(chars.iter().skip(end));
    result
}

pub fn string_replace(input: &str, repl: &str, length: i64, position: i64) -> String {
    let chars: Vec<char> = input.chars().collect();
    let Some(start) = start_index(position, chars.len()) else {
        return input.to_string();
    };
    let end = if length <= 0 {
        start
    } else {
        (start as i64)
            .saturating_add(length)
            .min(chars.len() as i64) as usize
    };
    let mut result = String::new();
    result.extend(chars.iter().take(start));
    result.push_str(repl);
    result.extend(chars.iter().skip(end));
    result
}

pub fn string_find(haystack: &str, needle: &str) -> Result<i16, RuntimeError> {
    let pos = haystack
        .find(needle)
        .map(|idx| haystack[..idx].chars().count() + 1)
        .unwrap_or(0);
    i16::try_from(pos).map_err(|_| RuntimeError::Overflow)
}

fn value_to_char(value: Value, wide: bool) -> Result<char, RuntimeError> {
    let code = match value {
        Value::Char(code) => u32::from(code),
        Value::WChar(code) => u32::from(code),
        Value::String(text) => {
            let mut chars = text.chars();
            let ch = chars.next().ok_or(RuntimeError::TypeMismatch)?;
            if chars.next().is_some() {
                return Err(RuntimeError::TypeMismatch);
            }
            ch as u32
        }
        Value::WString(text) => {
            let mut chars = text.chars();
            let ch = chars.next().ok_or(RuntimeError::TypeMismatch)?;
            if chars.next().is_some() {
                return Err(RuntimeError::TypeMismatch);
            }
            ch as u32
        }
        _ => return Err(RuntimeError::TypeMismatch),
    };
    let max_code = if wide {
        u32::from(u16::MAX)
    } else {
        u32::from(u8::MAX)
    };
    if code > max_code {
        return Err(RuntimeError::Overflow);
    }
    core::char::from_u32(code).ok_or(RuntimeError::TypeMismatch)
}

#[cfg(test)]
mod tests {
    use super::{
        check_string_result, read_string_element, string_capacity, string_capacity_cut,
        string_delete, string_element_count, string_find, string_insert, string_left, string_mid,
        string_replace, string_right, write_string_element, MAX_STRING_CAPACITY,
    };
    use crate::error::RuntimeError;
    use crate::value::Value;
    use alloc::string::String;

    #[test]
    fn a_narrow_capacity_counts_utf8_bytes_and_never_splits_a_character() {
        // "Ä" and "ö" are two bytes each, "€" three and "🙂" four.
        assert_eq!(string_capacity_cut("ABC", false, 3), None);
        assert_eq!(string_capacity_cut("ABCD", false, 3), Some(3));
        assert_eq!(string_capacity_cut("ÄBC", false, 4), None);
        assert_eq!(string_capacity_cut("ÄÖ", false, 3), Some(2));
        assert_eq!(string_capacity_cut("ÄÖ", false, 1), Some(0));
        assert_eq!(string_capacity_cut("A€", false, 3), Some(1));
        assert_eq!(string_capacity_cut("🙂", false, 3), Some(0));
        assert_eq!(string_capacity_cut("", false, 0), None);
    }

    #[test]
    fn a_wide_capacity_counts_utf16_code_units_and_never_splits_a_character() {
        // A character outside the Basic Multilingual Plane is two code units.
        assert_eq!(string_capacity_cut("αβγ", true, 3), None);
        assert_eq!(string_capacity_cut("αβγδ", true, 3), Some("αβγ".len()));
        assert_eq!(string_capacity_cut("A🙂", true, 3), None);
        assert_eq!(string_capacity_cut("A🙂", true, 2), Some(1));
        assert_eq!(string_capacity_cut("🙂🙂", true, 3), Some("🙂".len()));
    }

    #[test]
    fn a_type_without_a_length_has_the_default_capacity() {
        assert_eq!(string_capacity(0), 80);
        assert_eq!(string_capacity(80), 80);
        assert_eq!(string_capacity(5), 5);
        assert_eq!(string_capacity(u16::MAX), 65_535);
    }

    #[test]
    fn a_string_result_may_reach_but_not_pass_the_implementer_maximum() {
        let longest = "x".repeat(MAX_STRING_CAPACITY);
        assert_eq!(check_string_result(&longest, false), Ok(()));
        assert_eq!(check_string_result(&longest, true), Ok(()));
        let longer = "x".repeat(MAX_STRING_CAPACITY + 1);
        assert_eq!(
            check_string_result(&longer, false),
            Err(RuntimeError::StringCapacityExceeded)
        );
        assert_eq!(
            check_string_result(&longer, true),
            Err(RuntimeError::StringCapacityExceeded)
        );
        // 30,000 "€" are 90,000 bytes but 30,000 code units: too long for STRING only.
        let euros = "€".repeat(30_000);
        assert_eq!(
            check_string_result(&euros, false),
            Err(RuntimeError::StringCapacityExceeded)
        );
        assert_eq!(check_string_result(&euros, true), Ok(()));
    }

    #[test]
    fn narrow_string_semantics_count_elements_not_utf8_bytes() {
        assert_eq!(string_element_count("ÄB"), 2);
        assert_eq!(string_left("ÄB", 1), "Ä");
        assert_eq!(string_mid("ÄBC", 1, 2), "B");
        assert_eq!(string_find("ÄBC", "B"), Ok(2));
    }

    #[test]
    fn narrow_string_index_reads_and_writes_single_byte_chars() {
        assert_eq!(read_string_element("ÄB", 1, false), Ok(Value::Char(0xC4)));
        assert_eq!(
            write_string_element("ÄB", 2, Value::Char(b'X'), false, MAX_STRING_CAPACITY),
            Ok(String::from("ÄX"))
        );
    }

    #[test]
    fn narrow_string_index_rejects_out_of_range_chars() {
        let below_range = RuntimeError::IndexOutOfBounds {
            index: 0,
            lower: 1,
            upper: i64::MAX,
        };
        assert_eq!(
            read_string_element("AB", 0, false),
            Err(below_range.clone())
        );
        assert_eq!(
            write_string_element("AB", 0, Value::DInt(1), false, MAX_STRING_CAPACITY),
            Err(below_range)
        );

        let above_range = RuntimeError::IndexOutOfBounds {
            index: 3,
            lower: 1,
            upper: 2,
        };
        assert_eq!(
            read_string_element("AB", 3, false),
            Err(above_range.clone())
        );
        assert_eq!(
            write_string_element("AB", 3, Value::DInt(1), false, MAX_STRING_CAPACITY),
            Err(above_range)
        );

        assert_eq!(
            read_string_element("🙂", 1, false),
            Err(RuntimeError::Overflow)
        );
        assert_eq!(
            write_string_element(
                "AB",
                1,
                Value::String("🙂".into()),
                false,
                MAX_STRING_CAPACITY
            ),
            Err(RuntimeError::Overflow)
        );
        assert_eq!(
            write_string_element(
                "AB",
                1,
                Value::String("".into()),
                false,
                MAX_STRING_CAPACITY
            ),
            Err(RuntimeError::TypeMismatch)
        );
        assert_eq!(
            write_string_element(
                "AB",
                1,
                Value::String("XY".into()),
                false,
                MAX_STRING_CAPACITY
            ),
            Err(RuntimeError::TypeMismatch)
        );
        assert_eq!(
            write_string_element("AB", 1, Value::DInt(1), false, MAX_STRING_CAPACITY),
            Err(RuntimeError::TypeMismatch)
        );
    }

    #[test]
    fn a_char_write_cuts_a_narrow_string_to_its_capacity_without_splitting_a_character() {
        // A full STRING[3] = 'abc'. CHAR#16#C4 is 'Ä', two UTF-8 bytes, so the text
        // 'Äbc' is four bytes and is cut to 'Äb' (three bytes).
        assert_eq!(
            write_string_element("abc", 1, Value::Char(0xC4), false, 3),
            Ok(String::from("Äb"))
        );
        // The cut keeps whole characters: 'Ää' is four bytes, and the two-byte 'ä'
        // does not fit the remaining single byte, so the text becomes 'Ä'.
        assert_eq!(
            write_string_element("Äb", 2, Value::Char(0xE4), false, 3),
            Ok(String::from("Ä"))
        );
        // A replacement that still fits its capacity is not cut.
        assert_eq!(
            write_string_element("abc", 2, Value::Char(b'X'), false, 3),
            Ok(String::from("aXc"))
        );
    }

    #[test]
    fn a_char_write_to_a_wide_string_measures_its_capacity_in_utf16_code_units() {
        // A full WSTRING[3] = 'abc'. WCHAR#16#C4 is 'Ä', one UTF-16 code unit, so the
        // text 'Äbc' is three code units and stays whole even though it is four bytes.
        assert_eq!(
            write_string_element("abc", 1, Value::WChar(0x00C4), true, 3),
            Ok(String::from("Äbc"))
        );
    }

    #[test]
    fn wide_string_index_reads_and_writes_unicode_scalar_elements() {
        assert_eq!(read_string_element("ÄB", 1, true), Ok(Value::WChar(0x00C4)));
        assert_eq!(
            write_string_element("ÄB", 2, Value::WChar('Ω' as u16), true, MAX_STRING_CAPACITY,),
            Ok(String::from("ÄΩ"))
        );
        assert_eq!(
            write_string_element(
                "ÄB",
                2,
                Value::WString("🙂".into()),
                true,
                MAX_STRING_CAPACITY
            ),
            Err(RuntimeError::Overflow)
        );
        assert_eq!(
            write_string_element("ÄB", 2, Value::WChar(0xD800), true, MAX_STRING_CAPACITY),
            Err(RuntimeError::TypeMismatch)
        );
    }

    #[test]
    fn positions_and_lengths_past_u32_max_match_the_64_bit_results() {
        // 2^32 + 1 truncates to 1 in a 32-bit `usize`. Each call must still read the
        // position as past the end, or the length as past the end and clamped.
        let above_u32: i64 = (1 << 32) + 1;
        let max = i64::MAX;
        assert_eq!(string_mid("ABC", 1, above_u32), "");
        assert_eq!(string_mid("ABC", 1, max), "");
        assert_eq!(string_mid("ABC", above_u32, 2), "BC");
        assert_eq!(string_mid("ABC", max, 2), "BC");
        assert_eq!(string_left("ABC", above_u32), "ABC");
        assert_eq!(string_left("ABC", max), "ABC");
        assert_eq!(string_right("ABC", above_u32), "ABC");
        assert_eq!(string_right("ABC", max), "ABC");
        assert_eq!(string_delete("ABC", 1, above_u32), "ABC");
        assert_eq!(string_delete("ABC", 1, max), "ABC");
        assert_eq!(string_delete("ABC", above_u32, 2), "A");
        assert_eq!(string_delete("ABC", max, 2), "A");
        assert_eq!(string_insert("ABC", "X", above_u32), "ABCX");
        assert_eq!(string_insert("ABC", "X", max), "ABCX");
        assert_eq!(string_replace("ABC", "XY", 1, above_u32), "ABC");
        assert_eq!(string_replace("ABC", "XY", 1, max), "ABC");
        assert_eq!(string_replace("ABC", "XY", above_u32, 2), "AXY");
        assert_eq!(string_replace("ABC", "XY", max, 2), "AXY");
    }

    #[test]
    fn element_index_past_u32_max_is_out_of_bounds_on_every_target() {
        let above_u32: i64 = (1 << 32) + 1;
        let out_of_bounds = RuntimeError::IndexOutOfBounds {
            index: above_u32,
            lower: 1,
            upper: 2,
        };
        assert_eq!(
            read_string_element("AB", above_u32, false),
            Err(out_of_bounds.clone())
        );
        assert_eq!(
            write_string_element(
                "AB",
                above_u32,
                Value::Char(b'X'),
                false,
                MAX_STRING_CAPACITY
            ),
            Err(out_of_bounds)
        );
        let last = RuntimeError::IndexOutOfBounds {
            index: i64::MAX,
            lower: 1,
            upper: 2,
        };
        assert_eq!(read_string_element("AB", i64::MAX, false), Err(last));
    }
}
