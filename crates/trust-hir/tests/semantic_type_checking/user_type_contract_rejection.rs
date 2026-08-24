use crate::common::*;

#[test]
fn user_type_contract_rejects_ambiguous_unqualified_enum_literal() {
    check_has_error(
        "TYPE First : (Red, Green); Second : (Red, Blue); END_TYPE\nPROGRAM Main\nVAR value : First; END_VAR\nvalue := Red;\nEND_PROGRAM",
        DiagnosticCode::CannotResolve,
    );
}

#[test]
fn user_type_contract_rejects_cross_enum_assignment() {
    check_has_error(
        "TYPE First : (A, B); Second : (A, B); END_TYPE\nPROGRAM Main\nVAR left : First; right : Second; END_VAR\nleft := Second#A;\nEND_PROGRAM",
        DiagnosticCode::IncompatibleAssignment,
    );
}

#[test]
fn user_type_contract_rejects_integer_assignment_to_closed_enum() {
    check_has_error(
        "TYPE Color : (Red, Green); END_TYPE\nPROGRAM Main\nVAR color : Color; END_VAR\ncolor := 1;\nEND_PROGRAM",
        DiagnosticCode::IncompatibleAssignment,
    );
}

#[test]
fn user_type_contract_rejects_named_value_outside_base_type() {
    check_has_error(
        "TYPE Signal : SINT (Low := -128, TooHigh := 128); END_TYPE",
        DiagnosticCode::OutOfRange,
    );
}

#[test]
fn user_type_contract_rejects_reversed_subrange() {
    check_has_error(
        "TYPE Limited : INT (2..-2); END_TYPE",
        DiagnosticCode::OutOfRange,
    );
}

