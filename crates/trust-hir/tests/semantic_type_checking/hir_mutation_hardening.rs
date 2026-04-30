use crate::common::*;
use trust_hir::symbols::VarQualifier;
use trust_hir::{Diagnostic, TypeId};

fn error_diagnostics(source: &str) -> Vec<Diagnostic> {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    db.diagnostics(file)
        .iter()
        .filter(|diagnostic| diagnostic.is_error())
        .cloned()
        .collect()
}

fn assert_error_with_message(source: &str, code: DiagnosticCode, message: &str) -> Diagnostic {
    let errors = error_diagnostics(source);
    errors
        .iter()
        .find(|diagnostic| diagnostic.code == code && diagnostic.message == message)
        .cloned()
        .unwrap_or_else(|| panic!("expected {code:?} with message {message:?}, got {errors:?}"))
}

fn range_text<'a>(source: &'a str, diagnostic: &Diagnostic) -> &'a str {
    &source[usize::from(diagnostic.range.start())..usize::from(diagnostic.range.end())]
}

fn assert_duplicate_case_label(prelude: &str, first_label: &str, second_label: &str) {
    let source = format!(
        r#"
{prelude}

PROGRAM Test
VAR
    x : INT;
END_VAR
CASE x OF
    {first_label}: x := 1;
    {second_label}: x := 2;
    ELSE
        x := 3;
END_CASE;
END_PROGRAM
"#
    );
    assert_error_with_message(
        &source,
        DiagnosticCode::InvalidOperation,
        "duplicate CASE label",
    );
}

#[test]
fn case_label_const_eval_matrix_reports_exact_duplicate_case_diagnostics() {
    let global_const = r#"
VAR_GLOBAL CONSTANT
    K : INT := 5;
END_VAR
"#;
    for (first, second) in [("K", "5"), ("(5)", "5"), ("-1", "-1")] {
        assert_duplicate_case_label(global_const, first, second);
    }
}

#[test]
fn case_label_local_const_scope_chain_reports_exact_duplicate_case_diagnostic() {
    let source = r#"
PROGRAM Test
VAR CONSTANT
    LocalLimit : INT := 6;
END_VAR
VAR
    x : INT;
END_VAR
CASE x OF
    LocalLimit: x := 1;
    6: x := 2;
    ELSE
        x := 3;
END_CASE;
END_PROGRAM
"#;
    assert_error_with_message(
        source,
        DiagnosticCode::InvalidOperation,
        "duplicate CASE label",
    );
}

#[test]
fn type_resolution_const_eval_matrix_preserves_integer_expression_values() {
    let source = r#"
VAR_GLOBAL CONSTANT
    Five : INT := 5;
END_VAR

TYPE
    NameSized : STRING[Five];
    AddSized : STRING[2 + 3];
    SubSized : STRING[7 - 2];
    DivSized : WSTRING[4 / 2];
    ModSized : STRING[5 MOD 2];
    PowerSized : STRING[2 ** 3];
    ZeroPowerRange : INT (2 ** 0..2 ** 3);
END_TYPE
"#;
    check_no_errors(source);

    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    let symbols = db.file_symbols(file);

    for (name, expected) in [
        ("NameSized", 5),
        ("AddSized", 5),
        ("SubSized", 5),
        ("ModSized", 1),
        ("PowerSized", 8),
    ] {
        let type_id = symbols
            .lookup_registered_type_name(name)
            .expect("sized string type");
        let type_id = symbols.resolve_alias_type(type_id);
        let Type::String { max_len } = symbols.type_by_id(type_id).expect("string type") else {
            panic!("{name} should be a STRING type");
        };
        assert_eq!(*max_len, Some(expected), "{name}");
    }

    let type_id = symbols
        .lookup_registered_type_name("DivSized")
        .expect("sized WSTRING type");
    let type_id = symbols.resolve_alias_type(type_id);
    let Type::WString { max_len } = symbols.type_by_id(type_id).expect("wstring type") else {
        panic!("DivSized should be a WSTRING type");
    };
    assert_eq!(*max_len, Some(2));

    let range_id = symbols
        .lookup_registered_type_name("ZeroPowerRange")
        .expect("subrange type");
    let range_id = symbols.resolve_alias_type(range_id);
    let Type::Subrange { lower, upper, .. } = symbols.type_by_id(range_id).expect("subrange type")
    else {
        panic!("ZeroPowerRange should be a subrange type");
    };
    assert_eq!((*lower, *upper), (1, 8));
}

