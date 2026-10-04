use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

#[test]
fn case_ranges_match_unsigned_and_bit_string_selectors() {
    let source = r#"
PROGRAM Main
VAR
    u : USINT := USINT#3;
    ud : UDINT := UDINT#70000;
    b : BYTE := BYTE#16#20;
    hit_u : INT;
    hit_ud : INT;
    hit_b : INT;
    hit_untyped : INT;
END_VAR
CASE u OF
    USINT#1..USINT#5: hit_u := 1;
ELSE
    hit_u := -1;
END_CASE
CASE ud OF
    UDINT#0..UDINT#65535: hit_ud := 1;
    UDINT#65536..UDINT#100000: hit_ud := 2;
END_CASE
CASE b OF
    BYTE#16#00..BYTE#16#1F: hit_b := 1;
    BYTE#16#20..BYTE#16#FF: hit_b := 2;
END_CASE
CASE u OF
    0..2: hit_untyped := 1;
    3..255: hit_untyped := 2;
END_CASE
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.cycle();

    assert_eq!(harness.get_output("hit_u"), Some(Value::Int(1)));
    assert_eq!(harness.get_output("hit_ud"), Some(Value::Int(2)));
    assert_eq!(harness.get_output("hit_b"), Some(Value::Int(2)));
    assert_eq!(harness.get_output("hit_untyped"), Some(Value::Int(2)));
}
