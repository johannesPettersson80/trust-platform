//! Partial access to the bits, bytes, words and double words of bit-string variables (IEC 61131-3
//! Table 17), in a program, through an in-out and inside a function block.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

#[test]
fn bit_access_on_struct_byte_via_inout() {
    let source = r#"
TYPE Payload :
STRUCT
    b : BYTE;
END_STRUCT
END_TYPE

FUNCTION_BLOCK Mutator
VAR_IN_OUT
    payload : Payload;
END_VAR
VAR_OUTPUT
    before : BOOL;
END_VAR
before := payload.b.%X0;
payload.b.%X3 := TRUE;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    data : Payload := (b := BYTE#16#01);
    fb : Mutator;
    before : BOOL;
    result : BYTE;
END_VAR
fb(payload := data);
before := fb.before;
result := data.b;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    let err = harness.cycle().errors.into_iter().next();
    assert!(err.is_none(), "expected success, got {err:?}");

    assert_eq!(harness.get_output("before"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("result"), Some(Value::Byte(0x09)));
}

#[test]
fn table17() {
    let source = r#"
PROGRAM Main
VAR
    b : BYTE := BYTE#16#00;
    w : WORD := WORD#16#1234;
    d : DWORD := DWORD#16#89ABCDEF;
    l : LWORD := LWORD#16#0123_4567_89AB_CDEF;
    bit_val : BOOL;
    byte_val : BYTE;
    word_val : WORD;
    dword_val : DWORD;
END_VAR
bit_val := b.%X0;
bit_val := b.7;
byte_val := w.%B1;
word_val := d.%W1;
dword_val := l.%D1;
b.%X3 := TRUE;
w.%B0 := BYTE#16#FF;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.cycle();

    assert_eq!(harness.get_output("bit_val"), Some(Value::Bool(false)));
    assert_eq!(harness.get_output("byte_val"), Some(Value::Byte(0x12)));
    assert_eq!(harness.get_output("word_val"), Some(Value::Word(0x89AB)));
    assert_eq!(
        harness.get_output("dword_val"),
        Some(Value::DWord(0x0123_4567))
    );
    assert_eq!(harness.get_output("b"), Some(Value::Byte(0x08)));
    assert_eq!(harness.get_output("w"), Some(Value::Word(0x12FF)));
}

#[test]
fn table17_fb() {
    let source = r#"
FUNCTION_BLOCK BitAccessFb
VAR
    b : BYTE := BYTE#16#00;
    w : WORD := WORD#16#1234;
    d : DWORD := DWORD#16#89ABCDEF;
    l : LWORD := LWORD#16#0123_4567_89AB_CDEF;
    bit_val : BOOL;
    byte_val : BYTE;
    word_val : WORD;
    dword_val : DWORD;
END_VAR
bit_val := b.%X0;
bit_val := b.7;
byte_val := w.%B1;
word_val := d.%W1;
dword_val := l.%D1;
b.%X3 := TRUE;
w.%B0 := BYTE#16#FF;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    fb : BitAccessFb;
END_VAR
fb();
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    let err = harness.cycle().errors.into_iter().next();
    assert!(err.is_none(), "expected success, got {err:?}");

    for (name, expected) in [
        ("bit_val", Value::Bool(false)),
        ("byte_val", Value::Byte(0x12)),
        ("word_val", Value::Word(0x89AB)),
        ("dword_val", Value::DWord(0x0123_4567)),
        ("b", Value::Byte(0x08)),
        ("w", Value::Word(0x12FF)),
    ] {
        assert_eq!(
            harness.get_output(&format!("Main.fb.{name}")),
            Some(expected),
            "{name}"
        );
    }
}
