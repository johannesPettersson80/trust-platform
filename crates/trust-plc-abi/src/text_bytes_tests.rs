//! A string's bytes taken through a pointer under the CODESYS profile (specification 44 §7.3;
//! `docs/IEC_DEVIATIONS.md` DEV-039): the reference that names them, the bytes it reaches, and the
//! length that the bytes written through it give the text, which ends at its first 0 byte.

use core::cell::Cell;

use crate::kernel::WITHOUT_DEBUGGER;
use crate::module::run;
use crate::{plain_struct, Cx, FaultCode, Plain, RefValue, Str, AREA_GLOBALS};

/// A `STRING[8]` of the bytes `bytes`, whose length is `len`.
fn text(bytes: [u8; 8], len: u16) -> Str<8> {
    let text = Str::<8>::new();
    for (cell, byte) in text.data.iter().zip(bytes) {
        cell.set(byte);
    }
    text.len.set(len);
    text
}

/// The bytes of `text`.
fn bytes(text: &Str<8>) -> [u8; 8] {
    let mut bytes = [0; 8];
    for (byte, cell) in bytes.iter_mut().zip(&text.data) {
        *byte = cell.get();
    }
    bytes
}

/// Bytes copied one by one into an empty string, its 0 last, as a CODESYS `MemCopy` of a text and
/// its 0 copies them: each byte other than 0 written over the 0 after the text extends the text by
/// it, and the 0 ends it, so 'Hi' and its 0 give the length 2. Catches a length that stays 0, which
/// reads the string as empty after the copy (#453).
#[test]
fn bytes_copied_into_an_empty_string_become_its_text() {
    let text = text([0; 8], 0);
    for (index, byte) in b"Hi\0".iter().enumerate() {
        text.data[index].set(*byte);
        text.view().bytes_written(index, index + 1);
    }
    assert_eq!(text.len.get(), 2);
    assert_eq!(bytes(&text), *b"Hi\0\0\0\0\0\0");
}

/// A 0 written before the length ends the text there: 'Hello' with a 0 written at index 2 reads
/// 'He'. Catches a length kept at 5, which reads past the 0 where CODESYS stops.
#[test]
fn a_zero_written_inside_the_text_ends_it_there() {
    let text = text(*b"Hello\0\0\0", 5);
    text.data[2].set(0);
    text.view().bytes_written(2, 3);
    assert_eq!(text.len.get(), 2);
}

/// A byte other than 0 written over the 0 after the text extends the text to the next 0, through
/// the bytes an earlier, longer text left, as CODESYS reads them: 'Hi' written over 'Hello' leaves
/// H, i, 0, l, o, 0, and a 'y' over its 0 reads 'Hiylo'. Catches an extension by the written byte
/// alone (length 3), which drops what CODESYS reads after it.
#[test]
fn a_byte_written_over_the_end_extends_the_text_to_the_next_zero() {
    let text = text(*b"Hi\0lo\0\0\0", 2);
    text.data[2].set(b'y');
    text.view().bytes_written(2, 3);
    assert_eq!(text.len.get(), 5);
}

/// A byte written after the 0 that ends the text leaves the text as it was: 'Hi' with a 'z' at
/// index 4 still reads 'Hi'. Catches a length set to the end of the written byte (5).
#[test]
fn a_byte_written_after_the_end_leaves_the_text() {
    let text = text(*b"Hi\0\0\0\0\0\0", 2);
    text.data[4].set(b'z');
    text.view().bytes_written(4, 5);
    assert_eq!(text.len.get(), 2);
}

/// Bytes other than 0 up to the capacity give the capacity as the length: an 'h' over the 0 after
/// 'abcdefg' (7) fills `STRING[8]`, whose text then has no 0 after it. Catches a scan that runs
/// past the capacity.
#[test]
fn bytes_up_to_the_capacity_give_the_capacity() {
    let text = text(*b"abcdefg\0", 7);
    text.data[7].set(b'h');
    text.view().bytes_written(7, 8);
    assert_eq!(text.len.get(), 8);
}

/// Bytes written at once, an array through a pointer to an array of bytes, end the text at the
/// first 0 among them: 1, 2, 0, 4 over the first four bytes of 'Hello' read two bytes. Catches a
/// length taken from the last byte written.
#[test]
fn an_array_written_at_once_ends_the_text_at_its_first_zero() {
    let text = text(*b"Hello\0\0\0", 5);
    for (cell, byte) in text.data.iter().zip([1, 2, 0, 4]) {
        cell.set(byte);
    }
    text.view().bytes_written(0, 4);
    assert_eq!(text.len.get(), 2);
}

plain_struct! {
    struct Texts {
        count: Cell<u16>,
        text: Str<5>,
    }
}

fn context() -> Cx {
    Cx::new(&WITHOUT_DEBUGGER)
}

