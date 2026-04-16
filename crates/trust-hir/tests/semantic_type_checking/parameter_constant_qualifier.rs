use crate::common::*;
use trust_hir::symbols::{Symbol, VarQualifier};

fn diagnostics(source: &str) -> Vec<trust_hir::diagnostics::Diagnostic> {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    db.diagnostics(file).to_vec()
}

fn assert_has_error(source: &str) {
    let diagnostics = diagnostics(source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error),
        "expected an error, got: {diagnostics:?}"
    );
}

fn assert_constant_flag_debug(symbol: &Symbol) {
    let rendered = format!("{symbol:?}");
    assert!(
        rendered.contains("is_constant: true"),
        "expected Symbol debug output to contain is_constant: true, got {rendered}"
    );
}

fn find_symbol<'a>(
    symbols: &'a trust_hir::symbols::SymbolTable,
    name: &str,
) -> &'a trust_hir::symbols::Symbol {
    symbols
        .iter()
        .find(|symbol| symbol.name == name)
        .unwrap_or_else(|| panic!("missing symbol {name}"))
}

#[test]
fn var_input_constant_is_parameter_with_is_constant_flag() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
FUNCTION Echo : INT
    VAR_INPUT CONSTANT
        X : INT;
    END_VAR
    Echo := X;
END_FUNCTION
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);
    let x = find_symbol(&symbols, "X");
    assert!(matches!(
        x.kind,
        SymbolKind::Parameter {
            direction: ParamDirection::In
        }
    ));
    assert_constant_flag_debug(x);
}

#[test]
fn var_output_constant_is_parameter_with_is_constant_flag() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
FUNCTION_BLOCK FB_Test
    VAR_OUTPUT CONSTANT
        Y : INT;
    END_VAR
END_FUNCTION_BLOCK
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);
    let y = find_symbol(&symbols, "Y");
    assert!(matches!(
        y.kind,
        SymbolKind::Parameter {
            direction: ParamDirection::Out
        }
    ));
    assert_constant_flag_debug(y);
}

#[test]
fn var_in_out_constant_is_parameter_with_is_constant_flag() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
FUNCTION ReadFirst : INT
    VAR_IN_OUT CONSTANT
        Z : ARRAY[0..3] OF INT;
    END_VAR
    ReadFirst := Z[0];
END_FUNCTION
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);
    let z = find_symbol(&symbols, "Z");
    assert!(matches!(
        z.kind,
        SymbolKind::Parameter {
            direction: ParamDirection::InOut
        }
    ));
    assert_constant_flag_debug(z);
}

#[test]
fn var_temp_constant_is_variable_with_is_constant_flag() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
FUNCTION_BLOCK FB_Test
    VAR_TEMP CONSTANT
        T : INT := INT#1;
    END_VAR
END_FUNCTION_BLOCK
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);
    let t = find_symbol(&symbols, "T");
    assert!(matches!(
        t.kind,
        SymbolKind::Variable {
            qualifier: VarQualifier::Temp
        }
    ));
    assert_constant_flag_debug(t);
}

#[test]
fn var_global_constant_remains_symbol_kind_constant() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
VAR_GLOBAL CONSTANT
    PI : REAL := REAL#3.14;
END_VAR
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);
    let pi = find_symbol(&symbols, "PI");
    assert!(matches!(pi.kind, SymbolKind::Constant));
    assert_constant_flag_debug(pi);
}

#[test]
fn var_constant_local_remains_symbol_kind_constant() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
PROGRAM Test
    VAR CONSTANT
        PI : REAL := REAL#3.14;
    END_VAR
END_PROGRAM
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);
    let pi = find_symbol(&symbols, "PI");
    assert!(matches!(pi.kind, SymbolKind::Constant));
    assert_constant_flag_debug(pi);
}

