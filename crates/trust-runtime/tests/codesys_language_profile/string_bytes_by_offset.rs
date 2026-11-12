//! Under the CODESYS profile `s[i]` of a `STRING` is the `BYTE` at offset `i`, counted from 0, read
//! and written, and `ADR(s[i])` names it (docs/IEC_DEVIATIONS.md DEV-046; contributor issue #453).
//! The byte after a full text reads 0, where CODESYS holds the text's 0, and a write sets the length
//! the bytes give the text, as a write through a pointer does (DEV-039).

use super::*;

/// #453's `MemCopySecure(pDest := ADR(sJson[1]), ...)`: a `POINTER TO BYTE` argument from the address
/// of a string's byte; reads and writes by offset; and the CODESYS scan of a full text for its 0.
const BY_OFFSET: &str = r#"
FUNCTION PutByte : VOID
VAR_INPUT
    pDest : POINTER TO BYTE;
    value : BYTE;
END_VAR
pDest^ := value;
END_FUNCTION

PROGRAM Main
VAR
    sJson : STRING[32] := '{';
    sText : STRING[16] := 'Hello';
    sCut : STRING[16] := 'Hello';
    sFull : STRING[5] := 'World';
    first : BYTE;
    terminator : BYTE;
    afterFull : BYTE;
    pThird : POINTER TO BYTE;
    lenJson : INT;
    i : INT;
    scanned : INT;
END_VAR
PutByte(pDest := ADR(sJson[1]), value := 34);
lenJson := LEN(sJson);
first := sText[0];
terminator := sText[5];
afterFull := sFull[5];
sText[1] := 97;
sText[5] := 33;
pThird := ADR(sCut[2]);
pThird^ := 0;
sFull[5] := 0;
WHILE sFull[i] <> 0 DO
    i := i + 1;
END_WHILE
scanned := i;
END_PROGRAM
"#;

// Worked by hand, 'Hello' being 72 101 108 108 111 and its 0:
// - `ADR(sJson[1])` names byte 1 of '{', its 0; 34 ('"') written there extends the text: '{"', LEN 2;
// - offset 0 of 'Hello' reads 72 ('H'), offset 5 its 0, and offset 5 of the full STRING[5] 'World'
//   reads 0, where CODESYS holds the 0 after a full text;
// - 97 ('a') at offset 1 makes 'Hallo'; 33 ('!') at offset 5, over its 0, makes 'Hallo!';
// - a pointer to byte 2 of 'Hello' and a 0 written through it end it at 'He';
// - a 0 at offset 5 of 'World' changes nothing, and the scan for the 0 stops at offset 5.
// Catches the build that refused `ADR(sJson[1])` ("an index of a value that is no array", #453) and
// the index from 1 (offset 0 out of range), an element read as a character from 1 (72 would be
// 101), a write that left the length as it was ('Hallo' without its '!'), and a scan that faulted
// past the full text instead of reading its 0.
#[test]
fn a_strings_bytes_are_read_written_and_addressed_by_their_offsets() {
    let harness = codesys_run(BY_OFFSET, 1);
    for (name, expected) in [
        ("sJson", Value::String("{\"".into())),
        ("lenJson", Value::Int(2)),
        ("first", Value::Byte(72)),
        ("terminator", Value::Byte(0)),
        ("afterFull", Value::Byte(0)),
        ("sText", Value::String("Hallo!".into())),
        ("sCut", Value::String("He".into())),
        ("sFull", Value::String("World".into())),
        ("scanned", Value::Int(5)),
    ] {
        assert_eq!(harness.get_output(name), Some(expected), "{name}");
    }
}
