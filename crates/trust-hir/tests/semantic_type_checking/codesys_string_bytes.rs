//! Under the CODESYS profile `s[i]` of a `STRING` is the `BYTE` at offset `i`, counted from 0, up to
//! the 0 after a full text, and of a `WSTRING` the `WORD` there (docs/IEC_DEVIATIONS.md DEV-046;
//! CODESYS help on index access: "The result is of type BYTE", "The result is of type WORD"). IEC
//! 61131-3 Ed.3 §6.4.2.2 gives the `CHAR` or `WCHAR` at a position counted from 1, which the other
//! profiles keep.

use crate::common::*;

/// Reads and writes a `STRING[16]`'s bytes by their offsets, 0 and 16 among them, and takes the
/// address of byte 1, #453's `ADR(sJson[1])`.
const BYTES_BY_OFFSET: &str = r#"
PROGRAM Main
VAR
    text : STRING[16] := 'Hello';
    first : BYTE;
    afterFull : BYTE;
    pSecond : POINTER TO BYTE;
END_VAR
first := text[0];
afterFull := text[16];
text[1] := 16#61;
pSecond := ADR(text[1]);
END_PROGRAM
"#;

/// Under the CODESYS profile the program has no error: its elements are `BYTE`s from offset 0 to the
/// capacity. Under the IEC rules offset 0 is out of range, and each `CHAR` element given to a `BYTE`
/// is an incompatible assignment. Catches an element typed `CHAR` under the CODESYS profile, and an
/// index still counted from 1 there.
#[test]
fn a_strings_element_is_a_byte_from_offset_zero_under_codesys() {
    assert_eq!(check_errors_in(Dialect::Codesys, BYTES_BY_OFFSET), vec![]);
    let iec = check_errors_in(Dialect::Iec, BYTES_BY_OFFSET);
    assert!(iec.contains(&DiagnosticCode::OutOfRange), "{iec:?}");
}

/// An offset past the 0 after a full text is out of range under the CODESYS profile: 17 of a
/// `STRING[16]`. Catches a check that lost its upper bound with the move to offsets.
#[test]
fn an_offset_past_the_strings_bytes_is_out_of_range_under_codesys() {
    let source =
        "PROGRAM Main\nVAR text : STRING[16]; b : BYTE; END_VAR\nb := text[17];\nEND_PROGRAM";
    assert_eq!(
        check_errors_in(Dialect::Codesys, source),
        vec![DiagnosticCode::OutOfRange]
    );
}

/// Reads and writes a `WSTRING[8]`'s code units by their offsets, 0 and 8 among them, and takes the
/// address of unit 1.
const WORDS_BY_OFFSET: &str = r#"
PROGRAM Main
VAR
    text : WSTRING[8] := "Hello";
    first : WORD;
    afterFull : WORD;
    pSecond : POINTER TO WORD;
END_VAR
first := text[0];
afterFull := text[8];
text[1] := 16#61;
pSecond := ADR(text[1]);
END_PROGRAM
"#;

/// Under the CODESYS profile `ws[i]` of a `WSTRING` is the `WORD` at offset `i`, counted from 0 to the
/// capacity (CODESYS help on index access: "The result is of type WORD"), so the program has no error.
/// Under the IEC rules offset 0 is out of range. Catches a wide element still typed `WCHAR` under the
/// CODESYS profile, where `first := text[0]` and `pSecond := ADR(text[1])` would mismatch, and an
/// index still counted from 1 there.
#[test]
fn a_wide_strings_element_is_a_word_from_offset_zero_under_codesys() {
    assert_eq!(check_errors_in(Dialect::Codesys, WORDS_BY_OFFSET), vec![]);
    let iec = check_errors_in(Dialect::Iec, WORDS_BY_OFFSET);
    assert!(iec.contains(&DiagnosticCode::OutOfRange), "{iec:?}");
}

/// An offset past the 0 after a full text is out of range under the CODESYS profile: 9 of a
/// `WSTRING[8]`. Catches a wide check that lost its upper bound with the move to offsets.
#[test]
fn an_offset_past_the_wide_strings_units_is_out_of_range_under_codesys() {
    let source = "PROGRAM Main\nVAR ws : WSTRING[8]; w : WORD; END_VAR\nw := ws[9];\nEND_PROGRAM";
    assert_eq!(
        check_errors_in(Dialect::Codesys, source),
        vec![DiagnosticCode::OutOfRange]
    );
}

/// The IEC profile keeps the standard's `WCHAR` at a position counted from 1 (§6.4.2.2): `ws[1]` is a
/// `WCHAR`, and `ws[0]` is out of range. Catches the CODESYS element reaching the IEC profile.
#[test]
fn a_wide_strings_element_stays_a_wchar_from_position_one_under_iec() {
    let source = "PROGRAM Main\nVAR ws : WSTRING[8]; c : WCHAR; END_VAR\nc := ws[1];\nc := ws[0];\nEND_PROGRAM";
    assert_eq!(
        check_errors_in(Dialect::Iec, source),
        vec![DiagnosticCode::OutOfRange]
    );
}
