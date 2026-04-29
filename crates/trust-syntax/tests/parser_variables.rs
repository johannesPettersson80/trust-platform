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
fn test_var_with_array_initializer() {
    let parsed = parse(
        r#"PROGRAM Test
VAR
    values : ARRAY[1..3] OF INT := [1, 2, 3];
END_VAR
END_PROGRAM"#,
    );
    assert!(
        parsed.ok(),
        "expected array declaration initializer to parse, got: {:?}",
        parsed.errors()
    );
}

#[test]
fn test_var_with_named_aggregate_initializer() {
    let parsed = parse(
        r#"PROGRAM Test
VAR
    cfg : StepCfg := (cyl := 1, ext := TRUE);
END_VAR
END_PROGRAM"#,
    );
    assert!(
        parsed.ok(),
        "expected named aggregate declaration initializer to parse, got: {:?}",
        parsed.errors()
    );
    let syntax = parsed.syntax();
    let aggregate_count = syntax
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::InitializerList)
        .count();
    assert_eq!(
        aggregate_count,
        1,
        "syntax:\n{}",
        snapshot_parse(
            r#"PROGRAM Test
VAR
    cfg : StepCfg := (cyl := 1, ext := TRUE);
END_VAR
END_PROGRAM"#
        )
    );
}

#[test]
fn test_var_initializer_aggregate_shapes_and_recovery() {
    let source = r#"PROGRAM Test
VAR
    single : StepCfg := (cyl := 1);
    nested : OuterCfg := (inner := (cyl := 2, ext := TRUE));
    partial : StepCfg := (cyl := 3);
    arr : ARRAY[1..2] OF StepCfg := [(cyl := 4), (ext := TRUE)];
    repeated : ARRAY[1..3] OF StepCfg := [3((cyl := 5))];
    commented : StepCfg := (cyl := 6 (* comment *), ext := TRUE);
END_VAR
END_PROGRAM"#;
    let parsed = parse(source);
    assert!(
        parsed.ok(),
        "expected aggregate initializer shapes to parse, got: {:?}",
        parsed.errors()
    );
    let syntax = parsed.syntax();
    assert_eq!(
        syntax
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::InitializerList)
            .count(),
        8,
        "expected all nested, array, repeated, and commented aggregates to remain initializer lists:\n{}",
        snapshot_parse(source)
    );
}

#[test]
fn test_var_global_aggregate_initializer_parse() {
    let source = r#"VAR_GLOBAL
    cfg : StepCfg := (cyl := 1, ext := TRUE);
END_VAR"#;
    let parsed = parse(source);
    assert!(
        parsed.ok(),
        "expected file-scope VAR_GLOBAL aggregate initializer to parse, got: {:?}",
        parsed.errors()
    );
    assert!(parsed
        .syntax()
        .descendants()
        .any(|node| node.kind() == SyntaxKind::InitializerList));
}

