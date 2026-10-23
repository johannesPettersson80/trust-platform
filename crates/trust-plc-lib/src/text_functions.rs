//! The string functions of IEC §6.6.2.5.11, Table 34 (specification 07 §8), and the characters of
//! a `STRING` (specification 02 §6), with the runtime's rules: positions and lengths count
//! characters, not bytes (DEV-017); a count or a length below one is none; a position below one is
//! the first character and one past the end leaves the text as it is; a result above 65,535 bytes is
//! refused. A result goes into a destination the generated code made large enough for every result
//! its inputs can give, so that only the assignment that follows cuts it.

use trust_plc_abi::module::fault;
use trust_plc_abi::{Cx, Fault, FaultCode, StrView};

use crate::text::{assign_bytes, starts_character, write_parts, Piece, MAX_TEXT};

/// The characters of `text`.
fn characters(text: StrView<'_>) -> usize {
    (0..text.len())
        .filter(|&index| starts_character(text.byte(index)))
        .count()
}

/// The byte offset of the character at 0-based `index` of `text`, or its length when `index` is at
/// or past the end.
fn offset(text: StrView<'_>, index: usize) -> usize {
    let mut seen = 0;
    for at in 0..text.len() {
        if starts_character(text.byte(at)) {
            if seen == index {
                return at;
            }
            seen += 1;
        }
    }
    text.len()
}

/// The characters a count or a length argument stands for: none below one.
fn span(count: i64) -> usize {
    if count <= 0 {
        0
    } else {
        usize::try_from(count).unwrap_or(usize::MAX)
    }
}

/// The 0-based index of the character at 1-based `position` of a text of `count` characters, a
/// position below one the first character; `None` past the end.
fn first(position: i64, count: usize) -> Option<usize> {
    let start = if position <= 1 { 0 } else { position - 1 };
    usize::try_from(start).ok().filter(|start| *start < count)
}

