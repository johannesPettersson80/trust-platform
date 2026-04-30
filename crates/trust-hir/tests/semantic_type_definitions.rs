mod common;
use common::*;

// Type Definition Tests
#[test]
fn test_struct_type_fields() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
TYPE Point : STRUCT
    x : REAL;
    y : REAL;
END_STRUCT
END_TYPE
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);

    // Find the type symbol
    let point_sym = symbols.iter().find(|s| s.name == "Point").unwrap();
    assert!(matches!(point_sym.kind, SymbolKind::Type));

    // Check that the type was registered
    let type_id = symbols.lookup_registered_type_name("Point");
    assert!(type_id.is_some(), "Point type should be registered");
}

#[test]
fn test_struct_field_access() {
    check_no_errors(
        r#"
TYPE Point : STRUCT
    x : DINT;
END_STRUCT
END_TYPE

PROGRAM Test
    VAR p : Point; END_VAR
    p.x := 1;
END_PROGRAM
"#,
    );
}

#[test]
fn test_enum_type() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
TYPE Color : (Red, Green, Blue)
END_TYPE
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);

    // Find the type symbol
    let color_sym = symbols.iter().find(|s| s.name == "Color").unwrap();
    assert!(matches!(color_sym.kind, SymbolKind::Type));

    // Check that the type was registered
    let type_id = symbols.lookup_registered_type_name("Color");
    assert!(type_id.is_some(), "Color type should be registered");
}

#[test]
fn test_class_type_registered() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
CLASS Motor
VAR
    Speed : INT;
END_VAR
METHOD Start
END_METHOD
END_CLASS
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);
    let class_sym = symbols.iter().find(|s| s.name == "Motor").unwrap();
    assert!(matches!(class_sym.kind, SymbolKind::Class));

    let speed_sym = symbols.iter().find(|s| s.name == "Speed").unwrap();
    assert_eq!(speed_sym.parent, Some(class_sym.id));

    let type_id = symbols.lookup_registered_type_name("Motor");
    assert!(type_id.is_some(), "Motor type should be registered");
    let ty = symbols.type_by_id(type_id.unwrap()).unwrap();
    assert!(matches!(ty, Type::Class { .. }));
}

#[test]
fn test_class_modifiers_and_visibility_collected() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
CLASS ABSTRACT Motor
VAR
    Speed : INT;
END_VAR
VAR PRIVATE
    Secret : INT;
END_VAR
METHOD Start
END_METHOD
METHOD PUBLIC ABSTRACT Calibrate
END_METHOD
END_CLASS
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);
    let class_sym = symbols.iter().find(|s| s.name == "Motor").unwrap();
    assert!(matches!(class_sym.kind, SymbolKind::Class));
    assert!(class_sym.modifiers.is_abstract);
    assert!(!class_sym.modifiers.is_final);

    let speed_sym = symbols.iter().find(|s| s.name == "Speed").unwrap();
    assert_eq!(speed_sym.visibility, Visibility::Protected);

    let secret_sym = symbols.iter().find(|s| s.name == "Secret").unwrap();
    assert_eq!(secret_sym.visibility, Visibility::Private);

    let start_sym = symbols
        .iter()
        .find(|s| s.name == "Start" && matches!(s.kind, SymbolKind::Method { .. }))
        .unwrap();
    assert_eq!(start_sym.visibility, Visibility::Protected);

    let calibrate_sym = symbols
        .iter()
        .find(|s| s.name == "Calibrate" && matches!(s.kind, SymbolKind::Method { .. }))
        .unwrap();
    assert!(calibrate_sym.modifiers.is_abstract);
    assert_eq!(calibrate_sym.visibility, Visibility::Public);
}

#[test]
fn test_method_override_modifier_collected() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
CLASS Base
METHOD PUBLIC DoIt
END_METHOD
END_CLASS

CLASS Derived EXTENDS Base
METHOD PUBLIC OVERRIDE DoIt
END_METHOD
END_CLASS
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);
    let derived_sym = symbols.iter().find(|s| s.name == "Derived").unwrap();
    let override_sym = symbols
        .iter()
        .find(|s| s.name == "DoIt" && s.parent == Some(derived_sym.id))
        .unwrap();
    assert!(override_sym.modifiers.is_override);
}

#[test]
fn test_class_extends_final_error() {
    check_has_error(
        r#"
CLASS FINAL Base
END_CLASS

CLASS Derived EXTENDS Base
END_CLASS
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_override_requires_base_method() {
    check_has_error(
        r#"
CLASS Derived
METHOD PUBLIC OVERRIDE DoIt
END_METHOD
END_CLASS
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_override_requires_override_keyword() {
    check_has_error(
        r#"
CLASS Base
METHOD PUBLIC DoIt
END_METHOD
END_CLASS

CLASS Derived EXTENDS Base
METHOD PUBLIC DoIt
END_METHOD
END_CLASS
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_override_signature_mismatch_error() {
    check_has_error(
        r#"
CLASS Base
METHOD PUBLIC DoIt : INT
END_METHOD
END_CLASS

CLASS Derived EXTENDS Base
METHOD PUBLIC OVERRIDE DoIt : DINT
END_METHOD
END_CLASS
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_override_visibility_mismatch_error() {
    check_has_error(
        r#"
CLASS Base
METHOD PUBLIC DoIt
END_METHOD
END_CLASS

CLASS Derived EXTENDS Base
METHOD PROTECTED OVERRIDE DoIt
END_METHOD
END_CLASS
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_abstract_class_requires_abstract_method() {
    check_has_error(
        r#"
CLASS ABSTRACT Base
METHOD PUBLIC DoIt
END_METHOD
END_CLASS
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_abstract_method_requires_abstract_class() {
    check_has_error(
        r#"
CLASS Base
METHOD PUBLIC ABSTRACT DoIt
END_METHOD
END_CLASS
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_override_final_method_error() {
    check_has_error(
        r#"
CLASS Base
METHOD PUBLIC FINAL DoIt
END_METHOD
END_CLASS

CLASS Derived EXTENDS Base
METHOD PUBLIC OVERRIDE DoIt
END_METHOD
END_CLASS
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_non_abstract_class_missing_abstract_base_method_error() {
    check_has_error(
        r#"
CLASS ABSTRACT Base
METHOD PUBLIC ABSTRACT DoIt
END_METHOD
END_CLASS

CLASS Derived EXTENDS Base
END_CLASS
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_abstract_class_instantiation_error() {
    check_has_error(
        r#"
CLASS ABSTRACT Base
METHOD PUBLIC ABSTRACT DoIt
END_METHOD
END_CLASS

PROGRAM Test
VAR
    x: Base;
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_inherited_variable_name_conflict_error() {
    check_has_error(
        r#"
CLASS Base
VAR
    Value: INT;
END_VAR
END_CLASS

CLASS Derived EXTENDS Base
VAR
    Value: INT;
END_VAR
END_CLASS
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_method_conflicts_with_inherited_variable_error() {
    check_has_error(
        r#"
CLASS Base
VAR
    Value: INT;
END_VAR
END_CLASS

CLASS Derived EXTENDS Base
METHOD PUBLIC Value
END_METHOD
END_CLASS
"#,
        DiagnosticCode::InvalidOperation,
    );
}
