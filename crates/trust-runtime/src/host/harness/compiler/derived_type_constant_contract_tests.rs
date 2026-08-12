use crate::harness::{CompileSession, SourceFile};
use crate::Runtime;
use trust_hir::{Type, TypeId};

fn source_files(files: &[(&str, &str)]) -> Vec<SourceFile> {
    files
        .iter()
        .map(|(path, source)| SourceFile::with_path(*path, *source))
        .collect()
}

fn runtime(files: &[(&str, &str)]) -> Runtime {
    CompileSession::from_sources(source_files(files))
        .build_runtime()
        .unwrap_or_else(|error| panic!("derived-type constant fixture must compile: {error}"))
}

fn compile_error(files: &[(&str, &str)]) -> String {
    match CompileSession::from_sources(source_files(files)).build_runtime() {
        Ok(_) => panic!("derived-type constant fixture must fail"),
        Err(error) => error.to_string(),
    }
}

fn type_id(runtime: &Runtime, name: &str) -> TypeId {
    runtime
        .registry()
        .lookup(name)
        .unwrap_or_else(|| panic!("missing type {name}"))
}

fn resolved_type_id(runtime: &Runtime, mut type_id: TypeId) -> TypeId {
    loop {
        match runtime.registry().get(type_id) {
            Some(Type::Alias { target, .. }) => type_id = *target,
            _ => return type_id,
        }
    }
}

fn resolved_type<'a>(runtime: &'a Runtime, name: &str) -> &'a Type {
    let type_id = resolved_type_id(runtime, type_id(runtime, name));
    runtime
        .registry()
        .get(type_id)
        .unwrap_or_else(|| panic!("missing resolved type {name}"))
}

fn array_dimensions(runtime: &Runtime, name: &str) -> Vec<(i64, i64)> {
    match resolved_type(runtime, name) {
        Type::Array { dimensions, .. } => dimensions.clone(),
        other => panic!("expected array type {name}, got {other:?}"),
    }
}

fn string_capacity(runtime: &Runtime, name: &str) -> u32 {
    match resolved_type(runtime, name) {
        Type::String { max_len: Some(len) } | Type::WString { max_len: Some(len) } => *len,
        other => panic!("expected bounded string type {name}, got {other:?}"),
    }
}

fn subrange_bounds(runtime: &Runtime, name: &str) -> (i64, i64) {
    match resolved_type(runtime, name) {
        Type::Subrange { lower, upper, .. } => (*lower, *upper),
        other => panic!("expected subrange type {name}, got {other:?}"),
    }
}

fn enum_values(runtime: &Runtime, name: &str) -> Vec<(String, i64)> {
    match resolved_type(runtime, name) {
        Type::Enum { values, .. } => values
            .iter()
            .map(|(name, value)| (name.to_string(), *value))
            .collect(),
        other => panic!("expected enumeration type {name}, got {other:?}"),
    }
}

