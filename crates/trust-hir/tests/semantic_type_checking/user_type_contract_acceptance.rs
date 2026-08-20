use crate::common::*;

fn warning_messages(source: &str) -> Vec<String> {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    db.diagnostics(file)
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning)
        .map(|diagnostic| diagnostic.message.to_string())
        .collect()
}

fn is_array_cardinality_warning(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("array")
        && message.contains("initial")
        && (message.contains("excess")
            || message.contains("ignored")
            || message.contains("missing")
            || message.contains("default"))
}

#[test]
fn user_type_contract_accepts_qualified_and_unique_enum_literals() {
    check_no_errors(
        r#"
TYPE Color : (Red, Green, Blue); Mode : (Off, On); END_TYPE
PROGRAM Main
VAR color : Color; mode : Mode; END_VAR
color := Color#Green;
mode := On;
END_PROGRAM
"#,
    );
}

#[test]
fn user_type_contract_accepts_explicit_enum_type_default() {
    check_no_errors(
        "TYPE Color : (Red, Green, Blue) := Blue; END_TYPE\nPROGRAM Main\nVAR color : Color; END_VAR\nEND_PROGRAM",
    );
}

#[test]
fn user_type_contract_accepts_named_integer_values_in_arithmetic() {
    check_no_errors(
        r#"
TYPE Signal : INT (Low := 1, High := 2); END_TYPE
PROGRAM Main
VAR signal : Signal; END_VAR
signal := Signal#Low + 10;
END_PROGRAM
"#,
    );
}

#[test]
fn user_type_contract_accepts_unnamed_base_value_for_named_value_type() {
    check_no_errors(
        "TYPE Signal : INT (Low := 1, High := 2); END_TYPE\nPROGRAM Main\nVAR signal : Signal; END_VAR\nsignal := 27;\nEND_PROGRAM",
    );
}

#[test]
fn user_type_contract_accepts_subrange_inclusive_literal_bounds() {
    check_no_errors(
        "TYPE Limited : INT (-2..2); END_TYPE\nPROGRAM Main\nVAR low : Limited := -2; high : Limited := 2; END_VAR\nEND_PROGRAM",
    );
}

#[test]
fn user_type_contract_accepts_constant_expression_subrange_bounds() {
    check_no_errors(
        r#"
VAR_GLOBAL CONSTANT lower : INT := -2; upper : INT := 2; END_VAR
TYPE Limited : INT (lower..upper); END_TYPE
PROGRAM Main
VAR value : Limited; END_VAR
END_PROGRAM
"#,
    );
}

#[test]
fn user_type_contract_accepts_multidimensional_constant_array_bounds() {
    check_no_errors(
        r#"
VAR_GLOBAL CONSTANT low : INT := -1; high : INT := 1; END_VAR
TYPE Matrix : ARRAY[low..high, 2..4] OF INT; END_TYPE
PROGRAM Main
VAR values : Matrix; result : INT; END_VAR
result := values[0, 3];
END_PROGRAM
"#,
    );
}

#[test]
fn user_type_contract_partial_array_initializer_emits_cardinality_warning() {
    let warnings = warning_messages(
        "PROGRAM Main\nVAR values : ARRAY[1..3] OF INT := [1, 2]; END_VAR\nEND_PROGRAM",
    );
    assert!(
        warnings
            .iter()
            .any(|message| is_array_cardinality_warning(message)),
        "partial initializer must warn about default fill: {warnings:?}"
    );
}

#[test]
fn user_type_contract_excess_array_initializer_emits_cardinality_warning() {
    let warnings = warning_messages(
        "PROGRAM Main\nVAR values : ARRAY[1..2] OF INT := [1, 2, 3]; END_VAR\nEND_PROGRAM",
    );
    assert!(
        warnings
            .iter()
            .any(|message| is_array_cardinality_warning(message)),
        "excess initializer must warn about ignored values: {warnings:?}"
    );
}

#[test]
fn user_type_contract_exact_array_initializer_has_no_cardinality_warning() {
    let warnings = warning_messages(
        "PROGRAM Main\nVAR values : ARRAY[1..2] OF INT := [1, 2]; END_VAR\nEND_PROGRAM",
    );
    assert!(
        warnings
            .iter()
            .all(|message| !is_array_cardinality_warning(message)),
        "exact initializer must not receive cardinality warning: {warnings:?}"
    );
}

#[test]
fn user_type_contract_accepts_zero_count_array_repetition() {
    check_no_errors(
        "PROGRAM Main\nVAR values : ARRAY[1..2] OF INT := [0(9), 1, 2]; END_VAR\nEND_PROGRAM",
    );
}

#[test]
fn user_type_contract_accepts_nested_array_repetition() {
    check_no_errors(
        "PROGRAM Main\nVAR values : ARRAY[1..8] OF INT := [2(2(1, 2))]; END_VAR\nEND_PROGRAM",
    );
}

#[test]
fn user_type_contract_accepts_same_type_whole_structure_assignment() {
    check_no_errors(
        r#"
TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE
PROGRAM Main
VAR source : Point; target : Point; END_VAR
target := source;
END_PROGRAM
"#,
    );
}

#[test]
fn user_type_contract_accepts_nonoverlapping_relative_structure() {
    check_no_errors(
        "TYPE Packet : STRUCT head AT %B0 : INT; flag AT %X2.0 : BOOL; tail AT %B3 : BYTE; END_STRUCT; END_TYPE",
    );
}

#[test]
fn user_type_contract_accepts_overlap_structure_without_initializer() {
    check_no_errors(
        "TYPE Overlay : STRUCT OVERLAP wide AT %B0 : DWORD; narrow AT %B0 : WORD; END_STRUCT; END_TYPE",
    );
}

#[test]
fn user_type_contract_accepts_partial_named_structure_initializer() {
    check_no_errors(
        r#"
TYPE Point : STRUCT x : INT := 1; y : INT := 2; END_STRUCT; END_TYPE
PROGRAM Main
VAR value : Point := (x := 7); END_VAR
END_PROGRAM
"#,
    );
}

#[test]
fn user_type_contract_accepts_union_variant_defaults_and_partial_initializer() {
    check_no_errors(
        r#"
TYPE
Choice : UNION count : INT := 1; ready : BOOL := TRUE; END_UNION;
DefaultChoice : Choice := (count := 7);
END_TYPE
PROGRAM Main
VAR value : DefaultChoice := (ready := FALSE); END_VAR
END_PROGRAM
"#,
    );
}

#[test]
fn user_type_contract_accepts_same_type_whole_union_assignment() {
    check_no_errors(
        r#"
TYPE Choice : UNION count : INT; ready : BOOL; END_UNION; END_TYPE
PROGRAM Main
VAR source : Choice; target : Choice; END_VAR
target := source;
END_PROGRAM
"#,
    );
}

#[test]
fn user_type_contract_accepts_array_of_enum_and_subrange_elements() {
    check_no_errors(
        r#"
TYPE Color : (Red, Green); Limited : INT (4..6); END_TYPE
PROGRAM Main
VAR colors : ARRAY[1..2] OF Color; values : ARRAY[1..2] OF Limited; END_VAR
colors[1] := Green;
values[1] := 4;
END_PROGRAM
"#,
    );
}
