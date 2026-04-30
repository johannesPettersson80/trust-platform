use crate::common::{
    check_errors, check_has_error, check_no_errors, Database, DiagnosticCode, FileId,
    SemanticDatabase, SourceDatabase, Type,
};

#[test]
fn named_struct_initializer_unknown_field_reports_e107() {
    check_has_error(
        r#"
TYPE
    StepCfg : STRUCT
        cyl : INT;
    END_STRUCT;
END_TYPE

PROGRAM Main
VAR
    cfg : StepCfg := (missing := 1);
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::UndefinedField,
    );
}

#[test]
fn named_struct_initializer_duplicate_field_reports_e108() {
    check_has_error(
        r#"
TYPE
    StepCfg : STRUCT
        cyl : INT;
    END_STRUCT;
END_TYPE

PROGRAM Main
VAR
    cfg : StepCfg := (cyl := 1, cyl := 2);
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::DuplicateField,
    );
}

#[test]
fn field_default_string_length_uses_out_of_range() {
    check_has_error(
        r#"
TYPE
    StepCfg : STRUCT
        name : STRING[3] := 'hello';
    END_STRUCT;
END_TYPE

PROGRAM Main
VAR
    cfg : StepCfg;
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::OutOfRange,
    );
}

#[test]
fn type_level_aggregate_default_fields_are_checked() {
    let errors = check_errors(
        r#"
TYPE
    StepCfg : STRUCT
        cyl : INT;
    END_STRUCT;
    DefaultStep : StepCfg := (missing := 1, cyl := 2, cyl := 3);
END_TYPE

PROGRAM Main
VAR
    cfg : DefaultStep;
END_VAR
END_PROGRAM
"#,
    );
    assert!(
        errors.contains(&DiagnosticCode::UndefinedField),
        "{errors:?}"
    );
    assert!(
        errors.contains(&DiagnosticCode::DuplicateField),
        "{errors:?}"
    );
}

#[test]
fn cyclic_constant_default_reports_e305_not_silent_none() {
    check_has_error(
        r#"
VAR_GLOBAL CONSTANT
    a : INT := b;
    b : INT := a;
END_VAR
"#,
        DiagnosticCode::CyclicDependency,
    );
}

