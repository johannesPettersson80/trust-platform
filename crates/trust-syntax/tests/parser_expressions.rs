mod common;
use common::*;

// Expressions
#[test]
// IEC 61131-3 Ed.3 Table 71 (arithmetic operators)
fn test_arithmetic_operators() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    x := 1 + 2 - 3 * 4 / 5 MOD 6;
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 71 (comparison operators)
fn test_comparison_operators() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    b := (x = 1) OR (x <> 2) OR (x < 3) OR (x <= 4) OR (x > 5) OR (x >= 6);
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 71 (logical operators)
fn test_logical_operators() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    b := a AND b OR c XOR d;
    c := NOT a;
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 71 (operator precedence)
fn test_operator_precedence() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    x := 1 + 2 * 3;
    y := (1 + 2) * 3;
    z := 2 ** 3 ** 2;
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 71 (unary operators)
fn test_unary_operators() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    x := -a;
    y := +b;
    z := NOT c;
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 71 (function call syntax)
fn test_function_call() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    x := MyFunc(1, 2, 3);
    y := Add(a := 1, b := 2);
END_PROGRAM"#
    ));
}

#[test]
fn test_time_builtin_call() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    t := TIME();
END_PROGRAM"#
    ));
}

#[test]
fn test_field_access() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    x := obj.field;
    y := obj.nested.deep;
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 71 (array indexing)
fn test_array_indexing() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    x := arr[0];
    y := matrix[i, j];
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 71 (dereference operator)
fn test_pointer_dereference() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    x := ptr^;
    y := ptr^^;
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 71 (ADR/SIZEOF standard operators)
fn test_adr_sizeof() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    ptr := ADR(x);
    size := SIZEOF(INT);
END_PROGRAM"#
    ));
}

#[test]
fn test_this_super() {
    insta::assert_snapshot!(snapshot_parse(
        r#"FUNCTION_BLOCK FB_Test
    METHOD DoWork
        THIS.value := 1;
        SUPER.DoWork();
    END_METHOD
END_FUNCTION_BLOCK"#
    ));
}

#[test]
fn test_siemens_hash_prefixed_locals() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    #temp := #input + #fb.value;
    #fb(Enable := #temp > 0);
END_PROGRAM"#
    ));
}
