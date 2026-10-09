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

#[test]
fn untyped_integer_literals_take_the_full_range_of_their_bit_string_target() {
    // 16#FFFF_FFFF does not fit DINT; as a DWORD it is that type's largest value.
    let source = r#"
PROGRAM Main
VAR
    initial : DWORD := 16#FFFF_FFFF;
    d : DWORD;
    high : DWORD;
    is_max : BOOL;
    l : LWORD;
END_VAR
d := 16#FFFF_FFFF;
high := d AND 16#FFFF_0000;
is_max := d = 16#FFFF_FFFF;
l := 16#1_0000_0000;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);

    assert_eq!(
        harness.get_output("initial"),
        Some(Value::DWord(0xFFFF_FFFF))
    );
    assert_eq!(harness.get_output("d"), Some(Value::DWord(0xFFFF_FFFF)));
    assert_eq!(harness.get_output("high"), Some(Value::DWord(0xFFFF_0000)));
    assert_eq!(harness.get_output("is_max"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("l"), Some(Value::LWord(0x1_0000_0000)));
}

#[test]
fn integer_literals_zero_and_one_are_boolean_values() {
    // IEC 61131-3 Ed.3 6.3.2 and Table 5 features 7 and 9.
    let source = r#"
FUNCTION Pass : BOOL
VAR_INPUT
    flag : BOOL;
END_VAR
Pass := flag;
END_FUNCTION

TYPE Flags :
STRUCT
    active : BOOL := 1;
    inactive : BOOL := 0;
END_STRUCT
END_TYPE

PROGRAM Main
VAR
    x : BOOL;
    initial : BOOL := 1;
    typed : BOOL := BOOL#1;
    flags : Flags;
    list : ARRAY[1..2] OF BOOL := [1, 0];
    on_flag : BOOL;
    off_flag : BOOL;
    first : BOOL;
    second : BOOL;
    same : BOOL;
    passed : BOOL;
    cleared : BOOL := TRUE;
END_VAR
x := 1;
same := x = 1;
passed := Pass(flag := 1);
on_flag := flags.active;
off_flag := flags.inactive;
first := list[1];
second := list[2];
cleared := 0;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);

    for (name, expected) in [
        ("x", true),
        ("initial", true),
        ("typed", true),
        ("on_flag", true),
        ("off_flag", false),
        ("first", true),
        ("second", false),
        ("same", true),
        ("passed", true),
        ("cleared", false),
    ] {
        assert_eq!(
            harness.get_output(name),
            Some(Value::Bool(expected)),
            "{name}"
        );
    }
}
