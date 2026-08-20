mod common;
use common::*;

fn accepted(source: &str) {
    let parsed = parse(source);
    assert!(
        parsed.ok(),
        "expected type source to parse: {:?}",
        parsed.errors()
    );
}

fn rejected(source: &str) {
    assert!(!parse(source).ok(), "expected malformed type source");
}

#[test]
fn user_type_parser_accepts_ordinary_enumeration() {
    accepted("TYPE Color : (Red, Green, Blue); END_TYPE");
}

#[test]
fn user_type_parser_accepts_enumeration_default() {
    accepted("TYPE Color : (Red, Green, Blue) := Green; END_TYPE");
}

#[test]
fn user_type_parser_accepts_integer_named_values() {
    accepted("TYPE Signal : INT (Low := 1, High := 2) := High; END_TYPE");
}

#[test]
fn user_type_parser_accepts_named_value_expression() {
    accepted("TYPE Flags : DWORD (A := 1, B := A OR 2); END_TYPE");
}

#[test]
fn user_type_parser_accepts_subrange_with_signed_bounds() {
    accepted("TYPE Limited : INT (-10..10); END_TYPE");
}

#[test]
fn user_type_parser_accepts_subrange_constant_bounds() {
    accepted("TYPE Limited : INT (Lower..Upper); END_TYPE");
}

#[test]
fn user_type_parser_accepts_multidimensional_array() {
    accepted("TYPE Matrix : ARRAY[-1..1, 2..4] OF INT; END_TYPE");
}

#[test]
fn user_type_parser_accepts_array_repetition_initializer() {
    accepted("TYPE Values : ARRAY[1..6] OF INT := [3(1, 2)]; END_TYPE");
}

#[test]
fn user_type_parser_accepts_nested_array_repetition_initializer() {
    accepted("TYPE Values : ARRAY[1..8] OF INT := [2(2(1, 2))]; END_TYPE");
}

#[test]
fn user_type_parser_accepts_structure_members_and_defaults() {
    accepted("TYPE Point : STRUCT x : INT := 1; y : INT := 2; END_STRUCT; END_TYPE");
}

#[test]
fn user_type_parser_accepts_relative_structure_fields() {
    accepted("TYPE Packet : STRUCT head AT %B0 : INT; flag AT %X2.0 : BOOL; END_STRUCT; END_TYPE");
}

#[test]
fn user_type_parser_accepts_overlap_structure() {
    accepted(
        "TYPE Overlay : STRUCT OVERLAP bytes AT %B0 : DWORD; word_value AT %B0 : WORD; END_STRUCT; END_TYPE",
    );
}

#[test]
fn user_type_parser_accepts_union_variants_and_defaults() {
    accepted("TYPE Choice : UNION count : INT := 1; ready : BOOL := TRUE; END_UNION; END_TYPE");
}

#[test]
fn user_type_parser_accepts_directly_derived_union_initializer() {
    accepted(
        "TYPE Choice : UNION count : INT; ready : BOOL; END_UNION; DefaultChoice : Choice := (count := 7); END_TYPE",
    );
}

#[test]
fn user_type_parser_accepts_directly_derived_structure_initializer() {
    accepted(
        "TYPE PointBase : STRUCT x : INT; y : INT; END_STRUCT; Point : PointBase := (x := 1, y := 2); END_TYPE",
    );
}

#[test]
fn user_type_parser_accepts_array_of_structure_initializer() {
    accepted(
        "TYPE Point : STRUCT x : INT; END_STRUCT; Points : ARRAY[1..2] OF Point := [2((x := 1))]; END_TYPE",
    );
}

#[test]
fn user_type_parser_rejects_empty_enumeration() {
    rejected("TYPE Color : (); END_TYPE");
}

#[test]
fn user_type_parser_rejects_unclosed_enumeration() {
    rejected("TYPE Color : (Red, Green; END_TYPE");
}

#[test]
fn user_type_parser_rejects_subrange_without_upper_bound() {
    rejected("TYPE Limited : INT (0..); END_TYPE");
}

#[test]
fn user_type_parser_rejects_array_without_element_type() {
    rejected("TYPE Values : ARRAY[1..3] OF; END_TYPE");
}

#[test]
fn user_type_parser_rejects_array_without_upper_bound() {
    rejected("TYPE Values : ARRAY[1..] OF INT; END_TYPE");
}

#[test]
fn user_type_parser_rejects_structure_without_end_struct() {
    rejected("TYPE Point : STRUCT x : INT; END_TYPE");
}

#[test]
fn user_type_parser_rejects_union_without_end_union() {
    rejected("TYPE Choice : UNION count : INT; END_TYPE");
}

#[test]
fn user_type_parser_rejects_relative_field_without_type() {
    rejected("TYPE Packet : STRUCT head AT %B0; END_STRUCT; END_TYPE");
}

#[test]
fn user_type_parser_rejects_repetition_without_value() {
    rejected("TYPE Values : ARRAY[1..2] OF INT := [2()]; END_TYPE");
}
