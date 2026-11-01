use crate::harness::build::lower_application_with;
use crate::harness::{lower_application, CompileSession, SourceFile};
use crate::stdlib::conversions::ConversionProfile;
use crate::RuntimeBuilder;
use trust_hir::{Type, TypeId};

fn source_files(files: &[(&str, &str)]) -> Vec<SourceFile> {
    files
        .iter()
        .map(|(path, source)| SourceFile::with_path(*path, *source))
        .collect()
}

/// The runtime of `files` built under the CODESYS profile (docs/IEC_DEVIATIONS.md DEV-033).
fn codesys_runtime(files: &[(&str, &str)]) -> RuntimeBuilder {
    lower_application_with(
        &source_files(files),
        files.len() > 1,
        ConversionProfile::Codesys,
    )
    .and_then(|lowered| lowered.construct(&[]))
    .map(|(builder, _)| builder)
    .unwrap_or_else(|error| panic!("the CODESYS profile must build the fixture: {error}"))
}

/// The error of building `files` under the IEC rules.
fn iec_build_error(files: &[(&str, &str)]) -> String {
    match lower_application(&source_files(files), files.len() > 1) {
        Ok(_) => panic!("the IEC rules must refuse the fixture"),
        Err(error) => error.to_string(),
    }
}

fn compile_error(files: &[(&str, &str)]) -> String {
    match CompileSession::from_sources(source_files(files)).declarations() {
        Ok(_) => panic!("derived-type constant fixture must fail"),
        Err(error) => error.to_string(),
    }
}

fn type_id(runtime: &RuntimeBuilder, name: &str) -> TypeId {
    runtime
        .registry()
        .lookup(name)
        .unwrap_or_else(|| panic!("missing type {name}"))
}

fn resolved_type_id(runtime: &RuntimeBuilder, mut type_id: TypeId) -> TypeId {
    loop {
        match runtime.registry().get(type_id) {
            Some(Type::Alias { target, .. }) => type_id = *target,
            _ => return type_id,
        }
    }
}

fn resolved_type<'a>(runtime: &'a RuntimeBuilder, name: &str) -> &'a Type {
    let type_id = resolved_type_id(runtime, type_id(runtime, name));
    runtime
        .registry()
        .get(type_id)
        .unwrap_or_else(|| panic!("missing resolved type {name}"))
}

fn array_dimensions(runtime: &RuntimeBuilder, name: &str) -> Vec<(i64, i64)> {
    match resolved_type(runtime, name) {
        Type::Array { dimensions, .. } => dimensions.clone(),
        other => panic!("expected array type {name}, got {other:?}"),
    }
}

fn string_capacity(runtime: &RuntimeBuilder, name: &str) -> u32 {
    match resolved_type(runtime, name) {
        Type::String { max_len: Some(len) } | Type::WString { max_len: Some(len) } => *len,
        other => panic!("expected bounded string type {name}, got {other:?}"),
    }
}

