use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use crate::error::RuntimeError;

use super::Value;

pub fn string_element_count(text: &str) -> usize {
    text.chars().count()
}

pub fn string_element_position(text: &str, index: i64) -> Result<usize, RuntimeError> {
    if index < 1 {
        return Err(RuntimeError::IndexOutOfBounds {
            index,
            lower: 1,
            upper: i64::MAX,
        });
    }
    let position = usize::try_from(index - 1).map_err(|_| RuntimeError::Overflow)?;
    let upper = i64::try_from(string_element_count(text)).map_err(|_| RuntimeError::Overflow)?;
    if position >= upper as usize {
        return Err(RuntimeError::IndexOutOfBounds {
            index,
            lower: 1,
            upper,
        });
    }
    Ok(position)
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

pub fn string_mid(text: &str, length: i64, position: i64) -> String {
    let chars: Vec<char> = text.chars().collect();
    let start = if position <= 1 {
        0
    } else {
        position as usize - 1
    };
    if start >= chars.len() || length <= 0 {
        return String::new();
    }
    let end = (start as i64 + length).min(chars.len() as i64) as usize;
    chars.into_iter().skip(start).take(end - start).collect()
}

pub fn string_insert(input: &str, insert: &str, position: i64) -> String {
    let chars: Vec<char> = input.chars().collect();
    let idx = if position <= 0 {
        0
    } else if position as usize >= chars.len() {
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
    let start = if position <= 1 {
        0
    } else {
        position as usize - 1
    };
    if start >= chars.len() {
        return input.to_string();
    }
    let end = (start as i64 + length).min(chars.len() as i64) as usize;
    let mut result = String::new();
    result.extend(chars.iter().take(start));
    result.extend(chars.iter().skip(end));
    result
}

pub fn string_replace(input: &str, repl: &str, length: i64, position: i64) -> String {
    let chars: Vec<char> = input.chars().collect();
    let start = if position <= 1 {
        0
    } else {
        position as usize - 1
    };
    if start >= chars.len() {
        return input.to_string();
    }
    let end = if length <= 0 {
        start
    } else {
        (start as i64 + length).min(chars.len() as i64) as usize
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
            return Ok(ch);
        }
        Value::WString(text) => {
            let mut chars = text.chars();
            let ch = chars.next().ok_or(RuntimeError::TypeMismatch)?;
            if chars.next().is_some() {
                return Err(RuntimeError::TypeMismatch);
            }
            return Ok(ch);
        }
        _ => return Err(RuntimeError::TypeMismatch),
    };
    if !wide && code > u32::from(u8::MAX) {
        return Err(RuntimeError::Overflow);
    }
    core::char::from_u32(code).ok_or(RuntimeError::TypeMismatch)
}

#[cfg(test)]
mod tests {
    use super::{
        read_string_element, string_element_count, string_find, string_left, string_mid,
        write_string_element,
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
        assert_eq!(
            read_string_element("🙂", 1, false),
            Err(RuntimeError::Overflow)
        );
    }

    #[test]
    fn wide_string_index_reads_and_writes_unicode_scalar_elements() {
        assert_eq!(read_string_element("ÄB", 1, true), Ok(Value::WChar(0x00C4)));
        assert_eq!(
            write_string_element("ÄB", 2, Value::WChar('Ω' as u16), true),
            Ok(String::from("ÄΩ"))
        );
    }
}
