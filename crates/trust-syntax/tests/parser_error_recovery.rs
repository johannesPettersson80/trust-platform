mod common;
use common::*;

// Error Recovery
#[test]
fn test_invalid_signed_based_typed_literal() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    x := INT#-16#FF;
END_PROGRAM"#
    ));
}

#[test]
fn test_missing_end_program() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    x := 1;
"#
    ));
}

#[test]
fn test_missing_end_test_program() {
    insta::assert_snapshot!(snapshot_parse(
        r#"TEST_PROGRAM TestSuite
    x := 1;
"#
    ));
}

#[test]
fn test_missing_end_if() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
IF x > 0 THEN
    y := 1;

END_PROGRAM"#
    ));
}

#[test]
fn test_missing_then() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
IF x > 0
    y := 1;
END_IF;
END_PROGRAM"#
    ));
}

#[test]
fn test_unexpected_token() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    @@@ invalid @@@
END_PROGRAM"#
    ));
}

#[test]
fn test_missing_semicolon() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    x := 1
    y := 2;
END_PROGRAM"#
    ));
}

#[test]
fn test_hash_without_identifier() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    # := 1;
END_PROGRAM"#
    ));
}

#[test]
fn test_deep_unary_expression_is_bounded() {
    let source = format!("PROGRAM Test\n    x := {}1;\nEND_PROGRAM", "+".repeat(2048));

    let parsed = parse(&source);
    assert!(!parsed.ok());
    assert!(parsed.errors().iter().any(|err| err
        .message
        .contains("expression nesting exceeds parser limit")));
}

#[test]
fn test_deep_parenthesized_expression_is_bounded() {
    let source = format!(
        "PROGRAM Test\n    x := {}1{};\nEND_PROGRAM",
        "(".repeat(1500),
        ")".repeat(1500)
    );

    let parsed = parse(&source);
    assert!(!parsed.ok());
    assert!(parsed.errors().iter().any(|err| err
        .message
        .contains("expression nesting exceeds parser limit")));
}

#[test]
fn malformed_control_flow_delimiters_are_diagnosed() {
    let cases = [
        (
            "CASE without OF",
            "CASE x\n1: x := 2;\nEND_CASE",
            "expected OF",
        ),
        (
            "CASE branch without colon",
            "CASE x OF\n1 x := 2;\nEND_CASE",
            "expected ':' after CASE label",
        ),
        (
            "ELSIF without THEN",
            "IF x = 0 THEN\nx := 1;\nELSIF x = 1\nx := 2;\nEND_IF",
            "expected THEN",
        ),
        (
            "FOR without control variable",
            "FOR := 0 TO 2 DO\nx := x + 1;\nEND_FOR",
            "expected FOR control variable",
        ),
        (
            "FOR without assignment",
            "FOR i TO 2 DO\nx := x + 1;\nEND_FOR",
            "expected ':=' after FOR control variable",
        ),
        (
            "FOR without TO",
            "FOR i := 0 DO\nx := x + 1;\nEND_FOR",
            "expected TO",
        ),
        (
            "FOR without DO",
            "FOR i := 0 TO 2\nx := x + 1;\nEND_FOR",
            "expected DO",
        ),
        (
            "WHILE without DO",
            "WHILE x < 2\nx := x + 1;\nEND_WHILE",
            "expected DO",
        ),
        (
            "REPEAT without UNTIL",
            "REPEAT\nx := x + 1;\nEND_REPEAT",
            "expected UNTIL",
        ),
    ];

    let mut failures = Vec::new();
    for (label, body, expected) in cases {
        let source = format!("PROGRAM Test\nVAR x, i : INT; END_VAR\n{body}\nEND_PROGRAM");
        let parsed = parse(&source);
        if parsed.ok() {
            failures.push(format!(
                "{label} was accepted as a valid partial construct:\n{}",
                parsed.syntax()
            ));
        } else if !parsed
            .errors()
            .iter()
            .any(|error| error.message.contains(expected))
        {
            failures.push(format!(
                "{label} did not report {expected:?}; got {:?}",
                parsed.errors()
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
