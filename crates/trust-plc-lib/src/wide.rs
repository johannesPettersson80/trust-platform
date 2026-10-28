//! Wide texts (IEC §6.6.2.5.11, Table 34; specification 02 §6 and 07 §2.5, §8): a `WSTRING` holds
//! UTF-16 code units, and each function here does what its `STRING` counterpart does, with code
//! units in place of bytes. A text longer than a variable's capacity is cut there at the last whole
//! character, so that a surrogate pair is never split; a string function's result is at most 65,535
//! code units; `LEN`, indexing and the positions and lengths of the string functions count
//! characters, a surrogate pair one (DEV-017); and two texts compare by the Unicode scalar values of
//! their characters, which is the order of their UTF-8 bytes, so the order of the same texts as
//! `STRING`s.
//!
//! The specifications are silent on half of a surrogate pair, which no text of the runtime holds and
//! a `WCHAR_TO_WSTRING` of such a code makes: a lone surrogate is one character of its own, of its
//! code unit's value, and becomes U+FFFD in a `STRING`. A high surrogate followed by a low one is one
//! character wherever they meet, also where two parts of a result join. Bytes of a `STRING` that are
//! no UTF-8 become U+FFFD in a `WSTRING`.

use core::cell::Cell;
use core::cmp::Ordering;

use trust_plc_abi::module::fault;
use trust_plc_abi::{Cx, Fault, FaultCode, StrView, WStrView};

use crate::text::{starts_character, MAX_TEXT};
use crate::text_functions::{element, first, int, span};