#[test]
fn derived_type_constant_array_bounds_use_later_constant() {
    let runtime = runtime(&[(
        "main.st",
        r#"
TYPE Samples : ARRAY[Lower..Upper] OF INT;
END_TYPE
VAR_GLOBAL CONSTANT
    Upper : INT := INT#5;
    Lower : INT := INT#2;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert_eq!(array_dimensions(&runtime, "Samples"), [(2, 5)]);
}

#[test]
fn derived_type_constant_array_bounds_use_later_source_constant() {
    let runtime = runtime(&[
        (
            "types.st",
            r#"
TYPE Samples : ARRAY[Lower..Upper] OF INT;
END_TYPE
"#,
        ),
        (
            "constants.st",
            r#"
VAR_GLOBAL CONSTANT
    Lower : INT := INT#3;
    Upper : INT := INT#7;
END_VAR
"#,
        ),
        (
            "main.st",
            r#"
PROGRAM Main
END_PROGRAM
"#,
        ),
    ]);

    assert_eq!(array_dimensions(&runtime, "Samples"), [(3, 7)]);
}

#[test]
fn derived_type_constant_source_permutation_preserves_array_shape() {
    const TYPES: &str = r#"
TYPE Samples : ARRAY[Lower..Upper] OF INT;
END_TYPE
"#;
    const CONSTANTS: &str = r#"
VAR_GLOBAL CONSTANT
    Lower : INT := -INT#2;
    Upper : INT := INT#2;
END_VAR
"#;
    const PROGRAM: &str = r#"
PROGRAM Main
END_PROGRAM
"#;

    let first = runtime(&[
        ("types.st", TYPES),
        ("constants.st", CONSTANTS),
        ("main.st", PROGRAM),
    ]);
    let second = runtime(&[
        ("main.st", PROGRAM),
        ("constants.st", CONSTANTS),
        ("types.st", TYPES),
    ]);

    assert_eq!(array_dimensions(&first, "Samples"), [(-2, 2)]);
    assert_eq!(array_dimensions(&second, "Samples"), [(-2, 2)]);
}

#[test]
fn derived_type_constant_multidimensional_bounds_retain_source_order() {
    let runtime = runtime(&[(
        "main.st",
        r#"
TYPE Grid : ARRAY[RowLow..RowHigh, ColLow..ColHigh, Depth..Depth] OF BOOL;
END_TYPE
VAR_GLOBAL CONSTANT
    Depth : INT := INT#4;
    ColHigh : INT := INT#2;
    RowHigh : INT := INT#1;
    ColLow : INT := -INT#1;
    RowLow : INT := INT#0;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert_eq!(
        array_dimensions(&runtime, "Grid"),
        [(0, 1), (-1, 2), (4, 4)]
    );
}

#[test]
fn derived_type_constant_subrange_bounds_use_complete_dependency_graph() {
    let runtime = runtime(&[(
        "main.st",
        r#"
TYPE Window : INT(Lower..Upper);
END_TYPE
VAR_GLOBAL CONSTANT
    Upper : INT := Center + Radius;
    Lower : INT := Center - Radius;
    Radius : INT := INT#3;
    Center : INT := INT#5;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert_eq!(subrange_bounds(&runtime, "Window"), (2, 8));
}

#[test]
fn derived_type_constant_bracketed_string_capacities_use_later_constants() {
    let runtime = runtime(&[(
        "main.st",
        r#"
TYPE Label : STRING[TextSize];
WideLabel : WSTRING[WideSize];
END_TYPE
VAR_GLOBAL CONSTANT
    WideSize : INT := INT#9;
    TextSize : INT := INT#12;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert_eq!(string_capacity(&runtime, "Label"), 12);
    assert_eq!(string_capacity(&runtime, "WideLabel"), 9);
}

#[test]
fn derived_type_constant_parenthesized_string_capacities_share_dependency_rules() {
    let runtime = runtime(&[(
        "main.st",
        r#"
TYPE Label : STRING(TextSize + INT#1);
WideLabel : WSTRING(TextSize);
END_TYPE
VAR_GLOBAL CONSTANT
    TextSize : INT := INT#6;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert_eq!(string_capacity(&runtime, "Label"), 7);
    assert_eq!(string_capacity(&runtime, "WideLabel"), 6);
}

#[test]
fn derived_type_constant_explicit_enum_values_use_later_constants_and_checked_continuation() {
    let runtime = runtime(&[(
        "main.st",
        r#"
TYPE ResultCode : DINT (
    Ready := FirstCode,
    Busy,
    Complete := LastCode,
    Failed
);
END_TYPE
VAR_GLOBAL CONSTANT
    LastCode : DINT := DINT#20;
    FirstCode : DINT := DINT#10;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert_eq!(
        enum_values(&runtime, "ResultCode"),
        [
            ("Ready".into(), 10),
            ("Busy".into(), 11),
            ("Complete".into(), 20),
            ("Failed".into(), 21)
        ]
    );
}

#[test]
fn derived_type_constant_explicit_enum_value_uses_later_source_constant() {
    let runtime = runtime(&[
        (
            "types.st",
            r#"
TYPE ResultCode : DINT (Ready := BaseCode, Busy);
END_TYPE
"#,
        ),
        (
            "constants.st",
            r#"
VAR_GLOBAL CONSTANT
    BaseCode : DINT := DINT#30;
END_VAR
"#,
        ),
        (
            "main.st",
            r#"
PROGRAM Main
END_PROGRAM
"#,
        ),
    ]);

    assert_eq!(
        enum_values(&runtime, "ResultCode"),
        [("Ready".into(), 30), ("Busy".into(), 31)]
    );
}

#[test]
fn derived_type_constant_qualified_namespace_values_define_all_shape_kinds() {
    let runtime = runtime(&[(
        "main.st",
        r#"
NAMESPACE Limits
VAR_GLOBAL CONSTANT
    Low : INT := INT#2;
    High : INT := INT#6;
    TextSize : INT := INT#8;
    Code : DINT := DINT#40;
END_VAR
END_NAMESPACE
TYPE Samples : ARRAY[Limits.Low..Limits.High] OF INT;
Window : INT(Limits.Low..Limits.High);
Label : STRING[Limits.TextSize];
ResultCode : DINT (Ready := Limits.Code, Busy);
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert_eq!(array_dimensions(&runtime, "Samples"), [(2, 6)]);
    assert_eq!(subrange_bounds(&runtime, "Window"), (2, 6));
    assert_eq!(string_capacity(&runtime, "Label"), 8);
    assert_eq!(
        enum_values(&runtime, "ResultCode"),
        [("Ready".into(), 40), ("Busy".into(), 41)]
    );
}

#[test]
fn derived_type_constant_using_import_defines_all_shape_kinds() {
    let runtime = runtime(&[(
        "main.st",
        r#"
NAMESPACE Limits
VAR_GLOBAL CONSTANT
    Low : INT := INT#1;
    High : INT := INT#4;
    TextSize : INT := INT#7;
END_VAR
END_NAMESPACE
USING Limits;
TYPE Samples : ARRAY[Low..High] OF INT;
Window : INT(Low..High);
Label : STRING[TextSize];
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert_eq!(array_dimensions(&runtime, "Samples"), [(1, 4)]);
    assert_eq!(subrange_bounds(&runtime, "Window"), (1, 4));
    assert_eq!(string_capacity(&runtime, "Label"), 7);
}

#[test]
fn derived_type_constant_same_leaf_pou_constants_remain_namespace_local() {
    let runtime = runtime(&[(
        "main.st",
        r#"
NAMESPACE CellA
PROGRAM Main
VAR CONSTANT
    Limit : INT := INT#2;
END_VAR
VAR
    Values : ARRAY[0..Limit] OF INT;
END_VAR
END_PROGRAM
END_NAMESPACE
NAMESPACE CellB
PROGRAM Main
VAR CONSTANT
    Limit : INT := INT#4;
END_VAR
VAR
    Values : ARRAY[0..Limit] OF INT;
END_VAR
END_PROGRAM
END_NAMESPACE
"#,
    )]);

    let dimensions = |program_name: &str| {
        let program = runtime
            .programs()
            .values()
            .find(|program| program.name == program_name)
            .unwrap_or_else(|| panic!("missing program {program_name}"));
        let value = program
            .vars
            .iter()
            .find(|var| var.name == "Values")
            .unwrap_or_else(|| panic!("missing {program_name}.Values"));
        match runtime
            .registry()
            .get(resolved_type_id(&runtime, value.type_id))
        {
            Some(Type::Array { dimensions, .. }) => dimensions.clone(),
            other => panic!("expected {program_name}.Values array, got {other:?}"),
        }
    };

    assert_eq!(dimensions("CellA.Main"), [(0, 2)]);
    assert_eq!(dimensions("CellB.Main"), [(0, 4)]);
}

#[test]
fn derived_type_constant_enum_values_are_valid_array_bounds() {
    let runtime = runtime(&[(
        "main.st",
        r#"
TYPE Level : (Low := 1, High := 3);
Samples : ARRAY[Low..High] OF INT;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert_eq!(array_dimensions(&runtime, "Samples"), [(1, 3)]);
}

#[test]
fn derived_type_constant_integer_operator_matrix_is_preserved() {
    let runtime = runtime(&[(
        "main.st",
        r#"
TYPE AddSized : STRING[INT#2 + INT#3];
SubSized : STRING[INT#7 - INT#2];
MulSized : STRING[INT#2 * INT#3];
DivSized : WSTRING[INT#8 / INT#2];
ModSized : STRING[INT#5 MOD INT#2];
PowerRange : INT(INT#2 ** INT#0..INT#2 ** INT#3);
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert_eq!(string_capacity(&runtime, "AddSized"), 5);
    assert_eq!(string_capacity(&runtime, "SubSized"), 5);
    assert_eq!(string_capacity(&runtime, "MulSized"), 6);
    assert_eq!(string_capacity(&runtime, "DivSized"), 4);
    assert_eq!(string_capacity(&runtime, "ModSized"), 1);
    assert_eq!(subrange_bounds(&runtime, "PowerRange"), (1, 8));
}

#[test]
fn derived_type_constant_lookup_is_case_insensitive() {
    let runtime = runtime(&[(
        "main.st",
        r#"
TYPE Samples : ARRAY[lower..UPPER] OF INT;
Label : STRING[textsize];
END_TYPE
VAR_GLOBAL CONSTANT
    Lower : INT := INT#1;
    Upper : INT := INT#3;
    TextSize : INT := INT#5;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert_eq!(array_dimensions(&runtime, "Samples"), [(1, 3)]);
    assert_eq!(string_capacity(&runtime, "Label"), 5);
}

#[test]
fn derived_type_constant_rejects_mutable_array_bound() {
    let error = compile_error(&[(
        "mutable_array.st",
        r#"
VAR_GLOBAL
    Upper : INT := INT#3;
END_VAR
TYPE Samples : ARRAY[0..Upper] OF INT;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(
        error.to_ascii_lowercase().contains("constant")
            && error.to_ascii_lowercase().contains("upper"),
        "{error}"
    );
}

#[test]
fn derived_type_constant_rejects_mutable_subrange_bound() {
    let error = compile_error(&[(
        "mutable_subrange.st",
        r#"
VAR_GLOBAL
    Upper : INT := INT#3;
END_VAR
TYPE Window : INT(0..Upper);
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(
        error.to_ascii_lowercase().contains("constant")
            && error.to_ascii_lowercase().contains("upper"),
        "{error}"
    );
}

#[test]
fn derived_type_constant_rejects_mutable_string_capacity() {
    let error = compile_error(&[(
        "mutable_string.st",
        r#"
VAR_GLOBAL
    TextSize : INT := INT#8;
END_VAR
TYPE Label : STRING[TextSize];
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(
        error.to_ascii_lowercase().contains("constant")
            && error.to_ascii_lowercase().contains("textsize"),
        "{error}"
    );
}

#[test]
fn derived_type_constant_rejects_mutable_enum_value() {
    let error = compile_error(&[(
        "mutable_enum.st",
        r#"
VAR_GLOBAL
    BaseCode : DINT := DINT#10;
END_VAR
TYPE ResultCode : DINT (Ready := BaseCode, Busy);
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(
        error.to_ascii_lowercase().contains("constant")
            && error.to_ascii_lowercase().contains("basecode"),
        "{error}"
    );
}

#[test]
fn derived_type_constant_rejects_undefined_operand_with_source_label() {
    let error = compile_error(&[
        (
            "types.st",
            r#"
TYPE Samples : ARRAY[0..MissingLimit] OF INT;
END_TYPE
"#,
        ),
        (
            "main.st",
            r#"
PROGRAM Main
END_PROGRAM
"#,
        ),
    ]);

    assert!(error.contains("types.st:"), "{error}");
    assert!(
        error.to_ascii_lowercase().contains("missinglimit")
            && (error.to_ascii_lowercase().contains("undefined")
                || error.to_ascii_lowercase().contains("unknown")),
        "{error}"
    );
}

#[test]
fn derived_type_constant_rejects_cyclic_operand_graph() {
    let error = compile_error(&[(
        "cycle.st",
        r#"
VAR_GLOBAL CONSTANT
    Lower : INT := Upper - INT#1;
    Upper : INT := Lower + INT#1;
END_VAR
TYPE Samples : ARRAY[Lower..Upper] OF INT;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(error.to_ascii_lowercase().contains("cyclic"), "{error}");
}

#[test]
fn derived_type_constant_rejects_divide_by_zero_before_shape_validation() {
    let error = compile_error(&[(
        "divide_by_zero.st",
        r#"
VAR_GLOBAL CONSTANT
    Zero : INT := INT#0;
END_VAR
TYPE Samples : ARRAY[0..INT#4 / Zero] OF INT;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(
        error.to_ascii_lowercase().contains("divides by zero")
            || error.to_ascii_lowercase().contains("division by zero"),
        "{error}"
    );
}

#[test]
fn derived_type_constant_rejects_integer_overflow_before_shape_validation() {
    let error = compile_error(&[(
        "overflow.st",
        r#"
TYPE Label : STRING[LINT#9223372036854775807 + LINT#1];
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(error.to_ascii_lowercase().contains("overflow"), "{error}");
}

#[test]
fn derived_type_constant_rejects_reversed_array_dimension() {
    let error = compile_error(&[(
        "reversed_array.st",
        r#"
TYPE Samples : ARRAY[INT#5..INT#2] OF INT;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(
        error.to_ascii_lowercase().contains("bound")
            || error.to_ascii_lowercase().contains("range")
            || error.to_ascii_lowercase().contains("dimension"),
        "{error}"
    );
}

#[test]
fn derived_type_constant_rejects_reversed_subrange() {
    let error = compile_error(&[(
        "reversed_subrange.st",
        r#"
TYPE Window : INT(INT#5..INT#2);
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(
        error.to_ascii_lowercase().contains("bound")
            || error.to_ascii_lowercase().contains("range"),
        "{error}"
    );
}

#[test]
fn derived_type_constant_rejects_non_integer_shape_operand() {
    let error = compile_error(&[(
        "non_integer.st",
        r#"
VAR_GLOBAL CONSTANT
    Enabled : BOOL := TRUE;
END_VAR
TYPE Samples : ARRAY[0..Enabled] OF INT;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(
        error.to_ascii_lowercase().contains("integer")
            || error.to_ascii_lowercase().contains("any_int"),
        "{error}"
    );
}

#[test]
fn derived_type_constant_rejects_enum_explicit_value_outside_base() {
    let error = compile_error(&[(
        "enum_range.st",
        r#"
TYPE SmallCode : SINT (TooLarge := 128);
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(
        error.to_ascii_lowercase().contains("range")
            || error.to_ascii_lowercase().contains("represent"),
        "{error}"
    );
}

#[test]
fn derived_type_constant_rejects_enum_implicit_successor_overflow() {
    let error = compile_error(&[(
        "enum_successor.st",
        r#"
TYPE SmallCode : SINT (Last := 127, Overflow);
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(
        error.to_ascii_lowercase().contains("overflow")
            || error.to_ascii_lowercase().contains("range"),
        "{error}"
    );
}

#[test]
fn derived_type_constant_rejects_ambiguous_using_operand() {
    let error = compile_error(&[(
        "ambiguous.st",
        r#"
NAMESPACE Left
VAR_GLOBAL CONSTANT
    Limit : INT := INT#2;
END_VAR
END_NAMESPACE
NAMESPACE Right
VAR_GLOBAL CONSTANT
    Limit : INT := INT#4;
END_VAR
END_NAMESPACE
USING Left, Right;
TYPE Samples : ARRAY[0..Limit] OF INT;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(
        error.to_ascii_lowercase().contains("ambiguous")
            && error.to_ascii_lowercase().contains("limit"),
        "{error}"
    );
}

#[test]
fn derived_type_constant_does_not_leak_unimported_namespace_operand() {
    let error = compile_error(&[(
        "missing_import.st",
        r#"
NAMESPACE Limits
VAR_GLOBAL CONSTANT
    Limit : INT := INT#4;
END_VAR
END_NAMESPACE
TYPE Samples : ARRAY[0..Limit] OF INT;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(
        error.to_ascii_lowercase().contains("limit")
            && (error.to_ascii_lowercase().contains("undefined")
                || error.to_ascii_lowercase().contains("unknown")
                || error.to_ascii_lowercase().contains("cannot resolve")),
        "{error}"
    );
}
