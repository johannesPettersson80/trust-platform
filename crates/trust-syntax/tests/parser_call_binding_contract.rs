mod common;
use common::*;

fn assert_call_parses(source: &str) {
    let parsed = parse(source);
    assert!(
        parsed.ok(),
        "expected call source to parse, got: {:?}",
        parsed.errors()
    );
}

fn assert_call_rejected(source: &str) {
    let parsed = parse(source);
    assert!(
        !parsed.ok(),
        "expected malformed call source to be rejected"
    );
}

#[test]
fn call_parser_accepts_empty_argument_list() {
    assert_call_parses("PROGRAM P\nRun();\nEND_PROGRAM");
}

#[test]
fn call_parser_accepts_single_positional_argument() {
    assert_call_parses("PROGRAM P\nRun(value);\nEND_PROGRAM");
}

#[test]
fn call_parser_accepts_multiple_positional_arguments() {
    assert_call_parses("PROGRAM P\nRun(first, second, third);\nEND_PROGRAM");
}

#[test]
fn call_parser_accepts_named_input_assignment() {
    assert_call_parses("PROGRAM P\nRun(value := source);\nEND_PROGRAM");
}

#[test]
fn call_parser_accepts_named_output_connection() {
    assert_call_parses("PROGRAM P\nRun(value => target);\nEND_PROGRAM");
}

#[test]
fn call_parser_accepts_named_in_out_assignment() {
    assert_call_parses("PROGRAM P\nRun(target := state);\nEND_PROGRAM");
}

#[test]
fn call_parser_accepts_reordered_formal_arguments() {
    assert_call_parses("PROGRAM P\nRun(third := c, first := a, second => b);\nEND_PROGRAM");
}

#[test]
fn call_parser_accepts_positional_prefix_and_formal_suffix() {
    assert_call_parses("PROGRAM P\nRun(first, second := value, third => out);\nEND_PROGRAM");
}

#[test]
fn call_parser_retains_positional_after_formal_for_semantic_diagnostic() {
    assert_call_parses("PROGRAM P\nRun(first := value, second);\nEND_PROGRAM");
}

#[test]
fn call_parser_accepts_named_en_and_eno() {
    assert_call_parses("PROGRAM P\nRun(EN := enabled, value := input, ENO => ok);\nEND_PROGRAM");
}

#[test]
fn call_parser_accepts_nested_call_actuals() {
    assert_call_parses(
        "PROGRAM P\nresult := Outer(first := Inner(value), second := Next());\nEND_PROGRAM",
    );
}

#[test]
fn call_parser_accepts_method_call_arguments() {
    assert_call_parses(
        "PROGRAM P\nresult := object.Execute(input := value, output => target);\nEND_PROGRAM",
    );
}

#[test]
fn call_parser_accepts_indexed_output_target() {
    assert_call_parses("PROGRAM P\nPair(left => values[0], right => values[index]);\nEND_PROGRAM");
}

#[test]
fn call_parser_accepts_selected_output_target() {
    assert_call_parses("PROGRAM P\nRead(value => state.member);\nEND_PROGRAM");
}

#[test]
fn call_parser_accepts_dereferenced_in_out_target() {
    assert_call_parses("PROGRAM P\nMutate(target := value_ptr^);\nEND_PROGRAM");
}

#[test]
fn call_parser_rejects_missing_input_expression() {
    assert_call_rejected("PROGRAM P\nRun(value := );\nEND_PROGRAM");
}

#[test]
fn call_parser_rejects_missing_output_target() {
    assert_call_rejected("PROGRAM P\nRun(value => );\nEND_PROGRAM");
}

#[test]
fn call_parser_rejects_missing_comma_between_actuals() {
    assert_call_rejected("PROGRAM P\nRun(first := a second := b);\nEND_PROGRAM");
}

#[test]
fn call_parser_rejects_leading_comma() {
    assert_call_rejected("PROGRAM P\nRun(, value);\nEND_PROGRAM");
}

#[test]
fn call_parser_rejects_trailing_double_comma() {
    assert_call_rejected("PROGRAM P\nRun(value,, target);\nEND_PROGRAM");
}

#[test]
fn call_parser_rejects_unclosed_argument_list() {
    assert_call_rejected("PROGRAM P\nRun(value := input;\nEND_PROGRAM");
}

#[test]
fn call_parser_rejects_missing_call_terminator() {
    assert_call_rejected("PROGRAM P\nRun(value := input)\nEND_PROGRAM");
}