/// One part of a `CONCAT` of `WSTRING`s: a literal's code units or a string.
#[derive(Clone, Copy)]
pub enum WPiece<'a> {
    /// The UTF-16 code units of a literal.
    Units(&'a [u16]),
    /// A string's current value.
    WStr(WStrView<'a>),
    /// The code units of a string from the first to before the second offset, which lie at
    /// character boundaries.
    Range(WStrView<'a>, usize, usize),
}

impl WPiece<'_> {
    fn len(&self) -> usize {
        match self {
            WPiece::Units(units) => units.len(),
            WPiece::WStr(view) => view.len(),
            WPiece::Range(_, start, end) => end.saturating_sub(*start),
        }
    }

    fn unit(&self, index: usize) -> u16 {
        match self {
            WPiece::Units(units) => units.get(index).copied().unwrap_or(0),
            WPiece::WStr(view) => unit_at(*view, index),
            WPiece::Range(view, start, _) => unit_at(*view, start.saturating_add(index)),
        }
    }
}

/// The code unit at 0-based `index` of `text`, or 0 past its length.
fn unit_at(text: WStrView<'_>, index: usize) -> u16 {
    if index < text.len() {
        text.data.get(index).map_or(0, Cell::get)
    } else {
        0
    }
}

/// Whether `unit` is a high surrogate, the first half of a pair.
fn is_high(unit: u16) -> bool {
    (0xD800..=0xDBFF).contains(&unit)
}

/// Whether `unit` is a low surrogate, the second half of a pair.
fn is_low(unit: u16) -> bool {
    (0xDC00..=0xDFFF).contains(&unit)
}

/// Whether a character begins at unit `index` of `text`: at every unit but a low surrogate that
/// completes the high surrogate before it, and at the end.
fn boundary(text: WStrView<'_>, index: usize) -> bool {
    let second_half = is_low(unit_at(text, index))
        && index
            .checked_sub(1)
            .is_some_and(|before| is_high(unit_at(text, before)));
    !second_half
}

/// The characters of `text`.
fn characters(text: WStrView<'_>) -> usize {
    (0..text.len())
        .filter(|&index| boundary(text, index))
        .count()
}

/// The code unit offset of the character at 0-based `index` of `text`, or its length when `index`
/// is at or past the end.
fn offset(text: WStrView<'_>, index: usize) -> usize {
    let length = text.len();
    (0..length)
        .filter(|&at| boundary(text, at))
        .nth(index)
        .unwrap_or(length)
}

/// The code point of the character that begins at unit `at` of `text`, and its code units: a high
/// surrogate followed by a low one is the supplementary character of the two; any other unit, a
/// lone surrogate among them, is the character of its own value.
fn decode(text: WStrView<'_>, at: usize) -> (u32, usize) {
    let lead = unit_at(text, at);
    // Past the length the unit is 0, which is no low surrogate.
    let trail = unit_at(text, at + 1);
    if is_high(lead) && is_low(trail) {
        let code = 0x1_0000 + ((u32::from(lead) - 0xD800) << 10) + (u32::from(trail) - 0xDC00);
        (code, 2)
    } else {
        (u32::from(lead), 1)
    }
}

/// The code units of `parts` one after the other.
fn joined(parts: &[WPiece<'_>]) -> impl Iterator<Item = u16> {
    parts
        .iter()
        .flat_map(|part| (0..part.len()).map(move |index| part.unit(index)))
}

/// Writes `parts` one after the other into `dst` up to the last whole character that fits its
/// capacity, and sets its length.
fn write_parts(dst: WStrView<'_>, parts: &[WPiece<'_>]) {
    let total: usize = parts.iter().map(WPiece::len).sum();
    let mut end = total.min(dst.capacity());
    if end > 0 && end < total {
        // A cut between the two halves of a surrogate pair keeps neither.
        let mut units = joined(parts);
        if units.nth(end - 1).is_some_and(is_high) && units.next().is_some_and(is_low) {
            end -= 1;
        }
    }
    for (cell, unit) in dst.data.iter().zip(joined(parts)).take(end) {
        cell.set(unit);
    }
    // The capacity is at most 65,535 (specification 44 §7.6).
    dst.len.set(u16::try_from(end).unwrap_or(u16::MAX));
}

/// `dst := "literal"`: the literal's code units, cut at `dst`'s capacity.
pub fn assign_units(dst: WStrView<'_>, units: &[u16]) {
    write_parts(dst, &[WPiece::Units(units)]);
}

/// `dst := src`, cut at `dst`'s capacity. `dst` and `src` may be the same string.
pub fn assign_wide(dst: WStrView<'_>, src: WStrView<'_>) {
    write_parts(dst, &[WPiece::WStr(src)]);
}

/// `dst := CONCAT(parts…)`: the parts one after the other, cut at `dst`'s capacity. `dst` must not
/// be one of the parts; generated code concatenates into a temporary when it may be.
///
/// # Errors
///
/// `StringCapacityExceeded` for a result longer than 65,535 code units, before anything is written.
pub fn concat_wide(cx: &mut Cx, dst: WStrView<'_>, parts: &[WPiece<'_>]) -> Result<(), Fault> {
    result(cx, dst, parts)
}

/// `LEN(text)`: its characters, a surrogate pair one (DEV-017).
#[must_use]
pub fn len_wide(text: WStrView<'_>) -> usize {
    characters(text)
}

/// The order of two strings by the Unicode scalar values of their characters, a prefix first: `=`,
/// `<`, `>` of `WSTRING`s. A character above U+FFFF comes after every character below it, although
/// its first code unit, a high surrogate, is below those from U+E000.
#[must_use]
pub fn compare_wide(a: WStrView<'_>, b: WStrView<'_>) -> Ordering {
    // Two equal characters have the same code units, so one offset walks both texts.
    let mut at = 0;
    while at < a.len() && at < b.len() {
        let (code, units) = decode(a, at);
        match code.cmp(&decode(b, at).0) {
            Ordering::Equal => at += units,
            unequal => return unequal,
        }
    }
    a.len().cmp(&b.len())
}

/// Writes `parts` into `dst` as a string function's result.
///
/// # Errors
///
/// `StringCapacityExceeded` for a result longer than `MAX_TEXT` code units, before anything is
/// written.
fn result(cx: &mut Cx, dst: WStrView<'_>, parts: &[WPiece<'_>]) -> Result<(), Fault> {
    let total: usize = parts.iter().map(WPiece::len).sum();
    if total > MAX_TEXT {
        return Err(fault(
            cx,
            FaultCode::StringCapacityExceeded,
            i64::try_from(total).unwrap_or(i64::MAX),
            0,
            0,
        ));
    }
    write_parts(dst, parts);
    Ok(())
}

