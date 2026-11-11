//! Under the CODESYS profile a pointer to a string's bytes, `ADR(s)` where a `POINTER TO BYTE` or a
//! `POINTER TO ARRAY[l..u] OF BYTE` is wanted, names the string's bytes from its first one, and a
//! pointer to a `CHAR` names it as a `BYTE` (docs/IEC_DEVIATIONS.md DEV-039; contributor issue #453).
//! An element read through a pointer to an array is checked against the bytes the pointer names,
//! which for a string are its capacity, not the array type's.

use trust_runtime::error::RuntimeError;

use super::*;

/// The report's shapes: `ADR` of a string for a `POINTER TO BYTE` argument (WAGO's `MemCopy`) and
/// variable, for a `POINTER TO ARRAY[0..65535] OF BYTE` argument (WAGO's JSON parser) and variable,
/// for a pointer to an array shorter than the string, and of a `CHAR` for a `POINTER TO BYTE`.
const BYTE_POINTERS: &str = r#"
FUNCTION FirstByte : BYTE
VAR_INPUT
    pSource : POINTER TO BYTE;
END_VAR
FirstByte := pSource^;
END_FUNCTION

FUNCTION ByteAt : BYTE
VAR_INPUT
    pData : POINTER TO ARRAY[0..65535] OF BYTE;
    index : UDINT;
END_VAR
ByteAt := pData^[index];
END_FUNCTION

PROGRAM Main
VAR
    sPayload : STRING[16] := 'Hello';
    pByte : POINTER TO BYTE;
    pData : POINTER TO ARRAY[0..65535] OF BYTE;
    pHead : POINTER TO ARRAY[0..3] OF BYTE;
    head : ARRAY[0..3] OF BYTE;
    c : CHAR := 'Z';
    pFromChar : POINTER TO BYTE;
    first : BYTE;
    viaArgument : BYTE;
    second : BYTE;
    viaFunction : BYTE;
    headFirst : BYTE;
    headLast : BYTE;
    fromChar : BYTE;
END_VAR
pByte := ADR(sPayload);
first := pByte^;
viaArgument := FirstByte(pSource := ADR(sPayload));
pData := ADR(sPayload);
second := pData^[1];
viaFunction := ByteAt(pData := ADR(sPayload), index := 4);
pHead := ADR(sPayload);
head := pHead^;
headFirst := head[0];
headLast := head[3];
pData^[0] := 74;
pFromChar := ADR(c);
fromChar := pFromChar^;
END_PROGRAM
"#;

// Worked by hand: 'Hello' is the bytes 72, 101, 108, 108, 111. The pointer to a byte names the
// first, 72 ('H'), as a variable and as an argument; the long array pointer reads 101 at index 1
// and 111 at index 4; the four-byte array copies 72 to 108; writing 74 ('J') through the long
// array makes the string 'Jello'; the CHAR 'Z' reads as the BYTE 90. Catches the build that refused
// each conversion ("a variable of another type", the report on #453), a pointer to the string's
// length instead of its first byte (the first byte would read 5, the length), and an element read
// that checks the array type's size, 65536 bytes, against the string's (every read would fault).
#[test]
fn a_string_taken_as_bytes_names_its_bytes_from_the_first() {
    let harness = codesys_run(BYTE_POINTERS, 1);
    for (name, expected) in [
        ("first", 72),
        ("viaArgument", 72),
        ("second", 101),
        ("viaFunction", 111),
        ("headFirst", 72),
        ("headLast", 108),
        ("fromChar", 90),
    ] {
        assert_eq!(
            harness.get_output(name),
            Some(Value::Byte(expected)),
            "{name}"
        );
    }
    assert_eq!(
        harness.get_output("sPayload"),
        Some(Value::String("Jello".into()))
    );
}

/// A read through the long array pointer past the string's 16 bytes, at index 16, which the array
/// type's bounds 0 to 65535 would allow.
const PAST_THE_STRING: &str = r#"
PROGRAM Main
VAR
    sPayload : STRING[16] := 'Hello';
    pData : POINTER TO ARRAY[0..65535] OF BYTE;
    inside : BYTE;
    outside : BYTE := 7;
END_VAR
pData := ADR(sPayload);
inside := pData^[15];
outside := pData^[16];
END_PROGRAM
"#;

// Worked by hand: the pointer names the string's 16 bytes, indices 0 to 15, so index 15 reads (the
// string's bytes past its length are 0) and index 16 is refused with the bounds of those bytes; the
// refused read leaves `outside` at 7. Catches a pointer that reads past the string into the
// variables after it, as CODESYS's raw memory would.
#[test]
fn a_read_past_the_string_through_an_array_pointer_is_refused() {
    let mut harness = TestHarness::from_session(
        CompileSession::from_source(PAST_THE_STRING)
            .with_conversion_profile(ConversionProfile::Codesys),
    )
    .expect("the CODESYS profile builds it");
    let cycle = harness.cycle();
    assert_eq!(
        cycle.errors,
        vec![RuntimeError::IndexOutOfBounds {
            index: 16,
            lower: 0,
            upper: 15,
        }]
    );
    assert_eq!(harness.get_output("inside"), Some(Value::Byte(0)));
    assert_eq!(harness.get_output("outside"), Some(Value::Byte(7)));
}
