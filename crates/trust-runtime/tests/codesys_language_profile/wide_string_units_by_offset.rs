//! Under the CODESYS profile `ws[i]` of a `WSTRING` is the `WORD` at offset `i`, counted from 0, read
//! and written, and `ADR(ws[i])` names it (docs/IEC_DEVIATIONS.md DEV-046; CODESYS help on index
//! access: "you get the character at the offset of the index expression. The result is of type
//! WORD"). The unit after a full text reads 0, where CODESYS holds the text's 0, and a write sets the
//! length the units give the text, as CODESYS reads a `WSTRING` to its first 0.

use super::*;

/// Reads and writes by offset, a pointer to a unit given to a function and written through, and the
/// CODESYS scan of a full text for its 0.
const BY_OFFSET: &str = r#"
FUNCTION PutWord : VOID
VAR_INPUT
    pDest : POINTER TO WORD;
    value : WORD;
END_VAR
pDest^ := value;
END_FUNCTION

PROGRAM Main
VAR
    wsText : WSTRING[16] := "Hello";
    wsCut : WSTRING[16] := "Hello";
    wsFull : WSTRING[5] := "World";
    wsSign : WSTRING[4] := "€";
    first : WORD;
    terminator : WORD;
    afterFull : WORD;
    euro : WORD;
    lenText : INT;
    i : INT;
    scanned : INT;
END_VAR
first := wsText[0];
terminator := wsText[5];
afterFull := wsFull[5];
euro := wsSign[0];
wsText[1] := 97;
wsText[5] := 33;
lenText := LEN(wsText);
PutWord(pDest := ADR(wsCut[2]), value := 0);
wsFull[5] := 0;
WHILE wsFull[i] <> 0 DO
    i := i + 1;
END_WHILE
scanned := i;
END_PROGRAM
"#;

// Worked by hand, 'Hello' being the UTF-16 units 72 101 108 108 111 and its 0:
// - offset 0 of 'Hello' reads 72 ('H'), offset 5 its 0, and offset 5 of the full WSTRING[5] 'World'
//   reads 0, where CODESYS holds the 0 after a full text; offset 0 of '€' reads its unit 0x20AC;
// - 97 ('a') at offset 1 makes 'Hallo'; 33 ('!') at offset 5, over its 0, makes 'Hallo!', LEN 6;
// - a pointer to unit 2 of 'Hello', given to a function that writes 0 through it, ends it at 'He';
// - a 0 at offset 5 of 'World' changes nothing, and the scan for the 0 stops at offset 5.
// Catches the build that typed `ws[i]` as the IEC WCHAR from 1 (offset 0 out of range, a WCHAR given
// to a WORD), a unit read as a byte (101 would be 0), a write that left the length as it was ('Hallo'
// without its '!', 'Hello' with a 0 inside), and a scan that faulted past the full text.
#[test]
fn a_wide_strings_units_are_read_written_and_addressed_by_their_offsets() {
    let harness = codesys_run(BY_OFFSET, 1);
    for (name, expected) in [
        ("first", Value::Word(72)),
        ("terminator", Value::Word(0)),
        ("afterFull", Value::Word(0)),
        ("euro", Value::Word(0x20AC)),
        ("wsText", Value::WString("Hallo!".into())),
        ("lenText", Value::Int(6)),
        ("wsCut", Value::WString("He".into())),
        ("wsFull", Value::WString("World".into())),
        ("scanned", Value::Int(5)),
    ] {
        assert_eq!(harness.get_output(name), Some(expected), "{name}");
    }
}

/// Starting values computed from a constant, which the build works out before the program runs
/// (specification 44 §6.7), and the program's own reads of the same units.
const STARTING_UNITS: &str = r#"
VAR_GLOBAL CONSTANT
    cwText : WSTRING[4] := "Ä€";
END_VAR

PROGRAM Main
VAR
    first : WORD := cwText[0];
    second : WORD := cwText[1];
    terminator : WORD := cwText[2];
    atCapacity : WORD := cwText[4];
    readFirst : WORD;
    readSecond : WORD;
END_VAR
readFirst := cwText[0];
readSecond := cwText[1];
END_PROGRAM
"#;

// Worked by hand: "Ä€" is the UTF-16 units 0x00C4 and 0x20AC of a WSTRING[4] (specification 44
// §6.2), then its 0 at offset 2 and, at the capacity 4, the 0 that CODESYS holds after a full text.
// A starting value reads each unit as the program reads it (DEV-046). Catches the build's IEC element
// in a starting value (offset 0 refused as out of bounds, a WCHAR where a WORD is meant), a UTF-8 byte
// read where a unit is meant (0xC3 for 'Ä'), and a starting value that differs from the program's read
// of the same offset.
#[test]
fn a_starting_value_reads_a_wide_strings_unit_as_the_program_does() {
    let harness = codesys_run(STARTING_UNITS, 1);
    for (name, expected) in [
        ("first", 0x00C4),
        ("second", 0x20AC),
        ("terminator", 0),
        ("atCapacity", 0),
        ("readFirst", 0x00C4),
        ("readSecond", 0x20AC),
    ] {
        assert_eq!(
            harness.get_output(name),
            Some(Value::Word(expected)),
            "{name}"
        );
    }
}