/// `LEN(IN)`: the characters of `text`, a surrogate pair one.
///
/// # Errors
///
/// `Overflow` for more than 32,767 characters.
pub fn len_wide_int(cx: &mut Cx, text: WStrView<'_>) -> Result<i16, Fault> {
    int(cx, characters(text))
}

/// `LEFT(IN, L)`: the first `count` characters of `text`, into `dst`.
///
/// # Errors
///
/// None the inputs can cause; the result is never longer than `text`.
pub fn left_wide(
    cx: &mut Cx,
    dst: WStrView<'_>,
    text: WStrView<'_>,
    count: i64,
) -> Result<(), Fault> {
    let end = offset(text, span(count));
    result(cx, dst, &[WPiece::Range(text, 0, end)])
}

/// `RIGHT(IN, L)`: the last `count` characters of `text`, into `dst`.
///
/// # Errors
///
/// None the inputs can cause; the result is never longer than `text`.
pub fn right_wide(
    cx: &mut Cx,
    dst: WStrView<'_>,
    text: WStrView<'_>,
    count: i64,
) -> Result<(), Fault> {
    let length = characters(text);
    let start = length - span(count).min(length);
    result(
        cx,
        dst,
        &[WPiece::Range(text, offset(text, start), text.len())],
    )
}

/// `MID(IN, L, P)`: the `length` characters of `text` from 1-based `position`, into `dst`.
///
/// # Errors
///
/// None the inputs can cause; the result is never longer than `text`.
pub fn mid_wide(
    cx: &mut Cx,
    dst: WStrView<'_>,
    text: WStrView<'_>,
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
        &[WPiece::Range(text, offset(text, start), offset(text, end))],
    )
}

/// `INSERT(IN1, IN2, P)`: `inserted` placed after the `position`-th character of `text`, into
/// `dst`.
///
/// # Errors
///
/// `StringCapacityExceeded` for a result longer than 65,535 code units.
pub fn insert_wide(
    cx: &mut Cx,
    dst: WStrView<'_>,
    text: WStrView<'_>,
    inserted: WStrView<'_>,
    position: i64,
) -> Result<(), Fault> {
    let at = offset(text, span(position));
    result(
        cx,
        dst,
        &[
            WPiece::Range(text, 0, at),
            WPiece::WStr(inserted),
            WPiece::Range(text, at, text.len()),
        ],
    )
}

/// `DELETE(IN, L, P)`: `text` without the `length` characters from 1-based `position`, into `dst`.
///
/// # Errors
///
/// None the inputs can cause; the result is never longer than `text`.
pub fn delete_wide(
    cx: &mut Cx,
    dst: WStrView<'_>,
    text: WStrView<'_>,
    length: i64,
    position: i64,
) -> Result<(), Fault> {
    let count = characters(text);
    let Some(start) = first(position, count).filter(|_| length > 0) else {
        return result(cx, dst, &[WPiece::WStr(text)]);
    };
    let end = start.saturating_add(span(length)).min(count);
    result(
        cx,
        dst,
        &[
            WPiece::Range(text, 0, offset(text, start)),
            WPiece::Range(text, offset(text, end), text.len()),
        ],
    )
}

/// `REPLACE(IN1, IN2, L, P)`: `text` with the `length` characters from 1-based `position` replaced
/// by `replacement`, into `dst`; a length below one inserts it at the position.
///
/// # Errors
///
/// `StringCapacityExceeded` for a result longer than 65,535 code units.
pub fn replace_wide(
    cx: &mut Cx,
    dst: WStrView<'_>,
    text: WStrView<'_>,
    replacement: WStrView<'_>,
    length: i64,
    position: i64,
) -> Result<(), Fault> {
    let count = characters(text);
    let Some(start) = first(position, count) else {
        return result(cx, dst, &[WPiece::WStr(text)]);
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
            WPiece::Range(text, 0, offset(text, start)),
            WPiece::WStr(replacement),
            WPiece::Range(text, offset(text, end), text.len()),
        ],
    )
}

