mod common;
use common::*;

fn accepted(source: &str) {
    let parsed = parse(source);
    assert!(
        parsed.ok(),
        "expected textual ACTION analysis source to parse: {:?}",
        parsed.errors()
    );
}

fn rejected(source: &str) {
    let parsed = parse(source);
    assert!(
        !parsed.ok(),
        "expected textual ACTION analysis source to be rejected"
    );
}

#[test]
fn action_parser_accepts_empty_program_action_with_required_colon() {
    accepted("PROGRAM Main\nACTION Reset:\nEND_ACTION\nEND_PROGRAM");
}

#[test]
fn action_parser_accepts_function_block_action_with_required_colon() {
    accepted("FUNCTION_BLOCK Controller\nACTION Reset:\nEND_ACTION\nEND_FUNCTION_BLOCK");
}

#[test]
fn action_parser_accepts_multiple_actions_in_one_owner() {
    accepted(
        r#"
PROGRAM Main
ACTION Start:
END_ACTION
ACTION Stop:
END_ACTION
END_PROGRAM
"#,
    );
}

#[test]
fn action_parser_accepts_same_action_name_in_different_owners() {
    accepted(
        r#"
PROGRAM First
ACTION Reset:
END_ACTION
END_PROGRAM
PROGRAM Second
ACTION Reset:
END_ACTION
END_PROGRAM
"#,
    );
}

#[test]
fn action_parser_accepts_case_insensitive_delimiters() {
    accepted("pRoGrAm Main\naCtIoN Reset:\neNd_AcTiOn\neNd_PrOgRaM");
}

#[test]
fn action_parser_accepts_owner_variable_with_same_name() {
    accepted(
        r#"
PROGRAM Main
VAR
    Reset : BOOL;
END_VAR
ACTION Reset:
    Reset := FALSE;
END_ACTION
END_PROGRAM
"#,
    );
}

#[test]
fn action_parser_accepts_complete_st_statement_body() {
    accepted(
        r#"
PROGRAM Main
VAR
    i : INT;
    enabled : BOOL;
END_VAR
ACTION Work:
Start:
    IF enabled THEN
        FOR i := 0 TO 3 DO
            IF i = 2 THEN CONTINUE; END_IF;
        END_FOR;
    ELSE
        WHILE i > 0 DO
            i := i - 1;
            IF i = 1 THEN EXIT; END_IF;
        END_WHILE;
    END_IF;
    CASE i OF
        0: i := 1;
        ELSE i := 2;
    END_CASE;
    JMP Start;
END_ACTION
END_PROGRAM
"#,
    );
}

#[test]
fn action_parser_keeps_each_action_body_in_its_own_node() {
    let parsed = parse(
        r#"
PROGRAM Main
ACTION First:
    first := 1;
END_ACTION
ACTION Second:
    second := 2;
END_ACTION
END_PROGRAM
"#,
    );
    assert!(parsed.ok(), "{:?}", parsed.errors());
    let actions = parsed
        .syntax()
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::Action)
        .collect::<Vec<_>>();
    assert_eq!(actions.len(), 2);
    for action in actions {
        assert_eq!(
            action
                .children()
                .filter(|node| node.kind() == SyntaxKind::StmtList)
                .count(),
            1
        );
    }
}

#[test]
fn action_parser_accepts_owner_statements_around_analyzed_actions() {
    accepted(
        r#"
PROGRAM Main
VAR value : INT; END_VAR
value := 1;
ACTION Reset:
    value := 0;
END_ACTION
value := 2;
END_PROGRAM
"#,
    );
}

#[test]
fn action_parser_rejects_missing_required_colon() {
    rejected("PROGRAM Main\nACTION Reset\nEND_ACTION\nEND_PROGRAM");
}

#[test]
fn action_parser_rejects_semicolon_instead_of_required_colon() {
    rejected("PROGRAM Main\nACTION Reset;\nEND_ACTION\nEND_PROGRAM");
}

#[test]
fn action_parser_rejects_double_colon_after_name() {
    rejected("PROGRAM Main\nACTION Reset::\nEND_ACTION\nEND_PROGRAM");
}

#[test]
fn action_parser_rejects_missing_name() {
    rejected("PROGRAM Main\nACTION :\nEND_ACTION\nEND_PROGRAM");
}

#[test]
fn action_parser_rejects_reserved_keyword_as_name() {
    rejected("PROGRAM Main\nACTION PROGRAM:\nEND_ACTION\nEND_PROGRAM");
}

#[test]
fn action_parser_rejects_missing_end_action() {
    rejected("PROGRAM Main\nACTION Reset:\nvalue := 0;\nEND_PROGRAM");
}

#[test]
fn action_parser_rejects_file_scope_declaration() {
    rejected("ACTION Reset:\nEND_ACTION");
}

#[test]
fn action_parser_rejects_namespace_scope_declaration() {
    rejected("NAMESPACE Plant\nACTION Reset:\nEND_ACTION\nEND_NAMESPACE");
}

#[test]
fn action_parser_rejects_function_owned_declaration() {
    rejected("FUNCTION Calculate : INT\nACTION Reset:\nEND_ACTION\nCalculate := 0;\nEND_FUNCTION");
}

#[test]
fn action_parser_rejects_class_owned_declaration() {
    rejected("CLASS Controller\nACTION Reset:\nEND_ACTION\nEND_CLASS");
}

#[test]
fn action_parser_rejects_method_owned_declaration() {
    rejected("CLASS Controller\nMETHOD Run\nACTION Reset:\nEND_ACTION\nEND_METHOD\nEND_CLASS");
}

#[test]
fn action_parser_rejects_property_owned_declaration() {
    rejected(
        r#"
CLASS Controller
PROPERTY Value : INT
GET
ACTION Reset:
END_ACTION
END_GET
END_PROPERTY
END_CLASS
"#,
    );
}

#[test]
fn action_parser_rejects_nested_action_declaration() {
    rejected(
        r#"
PROGRAM Main
ACTION Outer:
    ACTION Inner:
    END_ACTION
END_ACTION
END_PROGRAM
"#,
    );
}

#[test]
fn action_parser_rejects_action_local_var_block() {
    rejected(
        r#"
PROGRAM Main
ACTION Reset:
VAR
    value : INT;
END_VAR
END_ACTION
END_PROGRAM
"#,
    );
}

#[test]
fn action_parser_rejects_textual_step_syntax_in_action_body() {
    rejected(
        r#"
PROGRAM Main
ACTION Reset:
STEP Waiting:
END_STEP
END_ACTION
END_PROGRAM
"#,
    );
}

#[test]
fn action_parser_rejects_textual_transition_syntax_in_action_body() {
    rejected(
        r#"
PROGRAM Main
ACTION Reset:
TRANSITION FROM Waiting TO Running:
    := TRUE;
END_TRANSITION
END_ACTION
END_PROGRAM
"#,
    );
}
