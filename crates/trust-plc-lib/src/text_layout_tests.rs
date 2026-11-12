//! The bytes a text leaves in its string (specification 44 §6.2; DEV-039 in
//! `docs/IEC_DEVIATIONS.md`): every write puts a 0 after the text when the capacity leaves room and
//! keeps the bytes after that 0, as CODESYS lays out a string, its text and its "terminating null
//! character", whose "contents of the (old) string, which exist after the terminating null
//! character" stay (CODESYS help, "Data type: STRING"). Under the CODESYS profile `ADR(s)` names
//! these bytes, so code that scans them for the 0 finds where the text ends.

use trust_plc_abi::kernel::WITHOUT_DEBUGGER;
use trust_plc_abi::{Cx, FaultCode, RefValue, Str, WStr};

use crate::*;

fn cx() -> Cx {
    Cx::new(&WITHOUT_DEBUGGER)
}

fn bytes<const N: usize>(text: &Str<N>) -> [u8; N] {
    core::array::from_fn(|index| text.data[index].get())
}

fn units<const N: usize>(text: &WStr<N>) -> [u16; N] {
    core::array::from_fn(|index| text.data[index].get())
}

// Worked by hand: 'Hello' is 72 101 108 108 111; 'Hi' over it writes 72 105, the 0 at index 2, and
// keeps 108 111. A full STRING[5] has no byte left for the 0. Catches a shorter text that leaves
// 'Hi' followed by 'llo' for a scan of the bytes (the bytes before this change: 72 105 108 108 111),
// and a 0 written past the capacity.
#[test]
fn a_shorter_text_ends_with_a_zero_byte_and_keeps_the_bytes_after_it() {
    let text = Str::<8>::new();
    assign_bytes(text.view(), b"Hello");
    assign_bytes(text.view(), b"Hi");
    assert_eq!(text.view().len(), 2);
    assert_eq!(bytes(&text), [72, 105, 0, 108, 111, 0, 0, 0]);

    let full = Str::<5>::new();
    assign_bytes(full.view(), b"Hello");
    assert_eq!(full.view().len(), 5);
    assert_eq!(bytes(&full), [72, 101, 108, 108, 111]);
    assign_bytes(full.view(), b"Hi");
    assert_eq!(bytes(&full), [72, 105, 0, 108, 111]);
}

// Worked by hand: 'aé!' is 0x61, 0xC3 0xA9, 0x21; writing 'e' (0x65) at character 2 makes 'ae!',
// three bytes, so the 0 moves to index 3, where the '!' stood before it moved left. Catches a
// character write that shortens the text and leaves its old last byte after it.
#[test]
fn a_character_write_that_shortens_the_text_moves_its_zero_byte() {
    let text = Str::<8>::new();
    assign_bytes(text.view(), "aé!".as_bytes());
    set_char(&mut cx(), text.view(), 2, b'e').expect("character 2 exists");
    assert_eq!(text.view().len(), 3);
    assert_eq!(bytes(&text), [0x61, 0x65, 0x21, 0, 0, 0, 0, 0]);
}

// Worked by hand, in UTF-16 code units: "Hello" over an empty WSTRING[8], then "Hi" writes 72 105,
// the 0 at index 2, and keeps 108 111. Catches the wide text leaving its old units after a shorter
// one.
#[test]
fn a_shorter_wide_text_ends_with_a_zero_unit_and_keeps_the_units_after_it() {
    let text = WStr::<8>::new();
    assign_units(text.view(), &[72, 101, 108, 108, 111]);
    assign_units(text.view(), &[72, 105]);
    assert_eq!(text.view().len(), 2);
    assert_eq!(units(&text), [72, 105, 0, 108, 111, 0, 0, 0]);
}

// Worked by hand: "a😀!" is 0x61, the pair 0xD83D 0xDE00, 0x21; writing 'e' (0x65) at character 2
// replaces the pair with one unit, so the text is 0x61 0x65 0x21 and the 0 moves to index 3. Catches
// the wide character write leaving its old last unit after the shorter text.
#[test]
fn a_wide_character_write_that_shortens_the_text_moves_its_zero_unit() {
    let text = WStr::<8>::new();
    assign_units(text.view(), &[0x61, 0xD83D, 0xDE00, 0x21]);
    set_char_wide(&mut cx(), text.view(), 2, 0x65).expect("character 2 exists");
    assert_eq!(text.view().len(), 3);
    assert_eq!(units(&text), [0x61, 0x65, 0x21, 0, 0, 0, 0, 0]);
}

// Worked by hand: converting 'Hi' into a WSTRING[8] that held "Hello" writes 72 105 and the 0 at
// index 2; converting "Hi" into a STRING[8] that held 'Hello' writes the same bytes. Catches either
// conversion leaving the target's old text after the new one.
#[test]
fn a_conversion_ends_its_text_with_a_zero() {
    let source = Str::<4>::new();
    assign_bytes(source.view(), b"Hi");
    let wide = WStr::<8>::new();
    assign_units(wide.view(), &[72, 101, 108, 108, 111]);
    string_to_wstring(wide.view(), source.view());
    assert_eq!(units(&wide), [72, 105, 0, 108, 111, 0, 0, 0]);

    let wide_source = WStr::<4>::new();
    assign_units(wide_source.view(), &[72, 105]);
    let narrow = Str::<8>::new();
    assign_bytes(narrow.view(), b"Hello");
    wstring_to_string(&mut cx(), narrow.view(), wide_source.view()).expect("ASCII converts");
    assert_eq!(bytes(&narrow), [72, 105, 0, 108, 111, 0, 0, 0]);
}

// Worked by hand: the bytes of a STRING[5] at offset 2 are named by a reference that spans 2 + 5 = 7
// bytes from there. Taken as an ARRAY[0..65535] OF BYTE from byte 0, element 3 is the string's byte
// 3 (index 3, the string's offset and extent kept), and element 5 lies past its 5 bytes:
// IndexOutOfBounds with the index and the bounds 0 to 4. From byte 2, the array names 3 bytes, 0 to
// 2, and its element 2 is byte 4. Catches an element reference that moves the offset, which a write
// through it would take for the string's start, and an element past the string's bytes.
#[test]
fn an_element_of_a_strings_bytes_names_the_string_from_its_byte() {
    let text = RefValue {
        area: 4,
        kind: RefValue::TEXT_BYTES,
        char_index: 0,
        offset: 2,
        extent: 7,
        generation: 1,
    };
    let mut cx = cx();
    assert_eq!(
        element_reference(&mut cx, text, 3, 0, 65535, 1),
        Ok(RefValue {
            char_index: 3,
            ..text
        })
    );
    assert!(element_reference(&mut cx, text, 5, 0, 65535, 1).is_err());
    assert_eq!(
        (cx.fault.code, cx.fault.detail),
        (FaultCode::IndexOutOfBounds as u32, [5, 0, 4])
    );
    let third = RefValue {
        char_index: 2,
        ..text
    };
    assert_eq!(
        element_reference(&mut cx, third, 2, 0, 65535, 1),
        Ok(RefValue {
            char_index: 4,
            ..text
        })
    );
    assert!(element_reference(&mut cx, third, 3, 0, 65535, 1).is_err());
    assert_eq!(cx.fault.detail, [3, 0, 2]);
}