/// `FIND(IN1, IN2)`: the 1-based position of the first character of the first occurrence of
/// `wanted` in `text`, 0 when there is none; an empty `wanted` is found at 1. An occurrence begins
/// and ends at characters of `text`, so that half of a surrogate pair never matches in a pair.
///
/// # Errors
///
/// `Overflow` for a position above 32,767.
pub fn find_wide(cx: &mut Cx, text: WStrView<'_>, wanted: WStrView<'_>) -> Result<i16, Fault> {
    let (length, needle) = (text.len(), wanted.len());
    let found = (needle <= length)
        .then(|| {
            (0..=length - needle).find(|&at| {
                boundary(text, at)
                    && boundary(text, at + needle)
                    && (0..needle).all(|index| unit_at(text, at + index) == unit_at(wanted, index))
            })
        })
        .flatten();
    let position = found.map_or(0, |at| {
        (0..at).filter(|&index| boundary(text, index)).count() + 1
    });
    int(cx, position)
}

/// `text[index]`: the character at 1-based `index` as a `WCHAR` (specification 02 §6); a lone
/// surrogate reads as its code unit.
///
/// # Errors
///
/// `IndexOutOfBounds` for an index outside 1 to the characters of `text`, and `Overflow` for a
/// character above U+FFFF, which no `WCHAR` holds.
pub fn char_at_wide(cx: &mut Cx, text: WStrView<'_>, index: i64) -> Result<u16, Fault> {
    let position = element(cx, index, characters(text))?;
    let (code, _) = decode(text, offset(text, position));
    u16::try_from(code).map_err(|_| fault(cx, FaultCode::Overflow, i64::from(code), 0, 0))
}

/// `text[index] := character`: the character at 1-based `index` replaced by the one code unit
/// `character` (specification 02 §6). A surrogate pair so replaced leaves one code unit, and the
/// rest of the text moves back by one; the text never grows, so it is never cut.
///
/// # Errors
///
/// `IndexOutOfBounds` for an index outside 1 to the characters of `text`, and `TypeMismatch` for a
/// `character` that is half of a surrogate pair, which is no character, as the runtime refuses it;
/// the text is unchanged.
pub fn set_char_wide(
    cx: &mut Cx,
    text: WStrView<'_>,
    index: i64,
    character: u16,
) -> Result<(), Fault> {
    let position = element(cx, index, characters(text))?;
    if is_high(character) || is_low(character) {
        return Err(fault(
            cx,
            FaultCode::TypeMismatch,
            i64::from(character),
            0,
            0,
        ));
    }
    let length = text.len();
    let at = offset(text, position);
    let (_, old) = decode(text, at);
    if old == 2 {
        // The units after the pair move back by one, each read before it is overwritten.
        for to in at + 1..length - 1 {
            if let Some(cell) = text.data.get(to) {
                cell.set(unit_at(text, to + 1));
            }
        }
    }
    if let Some(cell) = text.data.get(at) {
        cell.set(character);
    }
    // A pair gives up one code unit; the capacity is at most 65,535 (specification 44 §7.6).
    let kept = length + 1 - old;
    text.len.set(u16::try_from(kept).unwrap_or(u16::MAX));
    Ok(())
}

