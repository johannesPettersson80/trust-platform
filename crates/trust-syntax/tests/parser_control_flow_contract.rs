mod common;
use common::*;

fn assert_control_flow_parses(source: &str) {
    let parsed = parse(source);
    assert!(
        parsed.ok(),
        "expected control-flow source to parse, got: {:?}",
        parsed.errors()
    );
}

fn assert_control_flow_rejected(source: &str) {
    let parsed = parse(source);
    assert!(
        !parsed.ok(),
        "expected malformed control-flow source to be rejected"
    );
}

#[test]
fn control_flow_parser_accepts_if_without_else() {
    assert_control_flow_parses("PROGRAM P\nIF TRUE THEN ; END_IF;\nEND_PROGRAM");
}

#[test]
fn control_flow_parser_accepts_ordered_elsif_and_else() {
    assert_control_flow_parses(
        "PROGRAM P\nIF FALSE THEN ; ELSIF TRUE THEN ; ELSE ; END_IF;\nEND_PROGRAM",
    );
}

#[test]
fn control_flow_parser_accepts_nested_if() {
    assert_control_flow_parses(
        "PROGRAM P\nIF TRUE THEN IF FALSE THEN ; END_IF; END_IF;\nEND_PROGRAM",
    );
}

#[test]
fn control_flow_parser_accepts_case_scalar_labels() {
    assert_control_flow_parses("PROGRAM P\nCASE x OF 1: ; 2: ; ELSE ; END_CASE;\nEND_PROGRAM");
}

#[test]
fn control_flow_parser_accepts_case_comma_labels() {
    assert_control_flow_parses("PROGRAM P\nCASE x OF 1, 3, 5: ; ELSE ; END_CASE;\nEND_PROGRAM");
}

#[test]
fn control_flow_parser_accepts_case_inclusive_range() {
    assert_control_flow_parses("PROGRAM P\nCASE x OF 1..10: ; ELSE ; END_CASE;\nEND_PROGRAM");
}

#[test]
fn control_flow_parser_accepts_case_without_else() {
    assert_control_flow_parses("PROGRAM P\nCASE x OF 1: ; END_CASE;\nEND_PROGRAM");
}

#[test]
fn control_flow_parser_accepts_for_with_implicit_step() {
    assert_control_flow_parses("PROGRAM P\nFOR i := 1 TO 3 DO ; END_FOR;\nEND_PROGRAM");
}

#[test]
fn control_flow_parser_accepts_for_with_positive_step() {
    assert_control_flow_parses("PROGRAM P\nFOR i := 1 TO 9 BY 2 DO ; END_FOR;\nEND_PROGRAM");
}

#[test]
fn control_flow_parser_accepts_for_with_negative_step() {
    assert_control_flow_parses("PROGRAM P\nFOR i := 9 TO 1 BY -2 DO ; END_FOR;\nEND_PROGRAM");
}

#[test]
fn control_flow_parser_accepts_while_with_continue_and_exit() {
    assert_control_flow_parses("PROGRAM P\nWHILE TRUE DO CONTINUE; EXIT; END_WHILE;\nEND_PROGRAM");
}

#[test]
fn control_flow_parser_accepts_repeat_with_continue_and_exit() {
    assert_control_flow_parses(
        "PROGRAM P\nREPEAT CONTINUE; EXIT; UNTIL TRUE END_REPEAT;\nEND_PROGRAM",
    );
}

#[test]
fn control_flow_parser_accepts_bare_return() {
    assert_control_flow_parses("FUNCTION F : INT\nF := 1; RETURN;\nEND_FUNCTION");
}

#[test]
fn control_flow_parser_accepts_trust_value_return_extension() {
    assert_control_flow_parses("FUNCTION F : INT\nRETURN INT#1;\nEND_FUNCTION");
}

#[test]
fn control_flow_parser_accepts_deeply_nested_iteration() {
    assert_control_flow_parses(
        "PROGRAM P\nFOR i := 1 TO 2 DO WHILE TRUE DO REPEAT EXIT; UNTIL TRUE END_REPEAT; EXIT; END_WHILE; END_FOR;\nEND_PROGRAM",
    );
}

#[test]
fn control_flow_parser_rejects_if_without_then() {
    assert_control_flow_rejected("PROGRAM P\nIF TRUE x := 1; END_IF;\nEND_PROGRAM");
}

#[test]
fn control_flow_parser_rejects_elsif_without_then() {
    assert_control_flow_rejected("PROGRAM P\nIF FALSE THEN ; ELSIF TRUE ; END_IF;\nEND_PROGRAM");
}

#[test]
fn control_flow_parser_rejects_if_without_end_if() {
    assert_control_flow_rejected("PROGRAM P\nIF TRUE THEN ;\nEND_PROGRAM");
}

#[test]
fn control_flow_parser_rejects_case_without_of() {
    assert_control_flow_rejected("PROGRAM P\nCASE x 1: ; END_CASE;\nEND_PROGRAM");
}

#[test]
fn control_flow_parser_rejects_case_label_without_colon() {
    assert_control_flow_rejected("PROGRAM P\nCASE x OF 1 x := 1; END_CASE;\nEND_PROGRAM");
}

#[test]
fn control_flow_parser_rejects_for_without_assignment_operator() {
    assert_control_flow_rejected("PROGRAM P\nFOR i 1 TO 3 DO ; END_FOR;\nEND_PROGRAM");
}

#[test]
fn control_flow_parser_rejects_for_without_to() {
    assert_control_flow_rejected("PROGRAM P\nFOR i := 1 3 DO ; END_FOR;\nEND_PROGRAM");
}

#[test]
fn control_flow_parser_rejects_while_without_do() {
    assert_control_flow_rejected("PROGRAM P\nWHILE TRUE ; END_WHILE;\nEND_PROGRAM");
}

#[test]
fn control_flow_parser_rejects_repeat_without_until() {
    assert_control_flow_rejected("PROGRAM P\nREPEAT ; END_REPEAT;\nEND_PROGRAM");
}

#[test]
fn control_flow_parser_rejects_return_expression_without_semicolon() {
    assert_control_flow_rejected("FUNCTION F : INT\nRETURN INT#1\nEND_FUNCTION");
}
