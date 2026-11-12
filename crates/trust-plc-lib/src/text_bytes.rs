//! A string's bytes by their offset under the CODESYS profile (`docs/IEC_DEVIATIONS.md` DEV-046):
//! `s[i]` is the `BYTE` at offset `i`, counted from 0, as CODESYS reads and writes it. CODESYS lays a
//! `STRING(n)` out in n + 1 bytes, its text and a 0. A `STRING[n]` of truST holds n bytes, so the
//! byte at offset n, which CODESYS holds as the 0 after a full text, reads 0, and a 0 written there
//! changes nothing.

use trust_plc_abi::module::fault;
use trust_plc_abi::{Cx, Fault, FaultCode, StrView};

/// `text[index]` under the CODESYS profile: the byte at offset `index`, counted from 0, whatever the
/// text's length, as CODESYS reads the string's memory; 0 at the capacity, where CODESYS holds the 0
/// after a full text.
///
/// # Errors
///
/// `IndexOutOfBounds` with the index and the bounds 0 to the capacity, for an index outside them.
#[inline]
pub fn text_byte(cx: &mut Cx, text: StrView<'_>, index: i64) -> Result<u8, Fault> {
    let capacity = text.capacity();
    let at = usize::try_from(index).ok();
    match at.and_then(|at| text.data.get(at)) {
        Some(byte) => Ok(byte.get()),
        None if at == Some(capacity) => Ok(0),
        None => Err(fault(
            cx,
            FaultCode::IndexOutOfBounds,
            index,
            0,
            i64::try_from(capacity).unwrap_or(i64::MAX),
        )),
    }
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
    let capacity = text.capacity();
    let at = usize::try_from(index).ok();
    if let Some((at, cell)) = at.and_then(|at| text.data.get(at).map(|cell| (at, cell))) {
        cell.set(byte);
        text.bytes_written(at, at + 1);
        return Ok(());
    }
    if byte == 0 && at == Some(capacity) {
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