#[test]
fn type_resolution_const_eval_errors_report_primary_diagnostic() {
    let source = r#"
TYPE
    BadSized : STRING[4 / 0];
END_TYPE
"#;
    assert_error_with_message(
        source,
        DiagnosticCode::InvalidOperation,
        "constant expression divides by zero",
    );
}

#[test]
fn array_index_const_eval_matrix_reports_exact_out_of_range_values() {
    for (expr, lower, upper, expected_value) in [
        ("2 + 3", 1, 4, 5),
        ("7 - 2", 1, 4, 5),
        ("2 * 3", 1, 5, 6),
        ("4 / 2", 3, 5, 2),
        ("5 MOD 2", 2, 4, 1),
        ("2 ** 3", 1, 7, 8),
        ("2 ** 0", 2, 3, 1),
    ] {
        let source = format!(
            r#"
PROGRAM Test
VAR
    arr : ARRAY[{lower}..{upper}] OF INT;
END_VAR
arr[{expr}] := 1;
END_PROGRAM
"#
        );
        assert_error_with_message(
            &source,
            DiagnosticCode::OutOfRange,
            &format!("array index {expected_value} outside bounds {lower}..{upper}"),
        );
    }
}

#[test]
fn wstring_field_default_length_reports_e304_on_literal() {
    let source = r#"
TYPE
    StepCfg : STRUCT
        name : WSTRING[3] := "hello";
    END_STRUCT;
END_TYPE
"#;

    let diagnostic = assert_error_with_message(
        source,
        DiagnosticCode::OutOfRange,
        "WSTRING literal length 5 exceeds WSTRING[3] capacity",
    );
    assert_eq!(range_text(source, &diagnostic).trim(), r#""hello""#);
}

#[test]
fn union_aggregate_initializer_validates_variant_names_and_locations() {
    check_no_errors(
        r#"
TYPE
    Choice : UNION
        intValue : INT;
        boolValue : BOOL;
    END_UNION;
END_TYPE

PROGRAM Main
VAR
    choice : Choice := (boolValue := TRUE);
END_VAR
END_PROGRAM
"#,
    );

    let source = r#"
TYPE
    Choice : UNION
        intValue : INT;
    END_UNION;
END_TYPE

PROGRAM Main
VAR
    choice : Choice := (missing := 1);
END_VAR
END_PROGRAM
"#;
    let diagnostic = assert_error_with_message(
        source,
        DiagnosticCode::UndefinedField,
        "unknown aggregate field 'missing'",
    );
    assert_eq!(range_text(source, &diagnostic), "missing");
}

#[test]
fn aggregate_initializer_unknown_target_type_does_not_emit_cascade_field_errors() {
    let errors = error_diagnostics(
        r#"
PROGRAM Main
VAR
    cfg : MissingType := (field := 1);
END_VAR
END_PROGRAM
"#,
    );

    assert!(
        errors
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::UndefinedType),
        "{errors:?}"
    );
    assert!(
        errors.iter().all(
            |diagnostic| diagnostic.code != DiagnosticCode::UndefinedField
                && diagnostic.code != DiagnosticCode::DuplicateField
                && diagnostic.code != DiagnosticCode::TypeMismatch
        ),
        "unknown target type must not cascade aggregate validation errors: {errors:?}"
    );
}

#[test]
fn type_level_array_defaults_validate_elements_and_repetition() {
    for initializer in ["[0, 200]", "[2(200)]"] {
        let source = format!(
            r#"
TYPE
    SmallValues : ARRAY[1..2] OF SINT := {initializer};
END_TYPE
"#
        );
        assert_error_with_message(
            &source,
            DiagnosticCode::OutOfRange,
            "integer default 200 is outside target type range -128..127",
        );
    }
}

#[test]
fn direct_array_repeat_default_validates_repeated_element_type() {
    let source = r#"
TYPE
    SmallValues : ARRAY[1..2] OF SINT := 2(200);
END_TYPE
"#;
    assert_error_with_message(
        source,
        DiagnosticCode::OutOfRange,
        "integer default 200 is outside target type range -128..127",
    );
}

#[test]
fn array_default_rejects_non_repeat_call_expression() {
    let source = r#"
FUNCTION MakeValue : SINT
    MakeValue := 1;
END_FUNCTION

TYPE
    SmallValues : ARRAY[1..2] OF SINT := MakeValue();
END_TYPE
"#;
    assert_error_with_message(
        source,
        DiagnosticCode::TypeMismatch,
        "array default initializer requires an array initializer or repetition expression",
    );
}

