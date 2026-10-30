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

#[test]
fn mismatched_nested_terminator_preserves_outer_boundary_and_following_statement() {
    let source = r#"PROGRAM Test
VAR x : INT; END_VAR
IF x = 0 THEN
    WHILE x < 2 DO
        x := x + 1;
END_IF
x := 7;
END_PROGRAM"#;

    let parsed = parse(source);
    assert!(!parsed.ok(), "missing inner END_WHILE must be diagnosed");
    assert!(
        parsed
            .errors()
            .iter()
            .any(|error| error.message == "expected END_WHILE"),
        "expected the inner terminator diagnostic, got {:?}",
        parsed.errors()
    );
    assert!(
        parsed.errors().iter().all(|error| {
            error.message != "expected END_IF" && error.message != "expected END_PROGRAM"
        }),
        "recovery consumed an outer boundary: {:?}",
        parsed.errors()
    );
    assert_eq!(
        2,
        parsed
            .syntax()
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::AssignStmt)
            .count(),
        "the statement following the recovered IF must remain in the syntax tree"
    );
}

fn error_positions(parsed: &trust_syntax::parser::Parse) -> Vec<(&str, usize)> {
    parsed
        .errors()
        .iter()
        .map(|error| (error.message.as_str(), usize::from(error.range.start())))
        .collect()
}

// docs/specs/06-statements.md, malformed-source recovery: recovery stops at a statement
// boundary and leaves it to its owner, so a missing right-hand side is one error at the
// `;`. Catches the `;` consumed into the error node, which left the statement without its
// terminator and added "expected ';'" on the next line.
#[test]
fn missing_expression_reports_one_error_and_keeps_the_terminator() {
    let source = r#"PROGRAM Test
VAR Level, Next : INT; END_VAR
Level := ;
Next := 2;
END_PROGRAM"#;
    let semicolon = source.find("Level := ;").expect("assignment") + "Level := ".len();

    let parsed = parse(source);
    assert_eq!(
        error_positions(&parsed),
        vec![("expected expression", semicolon)]
    );

    let assignments = parsed
        .syntax()
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::AssignStmt)
        .collect::<Vec<_>>();
    assert_eq!(assignments.len(), 2, "the following statement must parse");
    assert!(
        assignments[0]
            .children_with_tokens()
            .any(|element| element.kind() == SyntaxKind::Semicolon),
        "the `;` must terminate the incomplete assignment"
    );
}

// docs/specs/06-statements.md, malformed-source recovery: a missing IF condition is one
// error, and THEN stays with the IF. Catches THEN consumed as the condition, which added
// "expected THEN" before the body.
#[test]
fn missing_if_condition_reports_one_error_and_keeps_then() {
    let source = r#"PROGRAM Test
VAR Level : INT; END_VAR
IF THEN
    Level := 1;
END_IF;
END_PROGRAM"#;
    let then = source.find("THEN").expect("THEN");

    let parsed = parse(source);
    assert_eq!(
        error_positions(&parsed),
        vec![("expected expression", then)]
    );

    let then_owner = parsed
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == SyntaxKind::KwThen)
        .and_then(|token| token.parent())
        .map(|node| node.kind());
    assert_eq!(
        then_owner,
        Some(SyntaxKind::IfStmt),
        "THEN must stay with the IF"
    );
}