#[test]
fn var_input_constant_participates_in_call_argument_resolution() {
    check_no_errors(
        r#"
FUNCTION Echo : INT
    VAR_INPUT CONSTANT
        X : INT;
    END_VAR
    Echo := X;
END_FUNCTION

PROGRAM Test
    VAR
        out_x : INT;
    END_VAR
    out_x := Echo(X := INT#3);
END_PROGRAM
"#,
    );
}

#[test]
fn var_in_out_constant_accepts_caller_storage_argument() {
    check_no_errors(
        r#"
FUNCTION ReadFirst : INT
    VAR_IN_OUT CONSTANT
        Z : ARRAY[0..3] OF INT;
    END_VAR
    ReadFirst := Z[0];
END_FUNCTION

PROGRAM Test
    VAR
        arr : ARRAY[0..3] OF INT;
        out_x : INT;
    END_VAR
    out_x := ReadFirst(Z := arr);
END_PROGRAM
"#,
    );
}

#[test]
fn var_input_constant_is_not_precollected_as_compile_time_expression() {
    assert_has_error(
        r#"
FUNCTION_BLOCK FB_Test
    VAR_INPUT CONSTANT
        Len : DINT;
    END_VAR
    VAR
        name : STRING[Len + 1];
    END_VAR
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn var_temp_constant_is_not_precollected_as_compile_time_expression() {
    assert_has_error(
        r#"
FUNCTION_BLOCK FB_Test
    VAR_TEMP CONSTANT
        Len : DINT := 4;
    END_VAR
    VAR
        name : STRING[Len + 1];
    END_VAR
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn var_global_constant_is_precollected() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
VAR_GLOBAL CONSTANT
    Len : DINT := 4;
END_VAR

FUNCTION_BLOCK FB_Test
    VAR
        name : STRING[Len + 1];
    END_VAR
END_FUNCTION_BLOCK
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);
    let name = find_symbol(&symbols, "name");
    let type_id = symbols.resolve_alias_type(name.type_id);
    let Type::String { max_len } = symbols.type_by_id(type_id).unwrap() else {
        panic!("expected string type");
    };
    assert_eq!(*max_len, Some(5));
}

#[test]
fn assign_to_var_input_constant_is_rejected() {
    check_has_error(
        r#"
FUNCTION Echo : INT
    VAR_INPUT CONSTANT
        X : INT;
    END_VAR
    X := INT#5;
    Echo := X;
END_FUNCTION
"#,
        DiagnosticCode::ConstantModification,
    );
}

#[test]
fn assign_to_var_output_constant_is_rejected() {
    check_has_error(
        r#"
FUNCTION_BLOCK FB_Test
    VAR_OUTPUT CONSTANT
        Y : INT;
    END_VAR
    Y := INT#5;
END_FUNCTION_BLOCK
"#,
        DiagnosticCode::ConstantModification,
    );
}

#[test]
fn assign_to_var_in_out_constant_is_rejected() {
    check_has_error(
        r#"
FUNCTION ReadFirst : INT
    VAR_IN_OUT CONSTANT
        Z : INT;
    END_VAR
    Z := INT#5;
    ReadFirst := Z;
END_FUNCTION
"#,
        DiagnosticCode::ConstantModification,
    );
}

#[test]
fn assign_through_var_in_out_constant_array_index_is_rejected() {
    check_has_error(
        r#"
FUNCTION ReadFirst : INT
    VAR_IN_OUT CONSTANT
        Z : ARRAY[0..3] OF INT;
    END_VAR
    Z[1] := INT#5;
    ReadFirst := Z[0];
END_FUNCTION
"#,
        DiagnosticCode::ConstantModification,
    );
}

#[test]
fn assign_through_var_in_out_constant_struct_field_is_rejected() {
    check_has_error(
        r#"
TYPE
    S : STRUCT
        F : INT;
    END_STRUCT
END_TYPE

FUNCTION ReadField : INT
    VAR_IN_OUT CONSTANT
        Z : S;
    END_VAR
    Z.F := INT#5;
    ReadField := Z.F;
END_FUNCTION
"#,
        DiagnosticCode::ConstantModification,
    );
}

#[test]
fn assign_to_var_temp_constant_is_rejected() {
    check_has_error(
        r#"
FUNCTION_BLOCK FB_Test
    VAR_TEMP CONSTANT
        T : INT := INT#1;
    END_VAR
    T := INT#5;
END_FUNCTION_BLOCK
"#,
        DiagnosticCode::ConstantModification,
    );
}

#[test]
fn assign_through_var_input_pointer_whose_target_is_constant_array_is_accepted() {
    check_no_errors(
        r#"
FUNCTION_BLOCK FB_Test
    VAR_INPUT
        PT : POINTER TO ARRAY[0..9] OF BYTE;
    END_VAR
    PT^[2] := BYTE#7;
END_FUNCTION_BLOCK
"#,
    );
}

#[test]
fn assign_through_var_in_out_constant_pointer_slot_and_deref_have_distinct_rules() {
    let diagnostics = diagnostics(
        r#"
FUNCTION_BLOCK FB_Test
    VAR_IN_OUT CONSTANT
        PT : POINTER TO INT;
    END_VAR
    VAR
        Other : INT;
    END_VAR
    PT := ADR(Other);
    PT^ := INT#5;
END_FUNCTION_BLOCK
"#,
    );

    let errors: Vec<_> = diagnostics
        .into_iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .collect();
    assert_eq!(
        errors.len(),
        1,
        "expected only the pointer-slot write to fail"
    );
    assert_eq!(errors[0].code, DiagnosticCode::ConstantModification);
    assert!(
        errors[0].message.contains("constant"),
        "unexpected diagnostic: {:?}",
        errors[0]
    );
}

#[test]
fn fb_instance_in_var_input_constant_is_rejected() {
    check_has_error(
        r#"
FUNCTION_BLOCK Inner
END_FUNCTION_BLOCK

FUNCTION_BLOCK Outer
    VAR_INPUT CONSTANT
        Inst : Inner;
    END_VAR
END_FUNCTION_BLOCK
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn fb_instance_in_var_temp_constant_is_rejected() {
    check_has_error(
        r#"
FUNCTION_BLOCK Inner
END_FUNCTION_BLOCK

FUNCTION_BLOCK Outer
    VAR_TEMP CONSTANT
        Inst : Inner;
    END_VAR
END_FUNCTION_BLOCK
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn fb_instance_in_var_in_out_constant_is_rejected() {
    check_has_error(
        r#"
FUNCTION_BLOCK Inner
END_FUNCTION_BLOCK

FUNCTION_BLOCK Outer
    VAR_IN_OUT CONSTANT
        Inst : Inner;
    END_VAR
END_FUNCTION_BLOCK
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn fb_instance_in_var_global_constant_is_rejected() {
    check_has_error(
        r#"
FUNCTION_BLOCK Inner
END_FUNCTION_BLOCK

VAR_GLOBAL CONSTANT
    Inst : Inner;
END_VAR
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn var_external_constant_regression_still_works() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        r#"
PROGRAM Main
VAR_EXTERNAL CONSTANT
    ANSWER : INT;
END_VAR
END_PROGRAM
"#
        .to_string(),
    );

    let symbols = db.file_symbols(file);
    let answer = find_symbol(&symbols, "ANSWER");
    assert!(matches!(answer.kind, SymbolKind::Constant));
    assert_constant_flag_debug(answer);
}

#[test]
fn var_in_out_constant_call_site_binding_unchanged() {
    check_has_error(
        r#"
FUNCTION ReadFirst : INT
    VAR_IN_OUT CONSTANT
        Z : INT;
    END_VAR
    ReadFirst := Z;
END_FUNCTION

PROGRAM Test
    VAR
        out_x : INT;
    END_VAR
    out_x := ReadFirst(Z := INT#3);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}