#[test]
fn nested_struct_and_union_defaults_validate_member_required_types() {
    let source = r#"
TYPE
    Inner : STRUCT
        ref : REF_TO INT;
        flag : BOOL;
    END_STRUCT;
    Choice : UNION
        ref : REF_TO INT;
        flag : BOOL;
    END_UNION;
    Wrapped : STRUCT
        inner : Inner := (flag := 1, ref := NULL);
        choice : Choice := (flag := 1);
    END_STRUCT;
END_TYPE
"#;

    let errors = error_diagnostics(source);
    let bool_mismatches = errors
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == DiagnosticCode::TypeMismatch
                && diagnostic.message == "BOOL default initializer requires a Boolean value"
        })
        .count();
    assert_eq!(
        bool_mismatches, 2,
        "struct and union nested defaults should each validate BOOL member defaults: {errors:?}"
    );
    assert!(
        errors
            .iter()
            .all(|diagnostic| diagnostic.message != "reference type/member defaults must be NULL"),
        "literal NULL in nested aggregate default should stay accepted: {errors:?}"
    );
}

#[test]
fn reference_null_default_is_allowed_but_non_null_reference_default_is_rejected() {
    check_no_errors(
        r#"
TYPE
    RefHolder : STRUCT
        next : REF_TO INT := NULL;
    END_STRUCT;
END_TYPE
"#,
    );

    let source = r#"
VAR_GLOBAL
    target : INT;
END_VAR

TYPE
    RefHolder : STRUCT
        next : REF_TO INT := REF(target);
    END_STRUCT;
END_TYPE
"#;
    assert_error_with_message(
        source,
        DiagnosticCode::InvalidOperation,
        "reference type/member defaults must be NULL",
    );

    let source = r#"
TYPE
    RefHolder : STRUCT
        next : REF_TO INT := FALSE;
    END_STRUCT;
END_TYPE
"#;
    assert_error_with_message(
        source,
        DiagnosticCode::InvalidOperation,
        "reference type/member defaults must be NULL",
    );

    let source = r#"
TYPE
    RefHolder : STRUCT
        next : REF_TO INT := REF(NULL);
    END_STRUCT;
END_TYPE
"#;
    assert_error_with_message(
        source,
        DiagnosticCode::InvalidOperation,
        "reference type/member defaults must be NULL",
    );
}

#[test]
fn integer_default_bounds_matrix_reports_each_out_of_range_type() {
    check_no_errors(
        r#"
TYPE
    Limits : STRUCT
        s8_min : SINT := -128;
        s8_max : SINT := 127;
        i16_min : INT := -32768;
        i16_max : INT := 32767;
        i32_min : DINT := -2147483648;
        i32_max : DINT := 2147483647;
        i64_max : LINT := 9223372036854775807;
        u8_min : USINT := 0;
        u8_max : USINT := 255;
        u16_max : UINT := 65535;
        u32_max : UDINT := 4294967295;
        u64_max : ULINT := 9223372036854775807;
    END_STRUCT;
END_TYPE
"#,
    );

    let errors = error_diagnostics(
        r#"
TYPE
    Limits : STRUCT
        s8 : SINT := 128;
        i16 : INT := 32768;
        i32 : DINT := 2147483648;
        u8 : USINT := 256;
        u16 : UINT := 65536;
        u32 : UDINT := 4294967296;
        u64 : ULINT := -1;
    END_STRUCT;
END_TYPE
"#,
    );
    let out_of_range = errors
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::OutOfRange)
        .count();
    assert_eq!(
        out_of_range, 7,
        "every integer type should report one range error: {errors:?}"
    );
}

#[test]
fn var_block_collection_preserves_parameter_visibility_and_non_config_scope() {
    let source = r#"
FUNCTION_BLOCK InitFb
VAR_INPUT
    enable : BOOL;
END_VAR
VAR_OUTPUT
    count : INT;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR_GLOBAL
    programScoped : INT;
END_VAR
END_PROGRAM
"#;
    check_no_errors(source);

    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    let symbols = db.file_symbols(file);
    let fb_id = symbols.lookup("InitFb").expect("function block symbol");

    for (name, direction) in [
        ("enable", ParamDirection::In),
        ("count", ParamDirection::Out),
    ] {
        let symbol = symbols
            .iter()
            .find(|symbol| symbol.name.as_str() == name && symbol.parent == Some(fb_id))
            .unwrap_or_else(|| panic!("expected {name} to be collected under InitFb"));
        assert_eq!(symbol.visibility, Visibility::Public, "{name}");
        assert_eq!(symbol.kind, SymbolKind::Parameter { direction }, "{name}");
    }

    assert!(
        symbols.lookup("programScoped").is_none(),
        "VAR_GLOBAL inside a PROGRAM outside CONFIGURATION must not enter global lookup"
    );
    let program_scoped = symbols
        .iter()
        .find(|symbol| symbol.name.eq_ignore_ascii_case("programScoped"))
        .expect("program-scoped VAR_GLOBAL symbol");
    assert!(matches!(
        program_scoped.kind,
        SymbolKind::Variable {
            qualifier: VarQualifier::Global
        }
    ));
}

