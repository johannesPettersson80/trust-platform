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