#[test]
fn constant_default_divide_by_zero_reports_e202() {
    check_has_error(
        r#"
VAR_GLOBAL CONSTANT
    a : INT := 1 / 0;
END_VAR
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn constant_default_overflow_reports_e202() {
    check_has_error(
        r#"
VAR_GLOBAL CONSTANT
    a : LINT := 9223372036854775807 + 1;
END_VAR
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn field_default_divide_by_zero_reports_e202() {
    check_has_error(
        r#"
TYPE
    StepCfg : STRUCT
        value : INT := 1 / 0;
    END_STRUCT;
END_TYPE
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn field_default_target_range_reports_e304() {
    check_has_error(
        r#"
TYPE
    StepCfg : STRUCT
        value : SINT := 200;
    END_STRUCT;
END_TYPE
"#,
        DiagnosticCode::OutOfRange,
    );
}

#[test]
fn field_default_bool_mismatch_reports_e201() {
    check_has_error(
        r#"
TYPE
    StepCfg : STRUCT
        value : BOOL := 5;
    END_STRUCT;
END_TYPE
"#,
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn reference_member_default_ref_expression_is_rejected() {
    check_has_error(
        r#"
VAR_GLOBAL
    target : INT;
END_VAR

TYPE
    Node : STRUCT
        next : REF_TO INT := REF(target);
    END_STRUCT;
END_TYPE
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn field_default_non_constant_reference_is_rejected() {
    check_has_error(
        r#"
TYPE
    StepCfg : STRUCT
        value : INT := runtimeValue;
    END_STRUCT;
END_TYPE
"#,
        DiagnosticCode::UndefinedVariable,
    );
}

#[test]
fn const_forward_reference_is_deterministic() {
    check_no_errors(
        r#"
VAR_GLOBAL CONSTANT
    a : INT := b;
    b : INT := 5;
END_VAR
"#,
    );
}

#[test]
fn cross_file_const_can_feed_field_default() {
    let mut db = Database::new();
    let file_lib = FileId(0);
    let file_main = FileId(1);
    db.set_source_text(
        file_lib,
        r#"
VAR_GLOBAL CONSTANT
    MAX_VAL : INT := 42;
END_VAR
"#
        .to_string(),
    );
    db.set_source_text(
        file_main,
        r#"
TYPE
    StepCfg : STRUCT
        value : INT := MAX_VAL;
    END_STRUCT;
END_TYPE
"#
        .to_string(),
    );

    let analysis = db.analyze(file_main);
    assert!(
        analysis.diagnostics.is_empty(),
        "cross-file CONST field default should resolve, got {:?}",
        analysis.diagnostics
    );
}

#[test]
fn field_default_sizeof_time_and_date_literals_are_accepted() {
    check_no_errors(
        r#"
TYPE
    Other : STRUCT
        x : DINT;
    END_STRUCT;
    StepCfg : STRUCT
        size : DINT := SIZEOF(Other);
        delay : TIME := T#100ms;
        day : DATE := D#2024-01-01;
    END_STRUCT;
END_TYPE
"#,
    );
}

#[test]
fn aggregate_initializer_against_non_aggregate_target_reports_e201() {
    check_has_error(
        r#"
PROGRAM Main
VAR
    x : INT := (a := 1);
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn aggregate_field_order_and_case_are_independent() {
    check_no_errors(
        r#"
TYPE
    RangeKind : (BIPOLAR_10V);
    Analog : STRUCT
        RANGE : RangeKind := BIPOLAR_10V;
        MIN_SCALE : INT := -100;
    END_STRUCT;
END_TYPE

PROGRAM Main
VAR
    cfg : Analog := (min_scale := -100, range := BIPOLAR_10V);
END_VAR
END_PROGRAM
"#,
    );
}

#[test]
fn valid_named_struct_initializer_has_no_hir_error() {
    check_no_errors(
        r#"
TYPE
    StepCfg : STRUCT
        cyl : INT := 2;
        ext : BOOL := TRUE;
    END_STRUCT;
END_TYPE

PROGRAM Main
VAR
    cfg : StepCfg := (cyl := 7);
END_VAR
END_PROGRAM
"#,
    );
}

#[test]
fn hir_union_variant_default_initializer_is_recorded() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
TYPE
    U : UNION
        a : INT := 2;
    END_UNION;
END_TYPE
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);
    let type_id = symbols.lookup_registered_type_name("U").expect("U type");
    let Type::Union { variants, .. } = symbols.type_by_id(type_id).expect("U definition") else {
        panic!("expected union type");
    };
    let initializer = variants[0]
        .default_initializer
        .expect("variant default initializer id");
    assert!(symbols.initializer(initializer).is_some());
}

#[test]
fn hir_type_level_default_initializer_is_recorded() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
TYPE
    StepCfg : STRUCT
        cyl : INT;
    END_STRUCT;
    DefaultStep : StepCfg := (cyl := 2);
END_TYPE
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);
    let type_id = symbols
        .lookup_registered_type_name("DefaultStep")
        .expect("DefaultStep type");
    let initializer = symbols
        .type_default_initializer(type_id)
        .expect("TYPE-level default initializer id");
    assert!(symbols.initializer(initializer).is_some());
}

#[test]
fn hir_struct_field_default_initializer_is_recorded() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
TYPE
    StepCfg : STRUCT
        cyl : INT := 2;
    END_STRUCT;
END_TYPE
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);
    let type_id = symbols
        .lookup_registered_type_name("StepCfg")
        .expect("StepCfg type");
    let Type::Struct { fields, .. } = symbols.type_by_id(type_id).expect("StepCfg definition")
    else {
        panic!("expected struct type");
    };
    let initializer = fields[0]
        .default_initializer
        .expect("field default initializer id");
    assert!(
        symbols.initializer(initializer).is_some(),
        "initializer id should resolve through the HIR SymbolTable catalog"
    );
}

#[test]
fn cross_file_import_translates_struct_field_default_initializer() {
    let mut db = Database::new();
    let file_lib = FileId(0);
    let file_main = FileId(1);
    db.set_source_text(
        file_lib,
        r#"
TYPE
    LibCfg : STRUCT
        value : INT := 42;
    END_STRUCT;
END_TYPE
"#
        .to_string(),
    );
    db.set_source_text(
        file_main,
        r#"
PROGRAM Main
VAR
    cfg : LibCfg;
END_VAR
END_PROGRAM
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file_main);
    let type_id = symbols
        .lookup_registered_type_name("LibCfg")
        .expect("imported LibCfg type");
    let Type::Struct { fields, .. } = symbols.type_by_id(type_id).expect("LibCfg definition")
    else {
        panic!("expected imported struct type");
    };
    let initializer = fields[0]
        .default_initializer
        .expect("imported field default initializer id");
    assert!(
        symbols.initializer(initializer).is_some(),
        "imported initializer id should resolve in the target table catalog"
    );
}

#[test]
fn field_default_edit_invalidates_hir_initializer_catalog() {
    let mut db = Database::new();
    let file = FileId(0);
    let first_source = r#"
TYPE
    StepCfg : STRUCT
        cyl : INT := 2;
    END_STRUCT;
END_TYPE
"#;
    db.set_source_text(file, first_source.to_string());

    let before = db.file_symbols(file);
    let before_type = before
        .lookup_registered_type_name("StepCfg")
        .expect("StepCfg type");
    let Type::Struct { fields, .. } = before.type_by_id(before_type).expect("StepCfg definition")
    else {
        panic!("expected struct type");
    };
    let before_record = before
        .initializer(fields[0].default_initializer.expect("initializer id"))
        .expect("initializer record")
        .clone();

    let second_source = first_source.replace(":= 2", ":= 200");
    db.set_source_text(file, second_source.clone());
    let after = db.file_symbols(file);
    assert!(
        !std::sync::Arc::ptr_eq(&before, &after),
        "editing a field default must invalidate the symbol table/catalog"
    );
    let after_type = after
        .lookup_registered_type_name("StepCfg")
        .expect("StepCfg type after edit");
    let Type::Struct { fields, .. } = after.type_by_id(after_type).expect("StepCfg definition")
    else {
        panic!("expected struct type after edit");
    };
    let after_record = after
        .initializer(
            fields[0]
                .default_initializer
                .expect("initializer id after edit"),
        )
        .expect("initializer record after edit");
    assert_ne!(before_record.range, after_record.range);
    let text = db.source_text(file);
    let range = after_record.range;
    let initializer_text = &text[usize::from(range.start())..usize::from(range.end())];
    assert_eq!(initializer_text.trim(), "200");
}

#[test]
fn function_block_initializer_allows_inputs_outputs_and_public_vars() {
    check_no_errors(
        r#"
FUNCTION_BLOCK InitFb
VAR_INPUT
    enable : BOOL;
END_VAR
VAR_OUTPUT
    count : INT;
END_VAR
VAR PUBLIC
    local : INT;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    fb : InitFb := (enable := TRUE, count := 3, local := 4);
END_VAR
END_PROGRAM
"#,
    );
}

#[test]
fn function_block_initializer_rejects_var_in_out_member() {
    check_has_error(
        r#"
FUNCTION_BLOCK InitFb
VAR_IN_OUT
    shared : INT;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    fb : InitFb := (shared := 1);
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn class_call_style_aggregate_initializer_reports_e202() {
    check_has_error(
        r#"
CLASS Device
VAR PUBLIC
    value : INT;
END_VAR
END_CLASS

PROGRAM Main
VAR
    dev : Device := Device(value := 1);
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}
