mod common;
use common::*;

fn accepted(source: &str) {
    let parsed = parse(source);
    assert!(
        parsed.ok(),
        "expected partial-access syntax to parse for semantic validation: {:?}",
        parsed.errors()
    );
}

fn rejected(source: &str) {
    assert!(
        !parse(source).ok(),
        "expected malformed partial-access syntax"
    );
}

#[test]
fn partial_access_parser_accepts_every_prefixed_selector_kind() {
    accepted(
        r#"
PROGRAM Main
VAR value : LWORD; bit_value : BOOL; byte_value : BYTE; word_value : WORD; dword_value : DWORD; END_VAR
bit_value := value.%X63;
byte_value := value.%B7;
word_value := value.%W3;
dword_value := value.%D1;
END_PROGRAM
"#,
    );
}

#[test]
fn partial_access_parser_accepts_unprefixed_bit_selector() {
    accepted(
        "PROGRAM Main\nVAR value : WORD; bit_value : BOOL; END_VAR\nbit_value := value.15;\nEND_PROGRAM",
    );
}

#[test]
fn partial_access_parser_accepts_case_insensitive_prefixes() {
    accepted(
        "PROGRAM Main\nVAR value : LWORD; bit_value : BOOL; byte_value : BYTE; word_value : WORD; dword_value : DWORD; END_VAR\nbit_value := value.%x0; byte_value := value.%b0; word_value := value.%w0; dword_value := value.%d0;\nEND_PROGRAM",
    );
}

#[test]
fn partial_access_parser_accepts_decimal_digit_separators() {
    accepted(
        "PROGRAM Main\nVAR value : LWORD; bit_value : BOOL; END_VAR\nbit_value := value.%X0_6_3;\nEND_PROGRAM",
    );
}

#[test]
fn partial_access_parser_accepts_partial_access_assignment_target() {
    accepted(
        "PROGRAM Main\nVAR value : DWORD; END_VAR\nvalue.%X31 := TRUE; value.%B3 := BYTE#16#AA; value.%W1 := WORD#16#1234;\nEND_PROGRAM",
    );
}

#[test]
fn partial_access_parser_accepts_nested_structure_and_array_bases() {
    accepted(
        r#"
TYPE Packet : STRUCT words : ARRAY[0..1] OF DWORD; END_STRUCT; END_TYPE
PROGRAM Main
VAR packet : Packet; bit_value : BOOL; END_VAR
packet.words[1].%X7 := TRUE;
bit_value := packet.words[0].%X0;
END_PROGRAM
"#,
    );
}

#[test]
fn partial_access_parser_accepts_dereferenced_base() {
    accepted(
        "PROGRAM Main\nVAR value : WORD; value_ptr : POINTER TO WORD; bit_value : BOOL; END_VAR\nvalue_ptr := ADR(value); bit_value := value_ptr^.%X0;\nEND_PROGRAM",
    );
}

#[test]
fn partial_access_parser_preserves_field_expression_nodes() {
    let parsed = parse(
        "PROGRAM Main\nVAR value : DWORD; byte_value : BYTE; END_VAR\nbyte_value := value.%B2;\nEND_PROGRAM",
    );
    assert!(parsed.ok(), "{:?}", parsed.errors());
    assert!(parsed.syntax().descendants().any(|node| {
        node.kind() == SyntaxKind::FieldExpr && node.text().to_string().trim() == "value.%B2"
    }));
}

#[test]
fn partial_access_parser_rejects_missing_selector_index() {
    rejected(
        "PROGRAM Main\nVAR value : WORD; bit_value : BOOL; END_VAR\nbit_value := value.%X;\nEND_PROGRAM",
    );
}

#[test]
fn partial_access_parser_rejects_signed_selector_index() {
    rejected(
        "PROGRAM Main\nVAR value : WORD; bit_value : BOOL; END_VAR\nbit_value := value.%X-1;\nEND_PROGRAM",
    );
}

#[test]
fn partial_access_parser_rejects_radix_selector_index() {
    rejected(
        "PROGRAM Main\nVAR value : WORD; bit_value : BOOL; END_VAR\nbit_value := value.%X16#0F;\nEND_PROGRAM",
    );
}

#[test]
fn partial_access_parser_rejects_identifier_selector_index() {
    rejected(
        "PROGRAM Main\nVAR value : WORD; bit_value : BOOL; index : INT; END_VAR\nbit_value := value.%Xindex;\nEND_PROGRAM",
    );
}

#[test]
fn partial_access_parser_rejects_parenthesized_selector_expression() {
    rejected(
        "PROGRAM Main\nVAR value : WORD; bit_value : BOOL; END_VAR\nbit_value := value.%X(1 + 1);\nEND_PROGRAM",
    );
}
