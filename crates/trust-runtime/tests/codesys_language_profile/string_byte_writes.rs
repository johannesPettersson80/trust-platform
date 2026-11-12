//! Under the CODESYS profile bytes written into a string through a pointer change its text: the text
//! ends at its first 0 byte, as CODESYS reads a string (docs/IEC_DEVIATIONS.md DEV-039;
//! specification 44 §7.3; contributor issue #453). A copy loop like the vendor `MemCopy`, an element,
//! a whole array, a function's in-out, a function block's in-out, an element of a function block's
//! array in-out and a pointer taken to an element write the bytes; each sets the length the bytes
//! give.

use super::*;

/// Each shape writes into a `STRING[16]` that held 'Hello' or 'Hi' over 'Hello'.
const BYTE_WRITES: &str = r#"
FUNCTION CopyBytes : VOID
VAR_INPUT
    pDest : POINTER TO ARRAY[0..65535] OF BYTE;
    pSource : POINTER TO ARRAY[0..65535] OF BYTE;
    size : UDINT;
END_VAR
VAR
    i : UDINT;
END_VAR
WHILE i < size DO
    pDest^[i] := pSource^[i];
    i := i + 1;
END_WHILE
END_FUNCTION

FUNCTION SetOk : VOID
VAR_IN_OUT
    buffer : ARRAY[0..3] OF BYTE;
END_VAR
buffer[0] := 79;
buffer[1] := 75;
buffer[2] := 0;
END_FUNCTION

FUNCTION_BLOCK Clear
VAR_IN_OUT
    first : BYTE;
END_VAR
first := 0;
END_FUNCTION_BLOCK

FUNCTION_BLOCK EndAfterFirst
VAR_IN_OUT
    bytes : ARRAY[0..3] OF BYTE;
END_VAR
bytes[1] := 0;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    sSource : STRING[16] := 'Hello';
    sCopy : STRING[16];
    sCut : STRING[16] := 'Hello';
    sGrown : STRING[16] := 'Hello';
    sAfter : STRING[16] := 'Hello';
    sWhole : STRING[16] := 'Hello';
    sInOut : STRING[16] := 'Hello';
    sBlock : STRING[16] := 'Hello';
    sBlockArray : STRING[16] := 'Hello';
    sElement : STRING[16] := 'Hello';
    pData : POINTER TO ARRAY[0..15] OF BYTE;
    pHead : POINTER TO ARRAY[0..3] OF BYTE;
    pByte : POINTER TO BYTE;
    head : ARRAY[0..3] OF BYTE := [74, 0, 0, 0];
    clearer : Clear;
    ender : EndAfterFirst;
    lenCopy : INT;
END_VAR
CopyBytes(pDest := ADR(sCopy), pSource := ADR(sSource), size := 6);
lenCopy := LEN(sCopy);
pData := ADR(sCut);
pData^[2] := 0;
sGrown := 'Hi';
pData := ADR(sGrown);
pData^[2] := 121;
sAfter := 'Hi';
pData := ADR(sAfter);
pData^[4] := 122;
pHead := ADR(sWhole);
pHead^ := head;
pHead := ADR(sInOut);
SetOk(buffer := pHead^);
pByte := ADR(sBlock);
clearer(first := pByte^);
pHead := ADR(sBlockArray);
ender(bytes := pHead^);
pData := ADR(sElement);
pByte := ADR(pData^[2]);
pByte^ := 0;
END_PROGRAM
"#;

// Worked by hand, 'Hello' being 72 101 108 108 111 and a 0:
// - the copy writes those 6 bytes into an empty string one by one, each byte extending the text
//   over its 0, and the last one, 0, ending it: 'Hello', LEN 5;
// - a 0 written at index 2 of 'Hello' ends it there: 'He';
// - 'Hi' over 'Hello' leaves 72 105 0 108 111 0; 121 ('y') over its 0 extends it to the next 0,
//   through the old bytes, as CODESYS reads them: 'Hiylo';
// - 122 written at index 4 of that 'Hi', after its 0, leaves it 'Hi';
// - the array 74 0 0 0 over the first four bytes of 'Hello' gives 'J';
// - the function's in-out writes 79 75 0 into them: 'OK';
// - the function block's in-out writes 0 into the first byte: the empty text;
// - another block's in-out, the first four bytes as an array, writes 0 into element 1: 'H';
// - a pointer taken to element 2 names byte 2, and its 0 ends 'Hello' there: 'He'.
// Catches each write that left the length as it was: the copy would read '' and LEN 0, the cut
// 'He' followed by a 0 and 'lo', the grown string 'Hi', the array 'J' followed by three 0s and 'o',
// the in-outs 'OK', a 0 and 'lo', a 0 followed by 'ello', and 'H', a 0 and 'llo', the element
// pointer like the cut.
#[test]
fn bytes_written_through_a_pointer_change_the_text() {
    let harness = codesys_run(BYTE_WRITES, 1);
    for (name, expected) in [
        ("sCopy", "Hello"),
        ("sCut", "He"),
        ("sGrown", "Hiylo"),
        ("sAfter", "Hi"),
        ("sWhole", "J"),
        ("sInOut", "OK"),
        ("sBlock", ""),
        ("sBlockArray", "H"),
        ("sElement", "He"),
    ] {
        assert_eq!(
            harness.get_output(name),
            Some(Value::String(expected.into())),
            "{name}"
        );
    }
    assert_eq!(harness.get_output("lenCopy"), Some(Value::Int(5)));
}
