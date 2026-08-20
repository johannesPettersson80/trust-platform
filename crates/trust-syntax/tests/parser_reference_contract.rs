mod common;
use common::*;

fn assert_reference_parses(source: &str) {
    let parsed = parse(source);
    assert!(parsed.ok(), "expected parse success: {:?}", parsed.errors());
}

fn assert_reference_rejected(source: &str) {
    assert!(!parse(source).ok(), "expected malformed reference source");
}

#[test]
fn reference_parser_accepts_ref_to_elementary_type() {
    assert_reference_parses("PROGRAM P\nVAR r : REF_TO INT; END_VAR\nEND_PROGRAM");
}

#[test]
fn reference_parser_accepts_ref_to_user_type() {
    assert_reference_parses("TYPE S : STRUCT x : INT; END_STRUCT END_TYPE\nPROGRAM P\nVAR r : REF_TO S; END_VAR\nEND_PROGRAM");
}

#[test]
fn reference_parser_accepts_pointer_to_elementary_type() {
    assert_reference_parses("PROGRAM P\nVAR p : POINTER TO INT; END_VAR\nEND_PROGRAM");
}

#[test]
fn reference_parser_accepts_pointer_to_array_type() {
    assert_reference_parses(
        "PROGRAM P\nVAR p : POINTER TO ARRAY[0..3] OF BYTE; END_VAR\nEND_PROGRAM",
    );
}

#[test]
fn reference_parser_accepts_nested_pointer_type() {
    assert_reference_parses("PROGRAM P\nVAR p : POINTER TO POINTER TO INT; END_VAR\nEND_PROGRAM");
}

#[test]
fn reference_parser_accepts_ref_call() {
    assert_reference_parses(
        "PROGRAM P\nVAR x : INT; r : REF_TO INT; END_VAR\nr := REF(x);\nEND_PROGRAM",
    );
}

#[test]
fn reference_parser_accepts_adr_call() {
    assert_reference_parses(
        "PROGRAM P\nVAR x : INT; p : POINTER TO INT; END_VAR\np := ADR(x);\nEND_PROGRAM",
    );
}

#[test]
fn reference_parser_accepts_dereference_read() {
    assert_reference_parses(
        "PROGRAM P\nVAR p : POINTER TO INT; x : INT; END_VAR\nx := p^;\nEND_PROGRAM",
    );
}

#[test]
fn reference_parser_accepts_dereference_write() {
    assert_reference_parses("PROGRAM P\nVAR p : POINTER TO INT; END_VAR\np^ := 1;\nEND_PROGRAM");
}

#[test]
fn reference_parser_accepts_nested_dereference() {
    assert_reference_parses(
        "PROGRAM P\nVAR p : POINTER TO POINTER TO INT; x : INT; END_VAR\nx := p^^;\nEND_PROGRAM",
    );
}

#[test]
fn reference_parser_accepts_deref_field_selection() {
    assert_reference_parses("PROGRAM P\nx := p^.field;\nEND_PROGRAM");
}

#[test]
fn reference_parser_accepts_deref_index_selection() {
    assert_reference_parses("PROGRAM P\nx := p^[2];\nEND_PROGRAM");
}

#[test]
fn reference_parser_accepts_null_assignment() {
    assert_reference_parses("PROGRAM P\nVAR r : REF_TO INT; END_VAR\nr := NULL;\nEND_PROGRAM");
}

#[test]
fn reference_parser_accepts_null_equality() {
    assert_reference_parses("PROGRAM P\nIF r = NULL THEN ; END_IF;\nEND_PROGRAM");
}

#[test]
fn reference_parser_accepts_null_inequality() {
    assert_reference_parses("PROGRAM P\nIF r <> NULL THEN ; END_IF;\nEND_PROGRAM");
}

#[test]
fn reference_parser_accepts_assignment_attempt() {
    assert_reference_parses("PROGRAM P\nleft ?= right;\nEND_PROGRAM");
}

#[test]
fn reference_parser_accepts_assignment_attempt_from_null() {
    assert_reference_parses("PROGRAM P\nleft ?= NULL;\nEND_PROGRAM");
}

#[test]
fn reference_parser_rejects_ref_to_without_target_type() {
    assert_reference_rejected("PROGRAM P\nVAR r : REF_TO; END_VAR\nEND_PROGRAM");
}

#[test]
fn reference_parser_rejects_pointer_without_to() {
    assert_reference_rejected("PROGRAM P\nVAR p : POINTER INT; END_VAR\nEND_PROGRAM");
}

#[test]
fn reference_parser_rejects_pointer_to_without_target_type() {
    assert_reference_rejected("PROGRAM P\nVAR p : POINTER TO; END_VAR\nEND_PROGRAM");
}

#[test]
fn reference_parser_retains_empty_ref_call_for_semantic_diagnostic() {
    assert_reference_parses("PROGRAM P\nr := REF();\nEND_PROGRAM");
}

#[test]
fn reference_parser_rejects_adr_without_argument() {
    assert_reference_rejected("PROGRAM P\np := ADR();\nEND_PROGRAM");
}

#[test]
fn reference_parser_rejects_unclosed_ref_call() {
    assert_reference_rejected("PROGRAM P\nr := REF(x;\nEND_PROGRAM");
}

#[test]
fn reference_parser_rejects_prefix_dereference() {
    assert_reference_rejected("PROGRAM P\nx := ^p;\nEND_PROGRAM");
}

#[test]
fn reference_parser_rejects_assignment_attempt_without_source() {
    assert_reference_rejected("PROGRAM P\nr ?= ;\nEND_PROGRAM");
}

#[test]
fn reference_parser_rejects_assignment_attempt_without_target() {
    assert_reference_rejected("PROGRAM P\n?= r;\nEND_PROGRAM");
}
