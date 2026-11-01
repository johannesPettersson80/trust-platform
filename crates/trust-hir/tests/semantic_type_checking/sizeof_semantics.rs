use super::common::*;
use trust_hir::types::POINTER_REFERENCE_HANDLE_SIZE_BYTES;

fn error_messages(source: &str) -> Vec<String> {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    db.diagnostics(file)
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .map(ToString::to_string)
        .collect()
}

#[test]
fn test_sizeof_accepts_variable_operand() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR
        size : DINT;
        x : DINT;
    END_VAR
    size := SIZEOF(x);
END_PROGRAM
"#,
    );
}

#[test]
fn test_sizeof_accepts_explicit_type_operand() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR size : DINT; END_VAR
    size := SIZEOF(DINT);
    size := SIZEOF(ARRAY[0..3] OF BYTE);
END_PROGRAM
"#,
    );
}

#[test]
fn test_sizeof_string_length_const_eval_error_reports_primary_diagnostic() {
    let messages = error_messages(
        r#"
PROGRAM Test
    VAR size : DINT; END_VAR
    size := SIZEOF(STRING[1 / 0]);
END_PROGRAM
"#,
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("constant expression divides by zero")),
        "expected divide-by-zero const-eval diagnostic, got {messages:?}"
    );
}

#[test]
fn test_sizeof_accepts_this_field_operand_inside_method() {
    check_no_errors(
        r#"
CLASS Counter
VAR
    value : DINT;
END_VAR

METHOD PUBLIC Measure : DINT
Measure := SIZEOF(THIS.value);
END_METHOD
END_CLASS
"#,
    );
}

#[test]
fn test_sizeof_accepts_variable_operand_in_array_bounds() {
    check_no_errors(
        r#"
TYPE Packet :
STRUCT
    a : DINT;
    b : BOOL;
END_STRUCT
END_TYPE

PROGRAM Test
VAR
    packet : Packet;
    bytes : ARRAY[0..SIZEOF(packet)-1] OF BYTE;
END_VAR
END_PROGRAM
"#,
    );
}

#[test]
fn test_sizeof_bare_name_prefers_variable_over_top_level_type_in_array_bounds() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
TYPE Packet : LWORD; END_TYPE

PROGRAM Test
VAR
    Packet : INT;
    bytes : ARRAY[0..SIZEOF(Packet)-1] OF BYTE;
END_VAR
END_PROGRAM
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);
    let bytes = symbols.iter().find(|s| s.name == "bytes").unwrap();
    let type_id = symbols.resolve_alias_type(bytes.type_id);
    let Type::Array { dimensions, .. } = symbols.type_by_id(type_id).unwrap() else {
        panic!("expected array type");
    };
    assert_eq!(dimensions, &vec![(0, 1)]);
}

#[test]
fn test_sizeof_pointer_operand_const_folds_in_array_bounds() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
PROGRAM Test
VAR
    p : POINTER TO INT;
    bytes : ARRAY[0..SIZEOF(p)-1] OF BYTE;
END_VAR
END_PROGRAM
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);
    let bytes = symbols.iter().find(|s| s.name == "bytes").unwrap();
    let type_id = symbols.resolve_alias_type(bytes.type_id);
    let Type::Array { dimensions, .. } = symbols.type_by_id(type_id).unwrap() else {
        panic!("expected array type");
    };
    let expected_upper =
        i64::try_from(POINTER_REFERENCE_HANDLE_SIZE_BYTES).expect("handle size fits") - 1;
    assert_eq!(dimensions, &vec![(0, expected_upper)]);
}

#[test]
fn test_sizeof_pointer_contract_matches_platform_word_size() {
    assert_eq!(
        POINTER_REFERENCE_HANDLE_SIZE_BYTES,
        std::mem::size_of::<usize>() as u64
    );
    #[cfg(target_pointer_width = "64")]
    assert_eq!(POINTER_REFERENCE_HANDLE_SIZE_BYTES, 8);
    #[cfg(target_pointer_width = "32")]
    assert_eq!(POINTER_REFERENCE_HANDLE_SIZE_BYTES, 4);
}