#[test]
fn subrange_default_bounds_are_enforced_at_both_edges() {
    check_no_errors(
        r#"
TYPE
    Small : INT (-2..2);
    Holder : STRUCT
        low : Small := -2;
        high : Small := 2;
    END_STRUCT;
END_TYPE
"#,
    );

    let errors = error_diagnostics(
        r#"
TYPE
    Small : INT (-2..2);
    Holder : STRUCT
        low : Small := -3;
        high : Small := 3;
    END_STRUCT;
END_TYPE
"#,
    );
    let out_of_range = errors
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::OutOfRange)
        .count();
    assert_eq!(out_of_range, 2, "{errors:?}");
}

#[test]
fn function_block_initializer_rejects_forbidden_member_kinds_with_locked_messages() {
    let source = r#"
FUNCTION_BLOCK InitFb
VAR_IN_OUT
    shared : INT;
END_VAR
VAR_TEMP
    scratch : INT;
END_VAR
VAR
    hidden : INT;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    fb : InitFb := (shared := 1, scratch := 2, hidden := 3);
END_VAR
END_PROGRAM
"#;

    for message in [
        "VAR_IN_OUT members cannot be initialized through aggregate syntax",
        "temporary and external members cannot be initialized through aggregate syntax",
        "private members cannot be initialized through aggregate syntax",
    ] {
        assert_error_with_message(source, DiagnosticCode::InvalidOperation, message);
    }
}

#[test]
fn cross_file_import_preserves_scalar_array_struct_union_and_alias_chain_types() {
    let mut db = Database::new();
    let lib = FileId(0);
    let main = FileId(1);
    db.set_source_text(
        lib,
        r#"
TYPE
    LibInt : DINT;
    LibArray : ARRAY[1..2] OF LibInt;
    LibStruct : STRUCT
        value : LibInt;
    END_STRUCT;
    LibUnion : UNION
        value : LibInt;
    END_UNION;
    LibAlias1 : LibInt;
    LibAlias2 : LibAlias1;
END_TYPE
"#
        .to_string(),
    );
    db.set_source_text(
        main,
        r#"
TYPE
    LocalCollision : STRUCT
        other : BOOL;
    END_STRUCT;
END_TYPE

PROGRAM Main
VAR
    arr : LibArray;
    st : LibStruct;
    un : LibUnion;
    aliasValue : LibAlias2;
END_VAR
END_PROGRAM
"#
        .to_string(),
    );

    let symbols = db.file_symbols_with_project(main);
    let array_id = symbols
        .lookup_registered_type_name("LibArray")
        .expect("imported array");
    let array_id = symbols.resolve_alias_type(array_id);
    let Type::Array {
        element,
        dimensions,
    } = symbols.type_by_id(array_id).expect("array type")
    else {
        panic!("LibArray should remain an array after import");
    };
    assert_eq!(dimensions, &vec![(1, 2)]);
    assert_eq!(symbols.resolve_alias_type(*element), TypeId::DINT);

    let struct_id = symbols
        .lookup_registered_type_name("LibStruct")
        .expect("imported struct");
    let Type::Struct { fields, .. } = symbols.type_by_id(struct_id).expect("struct type") else {
        panic!("LibStruct should remain a struct after import");
    };
    assert_eq!(fields.len(), 1);
    assert_eq!(symbols.resolve_alias_type(fields[0].type_id), TypeId::DINT);

    let union_id = symbols
        .lookup_registered_type_name("LibUnion")
        .expect("imported union");
    let Type::Union { variants, .. } = symbols.type_by_id(union_id).expect("union type") else {
        panic!("LibUnion should remain a union after import");
    };
    assert_eq!(variants.len(), 1);
    assert_eq!(
        symbols.resolve_alias_type(variants[0].type_id),
        TypeId::DINT
    );

    let alias_id = symbols
        .lookup_registered_type_name("LibAlias2")
        .expect("imported alias");
    assert_eq!(symbols.resolve_alias_type(alias_id), TypeId::DINT);
}

