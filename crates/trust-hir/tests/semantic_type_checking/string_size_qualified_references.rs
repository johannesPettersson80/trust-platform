//! Declaration sizes naming qualified constants: GVL-qualified, dotted
//! library-namespace-qualified, and flattened-library-container shapes
//! (IEC_DEVIATIONS 2026-10-21 and 2026-10-25).

use crate::common::{
    check_has_error, Database, DiagnosticCode, DiagnosticSeverity, Dialect, FileId, SemanticDatabase,
    SourceDatabase, Type,
};

fn string_capacity(db: &Database, file: FileId, variable: &str) -> u32 {
    // `analyze` runs the project analysis (cross-file const roots merged);
    // `file_symbols` is the single-file view without them.
    let symbols = db.analyze(file).symbols.clone();
    let symbol = symbols
        .iter()
        .find(|s| s.name == variable)
        .unwrap_or_else(|| panic!("symbol '{variable}' should be collected"));
    let type_id = symbols.resolve_alias_type(symbol.type_id);
    match symbols.type_by_id(type_id) {
        Some(Type::String { max_len }) => {
            max_len.unwrap_or_else(|| panic!("'{variable}' should have a folded size"))
        }
        other => panic!("'{variable}' should be a sized string, got {other:?}"),
    }
}

fn error_codes(db: &Database, file: FileId) -> Vec<DiagnosticCode> {
    db.diagnostics(file)
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .map(|diagnostic| diagnostic.code)
        .collect()
}

// The WagoTypesLog stub shape: the library's `LogParameter` GVL level is
// flattened into namespace-level constants, while the documentation-faithful
// references keep the dotted spelling (IEC_DEVIATIONS 2026-10-25).
#[test]
fn string_length_from_flattened_library_container_inside_namespace() {
    let mut db = Database::new();
    db.set_dialect(Dialect::Codesys);
    db.set_source_text(
        FileId(0),
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
"#
        .to_string(),
    );

    let errors = error_codes(&db, FileId(0));
    assert!(
        errors.is_empty(),
        "flattened-container dotted sizes must resolve, got {errors:?}"
    );
    assert_eq!(string_capacity(&db, FileId(0), "sLoggerName"), 20);
    assert_eq!(string_capacity(&db, FileId(0), "sMessage"), 128);
}

#[test]
fn string_length_from_dotted_library_path_across_files() {
    let mut db = Database::new();
    db.set_dialect(Dialect::Codesys);
    db.set_source_text(
        FileId(0),
        r#"
NAMESPACE WagoTypesLog
VAR_GLOBAL CONSTANT
    MAX_LOG_MSG_LEN : USINT := 128;
END_VAR
END_NAMESPACE
"#
        .to_string(),
    );
    db.set_source_text(
        FileId(1),
        r#"
FUNCTION_BLOCK fbLog
VAR
    sFrame : STRING(WagoTypesLog.LogParameter.MAX_LOG_MSG_LEN);
END_VAR
END_FUNCTION_BLOCK
"#
        .to_string(),
    );

    let errors = error_codes(&db, FileId(1));
    assert!(
        errors.is_empty(),
        "three-part dotted library size must resolve across files, got {errors:?}"
    );
    assert_eq!(string_capacity(&db, FileId(1), "sFrame"), 128);
}

// Bare names never get the tolerance: a constant that lives only inside a
// namespace stays unimported for callers without USING or a global-scope
// declaration — matching the harness no-leak contract for unimported
// namespace operands (the s4 stubs expose their constants at global scope
// as well, which is how their bare references resolve).
#[test]
fn bare_reference_to_namespaced_constant_stays_undefined() {
    let mut db = Database::new();
    db.set_dialect(Dialect::Codesys);
    db.set_source_text(
        FileId(0),
        r#"
NAMESPACE WagoAppJSON
VAR_GLOBAL CONSTANT
    JSON_MAX_PARAMETER_STRING : UDINT := 80;
END_VAR
END_NAMESPACE
"#
        .to_string(),
    );
    db.set_source_text(
        FileId(1),
        r#"
FUNCTION_BLOCK fbPayload
VAR
    asParameters : ARRAY[0..3] OF STRING(JSON_MAX_PARAMETER_STRING);
END_VAR
END_FUNCTION_BLOCK
"#
        .to_string(),
    );

    let errors = error_codes(&db, FileId(1));
    assert!(
        errors.contains(&DiagnosticCode::UndefinedVariable),
        "bare reference to a namespace-only constant must keep the strict diagnostic, got {errors:?}"
    );
}