/// The character of the `STRING` `src` that begins at byte `at`, and its bytes: a byte that is no
/// continuation byte and the continuation bytes after it, as the `STRING` functions count a
/// character, read as UTF-8; U+FFFD for bytes that encode no character, a continuation byte at the
/// start of the text among them.
fn narrow_character(src: StrView<'_>, at: usize) -> (char, usize) {
    let mut end = at + 1;
    while end < src.len() && !starts_character(src.byte(end)) {
        end += 1;
    }
    let lead = src.byte(at);
    // The bytes of the character `lead` begins, the bits it carries, and the smallest code point
    // of that many bytes, below which an encoding is overlong.
    let (bytes, bits, smallest) = match lead {
        0x00..=0x7F => (1, u32::from(lead), 0),
        0xC0..=0xDF => (2, u32::from(lead & 0x1F), 0x80),
        0xE0..=0xEF => (3, u32::from(lead & 0x0F), 0x800),
        0xF0..=0xF7 => (4, u32::from(lead & 0x07), 0x1_0000),
        _ => (0, 0, 0),
    };
    let character = if end - at == bytes {
        let code = (at + 1..end).fold(bits, |code, index| {
            (code << 6) | u32::from(src.byte(index) & 0x3F)
        });
        char::from_u32(code).filter(|_| code >= smallest)
    } else {
        None
    };
    (character.unwrap_or(char::REPLACEMENT_CHARACTER), end - at)
}

/// `STRING_TO_WSTRING(src)`: the characters of `src` in UTF-16 into `dst`, cut at its capacity at
/// the last whole character; bytes that are no UTF-8 become U+FFFD. A text of at most 65,535 bytes
/// never needs more code units, so the result is never too long.
pub fn string_to_wstring(dst: WStrView<'_>, src: StrView<'_>) {
    let (mut read, mut written) = (0, 0);
    while read < src.len() {
        let (character, bytes) = narrow_character(src, read);
        let mut buffer = [0u16; 2];
        let units: &[u16] = character.encode_utf16(&mut buffer);
        if written + units.len() > dst.capacity() {
            break;
        }
        for (cell, unit) in dst.data.iter().skip(written).zip(units) {
            cell.set(*unit);
        }
        written += units.len();
        read += bytes;
    }
    // The capacity is at most 65,535 (specification 44 §7.6).
    dst.len.set(u16::try_from(written).unwrap_or(u16::MAX));
}

/// `WSTRING_TO_STRING(src)`: the characters of `src` in UTF-8 into `dst`, cut at its capacity at
/// the last whole character; a lone surrogate becomes U+FFFD.
///
/// # Errors
///
/// `StringCapacityExceeded` for a text of more than [`MAX_TEXT`] bytes of UTF-8, before anything
/// is written (specification 07 §2.8, §8.1).
pub fn wstring_to_string(cx: &mut Cx, dst: StrView<'_>, src: WStrView<'_>) -> Result<(), Fault> {
    let mut total = 0;
    let mut read = 0;
    while read < src.len() {
        let (code, units) = decode(src, read);
        total += char::from_u32(code).map_or(3, char::len_utf8);
        read += units;
    }
    if total > MAX_TEXT {
        return Err(fault(
            cx,
            FaultCode::StringCapacityExceeded,
            i64::try_from(total).unwrap_or(i64::MAX),
            0,
            0,
        ));
    }
    let (mut read, mut written) = (0, 0);
    while read < src.len() {
        let (code, units) = decode(src, read);
        let character = char::from_u32(code).unwrap_or(char::REPLACEMENT_CHARACTER);
        let mut buffer = [0u8; 4];
        let bytes = character.encode_utf8(&mut buffer).as_bytes();
        if written + bytes.len() > dst.capacity() {
            break;
        }
        for (cell, byte) in dst.data.iter().skip(written).zip(bytes) {
            cell.set(*byte);
        }
        written += bytes.len();
        read += units;
    }
    // The capacity is at most 65,535 (specification 44 §7.6).
    dst.len.set(u16::try_from(written).unwrap_or(u16::MAX));
    Ok(())
}

/// `WCHAR_TO_WSTRING(code)`: the one character `code` into `dst`.
///
/// # Errors
///
/// `TypeMismatch` with the code for half of a surrogate pair, which is no character (DEV-017).
pub fn assign_wchar(cx: &mut Cx, dst: WStrView<'_>, code: u16) -> Result<(), Fault> {
    if (0xD800..=0xDFFF).contains(&code) {
        return Err(fault(cx, FaultCode::TypeMismatch, i64::from(code), 0, 0));
    }
    write_parts(dst, &[WPiece::Units(&[code])]);
    Ok(())
}