#[test]
fn cross_file_import_preserves_all_compound_type_shapes_under_type_id_collisions() {
    let mut db = Database::new();
    let lib = FileId(0);
    let main = FileId(1);
    db.set_source_text(
        lib,
        r#"
TYPE
    LibArray : ARRAY[1..2] OF DINT;
    LibUnion : UNION
        active : BOOL;
    END_UNION;
    LibEnum : (Idle := 1, Run := 2);
    LibPointer : POINTER TO DINT;
    LibReference : REF_TO DINT;
    LibSubrange : DINT (-2..2);
    LibAlias : LibArray;
    LibString : STRING[7];
    LibWString : WSTRING[5];
END_TYPE

FUNCTION_BLOCK LibFb
END_FUNCTION_BLOCK

CLASS LibClass
END_CLASS

INTERFACE LibInterface
END_INTERFACE

VAR_GLOBAL
    rawArray : ARRAY[1..2] OF DINT;
    rawUnion : LibUnion;
    rawEnum : LibEnum;
    rawPointer : POINTER TO DINT;
    rawReference : REF_TO DINT;
    rawSubrange : LibSubrange;
    rawInlineSubrange : DINT (-2..2);
    rawFb : LibFb;
    rawClass : LibClass;
    rawString : STRING[7];
    rawWString : WSTRING[5];
END_VAR
"#
        .to_string(),
    );
    db.set_source_text(
        main,
        r#"
TYPE
    Local01 : BOOL;
    Local02 : SINT;
    Local03 : INT;
    Local04 : DINT;
    Local05 : STRING[3];
    Local06 : ARRAY[1..1] OF BOOL;
    Local07 : STRUCT
        x : BOOL;
    END_STRUCT;
    Local08 : UNION
        x : BOOL;
    END_UNION;
    Local09 : (Only);
    Local10 : POINTER TO BOOL;
    Local11 : REF_TO BOOL;
    Local12 : INT (0..1);
END_TYPE

PROGRAM Main
VAR
    arr : LibArray;
    un : LibUnion;
    en : LibEnum;
    ptr : LibPointer;
    refValue : LibReference;
    sub : LibSubrange;
    aliasValue : LibAlias;
    s : LibString;
    ws : LibWString;
    fb : LibFb;
    cls : LibClass;
END_VAR
END_PROGRAM
"#
        .to_string(),
    );

    let symbols = db.file_symbols_with_project(main);

    let array_id = symbols.resolve_alias_type(
        symbols
            .lookup_registered_type_name("LibArray")
            .expect("LibArray"),
    );
    let Type::Array {
        element,
        dimensions,
    } = symbols.type_by_id(array_id).expect("LibArray type")
    else {
        panic!("LibArray must import as an ARRAY");
    };
    assert_eq!(symbols.resolve_alias_type(*element), TypeId::DINT);
    assert_eq!(dimensions, &vec![(1, 2)]);

    let union_id = symbols.resolve_alias_type(
        symbols
            .lookup_registered_type_name("LibUnion")
            .expect("LibUnion"),
    );
    let Type::Union { variants, .. } = symbols.type_by_id(union_id).expect("LibUnion type") else {
        panic!("LibUnion must import as a UNION");
    };
    assert_eq!(variants.len(), 1);
    assert_eq!(
        symbols.resolve_alias_type(variants[0].type_id),
        TypeId::BOOL
    );

    let enum_id = symbols.resolve_alias_type(
        symbols
            .lookup_registered_type_name("LibEnum")
            .expect("LibEnum"),
    );
    let Type::Enum { base, values, .. } = symbols.type_by_id(enum_id).expect("LibEnum type") else {
        panic!("LibEnum must import as an ENUM");
    };
    assert_eq!(*base, TypeId::INT);
    assert_eq!(
        values
            .iter()
            .map(|(name, value)| (name.as_str(), *value))
            .collect::<Vec<_>>(),
        vec![("Idle", 1), ("Run", 2)]
    );

    let pointer_id = symbols.resolve_alias_type(
        symbols
            .lookup_registered_type_name("LibPointer")
            .expect("LibPointer"),
    );
    let Type::Pointer { target } = symbols.type_by_id(pointer_id).expect("LibPointer type") else {
        panic!("LibPointer must import as a POINTER");
    };
    assert_eq!(symbols.resolve_alias_type(*target), TypeId::DINT);

    let reference_id = symbols.resolve_alias_type(
        symbols
            .lookup_registered_type_name("LibReference")
            .expect("LibReference"),
    );
    let Type::Reference { target } = symbols.type_by_id(reference_id).expect("LibReference type")
    else {
        panic!("LibReference must import as a REF_TO");
    };
    assert_eq!(symbols.resolve_alias_type(*target), TypeId::DINT);

    let subrange_id = symbols.resolve_alias_type(
        symbols
            .lookup_registered_type_name("LibSubrange")
            .expect("LibSubrange"),
    );
    let Type::Subrange { base, lower, upper } =
        symbols.type_by_id(subrange_id).expect("LibSubrange type")
    else {
        panic!("LibSubrange must import as a subrange");
    };
    assert_eq!(symbols.resolve_alias_type(*base), TypeId::DINT);
    assert_eq!((*lower, *upper), (-2, 2));

    let alias_id = symbols.resolve_alias_type(
        symbols
            .lookup_registered_type_name("LibAlias")
            .expect("LibAlias"),
    );
    assert!(matches!(
        symbols.type_by_id(alias_id).expect("LibAlias target"),
        Type::Array { .. }
    ));

    let string_id = symbols.resolve_alias_type(
        symbols
            .lookup_registered_type_name("LibString")
            .expect("LibString"),
    );
    let Type::String { max_len } = symbols.type_by_id(string_id).expect("LibString type") else {
        panic!("LibString must import as STRING[7]");
    };
    assert_eq!(*max_len, Some(7));

    let wstring_id = symbols.resolve_alias_type(
        symbols
            .lookup_registered_type_name("LibWString")
            .expect("LibWString"),
    );
    let Type::WString { max_len } = symbols.type_by_id(wstring_id).expect("LibWString type") else {
        panic!("LibWString must import as WSTRING[5]");
    };
    assert_eq!(*max_len, Some(5));

    let fb_id =
        symbols.resolve_alias_type(symbols.lookup_registered_type_name("LibFb").expect("LibFb"));
    assert!(matches!(
        symbols.type_by_id(fb_id).expect("LibFb type"),
        Type::FunctionBlock { .. }
    ));

    let class_id = symbols.resolve_alias_type(
        symbols
            .lookup_registered_type_name("LibClass")
            .expect("LibClass"),
    );
    assert!(matches!(
        symbols.type_by_id(class_id).expect("LibClass type"),
        Type::Class { .. }
    ));

    let interface_id = symbols
        .iter()
        .find(|symbol| symbol.name.eq_ignore_ascii_case("LibInterface"))
        .expect("LibInterface symbol")
        .type_id;
    let interface_id = symbols.resolve_alias_type(interface_id);
    assert!(matches!(
        symbols.type_by_id(interface_id).expect("LibInterface type"),
        Type::Interface { .. }
    ));

    let raw_type = |name: &str| {
        symbols
            .iter()
            .find(|symbol| symbol.name.eq_ignore_ascii_case(name))
            .unwrap_or_else(|| panic!("imported variable {name}"))
            .type_id
    };

    let Type::Array {
        element,
        dimensions,
    } = symbols
        .type_by_id(symbols.resolve_alias_type(raw_type("rawArray")))
        .expect("rawArray type")
    else {
        panic!("rawArray must import as ARRAY");
    };
    assert_eq!(symbols.resolve_alias_type(*element), TypeId::DINT);
    assert_eq!(dimensions, &vec![(1, 2)]);

    assert!(matches!(
        symbols
            .type_by_id(symbols.resolve_alias_type(raw_type("rawUnion")))
            .expect("rawUnion type"),
        Type::Union { .. }
    ));
    assert!(matches!(
        symbols
            .type_by_id(symbols.resolve_alias_type(raw_type("rawEnum")))
            .expect("rawEnum type"),
        Type::Enum { .. }
    ));
    assert!(matches!(
        symbols
            .type_by_id(symbols.resolve_alias_type(raw_type("rawPointer")))
            .expect("rawPointer type"),
        Type::Pointer { .. }
    ));
    assert!(matches!(
        symbols
            .type_by_id(symbols.resolve_alias_type(raw_type("rawReference")))
            .expect("rawReference type"),
        Type::Reference { .. }
    ));
    assert!(matches!(
        symbols
            .type_by_id(symbols.resolve_alias_type(raw_type("rawSubrange")))
            .expect("rawSubrange type"),
        Type::Subrange { .. }
    ));
    let Type::Subrange { base, lower, upper } = symbols
        .type_by_id(symbols.resolve_alias_type(raw_type("rawInlineSubrange")))
        .expect("rawInlineSubrange type")
    else {
        panic!("rawInlineSubrange must import as a direct subrange");
    };
    assert_eq!(symbols.resolve_alias_type(*base), TypeId::DINT);
    assert_eq!((*lower, *upper), (-2, 2));
    assert!(matches!(
        symbols
            .type_by_id(symbols.resolve_alias_type(raw_type("rawFb")))
            .expect("rawFb type"),
        Type::FunctionBlock { .. }
    ));
    assert!(matches!(
        symbols
            .type_by_id(symbols.resolve_alias_type(raw_type("rawClass")))
            .expect("rawClass type"),
        Type::Class { .. }
    ));
    assert!(matches!(
        symbols
            .type_by_id(symbols.resolve_alias_type(raw_type("rawString")))
            .expect("rawString type"),
        Type::String { max_len: Some(7) }
    ));
    assert!(matches!(
        symbols
            .type_by_id(symbols.resolve_alias_type(raw_type("rawWString")))
            .expect("rawWString type"),
        Type::WString { max_len: Some(5) }
    ));
}