#[test]
fn string_length_from_gvl_qualified_declared_variable_across_files() {
    let mut db = Database::new();
    db.set_dialect(Dialect::Codesys);
    db.set_source_text(
        FileId(0),
        r#"
NAMESPACE GVLMqtt
VAR_GLOBAL
    udiLenMsgBuffer : UDINT := 2048;
END_VAR
END_NAMESPACE
"#
        .to_string(),
    );
    db.set_source_text(
        FileId(1),
        r#"
FUNCTION_BLOCK fbPayload
VAR
    sPayload : STRING(GVLMqtt.udiLenMsgBuffer);
END_VAR
END_FUNCTION_BLOCK
"#
        .to_string(),
    );

    let errors = error_codes(&db, FileId(1));
    assert!(
        errors.is_empty(),
        "GVL-qualified declared size must resolve across files, got {errors:?}"
    );
    assert_eq!(string_capacity(&db, FileId(1), "sPayload"), 2048);
}

// G3: a consumer-side TEMP section may take its size from the referenced
// constant; TEMP exclusion only keeps TEMP initializers from BECOMING sizes.
#[test]
fn temp_section_string_size_resolves_from_gvl_constant() {
    let mut db = Database::new();
    db.set_dialect(Dialect::Codesys);
    db.set_source_text(
        FileId(0),
        r#"
NAMESPACE GVLMqtt
VAR_GLOBAL
    udiLenMsgBuffer : UDINT := 2048;
END_VAR
END_NAMESPACE
"#
        .to_string(),
    );
    db.set_source_text(
        FileId(1),
        r#"
PROGRAM MqttAutodiscovery
VAR
    x : INT := 2;
END_VAR
VAR_TEMP
    sDiag : STRING(GVLMqtt.udiLenMsgBuffer);
END_VAR
END_PROGRAM
"#
        .to_string(),
    );

    let errors = error_codes(&db, FileId(1));
    assert!(
        errors.is_empty(),
        "TEMP-section size site must resolve against the GVL constant, got {errors:?}"
    );
    assert_eq!(string_capacity(&db, FileId(1), "sDiag"), 2048);
}

#[test]
fn dut_member_string_size_from_flattened_library_constant() {
    let mut db = Database::new();
    db.set_dialect(Dialect::Codesys);
    db.set_source_text(
        FileId(0),
        r#"
NAMESPACE WagoTypesLog
VAR_GLOBAL CONSTANT
    MAX_CMP_NAME_LEN : USINT := 20;
END_VAR
END_NAMESPACE

TYPE stLogRecord : STRUCT
    sName : STRING(WagoTypesLog.LogParameter.MAX_CMP_NAME_LEN);
END_STRUCT
END_TYPE
"#
        .to_string(),
    );

    let errors = error_codes(&db, FileId(0));
    assert!(
        errors.is_empty(),
        "DUT-member dotted size must resolve, got {errors:?}"
    );
}

#[test]
fn ambiguous_leaf_stays_undefined() {
    let mut db = Database::new();
    db.set_dialect(Dialect::Codesys);
    db.set_source_text(
        FileId(0),
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
"#
        .to_string(),
    );
    db.set_source_text(
        FileId(1),
        r#"
FUNCTION_BLOCK fbUse
VAR
    sValue : STRING(Phantom.MAX_LEN);
END_VAR
END_FUNCTION_BLOCK
"#
        .to_string(),
    );

    let errors = error_codes(&db, FileId(1));
    assert!(
        errors.contains(&DiagnosticCode::UndefinedVariable),
        "ambiguous leaf must keep the strict undefined-constant diagnostic, got {errors:?}"
    );
}

// A dotted reference through a scope-local value (an FB instance member
// access) is not a qualified constant and must not rebind to a foreign
// same-named leaf.
#[test]
fn member_access_leaf_is_not_rebound_to_foreign_constant() {
    check_has_error(
        r#"
NAMESPACE WagoTypesLog
VAR_GLOBAL CONSTANT
    MAX_LOG_MSG_LEN : USINT := 128;
END_VAR
END_NAMESPACE

FUNCTION_BLOCK fbBase
VAR
    MAX_LOG_MSG_LEN : UINT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK fbUser
VAR
    inst : fbBase;
    sMessage : STRING(inst.MAX_LOG_MSG_LEN);
END_VAR
END_FUNCTION_BLOCK
"#,
        DiagnosticCode::UndefinedVariable,
    );
}