/// The fault and its numbers of a dereference as `T` of the reference to the bytes of
/// `Texts::text` from its byte `char_index`.
fn deref_outcome<T: Plain>(char_index: u16) -> (u32, [i64; 3]) {
    let mut words = [0u64; 2];
    let mut cx = context();
    cx.set_area(AREA_GLOBALS, words.as_mut_ptr().cast(), 16);
    let result = run(&raw mut cx, |cx, areas| {
        let texts: &Texts = areas.view(cx, AREA_GLOBALS)?;
        let bytes = areas.refer_text_bytes(cx, &texts.text)?;
        let _: &T = areas.deref(
            cx,
            &RefValue {
                char_index,
                ..bytes
            },
        )?;
        Ok(())
    });
    (result, cx.fault.detail)
}

/// A string's bytes are named by a reference to the whole string, its length and its bytes, from
/// the byte `char_index` (specification 44 §7.3): `text`, a `STRING[5]` after a 2-byte `count`,
/// lies at offset 2 and spans 2 + 5 = 7 bytes, without the byte that rounds its size up to 8. A byte
/// or an array of bytes from that byte is reached through it as far as the string's bytes go; an
/// array of 6 from byte 0 (5 bytes there), an array of 2 from byte 4 (1 byte there) and a
/// `STRING[5]` (8 bytes, aligned to 2) are refused with `TypeMismatch`, the bytes there and the
/// bytes asked for in the record. Catches a reference that names the bytes alone, whose length a
/// write cannot find, and an extent that counts the rounding byte as a sixth byte of the text.
#[test]
fn a_reference_to_a_strings_bytes_names_the_string_and_reaches_its_bytes() {
    let mut words = [0u64; 2];
    let mut cx = context();
    cx.set_area(AREA_GLOBALS, words.as_mut_ptr().cast(), 16);
    let result = run(&raw mut cx, |cx, areas| {
        let texts: &Texts = areas.view(cx, AREA_GLOBALS)?;
        let bytes = areas.refer_text_bytes(cx, &texts.text)?;
        assert_eq!(
            bytes,
            RefValue {
                area: AREA_GLOBALS,
                kind: RefValue::TEXT_BYTES,
                char_index: 0,
                offset: 2,
                extent: 7,
                generation: 1,
            }
        );
        let first: &Cell<u8> = areas.deref(cx, &bytes)?;
        first.set(b'H');
        let all: &[Cell<u8>; 5] = areas.deref(cx, &bytes)?;
        all[4].set(b'o');
        let fifth: &Cell<u8> = areas.deref(
            cx,
            &RefValue {
                char_index: 4,
                ..bytes
            },
        )?;
        assert_eq!(fifth.get(), b'o');
        assert_eq!(
            (texts.text.data[0].get(), texts.text.data[4].get()),
            (b'H', b'o')
        );
        Ok(())
    });
    assert_eq!(result, 0);
    let mismatch = FaultCode::TypeMismatch as u32;
    assert_eq!(deref_outcome::<[Cell<u8>; 6]>(0), (mismatch, [5, 6, 0]));
    assert_eq!(deref_outcome::<[Cell<u8>; 2]>(4), (mismatch, [1, 2, 0]));
    assert_eq!(deref_outcome::<Str<5>>(0), (mismatch, [5, 8, 0]));
    assert_eq!(deref_outcome::<Cell<u8>>(4).0, 0);
}

/// A write through the reference sets the length that the written bytes give the text, and a
/// reference of another kind sets none (specification 44 §7.3): 'H', 'i' and a 0 written as an
/// array of 3 from byte 0 of an empty `STRING[5]` give the length 2; five bytes other than 0 give
/// the capacity, 5; a 0 written at byte 1 then gives 1. Catches a write through a pointer that
/// leaves the length at 0, and a reference to a variable whose writes are taken as a string's.
#[test]
fn a_write_through_the_reference_sets_the_length_of_the_text() {
    let mut words = [0u64; 2];
    let mut cx = context();
    cx.set_area(AREA_GLOBALS, words.as_mut_ptr().cast(), 16);
    let result = run(&raw mut cx, |cx, areas| {
        let texts: &Texts = areas.view(cx, AREA_GLOBALS)?;
        let bytes = areas.refer_text_bytes(cx, &texts.text)?;
        let head: &[Cell<u8>; 3] = areas.deref(cx, &bytes)?;
        for (cell, byte) in head.iter().zip(*b"Hi\0") {
            cell.set(byte);
        }
        let variable = RefValue {
            kind: RefValue::VARIABLE,
            ..bytes
        };
        areas.bytes_written(cx, &variable, 3)?;
        assert_eq!(texts.text.len.get(), 0);
        areas.bytes_written(cx, &bytes, 3)?;
        assert_eq!(texts.text.len.get(), 2);
        let all: &[Cell<u8>; 5] = areas.deref(cx, &bytes)?;
        for cell in all {
            cell.set(b'x');
        }
        areas.bytes_written(cx, &bytes, 5)?;
        assert_eq!(texts.text.len.get(), 5);
        let second = RefValue {
            char_index: 1,
            ..bytes
        };
        let byte: &Cell<u8> = areas.deref(cx, &second)?;
        byte.set(0);
        areas.bytes_written(cx, &second, 1)?;
        assert_eq!(texts.text.len.get(), 1);
        Ok(())
    });
    assert_eq!(result, 0);
}
