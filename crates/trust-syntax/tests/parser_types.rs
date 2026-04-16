mod common;
use common::*;

// Type Declarations
#[test]
// IEC 61131-3 Ed.3 Table 11 (user-defined types)
fn test_type_alias() {
    insta::assert_snapshot!(snapshot_parse(
        r#"TYPE
    MyInt : INT;
    MyReal : REAL;
    MyRange : INT(0..100);
END_TYPE"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 11 (STRUCT type)
fn test_struct_type() {
    insta::assert_snapshot!(snapshot_parse(
        r#"TYPE
    Point : STRUCT
        x : REAL;
        y : REAL;
    END_STRUCT;
END_TYPE"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 11 (enumeration type)
fn test_enum_type() {
    insta::assert_snapshot!(snapshot_parse(
        r#"TYPE
    Color : (Red, Green, Blue);
    State : (Idle := 0, Running := 1, Stopped := 2);
    Defaulted : (Alpha, Beta) := Beta;
    Colors : DWORD
        (Red := 16#00FF0000,
         Green := 16#0000FF00,
         Blue := 16#000000FF)
        := Green;
END_TYPE"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 11 (ARRAY type)
fn test_array_type() {
    insta::assert_snapshot!(snapshot_parse(
        r#"TYPE
    IntArray : ARRAY[0..9] OF INT;
    Matrix : ARRAY[0..2, 0..2] OF REAL;
END_TYPE"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 12 (reference and pointer types)
fn test_pointer_type() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
VAR
    ptr : POINTER TO INT;
    ref_value : REF_TO REAL;
END_VAR
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 10 (STRING/WSTRING sizing)
fn test_string_type_with_length() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
VAR
    s1 : STRING[50];
    s2 : WSTRING[100];
END_VAR
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 10 (alternative STRING/WSTRING sizing syntax)
fn test_string_type_with_parenthesized_length() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
VAR CONSTANT
    Len : INT := 50;
END_VAR
VAR
    s1 : STRING(Len);
    s2 : WSTRING(Len + 10);
END_VAR
END_PROGRAM"#
    ));
}
