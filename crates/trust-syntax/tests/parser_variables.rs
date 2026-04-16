mod common;
use common::*;

// Variable Declarations
#[test]
// IEC 61131-3 Ed.3 Tables 13-14 (variable declarations)
fn test_var_block_types() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
VAR
    local : INT;
END_VAR
VAR_INPUT
    input : BOOL;
END_VAR
VAR_OUTPUT
    output : REAL;
END_VAR
VAR_IN_OUT
    inout : STRING;
END_VAR
VAR_TEMP
    temp : DINT;
END_VAR
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 13 (variable qualifiers)
fn test_var_modifiers() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
VAR CONSTANT
    PI : REAL := 3.14159;
END_VAR
VAR RETAIN
    counter : INT;
END_VAR
VAR PERSISTENT
    settings : INT;
END_VAR
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 16 (direct variable addressing)
fn test_var_at_address() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
VAR
    input AT %IB0 : BYTE;
    output AT %QW10 : WORD;
END_VAR
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 16 (direct variable addressing)
fn test_var_at_wildcard_address() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
VAR
    input AT %I* : BOOL;
END_VAR
END_PROGRAM"#
    ));
}

#[test]
fn test_var_with_initializer() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
VAR
    x : INT := 10;
    y : REAL := 3.14;
    s : STRING := 'hello';
END_VAR
END_PROGRAM"#
    ));
}

#[test]
fn test_file_scope_var_global() {
    let parsed = parse(
        r#"VAR_GLOBAL
    g_Shared : INT := 0;
END_VAR"#,
    );
    assert!(
        parsed.ok(),
        "expected file-scope VAR_GLOBAL to parse, got: {:?}",
        parsed.errors()
    );
    let syntax = parsed.syntax();
    assert!(
        syntax
            .children()
            .any(|child| child.kind() == SyntaxKind::VarBlock),
        "expected root-level VarBlock, got:\n{}",
        snapshot_parse(
            r#"VAR_GLOBAL
    g_Shared : INT := 0;
END_VAR"#
        )
    );
}

#[test]
fn parse_array_star_in_var_input() {
    let parsed = parse(
        r#"FUNCTION_BLOCK FB
VAR_INPUT
    arr : ARRAY[*] OF BYTE;
END_VAR
END_FUNCTION_BLOCK"#,
    );
    assert!(
        parsed.ok(),
        "expected ARRAY[*] in VAR_INPUT to parse, got: {:?}",
        parsed.errors()
    );
}

#[test]
fn parse_array_star_in_var_in_out() {
    let parsed = parse(
        r#"FUNCTION_BLOCK FB
VAR_IN_OUT
    arr : ARRAY[*] OF BYTE;
END_VAR
END_FUNCTION_BLOCK"#,
    );
    assert!(
        parsed.ok(),
        "expected ARRAY[*] in VAR_IN_OUT to parse, got: {:?}",
        parsed.errors()
    );
}

#[test]
fn parse_array_star_in_pointer_to_array() {
    let parsed = parse(
        r#"FUNCTION_BLOCK FB
VAR_INPUT
    pt : POINTER TO ARRAY[*] OF BYTE;
END_VAR
END_FUNCTION_BLOCK"#,
    );
    assert!(
        parsed.ok(),
        "expected POINTER TO ARRAY[*] to parse, got: {:?}",
        parsed.errors()
    );
}