#[test]
fn test_sizeof_rejects_call_operand() {
    check_has_error(
        r#"
FUNCTION Value : DINT
Value := DINT#1;
END_FUNCTION

PROGRAM Test
    VAR size : DINT; END_VAR
    size := SIZEOF(Value());
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_sizeof_rejects_non_lvalue_expression_operand() {
    check_has_error(
        r#"
PROGRAM Test
    VAR
        size : DINT;
        x : DINT;
    END_VAR
    size := SIZEOF(x + DINT#1);
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_sizeof_rejects_unknown_identifier_cleanly() {
    let messages = error_messages(
        r#"
PROGRAM Test
    VAR size : DINT; END_VAR
    size := SIZEOF(DoesNotExist);
END_PROGRAM
"#,
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("SIZEOF") && message.contains("DoesNotExist")),
        "expected SIZEOF-specific unknown-identifier diagnostic, got {messages:?}"
    );
}

#[test]
fn test_sizeof_ambiguous_using_value_reports_primary_only() {
    let codes = check_errors(
        r#"
NAMESPACE A
VAR_GLOBAL
    Shared : INT;
END_VAR
END_NAMESPACE

NAMESPACE B
VAR_GLOBAL
    Shared : INT;
END_VAR
END_NAMESPACE

USING A;
USING B;

PROGRAM Test
    VAR size : DINT; END_VAR
    size := SIZEOF(Shared);
END_PROGRAM
"#,
    );
    assert!(
        codes.contains(&DiagnosticCode::CannotResolve),
        "expected CannotResolve for ambiguous SIZEOF operand, got {codes:?}"
    );
    assert_eq!(
        codes
            .iter()
            .filter(|code| **code == DiagnosticCode::CannotResolve)
            .count(),
        1,
        "ambiguous SIZEOF operand must emit one primary CannotResolve, got {codes:?}"
    );
    assert!(
        !codes.contains(&DiagnosticCode::InvalidOperation),
        "ambiguous SIZEOF operand must not degrade into InvalidOperation, got {codes:?}"
    );
}

#[test]
fn test_sizeof_rejects_open_array_operand() {
    check_has_error(
        r#"
FUNCTION_BLOCK BufferUser
VAR_IN_OUT
    arr : ARRAY[*] OF BYTE;
END_VAR
VAR
    size : DINT;
END_VAR
size := SIZEOF(arr);
END_FUNCTION_BLOCK
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_sizeof_rejects_function_block_instance_operand() {
    check_has_error(
        r#"
FUNCTION_BLOCK Counter
VAR
    value : DINT;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Test
    VAR
        size : DINT;
        fb : Counter;
    END_VAR
    size := SIZEOF(fb);
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_sizeof_rejects_this_operand_for_unsupported_receiver_storage_size() {
    check_has_error(
        r#"
CLASS Counter
VAR
    value : DINT;
END_VAR

METHOD PUBLIC Measure : DINT
Measure := SIZEOF(THIS);
END_METHOD
END_CLASS
"#,
        DiagnosticCode::InvalidOperation,
    );
}

/// The dimensions of the array type `name`, which `source` declares without an error.
fn declared_array_dimensions(source: &str, name: &str) -> Vec<(i64, i64)> {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    let diagnostics = db.diagnostics(file);
    assert!(
        !diagnostics.iter().any(|diagnostic| diagnostic.is_error()),
        "{diagnostics:?}"
    );
    let symbols = db.file_symbols(file);
    let symbol = symbols
        .get(symbols.lookup(name).expect("type declaration"))
        .expect("declared symbol");
    let resolved = symbols.resolve_alias_type(symbol.type_id);
    let Some(Type::Array { dimensions, .. }) = symbols.type_by_id(resolved) else {
        panic!("expected array type")
    };
    dimensions.clone()
}

#[test]
fn test_sizeof_temporal_types_are_eight_bytes_in_array_bounds() {
    // TIME, DATE, TOD and DT occupy eight bytes on every platform, so SIZEOF(..)-1 is 7; a
    // four-byte type would give 3.
    for type_name in ["TIME", "DATE", "TOD", "DT"] {
        let dimensions = declared_array_dimensions(
            &format!("TYPE Bytes : ARRAY[0..SIZEOF({type_name})-1] OF BYTE; END_TYPE"),
            "Bytes",
        );
        assert_eq!(dimensions, vec![(0, 7)], "{type_name}");
    }
}

#[test]
fn test_sizeof_located_structure_includes_gaps_and_temporal_extent() {
    // marker takes byte 0 and stamp takes bytes 8 through 15, so the structure extends over 16
    // bytes, the gap included. Summing the field sizes would give 9.
    let dimensions = declared_array_dimensions(
        r#"
TYPE Packet :
STRUCT
    marker AT %B0 : BYTE;
    stamp AT %B8 : TIME;
END_STRUCT;
Bytes : ARRAY[0..SIZEOF(Packet)-1] OF BYTE;
END_TYPE
"#,
        "Bytes",
    );
    assert_eq!(dimensions, vec![(0, 15)]);
}

#[test]
fn test_relative_layout_gives_a_temporal_field_eight_bytes() {
    // stamp takes bytes 0 through 7: a field at byte 4 overlaps it (a four-byte TIME would not)
    // and a field at byte 8 does not (a longer TIME would).
    let overlaps = |tail_address: &str| {
        error_messages(&format!(
            "TYPE Packet : STRUCT stamp AT %B0 : TIME; tail AT {tail_address} : INT; \
             END_STRUCT; END_TYPE"
        ))
        .iter()
        .any(|message| message.contains("relative structure fields overlap"))
    };
    assert!(overlaps("%B4"));
    assert!(!overlaps("%B8"));
}

#[test]
fn test_sizeof_result_binds_unsigned_and_warns_into_narrower_integers() {
    // CODESYS parity: SIZEOF yields an unsigned magnitude (the vendor
    // udiSize idiom), and narrower integer destinations take it under the
    // codesys narrowing warning — never an error. Under the default
    // profile the DINT-typed pins above keep governing.
    check_no_errors_in(
        Dialect::Codesys,
        r#"
PROGRAM Test
VAR
    x : DINT;
    unsignedSize : UDINT;
    signedSize : DINT;
    byteSize : BYTE;
END_VAR
unsignedSize := SIZEOF(x);
signedSize := SIZEOF(x);
byteSize := SIZEOF(x);
END_PROGRAM
"#,
    );
}