#[test]
fn user_type_contract_rejects_noninteger_subrange_base() {
    check_has_error(
        "TYPE Limited : REAL (0..1); END_TYPE",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn user_type_contract_rejects_mutable_subrange_bound() {
    check_has_error(
        "VAR_GLOBAL lower : INT := 0; END_VAR\nTYPE Limited : INT (lower..10); END_TYPE",
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn user_type_contract_rejects_out_of_range_subrange_initializer() {
    check_has_error(
        "TYPE Limited : INT (-2..2); END_TYPE\nPROGRAM Main\nVAR value : Limited := 3; END_VAR\nEND_PROGRAM",
        DiagnosticCode::OutOfRange,
    );
}

#[test]
fn user_type_contract_rejects_noninteger_array_index() {
    check_has_error(
        "PROGRAM Main\nVAR values : ARRAY[1..2] OF INT; index : REAL; result : INT; END_VAR\nresult := values[index];\nEND_PROGRAM",
        DiagnosticCode::InvalidArrayIndex,
    );
}

#[test]
fn user_type_contract_rejects_literal_array_index_below_bound() {
    check_has_error(
        "PROGRAM Main\nVAR values : ARRAY[1..2] OF INT; result : INT; END_VAR\nresult := values[0];\nEND_PROGRAM",
        DiagnosticCode::InvalidArrayIndex,
    );
}

#[test]
fn user_type_contract_rejects_literal_array_index_above_bound() {
    check_has_error(
        "PROGRAM Main\nVAR values : ARRAY[1..2] OF INT; result : INT; END_VAR\nresult := values[3];\nEND_PROGRAM",
        DiagnosticCode::InvalidArrayIndex,
    );
}

#[test]
fn user_type_contract_rejects_missing_array_dimension() {
    check_has_error(
        "PROGRAM Main\nVAR values : ARRAY[1..2, 1..2] OF INT; result : INT; END_VAR\nresult := values[1];\nEND_PROGRAM",
        DiagnosticCode::InvalidArrayIndex,
    );
}

#[test]
fn user_type_contract_rejects_extra_array_dimension() {
    check_has_error(
        "PROGRAM Main\nVAR values : ARRAY[1..2] OF INT; result : INT; END_VAR\nresult := values[1, 1];\nEND_PROGRAM",
        DiagnosticCode::InvalidArrayIndex,
    );
}

#[test]
fn user_type_contract_validates_invalid_excess_array_element() {
    check_has_error(
        "PROGRAM Main\nVAR values : ARRAY[1..2] OF INT := [1, 2, missing]; END_VAR\nEND_PROGRAM",
        DiagnosticCode::UndefinedVariable,
    );
}

#[test]
fn user_type_contract_rejects_negative_array_repetition_count() {
    check_has_error(
        "PROGRAM Main\nVAR values : ARRAY[1..2] OF INT := [-1(1)]; END_VAR\nEND_PROGRAM",
        DiagnosticCode::OutOfRange,
    );
}

#[test]
fn user_type_contract_rejects_mutable_array_repetition_count() {
    check_has_error(
        "PROGRAM Main\nVAR count : INT := 2; values : ARRAY[1..2] OF INT := [count(1)]; END_VAR\nEND_PROGRAM",
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn user_type_contract_rejects_assignment_between_distinct_struct_types() {
    check_has_error(
        "TYPE First : STRUCT x : INT; END_STRUCT; Second : STRUCT x : INT; END_STRUCT; END_TYPE\nPROGRAM Main\nVAR left : First; right : Second; END_VAR\nleft := right;\nEND_PROGRAM",
        DiagnosticCode::IncompatibleAssignment,
    );
}

#[test]
fn user_type_contract_rejects_unknown_union_initializer_variant() {
    check_has_error(
        "TYPE Choice : UNION count : INT; END_UNION; END_TYPE\nPROGRAM Main\nVAR value : Choice := (missing := 1); END_VAR\nEND_PROGRAM",
        DiagnosticCode::UndefinedField,
    );
}

#[test]
fn user_type_contract_rejects_duplicate_union_initializer_variant() {
    check_has_error(
        "TYPE Choice : UNION count : INT; END_UNION; END_TYPE\nPROGRAM Main\nVAR value : Choice := (count := 1, COUNT := 2); END_VAR\nEND_PROGRAM",
        DiagnosticCode::DuplicateField,
    );
}

#[test]
fn user_type_contract_rejects_assignment_between_distinct_union_types() {
    check_has_error(
        "TYPE First : UNION value : INT; END_UNION; Second : UNION value : INT; END_UNION; END_TYPE\nPROGRAM Main\nVAR left : First; right : Second; END_VAR\nleft := right;\nEND_PROGRAM",
        DiagnosticCode::IncompatibleAssignment,
    );
}

#[test]
fn user_type_contract_rejects_overlap_without_overlap_keyword() {
    check_has_error(
        "TYPE Packet : STRUCT wide AT %B0 : DWORD; narrow AT %B0 : WORD; END_STRUCT; END_TYPE",
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn user_type_contract_rejects_overlap_member_initializer() {
    check_has_error(
        "TYPE Overlay : STRUCT OVERLAP wide AT %B0 : DWORD := 1; narrow AT %B0 : WORD; END_STRUCT; END_TYPE",
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn user_type_contract_rejects_overlap_variable_initializer() {
    check_has_error(
        "TYPE Overlay : STRUCT OVERLAP wide AT %B0 : DWORD; narrow AT %B0 : WORD; END_STRUCT; END_TYPE\nPROGRAM Main\nVAR value : Overlay := (wide := 1); END_VAR\nEND_PROGRAM",
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn user_type_contract_rejects_duplicate_structure_field_case_insensitively() {
    check_has_error(
        "TYPE Point : STRUCT Value : INT; value : INT; END_STRUCT; END_TYPE",
        DiagnosticCode::DuplicateDeclaration,
    );
}

#[test]
fn user_type_contract_rejects_recursive_structure_by_value() {
    check_has_error(
        "TYPE Node : STRUCT next : Node; END_STRUCT; END_TYPE",
        DiagnosticCode::CyclicDependency,
    );
}

#[test]
fn user_type_contract_does_not_publish_any_invalid_type_formation() {
    let cases = [
        (
            "BrokenDependency",
            "TYPE Good : INT; BrokenDependency : MissingType; END_TYPE",
        ),
        (
            "BrokenBound",
            "TYPE Good : INT; BrokenBound : INT (2..-2); END_TYPE",
        ),
        (
            "BrokenMember",
            "TYPE Good : INT; BrokenMember : STRUCT value : MissingType; END_STRUCT; END_TYPE",
        ),
        (
            "BrokenVariant",
            "TYPE Good : INT; BrokenVariant : UNION Value : INT; value : BOOL; END_UNION; END_TYPE",
        ),
        (
            "BrokenRepetition",
            "TYPE Good : INT; BrokenRepetition : ARRAY[1..2] OF INT := [-1(1)]; END_TYPE",
        ),
        (
            "BrokenDefault",
            "TYPE Good : INT; BrokenDefault : INT := missing; END_TYPE",
        ),
    ];

    for (invalid_name, source) in cases {
        let mut db = Database::new();
        let file = FileId(0);
        db.set_source_text(file, source.to_string());
        let analysis = db.analyze(file);
        let errors = analysis
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
            .collect::<Vec<_>>();

        assert!(
            !errors.is_empty(),
            "{invalid_name} must be rejected before publication"
        );
        assert!(
            analysis
                .symbols
                .lookup_registered_type_name(invalid_name)
                .is_none(),
            "{invalid_name} entered the public type registry despite {errors:?}"
        );
        assert!(
            analysis
                .symbols
                .lookup_registered_type_name("Good")
                .is_some(),
            "a rejected sibling must not suppress valid type publication"
        );
    }
}