// docs/specs/06-statements.md 3.1: recovery from a malformed actual stops at the call
// boundary. A missing positional actual is one error at the delimiter after it, a named actual
// without a value parses (an IEC project's analysis reports it at the same delimiter), and the
// `,` and `)` stay in the argument list. Catches the delimiters consumed into error nodes, which
// broke the call apart and added "expected )" and "expected ';'".
#[test]
fn missing_call_arguments_report_one_error_each_and_keep_the_delimiters() {
    let source = "PROGRAM Test\nRun(, Value := );\nEND_PROGRAM";
    let comma = source.find("(,").expect("call") + 1;

    let parsed = parse(source);
    assert_eq!(
        error_positions(&parsed),
        vec![("expected expression", comma)]
    );

    let arg_list = parsed
        .syntax()
        .descendants()
        .find(|node| node.kind() == SyntaxKind::ArgList)
        .expect("argument list");
    assert_eq!(
        arg_list
            .children_with_tokens()
            .filter(|element| matches!(element.kind(), SyntaxKind::Comma | SyntaxKind::RParen))
            .count(),
        2,
        "the `,` and `)` must stay in the argument list"
    );
}

// docs/specs/06-statements.md, malformed-source recovery: recovery advances past a stray
// token. A CASE branch starting with `)` has no label, and label recovery leaves the `)`
// to its owner, so the branch loop must skip it. Catches the CASE parser looping forever
// on the `)`, and recovery that swallows the next branch or END_CASE.
#[test]
fn case_branch_recovery_advances_past_a_delimiter_left_by_a_missing_label() {
    let source = r#"PROGRAM Test
VAR Level : INT; END_VAR
CASE Level OF
) 1: Level := 2;
END_CASE;
END_PROGRAM"#;
    let stray = source.find(") 1:").expect("stray delimiter");

    let parsed = parse(source);
    assert!(
        error_positions(&parsed).contains(&("expected expression", stray)),
        "the missing label must be diagnosed at the `)`, got {:?}",
        parsed.errors()
    );
    assert!(
        parsed.errors().iter().all(|error| {
            error.message != "expected END_CASE" && error.message != "expected END_PROGRAM"
        }),
        "recovery consumed an outer boundary: {:?}",
        parsed.errors()
    );
    assert_eq!(
        parsed
            .syntax()
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::AssignStmt)
            .filter(|node| node
                .parent()
                .is_some_and(|parent| parent.kind() == SyntaxKind::CaseBranch))
            .count(),
        1,
        "the branch after the stray `)` must parse"
    );
}

// Recovery from a missing expression leaves a boundary token (`;`, `)`, `,`, `THEN`, ...) to its owner, so every loop
// that parses expressions must still advance by itself. Catches a loop that waits on a token nobody consumes: the parse
// would never end. Each prefix of a program that uses every statement form, and the program with each word removed, must
// parse within the time limit.
#[test]
fn recovery_ends_on_every_prefix_and_every_removed_word() {
    let source = r#"PROGRAM Main
VAR x : INT; a : ARRAY[1..3] OF INT := [1, 2, 3]; s : Pair := (first := 1, second := 2); END_VAR
IF x > 0 THEN x := Add(a := 1, b := (x + 2) * 3); ELSIF x < 0 THEN x := -x; ELSE x := 0; END_IF;
CASE x OF 1, 2: x := 3; 4..5: x := 6; ELSE x := 7; END_CASE;
FOR x := 1 TO 10 BY 2 DO a[x] := x; END_FOR;
WHILE x < 10 DO x := x + 1; END_WHILE;
REPEAT x := x - 1; UNTIL x = 0 END_REPEAT;
s.first := a[1] + a[2];
END_PROGRAM
"#;
    for end in (0..=source.len()).filter(|end| source.is_char_boundary(*end)) {
        parses_in_time(&source[..end]);
    }
    let words: Vec<&str> = source.split_whitespace().collect();
    for removed in 0..words.len() {
        let mut kept = words.clone();
        kept.remove(removed);
        parses_in_time(&kept.join(" "));
    }
}

fn parses_in_time(text: &str) {
    let (done, finished) = std::sync::mpsc::channel();
    let input = text.to_string();
    std::thread::spawn(move || {
        let _ = parse(&input);
        let _ = done.send(());
    });
    assert!(
        finished
            .recv_timeout(std::time::Duration::from_secs(5))
            .is_ok(),
        "the parser did not finish on:\n{text}"
    );
}
