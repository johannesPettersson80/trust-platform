mod common;
use common::*;

fn accepted(source: &str) {
    let parsed = parse(source);
    assert!(
        parsed.ok(),
        "expected string-index syntax to parse for semantic validation: {:?}",
        parsed.errors()
    );
}

fn rejected(source: &str) {
    assert!(
        !parse(source).ok(),
        "expected malformed string-index syntax"
    );
}

#[test]
fn string_index_parser_accepts_read_expression() {
    accepted("PROGRAM Main\nVAR text : STRING[8]; ch : CHAR; END_VAR\nch := text[1];\nEND_PROGRAM");
}

#[test]
fn string_index_parser_accepts_assignment_target() {
    accepted("PROGRAM Main\nVAR text : WSTRING[8]; END_VAR\ntext[2] := \"Z\";\nEND_PROGRAM");
}

#[test]
fn string_index_parser_accepts_computed_integer_expression() {
    accepted(
        "PROGRAM Main\nVAR text : STRING[8]; index : INT; ch : CHAR; END_VAR\nch := text[index + 1];\nEND_PROGRAM",
    );
}

#[test]
fn string_index_parser_accepts_nested_structure_and_array_base() {
    accepted(
        r#"
TYPE Packet : STRUCT labels : ARRAY[0..1] OF STRING[8]; END_STRUCT; END_TYPE
PROGRAM Main
VAR packet : Packet; ch : CHAR; END_VAR
packet.labels[1][2] := 'Z';
ch := packet.labels[0][1];
END_PROGRAM
"#,
    );
}

#[test]
fn string_index_parser_accepts_dereferenced_base() {
    accepted(
        "PROGRAM Main\nVAR text : STRING[8]; text_ptr : POINTER TO STRING[8]; ch : CHAR; END_VAR\ntext_ptr := ADR(text); ch := text_ptr^[1];\nEND_PROGRAM",
    );
}

#[test]
fn string_index_parser_preserves_index_expression_node() {
    let parsed = parse(
        "PROGRAM Main\nVAR text : STRING[8]; ch : CHAR; END_VAR\nch := text[2];\nEND_PROGRAM",
    );
    assert!(parsed.ok(), "{:?}", parsed.errors());
    assert!(parsed.syntax().descendants().any(|node| {
        node.kind() == SyntaxKind::IndexExpr && node.text().to_string().trim() == "text[2]"
    }));
}

#[test]
fn string_index_parser_rejects_missing_index() {
    rejected("PROGRAM Main\nVAR text : STRING[8]; ch : CHAR; END_VAR\nch := text[];\nEND_PROGRAM");
}

#[test]
fn string_index_parser_rejects_trailing_comma() {
    rejected(
        "PROGRAM Main\nVAR text : STRING[8]; ch : CHAR; END_VAR\nch := text[1,];\nEND_PROGRAM",
    );
}

#[test]
fn string_index_parser_rejects_missing_closing_bracket() {
    rejected("PROGRAM Main\nVAR text : STRING[8]; ch : CHAR; END_VAR\nch := text[1;\nEND_PROGRAM");
}

#[test]
fn string_index_parser_rejects_empty_second_index() {
    rejected(
        "PROGRAM Main\nVAR text : STRING[8]; ch : CHAR; END_VAR\nch := text[1,,2];\nEND_PROGRAM",
    );
}