#[test]
fn cross_file_import_preserves_namespace_scope_and_merges_existing_namespaces() {
    let mut db = Database::new();
    let lib = FileId(0);
    let main = FileId(1);
    db.set_source_text(
        lib,
        r#"
NAMESPACE Vendor.Tools
FUNCTION ImportedValue : DINT
    ImportedValue := 7;
END_FUNCTION
END_NAMESPACE
"#
        .to_string(),
    );
    db.set_source_text(
        main,
        r#"
NAMESPACE Vendor.Tools
VAR_GLOBAL
    LocalValue : DINT;
END_VAR
END_NAMESPACE

USING Vendor.Tools;
PROGRAM Main
VAR
    value : DINT;
END_VAR
value := ImportedValue() + LocalValue;
END_PROGRAM
"#
        .to_string(),
    );
    check_no_errors_multi(&[
        r#"
NAMESPACE Vendor.Tools
FUNCTION ImportedValue : DINT
    ImportedValue := 7;
END_FUNCTION
END_NAMESPACE
"#,
        r#"
NAMESPACE Vendor.Tools
VAR_GLOBAL
    LocalValue : DINT;
END_VAR
END_NAMESPACE

USING Vendor.Tools;
PROGRAM Main
VAR
    value : DINT;
END_VAR
value := ImportedValue() + LocalValue;
END_PROGRAM
"#,
    ]);

    let symbols = db.file_symbols_with_project(main);
    let imported = symbols
        .iter()
        .find(|symbol| symbol.name.eq_ignore_ascii_case("ImportedValue"))
        .expect("imported function should resolve through merged namespace scope");
    assert!(matches!(imported.kind, SymbolKind::Function { .. }));
    assert!(imported.origin.is_some());
    let parent = imported
        .parent
        .and_then(|id| symbols.get(id))
        .expect("imported function should keep namespace parent");
    assert_eq!(parent.name.as_str(), "Tools");
    assert!(matches!(parent.kind, SymbolKind::Namespace));
    let namespace_scope = symbols
        .scope_for_owner(parent.id)
        .expect("merged imported namespace should own a scope");
    let scoped_import = symbols
        .lookup_in_scope(namespace_scope, "ImportedValue")
        .and_then(|id| symbols.get(id))
        .expect("imported function should be defined in merged namespace scope");
    assert_eq!(scoped_import.id, imported.id);
}

