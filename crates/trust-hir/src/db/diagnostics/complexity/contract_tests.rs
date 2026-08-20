use super::*;

use crate::DiagnosticSeverity;
use trust_syntax::parser::parse;

fn pou(source: &str, kind: SyntaxKind) -> SyntaxNode {
    parse(source)
        .syntax()
        .descendants()
        .find(|node| node.kind() == kind)
        .expect("POU node")
}

fn repeated_ifs(count: usize) -> String {
    let mut body = String::new();
    for _ in 0..count {
        body.push_str("IF TRUE THEN x := x + 1; END_IF;\n");
    }
    format!(
        r#"
PROGRAM Main
VAR x : INT; END_VAR
{body}
END_PROGRAM
"#
    )
}

fn complexity_diagnostics(source: &str) -> Vec<Diagnostic> {
    let syntax = parse(source).syntax();
    let mut diagnostics = DiagnosticBuilder::new();
    check_cyclomatic_complexity(&syntax, &mut diagnostics);
    diagnostics.finish()
}

#[test]
fn straight_line_pou_has_base_complexity_one_and_no_points() {
    let program = pou(
        "PROGRAM Main\nVAR x : INT; END_VAR\nx := 1;\nEND_PROGRAM\n",
        SyntaxKind::Program,
    );

    assert_eq!(cyclomatic_complexity(&program), (1, Vec::new()));
}

#[test]
fn if_and_each_elsif_are_distinct_decision_points() {
    let program = pou(
        r#"
PROGRAM Main
IF a THEN
ELSIF b THEN
ELSIF c THEN
ELSE
END_IF;
END_PROGRAM
"#,
        SyntaxKind::Program,
    );
    let (complexity, points) = cyclomatic_complexity(&program);

    assert_eq!(complexity, 4);
    assert_eq!(points.len(), 3);
}

#[test]
fn each_case_branch_counts_but_case_container_and_else_do_not() {
    let program = pou(
        r#"
PROGRAM Main
VAR selector : INT; x : INT; END_VAR
CASE selector OF
    1: x := 1;
    2, 3: x := 2;
    4..6: x := 3;
ELSE
    x := 0;
END_CASE;
END_PROGRAM
"#,
        SyntaxKind::Program,
    );
    let (complexity, points) = cyclomatic_complexity(&program);

    assert_eq!(complexity, 4);
    assert_eq!(points.len(), 3);
}

#[test]
fn for_while_and_repeat_each_add_one_decision() {
    let program = pou(
        r#"
PROGRAM Main
VAR i : INT; END_VAR
FOR i := 0 TO 1 DO
END_FOR;
WHILE FALSE DO
END_WHILE;
REPEAT
UNTIL TRUE
END_REPEAT;
END_PROGRAM
"#,
        SyntaxKind::Program,
    );

    assert_eq!(cyclomatic_complexity(&program).0, 4);
}

#[test]
fn nested_method_decisions_are_excluded_from_function_block_owner() {
    let syntax = parse(
        r#"
FUNCTION_BLOCK Controller
IF TRUE THEN
END_IF;

METHOD Step
IF TRUE THEN
ELSIF FALSE THEN
END_IF;
END_METHOD
END_FUNCTION_BLOCK
"#,
    )
    .syntax();
    let function_block = syntax
        .descendants()
        .find(|node| node.kind() == SyntaxKind::FunctionBlock)
        .expect("function block");
    let method = syntax
        .descendants()
        .find(|node| node.kind() == SyntaxKind::Method)
        .expect("method");

    assert_eq!(cyclomatic_complexity(&function_block).0, 2);
    assert_eq!(cyclomatic_complexity(&method).0, 3);
}

#[test]
fn decision_points_remain_in_lexical_order() {
    let program = pou(
        r#"
PROGRAM Main
IF TRUE THEN END_IF;
WHILE FALSE DO END_WHILE;
FOR i := 0 TO 1 DO END_FOR;
END_PROGRAM
"#,
        SyntaxKind::Program,
    );
    let (_, points) = cyclomatic_complexity(&program);

    assert!(points
        .windows(2)
        .all(|pair| pair[0].start() < pair[1].start()));
}

#[test]
fn threshold_value_fifteen_does_not_warn() {
    let diagnostics = complexity_diagnostics(&repeated_ifs(14));

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != DiagnosticCode::HighComplexity),
        "complexity 15 must not warn: {diagnostics:?}"
    );
}

#[test]
fn first_value_above_threshold_warns_with_exact_value_and_owner() {
    let diagnostics = complexity_diagnostics(&repeated_ifs(15));
    let warning = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == DiagnosticCode::HighComplexity)
        .expect("high complexity warning");

    assert_eq!(warning.severity, DiagnosticSeverity::Warning);
    assert!(warning.message.contains("Cyclomatic complexity 16"));
    assert!(warning.message.contains("exceeds 15"));
    assert!(warning.message.contains("'Main'"));
}

#[test]
fn warning_retains_at_most_first_three_related_decision_points() {
    let source = repeated_ifs(20);
    let syntax = parse(&source).syntax();
    let program = syntax
        .descendants()
        .find(|node| node.kind() == SyntaxKind::Program)
        .expect("program");
    let (_, expected_points) = cyclomatic_complexity(&program);
    let mut builder = DiagnosticBuilder::new();
    check_cyclomatic_complexity(&syntax, &mut builder);
    let warning = builder
        .finish()
        .into_iter()
        .find(|diagnostic| diagnostic.code == DiagnosticCode::HighComplexity)
        .expect("warning");

    assert_eq!(warning.related.len(), MAX_RELATED_POINTS);
    assert_eq!(
        warning
            .related
            .iter()
            .map(|related| related.range)
            .collect::<Vec<_>>(),
        expected_points[..MAX_RELATED_POINTS]
    );
    assert!(warning
        .related
        .iter()
        .all(|related| related.message == "Decision point"));
}

#[test]
fn separate_pous_receive_independent_complexity_diagnostics() {
    let first_body = repeated_ifs(15)
        .replace("PROGRAM Main", "PROGRAM First")
        .replace("END_PROGRAM", "");
    let second_body = repeated_ifs(15)
        .replace("PROGRAM Main", "PROGRAM Second")
        .replace("END_PROGRAM", "");
    let source = format!("{first_body}\nEND_PROGRAM\n{second_body}\nEND_PROGRAM\n");
    let diagnostics = complexity_diagnostics(&source);
    let messages = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::HighComplexity)
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();

    assert_eq!(messages.len(), 2);
    assert!(messages.iter().any(|message| message.contains("'First'")));
    assert!(messages.iter().any(|message| message.contains("'Second'")));
}