#[test]
fn derived_type_constant_rejects_mutable_array_bound() {
    let error = compile_error(&[(
        "mutable_array.st",
        r#"
VAR_GLOBAL
    Upper : INT := INT#3;
    Via : INT := Upper;
END_VAR
TYPE Samples : ARRAY[0..Via] OF INT;
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
    Via : INT := TextSize;
END_VAR
TYPE Label : STRING[Via];
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

// docs/IEC_DEVIATIONS.md DEV-033: under the CODESYS profile a plain global with a literal
// initializer is a valid declaration size, so the DUT's STRING capacity is the declared 8. Under
// the IEC rules a size is a literal or a constant expression (IEC 61131-3 Ed.3 §6.4.2.2), so the
// build is refused. Catches the CODESYS size lost in the runtime build (no capacity or the default
// 254) and the CODESYS rule applied to an IEC project.
#[test]
fn derived_type_constant_accepts_initialized_global_string_capacity() {
    let files = [(
        "sized_string.st",
        r#"
VAR_GLOBAL
    TextSize : INT := INT#8;
END_VAR
TYPE Label : STRING[TextSize];
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    )];

    assert_eq!(string_capacity(&codesys_runtime(&files), "Label"), 8);
    let error = iec_build_error(&files);
    assert!(error.to_ascii_lowercase().contains("textsize"), "{error}");
}

// DEV-033 for an array bound: under the CODESYS profile `ARRAY[0..Upper]` with `Upper := 3` has
// the dimension 0..3; the IEC rules refuse the bound (IEC 61131-3 Ed.3 §6.4.4.5.1). Catches the
// bound read as 0 or lost, and the CODESYS rule applied to an IEC project.
#[test]
fn derived_type_constant_accepts_initialized_global_array_bound() {
    let files = [(
        "sized_array.st",
        r#"
VAR_GLOBAL
    Upper : INT := INT#3;
END_VAR
TYPE Samples : ARRAY[0..Upper] OF INT;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    )];

    assert_eq!(
        array_dimensions(&codesys_runtime(&files), "Samples"),
        vec![(0, 3)]
    );
    let error = iec_build_error(&files);
    assert!(error.to_ascii_lowercase().contains("upper"), "{error}");
}

// DEV-033 across files, as an e!COCKPIT project keeps its GVL in a file of its own: under the
// CODESYS profile the DUTs of one file take their sizes from the plain globals of another. Worked
// by hand: Label holds 8 characters and Samples has the bounds 0..3. Catches the runtime build
// reading a size only from its own file.
#[test]
fn derived_type_constant_accepts_initialized_global_of_another_file() {
    let runtime = codesys_runtime(&[
        (
            "gvl.st",
            r#"
VAR_GLOBAL
    TextSize : INT := INT#8;
    Upper : INT := INT#3;
END_VAR
"#,
        ),
        (
            "types.st",
            r#"
TYPE Label : STRING[TextSize];
END_TYPE
TYPE Samples : ARRAY[0..Upper] OF INT;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
        ),
    ]);

    assert_eq!(string_capacity(&runtime, "Label"), 8);
    assert_eq!(array_dimensions(&runtime, "Samples"), vec![(0, 3)]);
}

// IEC_DEVIATIONS 2026-10-25: a library stub that flattens the GVL level
// into namespace-level constants still resolves documentation-faithful
// dotted size references through the unique-leaf tolerance — both inside
// the namespace and through the full dotted path from outside.
#[test]
fn derived_type_constant_flattened_library_container_sizes_fold_exactly() {
    let runtime = codesys_runtime(&[
        (
            "stubs.st",
            r#"
NAMESPACE WagoTypesLog
VAR_GLOBAL CONSTANT
    MAX_CMP_NAME_LEN : USINT := 20;
    MAX_LOG_MSG_LEN : USINT := 128;
END_VAR

INTERFACE I_LogChannel
METHOD Msg
VAR_IN_OUT
    sLoggerName : STRING(LogParameter.MAX_CMP_NAME_LEN);
    sMessage : STRING(LogParameter.MAX_LOG_MSG_LEN);
END_VAR
END_METHOD
END_INTERFACE
END_NAMESPACE
"#,
        ),
        (
            "types.st",
            r#"
TYPE Label : STRING(LogParameter.MAX_CMP_NAME_LEN);
Frame : STRING(WagoTypesLog.LogParameter.MAX_LOG_MSG_LEN);
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
        ),
    ]);

    assert_eq!(string_capacity(&runtime, "Label"), 20);
    assert_eq!(string_capacity(&runtime, "Frame"), 128);
}

#[test]
fn derived_type_constant_gvl_qualified_declared_size_folds_exactly() {
    let runtime = codesys_runtime(&[
        (
            "gvl.st",
            r#"
NAMESPACE GVLMqtt
VAR_GLOBAL
    udiLenMsgBuffer : UDINT := 2048;
END_VAR
END_NAMESPACE
"#,
        ),
        (
            "types.st",
            r#"
TYPE Payload : STRING(GVLMqtt.udiLenMsgBuffer);
Samples : ARRAY[0..(GVLMqtt.udiLenMsgBuffer - 1)] OF INT;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
        ),
    ]);

    assert_eq!(string_capacity(&runtime, "Payload"), 2048);
    assert_eq!(array_dimensions(&runtime, "Samples"), [(0, 2047)]);
}

#[test]
fn derived_type_constant_rejects_ambiguous_leaf_container_reference() {
    let error = compile_error(&[
        (
            "stubs.st",
            r#"
NAMESPACE CellA
VAR_GLOBAL CONSTANT
    MAX_LEN : USINT := 20;
END_VAR
END_NAMESPACE

NAMESPACE CellB
VAR_GLOBAL CONSTANT
    MAX_LEN : USINT := 128;
END_VAR
END_NAMESPACE
"#,
        ),
        (
            "types.st",
            r#"
TYPE Label : STRING(Phantom.MAX_LEN);
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
        ),
    ]);

    assert!(error.to_ascii_lowercase().contains("max_len"), "{error}");
}

// The unique-leaf tolerance lives only in declaration-size lowering: an
// ordinary initializer through the same flattened-container spelling must
// keep the strict diagnostic.
#[test]
fn derived_type_constant_rejects_flattened_container_in_plain_initializer() {
    let error = compile_error(&[(
        "initializer.st",
        r#"
NAMESPACE WagoTypesLog
VAR_GLOBAL CONSTANT
    MAX_LOG_MSG_LEN : USINT := 128;
END_VAR
END_NAMESPACE
VAR_GLOBAL
    Copied : INT := LogParameter.MAX_LOG_MSG_LEN;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(
        error.to_ascii_lowercase().contains("max_log_msg_len"),
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
