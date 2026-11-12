//! A string's bytes, and a `WSTRING`'s code units, by their offset under the CODESYS profile
//! (`docs/IEC_DEVIATIONS.md` DEV-046): `s[i]` is the `BYTE` at offset `i`, counted from 0, and
//! `ws[i]` the `WORD` there, as CODESYS reads and writes them. CODESYS lays a `STRING(n)` out in
//! n + 1 bytes, its text and a 0, and a `WSTRING(n)` in n + 1 code units. A `STRING[n]` of truST
//! holds n bytes and a `WSTRING[n]` n units, so the element at offset n, which CODESYS holds as the
//! 0 after a full text, reads 0, and a 0 written there changes nothing.

use core::cell::Cell;

use trust_plc_abi::module::fault;
use trust_plc_abi::{Cx, Fault, FaultCode, StrView, WStrView};

/// `text[index]` under the CODESYS profile: the byte at offset `index`, counted from 0, whatever the
/// text's length, as CODESYS reads the string's memory; 0 at the capacity, where CODESYS holds the 0
/// after a full text.
///
/// # Errors
///
/// `IndexOutOfBounds` with the index and the bounds 0 to the capacity, for an index outside them.
#[inline]
pub fn text_byte(cx: &mut Cx, text: StrView<'_>, index: i64) -> Result<u8, Fault> {
    element(cx, text.data, index)
}

/// `text[index] := byte` under the CODESYS profile: writes the byte at offset `index`, counted from
/// 0, and sets the length that the bytes give the text, as a write through a pointer to them does
/// ([`StrView::bytes_written`]). A 0 at the capacity, where CODESYS holds the 0 after a full text,
/// changes nothing.
///
/// # Errors
///
/// `IndexOutOfBounds` with the index and the bounds 0 to the capacity - 1, for an index outside
/// them and for a byte other than 0 at the capacity, which a full text has no room for.
#[inline]
pub fn set_text_byte(cx: &mut Cx, text: StrView<'_>, index: i64, byte: u8) -> Result<(), Fault> {
    set_element(cx, text.data, index, byte, |at| {
        text.bytes_written(at, at + 1);
    })
}

/// `text[index]` of a `WSTRING` under the CODESYS profile: the code unit at offset `index`, counted
/// from 0, as a `WORD`, whatever the text's length; 0 at the capacity, where CODESYS holds the 0
/// after a full text (CODESYS help on index access: "The result is of type WORD").
///
/// # Errors
///
/// `IndexOutOfBounds` with the index and the bounds 0 to the capacity, for an index outside them.
#[inline]
pub fn text_word(cx: &mut Cx, text: WStrView<'_>, index: i64) -> Result<u16, Fault> {
    element(cx, text.data, index)
}

/// `text[index] := word` of a `WSTRING` under the CODESYS profile: writes the code unit at offset
/// `index`, counted from 0, and sets the length that the units give the text
/// ([`WStrView::units_written`]). A 0 at the capacity changes nothing.
///
/// # Errors
///
/// `IndexOutOfBounds` with the index and the bounds 0 to the capacity - 1, for an index outside
/// them and for a unit other than 0 at the capacity, which a full text has no room for.
#[inline]
pub fn set_text_word(cx: &mut Cx, text: WStrView<'_>, index: i64, word: u16) -> Result<(), Fault> {
    set_element(cx, text.data, index, word, |at| {
        text.units_written(at, at + 1);
    })
}

/// The element of `data`, a string's bytes or code units, at offset `index`; 0 at the capacity.
#[inline]
fn element<T: Copy + Default>(cx: &mut Cx, data: &[Cell<T>], index: i64) -> Result<T, Fault> {
    let capacity = data.len();
    let at = usize::try_from(index).ok();
    match at.and_then(|at| data.get(at)) {
        Some(element) => Ok(element.get()),
        None if at == Some(capacity) => Ok(T::default()),
        None => Err(fault(
            cx,
            FaultCode::IndexOutOfBounds,
            index,
            0,
            i64::try_from(capacity).unwrap_or(i64::MAX),
        )),
    }
}

/// Writes `value` at offset `index` of `data`, a string's bytes or code units, and gives `written`
/// the offset, to set the length; a 0 at the capacity changes nothing.
#[inline]
fn set_element<T: Copy + Default + PartialEq>(
    cx: &mut Cx,
    data: &[Cell<T>],
    index: i64,
    value: T,
    written: impl FnOnce(usize),
) -> Result<(), Fault> {
    let capacity = data.len();
    let at = usize::try_from(index).ok();
    if let Some((at, cell)) = at.and_then(|at| data.get(at).map(|cell| (at, cell))) {
        cell.set(value);
        written(at);
        return Ok(());
    }
    if value == T::default() && at == Some(capacity) {
        return Ok(());
    }
    Err(fault(
        cx,
        FaultCode::IndexOutOfBounds,
        index,
        0,
        i64::try_from(capacity).unwrap_or(i64::MAX) - 1,
    ))
}
