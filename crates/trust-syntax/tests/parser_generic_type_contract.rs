mod common;
use common::*;

fn accepted(source: &str) {
    let parsed = parse(source);
    assert!(
        parsed.ok(),
        "expected generic type syntax to parse for semantic validation: {:?}",
        parsed.errors()
    );
}

fn rejected(source: &str) {
    assert!(
        !parse(source).ok(),
        "expected malformed generic type syntax"
    );
}

#[test]
fn generic_type_parser_accepts_every_any_keyword_as_type_reference() {
    accepted(
        r#"
PROGRAM Main
VAR
    a : ANY;
    b : ANY_DERIVED;
    c : ANY_ELEMENTARY;
    d : ANY_MAGNITUDE;
    e : ANY_NUM;
    f : ANY_INT;
    g : ANY_SIGNED;
    h : ANY_UNSIGNED;
    i : ANY_REAL;
    j : ANY_DURATION;
    k : ANY_BIT;
    l : ANY_CHARS;
    m : ANY_STRING;
    n : ANY_CHAR;
    o : ANY_DATE;
END_VAR
END_PROGRAM
"#,
    );
}

#[test]
fn generic_type_parser_accepts_case_insensitive_any_keywords() {
    accepted(
        "PROGRAM Main\nVAR a : any_int; b : AnY_sTrInG; c : aNy_DeRiVeD; END_VAR\nEND_PROGRAM",
    );
}

#[test]
fn generic_type_parser_accepts_generic_pou_signature_positions() {
    accepted(
        r#"
FUNCTION Generic : ANY
VAR_INPUT value : ANY_ELEMENTARY; END_VAR
VAR_OUTPUT copy : ANY_DERIVED; END_VAR
VAR_IN_OUT target : ANY; END_VAR
END_FUNCTION
"#,
    );
}

#[test]
fn generic_type_parser_accepts_generic_derived_type_shapes() {
    accepted(
        r#"
TYPE
    GenericAlias : ANY_NUM;
    GenericArray : ARRAY[0..1] OF ANY_INT;
    GenericPointer : POINTER TO ANY_BIT;
    GenericReference : REF_TO ANY_STRING;
    GenericStruct : STRUCT
        value : ANY_DATE;
    END_STRUCT;
END_TYPE
"#,
    );
}

#[test]
fn generic_type_parser_preserves_generic_type_ref_nodes() {
    let parsed = parse("PROGRAM Main\nVAR value : ANY_INT; END_VAR\nEND_PROGRAM");
    assert!(parsed.ok(), "{:?}", parsed.errors());
    let type_refs = parsed
        .syntax()
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::TypeRef)
        .collect::<Vec<_>>();
    assert_eq!(type_refs.len(), 1);
    assert_eq!(type_refs[0].text().to_string().trim(), "ANY_INT");
}

#[test]
fn generic_type_parser_rejects_any_keyword_as_declaration_name() {
    rejected("PROGRAM Main\nVAR ANY_INT : INT; END_VAR\nEND_PROGRAM");
}

#[test]
fn generic_type_parser_rejects_missing_array_element_after_any_context() {
    rejected("PROGRAM Main\nVAR value : ARRAY[0..1] OF; END_VAR\nEND_PROGRAM");
}

#[test]
fn generic_type_parser_rejects_missing_pointer_target_after_any_context() {
    rejected("PROGRAM Main\nVAR value : POINTER TO; END_VAR\nEND_PROGRAM");
}