#[test]
fn cross_file_import_creates_scope_for_source_only_namespace() {
    let mut db = Database::new();
    let lib = FileId(0);
    let main = FileId(1);
    db.set_source_text(
        lib,
        r#"
NAMESPACE Solo.Tools
FUNCTION ImportedValue : DINT
    ImportedValue := 7;
END_FUNCTION
END_NAMESPACE
"#
        .to_string(),
    );
    db.set_source_text(
        main,
        r#"
USING Solo.Tools;
PROGRAM Main
VAR
    value : DINT;
END_VAR
value := ImportedValue();
END_PROGRAM
"#
        .to_string(),
    );
    check_no_errors_multi(&[
        r#"
NAMESPACE Solo.Tools
FUNCTION ImportedValue : DINT
    ImportedValue := 7;
END_FUNCTION
END_NAMESPACE
"#,
        r#"
USING Solo.Tools;
PROGRAM Main
VAR
    value : DINT;
END_VAR
value := ImportedValue();
END_PROGRAM
"#,
    ]);

    let symbols = db.file_symbols_with_project(main);
    let imported = symbols
        .iter()
        .find(|symbol| symbol.name.eq_ignore_ascii_case("ImportedValue"))
        .expect("source-only namespace function should import");
    let namespace = imported
        .parent
        .and_then(|id| symbols.get(id))
        .expect("source-only namespace function should keep namespace parent");
    assert_eq!(namespace.name.as_str(), "Tools");
    let namespace_scope = symbols
        .scope_for_owner(namespace.id)
        .expect("source-only imported namespace should get a scope");
    let scoped_import = symbols
        .lookup_in_scope(namespace_scope, "ImportedValue")
        .and_then(|id| symbols.get(id))
        .expect("source-only namespace scope should contain imported function");
    assert_eq!(scoped_import.id, imported.id);
}

