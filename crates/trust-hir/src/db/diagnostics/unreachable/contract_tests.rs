use super::*;

use trust_syntax::parser::parse;

fn syntax(source: &str) -> SyntaxNode {
    parse(source).syntax()
}

fn warnings(source: &str) -> Vec<Diagnostic> {
    let root = syntax(source);
    let mut diagnostics = DiagnosticBuilder::new();
    check_unreachable_statements(&root, &mut diagnostics);
    diagnostics
        .finish()
        .into_iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::UnreachableCode)
        .collect()
}

fn first_expression(source: &str) -> SyntaxNode {
    syntax(&format!(
        "PROGRAM Main\nVAR x : BOOL; END_VAR\nIF {source} THEN END_IF;\nEND_PROGRAM\n"
    ))
    .descendants()
    .find(|node| is_expression_kind(node.kind()))
    .expect("expression")
}

#[test]
fn boolean_constant_folder_handles_literals_case_and_parentheses() {
    for (source, expected) in [
        ("TRUE", Some(true)),
        ("false", Some(false)),
        ("(((TRUE)))", Some(true)),
        ("((FALSE))", Some(false)),
    ] {
        assert_eq!(
            const_bool_expr(&first_expression(source)),
            expected,
            "{source}"
        );
    }
}

#[test]
fn boolean_constant_folder_handles_not_and_binary_operators() {
    for (source, expected) in [
        ("NOT FALSE", Some(true)),
        ("NOT TRUE", Some(false)),
        ("TRUE AND FALSE", Some(false)),
        ("TRUE OR FALSE", Some(true)),
        ("TRUE XOR TRUE", Some(false)),
        ("FALSE XOR TRUE", Some(true)),
        ("NOT (TRUE AND FALSE)", Some(true)),
    ] {
        assert_eq!(
            const_bool_expr(&first_expression(source)),
            expected,
            "{source}"
        );
    }
}

#[test]
fn boolean_constant_folder_refuses_names_calls_comparisons_and_numeric_literals() {
    for source in ["x", "Check()", "1 = 1", "1"] {
        assert_eq!(const_bool_expr(&first_expression(source)), None, "{source}");
    }
}

#[test]
fn return_marks_every_following_sibling_statement_unreachable() {
    let diagnostics = warnings(
        r#"
FUNCTION Fn : INT
VAR x : INT; END_VAR
RETURN;
x := 1;
x := 2;
END_FUNCTION
"#,
    );

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.message == "unreachable statement"));
    assert!(diagnostics[0].range.start() < diagnostics[1].range.start());
}

#[test]
fn exit_continue_and_jump_are_statement_list_terminators() {
    for terminator in ["EXIT;", "CONTINUE;", "JMP done;"] {
        let source =
            format!("PROGRAM Main\nVAR x : INT; END_VAR\n{terminator}\nx := 1;\nEND_PROGRAM\n");
        assert_eq!(warnings(&source).len(), 1, "{terminator}");
    }
}

#[test]
fn nested_terminator_does_not_mark_outer_following_statement_unreachable() {
    let diagnostics = warnings(
        r#"
PROGRAM Main
VAR x : INT; END_VAR
IF x = 0 THEN
    RETURN;
END_IF;
x := 1;
END_PROGRAM
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn false_if_condition_marks_each_then_statement_unreachable() {
    let diagnostics = warnings(
        r#"
PROGRAM Main
VAR x : INT; END_VAR
IF FALSE THEN
    x := 1;
    x := 2;
ELSE
    x := 3;
END_IF;
END_PROGRAM
"#,
    );

    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn false_elsif_marks_only_that_branch_unreachable() {
    let diagnostics = warnings(
        r#"
PROGRAM Main
VAR x : INT; flag : BOOL; END_VAR
IF flag THEN
    x := 1;
ELSIF FALSE THEN
    x := 2;
    x := 3;
ELSE
    x := 4;
END_IF;
END_PROGRAM
"#,
    );

    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn true_if_marks_all_later_elsif_and_else_statements_unreachable() {
    let diagnostics = warnings(
        r#"
PROGRAM Main
VAR x : INT; END_VAR
IF TRUE THEN
    x := 1;
ELSIF FALSE THEN
    x := 2;
ELSIF TRUE THEN
    x := 3;
ELSE
    x := 4;
END_IF;
END_PROGRAM
"#,
    );

    assert_eq!(diagnostics.len(), 3);
}

#[test]
fn true_elsif_after_unknown_condition_makes_every_later_branch_unreachable() {
    let diagnostics = warnings(
        r#"
PROGRAM Main
VAR x : INT; flag : BOOL; END_VAR
IF flag THEN
    x := 1;
ELSIF TRUE THEN
    x := 2;
ELSIF flag THEN
    x := 3;
ELSE
    x := 4;
END_IF;
END_PROGRAM
"#,
    );

    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn true_elsif_after_false_and_unknown_conditions_still_closes_chain() {
    let diagnostics = warnings(
        r#"
PROGRAM Main
VAR x : INT; flag : BOOL; END_VAR
IF FALSE THEN
    x := 1;
ELSIF flag THEN
    x := 2;
ELSIF TRUE THEN
    x := 3;
ELSE
    x := 4;
END_IF;
END_PROGRAM
"#,
    );

    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn all_false_chain_marks_each_branch_but_keeps_else_reachable() {
    let diagnostics = warnings(
        r#"
PROGRAM Main
VAR x : INT; END_VAR
IF FALSE THEN
    x := 1;
ELSIF NOT TRUE THEN
    x := 2;
ELSE
    x := 3;
END_IF;
END_PROGRAM
"#,
    );

    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn unknown_conditions_do_not_create_wrong_reason_unreachable_warning() {
    let diagnostics = warnings(
        r#"
PROGRAM Main
VAR x : INT; first : BOOL; second : BOOL; END_VAR
IF first THEN
    x := 1;
ELSIF second THEN
    x := 2;
ELSE
    x := 3;
END_IF;
END_PROGRAM
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn constant_binary_false_expression_marks_then_branch() {
    for condition in [
        "TRUE AND FALSE",
        "FALSE OR FALSE",
        "TRUE XOR TRUE",
        "NOT TRUE",
    ] {
        let source = format!(
            "PROGRAM Main\nVAR x : INT; END_VAR\nIF {condition} THEN\nx := 1;\nEND_IF;\nEND_PROGRAM\n"
        );
        assert_eq!(warnings(&source).len(), 1, "{condition}");
    }
}

#[test]
fn empty_unreachable_branch_creates_no_synthetic_warning() {
    let diagnostics = warnings(
        r#"
PROGRAM Main
IF FALSE THEN
ELSE
END_IF;
END_PROGRAM
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}
