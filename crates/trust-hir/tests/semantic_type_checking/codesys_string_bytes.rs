//! Under the CODESYS profile `s[i]` of a `STRING` is the `BYTE` at offset `i`, counted from 0, up to
//! the 0 after a full text (docs/IEC_DEVIATIONS.md DEV-046; CODESYS help on index access: "The
//! result is of type BYTE"). IEC 61131-3 Ed.3 §6.4.2.2 gives the `CHAR` at a position counted from 1,
//! which the other profiles keep.

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

/// A `WSTRING` keeps the IEC element under every profile until its `WORD` by offset follows: `ws[1]`
/// is a `WCHAR`, and `ws[0]` is out of range. Catches a change of the narrow string's element that
/// reached the wide one.
#[test]
fn a_wide_strings_element_stays_a_wchar_from_position_one() {
    let source = "PROGRAM Main\nVAR ws : WSTRING[8]; c : WCHAR; END_VAR\nc := ws[1];\nc := ws[0];\nEND_PROGRAM";
    assert_eq!(
        check_errors_in(Dialect::Codesys, source),
        vec![DiagnosticCode::OutOfRange]
    );
}
