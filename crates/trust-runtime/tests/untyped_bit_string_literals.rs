use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

#[test]
fn untyped_integer_literals_assign_to_and_combine_with_bit_strings() {
    let source = r#"
PROGRAM Main
VAR
    b : BYTE;
    w : WORD;
    d : DWORD := 16#F0;
    masked : DWORD;
    flipped : BYTE;
    is_zero : BOOL;
    is_ff : BOOL;
    sel : WORD;
END_VAR
b := 16#FF;
w := 65535;
masked := d AND 16#30;
d := d OR 1;
flipped := b XOR 16#0F;
is_zero := masked = 0;
is_ff := 16#FF = b;
CASE b OF
    0: sel := 1;
    16#FF: sel := 16#ABCD;
END_CASE
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.cycle();

    assert_eq!(harness.get_output("b"), Some(Value::Byte(0xFF)));
    assert_eq!(harness.get_output("w"), Some(Value::Word(0xFFFF)));
    assert_eq!(harness.get_output("masked"), Some(Value::DWord(0x30)));
    assert_eq!(harness.get_output("d"), Some(Value::DWord(0xF1)));
    assert_eq!(harness.get_output("flipped"), Some(Value::Byte(0xF0)));
    assert_eq!(harness.get_output("is_zero"), Some(Value::Bool(false)));
    assert_eq!(harness.get_output("is_ff"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("sel"), Some(Value::Word(0xABCD)));
}
