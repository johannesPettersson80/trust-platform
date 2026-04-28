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
fn test_type_level_named_aggregate_defaults() {
    let source = r#"TYPE
    StepCfg : STRUCT
        cyl : INT;
        ext : BOOL;
    END_STRUCT;
    DefaultStep : StepCfg := (cyl := 1, ext := TRUE);
    StepArray : ARRAY[1..2] OF StepCfg := [(cyl := 2, ext := FALSE), (cyl := 3, ext := TRUE)];
END_TYPE"#;
    let parsed = parse(source);
    assert!(
        parsed.ok(),
        "expected TYPE-level aggregate defaults to parse, got: {:?}",
        parsed.errors()
    );
    let syntax = parsed.syntax();
    let aggregate_count = syntax
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::InitializerList)
        .count();
    assert_eq!(
        aggregate_count,
        3,
        "expected TYPE alias and array element aggregate initializers:\n{}",
        snapshot_parse(source)
    );
}

#[test]
fn test_type_level_defaults_cover_directly_derived_shapes() {
    let source = r#"TYPE
    Limited : INT := 100;
    IntArray : ARRAY[1..3] OF INT := [1, 2, 3];
    StructAlias : StepCfg := (cyl := 1, ext := TRUE);
    StructArray : ARRAY[1..1] OF StepCfg := [(cyl := 2, ext := FALSE)];
    StepCfg : STRUCT
        cyl : INT;
        ext : BOOL;
    END_STRUCT;
END_TYPE"#;
    let parsed = parse(source);
    assert!(
        parsed.ok(),
        "expected all TYPE-level default shapes to parse, got: {:?}",
        parsed.errors()
    );
    let syntax = parsed.syntax();
    assert_eq!(
        syntax
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::InitializerList)
            .count(),
        2,
        "expected the struct alias and array-of-struct defaults to use InitializerList"
    );
    assert_eq!(
        syntax
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::ArrayInitializer)
            .count(),
        2,
        "expected scalar and struct array defaults to use ArrayInitializer"
    );
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
