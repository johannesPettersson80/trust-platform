mod common;
use common::*;

// Literals
#[test]
// IEC 61131-3 Ed.3 Table 5 (integer literals)
fn test_integer_literals() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    x := 42;
    y := 16#FF;
    z := 2#1010;
    w := 8#77;
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 5 (real literals)
fn test_real_literals() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    x := 3.14;
    y := 1.0E10;
    z := 2.5e-3;
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Tables 6-7 (string literals)
fn test_string_literals() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    s1 := 'hello';
    s2 := "world";
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 5 (boolean literals)
fn test_boolean_literals() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    a := TRUE;
    b := FALSE;
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 5 (typed literals)
fn test_typed_literals() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    x := INT#100;
    y := REAL#3.14;
    z := BYTE#16#FF;
    w := INT#-123;
    v := REAL#+1.0E-3;
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 8 (duration literals)
fn test_time_literals() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    t1 := T#1h30m;
    t2 := TIME#500ms;
    t3 := LT#14.7s;
    t4 := LTIME#5m_30s_500ms_100.1us;
    t5 := T#-14ms;
    t6 := t#12h4m34ms230us400ns;
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 9 (date/time literals)
fn test_date_literals() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    d1 := D#2024-01-15;
    d2 := LDATE#2012-02-29;
    d3 := LD#1984-06-25;
    tod1 := TOD#14:30:00;
    tod2 := TIME_OF_DAY#15:36:55.36;
    tod3 := LTOD#15:36:55.360_227_400;
    tod4 := LTIME_OF_DAY#15:36:55.360_227_400;
    dt1 := DT#2024-01-15-14:30:00;
    dt2 := DATE_AND_TIME#1984-06-25-15:36:55.360227400;
    dt3 := LDT#1984-06-25-15:36:55.360_227_400;
    dt4 := LDATE_AND_TIME#1984-06-25-15:36:55.360_227_400;
END_PROGRAM"#
    ));
}

/// Specification 05 §5.2 (IEC 61131-3 Ed.3 Annex A, `Enum_Value : (Enum_Type_Name '#')? Identifier`):
/// the type of an enumerated value is a plain type name, so `PIDMode#Auto` parses and
/// `Control.PIDMode#Auto` does not; a namespace's type is named through `USING`, or under the CODESYS
/// profile the value with dots, `Control.PIDMode.Auto`, a field access to the parser. Catches a parser
/// that took a namespace before the type of a typed value, a form neither the standard nor CODESYS has.
#[test]
fn a_typed_enumeration_value_takes_no_namespace_before_its_type() {
    let program = |value: &str| {
        format!("PROGRAM Main\nVAR mode : INT; END_VAR\nmode := {value};\nEND_PROGRAM")
    };
    for accepted in ["PIDMode#Auto", "Control.PIDMode.Auto"] {
        let parsed = parse(&program(accepted));
        assert!(parsed.ok(), "{accepted}: {:?}", parsed.errors());
    }
    assert!(
        !parse(&program("Control.PIDMode#Auto")).ok(),
        "Control.PIDMode#Auto"
    );
}
