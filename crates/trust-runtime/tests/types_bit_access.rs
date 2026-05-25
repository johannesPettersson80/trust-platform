use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

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

    let main_id = match harness.runtime().storage().get_global("Main") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected Main instance, got {other:?}"),
    };
    let fb_id = match harness.runtime().storage().get_instance_var(main_id, "fb") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected FB instance, got {other:?}"),
    };
    let storage = harness.runtime().storage();

    assert_eq!(
        storage.get_instance_var(fb_id, "bit_val"),
        Some(&Value::Bool(false))
    );
    assert_eq!(
        storage.get_instance_var(fb_id, "byte_val"),
        Some(&Value::Byte(0x12))
    );
    assert_eq!(
        storage.get_instance_var(fb_id, "word_val"),
        Some(&Value::Word(0x89AB))
    );
    assert_eq!(
        storage.get_instance_var(fb_id, "dword_val"),
        Some(&Value::DWord(0x0123_4567))
    );
    assert_eq!(
        storage.get_instance_var(fb_id, "b"),
        Some(&Value::Byte(0x08))
    );
    assert_eq!(
        storage.get_instance_var(fb_id, "w"),
        Some(&Value::Word(0x12FF))
    );
}
