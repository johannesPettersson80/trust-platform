use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use crate::error::RuntimeError;

use super::Value;

pub fn string_element_count(text: &str) -> usize {
    text.chars().count()
}

pub fn truncate_string_elements(text: &str, max_len: u32) -> String {
    let max_len = max_len as usize;
    if string_element_count(text) <= max_len {
        text.to_string()
    } else {
        string_left(text, max_len as i64)
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

pub fn write_string_element(
    text: &str,
    index: i64,
    value: Value,
    wide: bool,
) -> Result<String, RuntimeError> {
    let position = string_element_position(text, index)?;
    let mut chars: Vec<char> = text.chars().collect();
    chars[position] = value_to_char(value, wide)?;
    Ok(chars.into_iter().collect())
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
        read_string_element, string_delete, string_element_count, string_find, string_insert,
        string_left, string_mid, string_replace, string_right, write_string_element,
    };
    use crate::error::RuntimeError;
    use crate::value::Value;
    use alloc::string::String;

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
            write_string_element("ÄB", 2, Value::Char(b'X'), false),
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
            write_string_element("AB", 0, Value::DInt(1), false),
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
            write_string_element("AB", 3, Value::DInt(1), false),
            Err(above_range)
        );

        assert_eq!(
            read_string_element("🙂", 1, false),
            Err(RuntimeError::Overflow)
        );
        assert_eq!(
            write_string_element("AB", 1, Value::String("🙂".into()), false),
            Err(RuntimeError::Overflow)
        );
        assert_eq!(
            write_string_element("AB", 1, Value::String("".into()), false),
            Err(RuntimeError::TypeMismatch)
        );
        assert_eq!(
            write_string_element("AB", 1, Value::String("XY".into()), false),
            Err(RuntimeError::TypeMismatch)
        );
        assert_eq!(
            write_string_element("AB", 1, Value::DInt(1), false),
            Err(RuntimeError::TypeMismatch)
        );
    }

    #[test]
    fn wide_string_index_reads_and_writes_unicode_scalar_elements() {
        assert_eq!(read_string_element("ÄB", 1, true), Ok(Value::WChar(0x00C4)));
        assert_eq!(
            write_string_element("ÄB", 2, Value::WChar('Ω' as u16), true),
            Ok(String::from("ÄΩ"))
        );
        assert_eq!(
            write_string_element("ÄB", 2, Value::WString("🙂".into()), true),
            Err(RuntimeError::Overflow)
        );
        assert_eq!(
            write_string_element("ÄB", 2, Value::WChar(0xD800), true),
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
            write_string_element("AB", above_u32, Value::Char(b'X'), false),
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