#[test]
fn cross_file_import_translates_callable_symbol_kind_type_ids() {
    let mut db = Database::new();
    let lib = FileId(0);
    let main = FileId(1);
    db.set_source_text(
        lib,
        r#"
TYPE
    LibText : STRING[7];
END_TYPE

FUNCTION MakeText : LibText
    MakeText := '';
END_FUNCTION

CLASS Tool
METHOD PUBLIC Build : LibText
    Build := '';
END_METHOD
PUBLIC PROPERTY Value : LibText
GET
    RETURN '';
END_GET
END_PROPERTY
END_CLASS
"#
        .to_string(),
    );
    db.set_source_text(
        main,
        r#"
TYPE
    Local01 : BOOL;
    Local02 : DINT;
    Local03 : STRING[3];
END_TYPE

PROGRAM Main
VAR
    text : LibText;
    tool : Tool;
END_VAR
text := MakeText();
text := tool.Build();
text := tool.Value;
END_PROGRAM
"#
        .to_string(),
    );

    check_no_errors_multi(&[
        r#"
TYPE
    LibText : STRING[7];
END_TYPE

FUNCTION MakeText : LibText
    MakeText := '';
END_FUNCTION

CLASS Tool
METHOD PUBLIC Build : LibText
    Build := '';
END_METHOD
PUBLIC PROPERTY Value : LibText
GET
    RETURN '';
END_GET
END_PROPERTY
END_CLASS
"#,
        r#"
TYPE
    Local01 : BOOL;
    Local02 : DINT;
    Local03 : STRING[3];
END_TYPE

PROGRAM Main
VAR
    text : LibText;
    tool : Tool;
END_VAR
text := MakeText();
text := tool.Build();
text := tool.Value;
END_PROGRAM
"#,
    ]);

    let symbols = db.file_symbols_with_project(main);
    let imported_string_len = |type_id: TypeId| {
        let type_id = symbols.resolve_alias_type(type_id);
        let Type::String { max_len } = symbols.type_by_id(type_id).expect("imported type") else {
            panic!("expected imported callable type to resolve to STRING[7]");
        };
        *max_len
    };

    let function = symbols
        .iter()
        .find(|symbol| symbol.name.eq_ignore_ascii_case("MakeText"))
        .expect("imported function");
    let SymbolKind::Function { return_type, .. } = function.kind else {
        panic!("MakeText must import as a function");
    };
    assert_eq!(imported_string_len(return_type), Some(7));

    let tool_id = symbols.lookup("Tool").expect("imported Tool class");
    let method = symbols
        .iter()
        .find(|symbol| symbol.name.as_str() == "Build" && symbol.parent == Some(tool_id))
        .expect("imported method");
    let SymbolKind::Method {
        return_type: Some(return_type),
        ..
    } = method.kind
    else {
        panic!("Build must import as a method with a return type");
    };
    assert_eq!(imported_string_len(return_type), Some(7));

    let property = symbols
        .iter()
        .find(|symbol| symbol.name.as_str() == "Value" && symbol.parent == Some(tool_id))
        .expect("imported property");
    let SymbolKind::Property { prop_type, .. } = property.kind else {
        panic!("Value must import as a property");
    };
    assert_eq!(imported_string_len(prop_type), Some(7));
}

#[test]
fn cross_file_import_translates_union_variant_default_initializer() {
    let mut db = Database::new();
    let lib = FileId(0);
    let main = FileId(1);
    db.set_source_text(
        lib,
        r#"
TYPE
    LibUnion : UNION
        value : INT := 42;
    END_UNION;
END_TYPE
"#
        .to_string(),
    );
    db.set_source_text(
        main,
        r#"
PROGRAM Main
VAR
    un : LibUnion;
END_VAR
END_PROGRAM
"#
        .to_string(),
    );

    let symbols = db.file_symbols_with_project(main);
    let type_id = symbols
        .lookup_registered_type_name("LibUnion")
        .expect("imported LibUnion type");
    let Type::Union { variants, .. } = symbols.type_by_id(type_id).expect("LibUnion definition")
    else {
        panic!("expected imported union type");
    };
    let initializer = variants[0]
        .default_initializer
        .expect("imported variant default initializer id");
    assert!(
        symbols.initializer(initializer).is_some(),
        "imported union initializer id should resolve in the target table catalog"
    );
}