/// Writes `parts` into `dst` as a string function's result.
///
/// # Errors
///
/// `StringCapacityExceeded` for a result longer than [`MAX_TEXT`] bytes, before anything is written.
fn result(cx: &mut Cx, dst: StrView<'_>, parts: &[Piece<'_>]) -> Result<(), Fault> {
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

/// A count of characters as an `INT`, the type of `LEN` and `FIND`.
///
/// # Errors
///
/// `Overflow` for a count above 32,767.
fn int(cx: &mut Cx, count: usize) -> Result<i16, Fault> {
    i16::try_from(count).map_err(|_| {
        fault(
            cx,
            FaultCode::Overflow,
            i64::try_from(count).unwrap_or(i64::MAX),
            0,
            0,
        )
    })
}

/// `LEN(IN)`: the characters of `text`.
///
/// # Errors
///
/// `Overflow` for more than 32,767 characters.
pub fn len_int(cx: &mut Cx, text: StrView<'_>) -> Result<i16, Fault> {
    int(cx, characters(text))
}

/// `LEFT(IN, L)`: the first `count` characters of `text`, into `dst`.
///
/// # Errors
///
/// None the inputs can cause; the result is never longer than `text`.
pub fn left(cx: &mut Cx, dst: StrView<'_>, text: StrView<'_>, count: i64) -> Result<(), Fault> {
    let end = offset(text, span(count));
    result(cx, dst, &[Piece::Range(text, 0, end)])
}

/// `RIGHT(IN, L)`: the last `count` characters of `text`, into `dst`.
///
/// # Errors
///
/// None the inputs can cause; the result is never longer than `text`.
pub fn right(cx: &mut Cx, dst: StrView<'_>, text: StrView<'_>, count: i64) -> Result<(), Fault> {
    let length = characters(text);
    let start = length - span(count).min(length);
    result(
        cx,
        dst,
        &[Piece::Range(text, offset(text, start), text.len())],
    )
}

/// `MID(IN, L, P)`: the `length` characters of `text` from 1-based `position`, into `dst`.
///
/// # Errors
///
/// None the inputs can cause; the result is never longer than `text`.
pub fn mid(
    cx: &mut Cx,
    dst: StrView<'_>,
    text: StrView<'_>,
    length: i64,
    position: i64,
) -> Result<(), Fault> {
    let count = characters(text);
    let Some(start) = first(position, count).filter(|_| length > 0) else {
        return result(cx, dst, &[]);
    };
    let end = start.saturating_add(span(length)).min(count);
    result(
        cx,
        dst,
        &[Piece::Range(text, offset(text, start), offset(text, end))],
    )
}

/// `INSERT(IN1, IN2, P)`: `inserted` placed after the `position`-th character of `text`, into
/// `dst`.
///
/// # Errors
///
/// `StringCapacityExceeded` for a result longer than 65,535 bytes.
pub fn insert(
    cx: &mut Cx,
    dst: StrView<'_>,
    text: StrView<'_>,
    inserted: StrView<'_>,
    position: i64,
) -> Result<(), Fault> {
    let at = offset(text, span(position));
    result(
        cx,
        dst,
        &[
            Piece::Range(text, 0, at),
            Piece::Str(inserted),
            Piece::Range(text, at, text.len()),
        ],
    )
}

/// `DELETE(IN, L, P)`: `text` without the `length` characters from 1-based `position`, into `dst`.
///
/// # Errors
///
/// None the inputs can cause; the result is never longer than `text`.
pub fn delete(
    cx: &mut Cx,
    dst: StrView<'_>,
    text: StrView<'_>,
    length: i64,
    position: i64,
) -> Result<(), Fault> {
    let count = characters(text);
    let Some(start) = first(position, count).filter(|_| length > 0) else {
        return result(cx, dst, &[Piece::Str(text)]);
    };
    let end = start.saturating_add(span(length)).min(count);
    result(
        cx,
        dst,
        &[
            Piece::Range(text, 0, offset(text, start)),
            Piece::Range(text, offset(text, end), text.len()),
        ],
    )
}

/// `REPLACE(IN1, IN2, L, P)`: `text` with the `length` characters from 1-based `position` replaced
/// by `replacement`, into `dst`; a length below one inserts it at the position.
///
/// # Errors
///
/// `StringCapacityExceeded` for a result longer than 65,535 bytes.
pub fn replace(
    cx: &mut Cx,
    dst: StrView<'_>,
    text: StrView<'_>,
    replacement: StrView<'_>,
    length: i64,
    position: i64,
) -> Result<(), Fault> {
    let count = characters(text);
    let Some(start) = first(position, count) else {
        return result(cx, dst, &[Piece::Str(text)]);
    };
    let end = if length <= 0 {
        start
    } else {
        start.saturating_add(span(length)).min(count)
    };
    result(
        cx,
        dst,
        &[
            Piece::Range(text, 0, offset(text, start)),
            Piece::Str(replacement),
            Piece::Range(text, offset(text, end), text.len()),
        ],
    )
}

/// `FIND(IN1, IN2)`: the 1-based position of the first character of the first occurrence of
/// `wanted` in `text`, 0 when there is none; an empty `wanted` is found at 1.
///
/// # Errors
///
/// `Overflow` for a position above 32,767.
pub fn find(cx: &mut Cx, text: StrView<'_>, wanted: StrView<'_>) -> Result<i16, Fault> {
    let (length, needle) = (text.len(), wanted.len());
    // A valid UTF-8 needle begins with no continuation byte, so a byte match starts at a character.
    let found = (needle <= length)
        .then(|| {
            (0..=length - needle)
                .find(|&at| (0..needle).all(|index| text.byte(at + index) == wanted.byte(index)))
        })
        .flatten();
    let position = found.map_or(0, |at| {
        (0..at)
            .filter(|&index| starts_character(text.byte(index)))
            .count()
            + 1
    });
    int(cx, position)
}

/// The 0-based index of the character that 1-based `index` selects in a text of `count`
/// characters.
///
/// # Errors
///
/// `IndexOutOfBounds` with the index and the valid range, 1 to the count, or to `i64::MAX` for an
/// index below one, as the runtime names them.
fn element(cx: &mut Cx, index: i64, count: usize) -> Result<usize, Fault> {
    if index < 1 {
        return Err(fault(cx, FaultCode::IndexOutOfBounds, index, 1, i64::MAX));
    }
    let upper = i64::try_from(count).unwrap_or(i64::MAX);
    if index > upper {
        return Err(fault(cx, FaultCode::IndexOutOfBounds, index, 1, upper));
    }
    Ok(usize::try_from(index - 1).unwrap_or(usize::MAX))
}

/// The code point of the UTF-8 character that begins at byte `at` of `text`.
fn decode(text: StrView<'_>, at: usize) -> u32 {
    let lead = text.byte(at);
    let (mut code, continuation) = match lead {
        0x00..=0x7F => (u32::from(lead), 0),
        0xC0..=0xDF => (u32::from(lead & 0x1F), 1),
        0xE0..=0xEF => (u32::from(lead & 0x0F), 2),
        _ => (u32::from(lead & 0x07), 3),
    };
    for index in 1..=continuation {
        code = (code << 6) | u32::from(text.byte(at + index) & 0x3F);
    }
    code
}

/// `text[index]`: the character at 1-based `index` as a `CHAR` (specification 02 §6).
///
/// # Errors
///
/// `IndexOutOfBounds` for an index outside 1 to the characters of `text`, and `Overflow` for a
/// character above 255, which no `CHAR` holds.
pub fn char_at(cx: &mut Cx, text: StrView<'_>, index: i64) -> Result<u8, Fault> {
    let position = element(cx, index, characters(text))?;
    let code = decode(text, offset(text, position));
    u8::try_from(code).map_err(|_| fault(cx, FaultCode::Overflow, i64::from(code), 0, 0))
}

/// `text[index] := character`: the character at 1-based `index` replaced by `character`, U+0000 to
/// U+00FF, and the text cut at its capacity at the last whole character when the replacement made it
/// longer (specification 02 §6; `docs/IEC_DECISIONS.md` 2026-10-17).
///
/// # Errors
///
/// `IndexOutOfBounds` for an index outside 1 to the characters of `text`; the text is unchanged.
pub fn set_char(cx: &mut Cx, text: StrView<'_>, index: i64, character: u8) -> Result<(), Fault> {
    let position = element(cx, index, characters(text))?;
    let at = offset(text, position);
    let old = offset(text, position + 1) - at;
    let (encoded, new): ([u8; 2], usize) = if character < 0x80 {
        ([character, 0], 1)
    } else {
        ([0xC0 | (character >> 6), 0x80 | (character & 0x3F)], 2)
    };
    let length = text.len();
    let total = length - old + new;
    // The byte at `index` of the replaced text, read from the text as it is.
    let replaced = |index: usize| -> u8 {
        if index < at {
            text.byte(index)
        } else if index < at + new {
            encoded.get(index - at).copied().unwrap_or(0)
        } else {
            text.byte(index - new + old)
        }
    };
    let mut end = total.min(text.capacity());
    while end > 0 && end < total && !starts_character(replaced(end)) {
        end -= 1;
    }
    let tail = at + new;
    if new > old {
        // The tail moves right: copy from its end, so that no byte is overwritten before it is read.
        for index in (tail..end).rev() {
            if let Some(cell) = text.data.get(index) {
                cell.set(text.byte(index - new + old));
            }
        }
    } else if new < old {
        for index in tail..end {
            if let Some(cell) = text.data.get(index) {
                cell.set(text.byte(index - new + old));
            }
        }
    }
    for (index, byte) in encoded.iter().take(new).enumerate() {
        if at + index < end
            && let Some(cell) = text.data.get(at + index)
        {
            cell.set(*byte);
        }
    }
    // The capacity is at most 65,535 (specification 44 §7.6).
    text.len.set(u16::try_from(end).unwrap_or(u16::MAX));
    Ok(())
}

/// `STRING_TO_CHAR(text)`: the one character of `text` as a `CHAR`.
///
/// # Errors
///
/// `TypeMismatch` for a text of no character or of more than one, and `Overflow` for a character
/// above 255, which no `CHAR` holds.
pub fn text_to_char(cx: &mut Cx, text: StrView<'_>) -> Result<u8, Fault> {
    if characters(text) != 1 {
        return Err(fault(cx, FaultCode::TypeMismatch, 0, 0, 0));
    }
    let code = decode(text, 0);
    u8::try_from(code).map_err(|_| fault(cx, FaultCode::Overflow, i64::from(code), 0, 0))
}

/// `CHAR_TO_STRING(code)`: the character of `code`, U+0000 to U+00FF, in UTF-8 into `dst`.
pub fn write_char(dst: StrView<'_>, code: u8) {
    let (encoded, used): ([u8; 2], usize) = if code < 0x80 {
        ([code, 0], 1)
    } else {
        ([0xC0 | (code >> 6), 0x80 | (code & 0x3F)], 2)
    };
    assign_bytes(dst, encoded.get(..used).unwrap_or(&[]));
}

/// `WCHAR_TO_STRING(code)`: the character of `code` in UTF-8 into `dst`.
///
/// # Errors
///
/// `TypeMismatch` for a code of half a surrogate pair, which is no character.
pub fn write_wchar(cx: &mut Cx, dst: StrView<'_>, code: u16) -> Result<(), Fault> {
    let Some(character) = char::from_u32(u32::from(code)) else {
        return Err(fault(cx, FaultCode::TypeMismatch, i64::from(code), 0, 0));
    };
    let mut encoded = [0u8; 4];
    assign_bytes(dst, character.encode_utf8(&mut encoded).as_bytes());
    Ok(())
}