#[test]
fn test_fb_instance_aggregate_initializer_parse() {
    let source = r#"PROGRAM Test
VAR
    timer : TON := (PT := T#1s);
END_VAR
END_PROGRAM"#;
    let parsed = parse(source);
    assert!(
        parsed.ok(),
        "expected FB instance aggregate initializer to parse, got: {:?}",
        parsed.errors()
    );
    assert!(parsed
        .syntax()
        .descendants()
        .any(|node| node.kind() == SyntaxKind::InitializerList));
}

#[test]
fn test_call_arguments_remain_call_arguments() {
    let parsed = parse(
        r#"PROGRAM Test
f(a := 1);
END_PROGRAM"#,
    );
    assert!(
        parsed.ok(),
        "expected named call argument to parse, got: {:?}",
        parsed.errors()
    );
    let syntax = parsed.syntax();
    assert!(
        syntax
            .descendants()
            .any(|node| node.kind() == SyntaxKind::CallExpr),
        "expected CallExpr, got:\n{}",
        snapshot_parse(
            r#"PROGRAM Test
f(a := 1);
END_PROGRAM"#
        )
    );
    assert!(
        !syntax
            .descendants()
            .any(|node| node.kind() == SyntaxKind::InitializerList),
        "function-call arguments must not be parsed as aggregate initializers:\n{}",
        snapshot_parse(
            r#"PROGRAM Test
f(a := 1);
END_PROGRAM"#
        )
    );
}

#[test]
fn test_initializer_parser_is_not_used_for_enum_values_or_calls() {
    let source = r#"TYPE
    State : (Idle := 0, Running := 1);
END_TYPE

PROGRAM Test
f(a := 1);
END_PROGRAM"#;
    let parsed = parse(source);
    assert!(
        parsed.ok(),
        "expected enum values and named call args to parse, got: {:?}",
        parsed.errors()
    );
    assert!(
        !parsed
            .syntax()
            .descendants()
            .any(|node| node.kind() == SyntaxKind::InitializerList),
        "initializer lists must not be emitted for enum value assignments or function-call arguments:\n{}",
        snapshot_parse(source)
    );
}

#[test]
fn test_positional_and_empty_aggregate_recovery_is_bounded() {
    const POSITIONAL_MESSAGE: &str =
        "positional struct initializers are not supported; use named field initializers";

    for (label, initializer) in [
        ("integer", "(1, 2)"),
        ("bool", "(TRUE, FALSE)"),
        ("identifier", "(MyConst, 5)"),
        ("string", "('a', 'b')"),
    ] {
        let positional = parse(&format!(
            r#"PROGRAM Test
VAR
    cfg : StepCfg := {initializer};
END_VAR
END_PROGRAM"#
        ));
        assert_eq!(
            positional.errors().len(),
            1,
            "{label} positional aggregate should produce one targeted diagnostic, got: {:?}",
            positional.errors()
        );
        assert_eq!(
            positional.errors()[0].message,
            POSITIONAL_MESSAGE,
            "{label} positional aggregate should use locked wording"
        );
    }

    let empty = parse(
        r#"PROGRAM Test
VAR
    cfg : Empty := ();
END_VAR
END_PROGRAM"#,
    );
    assert_eq!(
        empty.errors().len(),
        1,
        "empty aggregate should produce one targeted diagnostic, got: {:?}",
        empty.errors()
    );

    let malformed = parse(
        r#"PROGRAM Test
VAR
    a : StepCfg := (cyl := );
    b : StepCfg := (ext := );
    c : StepCfg := (missing := );
END_VAR
END_PROGRAM"#,
    );
    assert_eq!(
        malformed.errors().len(),
        3,
        "three malformed initializer declarations should produce one diagnostic each, got: {:?}",
        malformed.errors()
    );

    for (label, source) in [
        (
            "end_var_boundary",
            r#"PROGRAM Test
VAR
    cfg : StepCfg := (cyl :=
END_VAR
END_PROGRAM"#,
        ),
        (
            "eof_boundary",
            "PROGRAM Test\nVAR\n    cfg : StepCfg := (cyl :=",
        ),
    ] {
        let parsed = parse(source);
        assert!(
            parsed
                .errors()
                .iter()
                .any(|error| error.message == "expected aggregate initializer value"),
            "{label} should report the missing aggregate initializer value at the declaration boundary, got: {:?}",
            parsed.errors()
        );
    }
}

#[test]
fn test_positional_initializer_recovery_preserves_declaration_boundaries() {
    const POSITIONAL_MESSAGE: &str =
        "positional struct initializers are not supported; use named field initializers";

    for (label, initializer) in [
        ("missing_close", "(TRUE, FALSE"),
        ("nested_positional", "(inner := (TRUE, FALSE))"),
        ("nested_missing_close", "(inner := (MyConst, 5)"),
        ("array_nested", "([1, 2], [3, 4])"),
    ] {
        let source = format!(
            r#"PROGRAM Test
VAR
    bad : StepCfg := {initializer};
    next : INT := 1;
END_VAR
END_PROGRAM"#
        );
        let parsed = parse(&source);
        assert!(
            parsed
                .errors()
                .iter()
                .any(|error| error.message == POSITIONAL_MESSAGE),
            "{label} should emit the locked positional diagnostic, got: {:?}",
            parsed.errors()
        );
        assert_eq!(
            parsed
                .syntax()
                .descendants()
                .filter(|node| node.kind() == SyntaxKind::VarDecl)
                .count(),
            2,
            "{label} recovery must not consume the following declaration:\n{}",
            snapshot_parse(&source)
        );
    }
}

#[test]
fn test_initializer_recovery_property_smoke_for_generated_positional_shapes() {
    const POSITIONAL_MESSAGE: &str =
        "positional struct initializers are not supported; use named field initializers";
    let atoms = [
        "1", "TRUE", "MyConst", "'a'", "(1 + 2)", "[1, 2]", "F(1, 2)",
    ];

    for left in atoms {
        for right in atoms {
            let source = format!(
                r#"PROGRAM Test
VAR
    bad : StepCfg := ({left} (* trivia *), {right});
    next : INT := 1;
END_VAR
END_PROGRAM"#
            );
            let parsed = parse(&source);
            assert!(
                parsed
                    .errors()
                    .iter()
                    .any(|error| error.message == POSITIONAL_MESSAGE),
                "generated shape ({left}, {right}) should emit the locked positional diagnostic, got: {:?}",
                parsed.errors()
            );
            assert!(
                parsed.errors().len() <= 2,
                "generated shape ({left}, {right}) should stay bounded, got: {:?}\n{}",
                parsed.errors(),
                snapshot_parse(&source)
            );
            assert_eq!(
                parsed
                    .syntax()
                    .descendants()
                    .filter(|node| node.kind() == SyntaxKind::VarDecl)
                    .count(),
                2,
                "generated shape ({left}, {right}) consumed the following declaration:\n{}",
                snapshot_parse(&source)
            );
        }
    }
}

#[test]
fn test_var_with_partial_array_initializer() {
    let parsed = parse(
        r#"PROGRAM Test
VAR
    values : ARRAY[1..5] OF INT := [1, 2];
END_VAR
END_PROGRAM"#,
    );
    assert!(
        parsed.ok(),
        "expected partial array declaration initializer to parse, got: {:?}",
        parsed.errors()
    );
}

#[test]
fn test_var_with_repetition_array_initializer() {
    let parsed = parse(
        r#"PROGRAM Test
VAR
    values : ARRAY[1..6] OF INT := [3(1, 2)];
END_VAR
END_PROGRAM"#,
    );
    assert!(
        parsed.ok(),
        "expected repetition-count array initializer to parse, got: {:?}",
        parsed.errors()
    );
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
