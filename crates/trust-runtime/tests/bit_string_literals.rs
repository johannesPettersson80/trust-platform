//! Untyped literals where a bit string goes, run as the compiled program (IEC 61131-3 Ed.3 §6.3.2
//! and §6.6.1.7.2; specification 05 §5.2.1; checklist G224): a based number, or a decimal one that
//! is not negative, is a bit string literal at the input of `SHL`, `SHR`, `ROL` and `ROR`, as the
//! operand of `NOT`, and as both operands of `AND`, `OR` and `XOR`. The expression takes the bit
//! string type where it goes when its literals fit it, else the smallest that holds them; a typed
//! input keeps its own type. Every expected value below is worked out by hand from that rule.

use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

const SOURCE: &str = r#"
FUNCTION Mask : WORD
VAR_INPUT
    x : WORD;
END_VAR
Mask := x;
END_FUNCTION

PROGRAM Main
VAR
    n : INT := 15;
    wide : WORD;
    narrow : BYTE;
    typed : WORD;
    rotated : DWORD;
    inverted : WORD;
    masked : BYTE;
    compared : BOOL;
    passed : WORD;
END_VAR
wide := SHL(16#0001, n);
narrow := SHL(16#01, 7);
typed := SHL(BYTE#16#01, 8);
rotated := ROL(16#8000_0001, 1);
inverted := NOT 16#00FF;
masked := 16#0F AND 16#3C;
compared := SHL(16#01, n) = 16#8000;
passed := Mask(SHL(16#0001, 12));
END_PROGRAM
"#;

#[test]
fn an_untyped_bit_string_literal_takes_the_type_where_it_goes() {
    // wide: SHL(16#0001, 15) shifts the WORD of its target, 16#8000 (the external suite's
    // ops/bit_literals, cycle 2); in the literal's smallest type, a BYTE, the bit would leave it.
    // narrow: SHL(16#01, 7) in a BYTE is 16#80. typed: BYTE#16#01 keeps its type although the
    // target is a WORD (IEC §6.6.1.7.2), so shifting it 8 bits leaves 0. rotated: 16#8000_0001
    // needs a DWORD; rotated left by 1 it is 16#0000_0003. inverted: NOT of 16#00FF in its WORD
    // target is 16#FF00, where a BYTE would give 0. masked: 16#0F AND 16#3C in its BYTE target is
    // 16#0C. compared:
    // SHL(16#01, 15) beside the literal 16#8000 is a WORD shift, 16#8000, so the two are equal.
    // passed: the parameter's WORD carries into SHL(16#0001, 12), 16#1000, where a BYTE would
    // give 0. Catches a literal shift computed in the literal's smallest type, a typed input
    // widened by its target, and a call that a comparison or a parameter leaves at a BYTE.
    let mut harness =
        TestHarness::from_source(SOURCE).unwrap_or_else(|error| panic!("compiles: {error}"));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    let read = |name: &str| harness.get_output(name);
    assert_eq!(read("wide"), Some(Value::Word(0x8000)));
    assert_eq!(read("narrow"), Some(Value::Byte(0x80)));
    assert_eq!(read("typed"), Some(Value::Word(0)));
    assert_eq!(read("rotated"), Some(Value::DWord(0x0000_0003)));
    assert_eq!(read("inverted"), Some(Value::Word(0xFF00)));
    assert_eq!(read("masked"), Some(Value::Byte(0x0C)));
    assert_eq!(read("compared"), Some(Value::Bool(true)));
    assert_eq!(read("passed"), Some(Value::Word(0x1000)));
}

#[test]
fn a_negative_literal_where_a_bit_string_goes_is_refused() {
    // IEC §6.3.2 makes only a based number or a positive one a bit string literal. Catches -1
    // compiled as a bit string, whose bits no bit string type holds.
    let error = TestHarness::from_source(
        "PROGRAM Main\nVAR w : WORD; END_VAR\nw := SHL(-1, 3);\nEND_PROGRAM",
    )
    .err()
    .expect("SHL(-1, 3) does not compile");
    assert!(
        error
            .to_string()
            .contains("expected bit string input: a negative literal is no bit string literal"),
        "{error}"
    );
}
