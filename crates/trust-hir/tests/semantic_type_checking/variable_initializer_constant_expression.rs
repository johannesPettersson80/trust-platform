use crate::common::{
    Database, DiagnosticCode, DiagnosticSeverity, FileId, SemanticDatabase, SourceDatabase,
};

fn initializer_errors(source: &str) -> Vec<trust_hir::diagnostics::Diagnostic> {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    db.diagnostics(file)
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .cloned()
        .collect()
}

fn has_initializer_constant_expression_error(
    errors: &[trust_hir::diagnostics::Diagnostic],
) -> bool {
    errors.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::InvalidOperation
            && diagnostic
                .message
                .contains("variable initializer must be a literal or constant expression")
    })
}

#[test]
fn undefined_noninteger_initializer_reports_only_its_primary_resolution_error() {
    let errors = initializer_errors(
        r#"
PROGRAM Main
VAR
    derived : BOOL := missing_value;
END_VAR
END_PROGRAM
"#,
    );

    assert!(
        errors
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::UndefinedVariable),
        "expected primary undefined-variable diagnostic, got {errors:?}"
    );
    assert!(
        !has_initializer_constant_expression_error(&errors),
        "undefined initializer emitted cascading E202: {errors:?}"
    );
    assert_eq!(
        errors
            .iter()
            .filter(|diagnostic| diagnostic.code == DiagnosticCode::UndefinedVariable)
            .count(),
        1,
        "undefined initializer must emit exactly one primary diagnostic: {errors:?}"
    );
}

#[test]
fn ambiguous_noninteger_initializer_reports_only_its_primary_resolution_error() {
    let errors = initializer_errors(
        r#"
NAMESPACE A
VAR_GLOBAL
    Shared : BOOL := TRUE;
END_VAR
END_NAMESPACE
NAMESPACE B
VAR_GLOBAL
    Shared : BOOL := FALSE;
END_VAR
END_NAMESPACE
USING A;
USING B;
PROGRAM Main
VAR
    derived : BOOL := Shared;
END_VAR
END_PROGRAM
"#,
    );

    assert_eq!(
        errors
            .iter()
            .filter(|diagnostic| diagnostic.code == DiagnosticCode::CannotResolve)
            .count(),
        1,
        "ambiguous initializer must emit exactly one primary diagnostic: {errors:?}"
    );
    assert!(
        !has_initializer_constant_expression_error(&errors),
        "ambiguous initializer emitted cascading E202: {errors:?}"
    );
}

#[test]
fn undefined_integer_initializer_reports_only_its_primary_resolution_error() {
    let errors = initializer_errors(
        r#"
PROGRAM Main
VAR
    derived : INT := missing_value;
END_VAR
END_PROGRAM
"#,
    );

    assert_eq!(
        errors
            .iter()
            .filter(|diagnostic| diagnostic.code == DiagnosticCode::UndefinedVariable)
            .count(),
        1,
        "undefined integer initializer must emit exactly one primary diagnostic: {errors:?}"
    );
    assert!(
        !has_initializer_constant_expression_error(&errors),
        "undefined integer initializer emitted cascading E202: {errors:?}"
    );
}

#[test]
fn ambiguous_integer_initializer_reports_only_its_primary_resolution_error() {
    let errors = initializer_errors(
        r#"
NAMESPACE A
VAR_GLOBAL CONSTANT
    Shared : INT := 1;
END_VAR
END_NAMESPACE
NAMESPACE B
VAR_GLOBAL CONSTANT
    Shared : INT := 2;
END_VAR
END_NAMESPACE
USING A;
USING B;
PROGRAM Main
VAR
    derived : INT := Shared;
END_VAR
END_PROGRAM
"#,
    );

    assert_eq!(
        errors
            .iter()
            .filter(|diagnostic| diagnostic.code == DiagnosticCode::CannotResolve)
            .count(),
        1,
        "ambiguous integer initializer must emit exactly one primary diagnostic: {errors:?}"
    );
    assert!(
        !has_initializer_constant_expression_error(&errors),
        "ambiguous integer initializer emitted cascading E202: {errors:?}"
    );
}

#[test]
fn integer_constant_reference_remains_a_valid_initializer() {
    let errors = initializer_errors(
        r#"
PROGRAM Main
VAR CONSTANT
    limit : INT := 7;
END_VAR
VAR
    value : INT := limit;
END_VAR
END_PROGRAM
"#,
    );

    assert!(
        errors.is_empty(),
        "integer constant reference must remain a valid initializer: {errors:?}"
    );
}

#[test]
fn reference_initializer_may_name_existing_mutable_storage() {
    let errors = initializer_errors(
        r#"
PROGRAM Main
VAR
    target : INT := 7;
    target_ref : REF_TO INT := REF(target);
END_VAR
END_PROGRAM
"#,
    );

    assert!(
        errors.is_empty(),
        "IEC reference initialization may name existing mutable storage: {errors:?}"
    );
}

#[test]
fn reference_initializer_keeps_function_local_lifetime_restriction() {
    let errors = initializer_errors(
        r#"
FUNCTION Probe : INT
VAR
    target : INT;
    target_ref : REF_TO INT := REF(target);
END_VAR
Probe := 0;
END_FUNCTION
"#,
    );

    assert!(
        errors.iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::InvalidOperation
                && diagnostic
                    .message
                    .contains("REF cannot take a reference to function-local variables")
        }),
        "reference initializer must retain the REF lifetime diagnostic: {errors:?}"
    );
    assert!(
        !has_initializer_constant_expression_error(&errors),
        "REF lifetime failure must not be replaced by a constant-expression cascade: {errors:?}"
    );
}

#[test]
fn inherited_mutable_noninteger_initializer_dependency_is_rejected() {
    let errors = initializer_errors(
        r#"
CLASS Base
VAR
    source : BOOL := TRUE;
END_VAR
END_CLASS
CLASS Derived EXTENDS Base
VAR
    derived : BOOL := source;
END_VAR
END_CLASS
"#,
    );

    assert!(
        has_initializer_constant_expression_error(&errors),
        "expected inherited mutable dependency rejection, got {errors:?}"
    );
}

#[test]
fn array_repetition_initializer_is_not_rejected_as_nonconstant() {
    let errors = initializer_errors(
        r#"
PROGRAM Main
VAR
    values : ARRAY[1..6] OF INT := [3(1, 2)];
END_VAR
END_PROGRAM
"#,
    );

    assert!(
        errors.is_empty(),
        "array repetition must remain a valid initializer: {errors:?}"
    );
}

#[test]
fn typed_struct_initializer_is_not_rejected_as_nonconstant() {
    let errors = initializer_errors(
        r#"
TYPE Sample : STRUCT
    value : INT;
    ready : BOOL;
END_STRUCT;
END_TYPE
PROGRAM Main
VAR
    sample : Sample := Sample(value := 1, ready := TRUE);
END_VAR
END_PROGRAM
"#,
    );

    assert!(
        errors.is_empty(),
        "typed structure constructor must remain a valid initializer: {errors:?}"
    );
}

#[test]
fn mutable_noninteger_variable_initializers_are_rejected_as_nonconstant() {
    let errors = initializer_errors(
        r#"
PROGRAM Main
VAR
    source_bool : BOOL := TRUE;
    derived_bool : BOOL := source_bool;
    source_real : REAL := 1.5;
    derived_real : REAL := source_real;
    source_string : STRING := 'source';
    derived_string : STRING := source_string;
END_VAR
END_PROGRAM
"#,
    );

    let initializer_errors = errors
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == DiagnosticCode::InvalidOperation
                && diagnostic
                    .message
                    .contains("variable initializer must be a literal or constant expression")
        })
        .count();
    assert_eq!(
        initializer_errors, 3,
        "expected BOOL, REAL, and STRING mutable initializer rejections, got {errors:?}"
    );
}

#[test]
fn mutable_aggregate_member_initializer_is_rejected_as_nonconstant() {
    let errors = initializer_errors(
        r#"
TYPE Sample : STRUCT
    value : INT;
END_STRUCT;
END_TYPE
PROGRAM Main
VAR
    source : INT := 1;
    derived : Sample := (value := source);
END_VAR
END_PROGRAM
"#,
    );

    assert!(
        errors.iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::InvalidOperation
                && diagnostic
                    .message
                    .contains("variable initializer must be a literal or constant expression")
        }),
        "expected mutable aggregate-member initializer rejection, got {errors:?}"
    );
}

#[test]
fn forward_declared_mutable_noninteger_initializer_dependency_is_rejected() {
    let errors = initializer_errors(
        r#"
PROGRAM Main
VAR
    derived : BOOL := source;
    source : BOOL := TRUE;
END_VAR
END_PROGRAM
"#,
    );

    assert!(
        has_initializer_constant_expression_error(&errors),
        "expected forward mutable dependency rejection, got {errors:?}"
    );
}

#[test]
fn qualified_namespace_mutable_initializer_dependency_is_rejected() {
    let errors = initializer_errors(
        r#"
NAMESPACE Shared
VAR_GLOBAL
    source : BOOL := TRUE;
END_VAR
END_NAMESPACE
PROGRAM Main
VAR
    derived : BOOL := Shared.source;
END_VAR
END_PROGRAM
"#,
    );

    assert!(
        has_initializer_constant_expression_error(&errors),
        "expected qualified mutable dependency rejection, got {errors:?}"
    );
}

#[test]
fn noninteger_function_call_initializer_is_rejected_as_nonconstant() {
    let errors = initializer_errors(
        r#"
FUNCTION MakeBool : BOOL
MakeBool := TRUE;
END_FUNCTION
PROGRAM Main
VAR
    derived : BOOL := MakeBool();
END_VAR
END_PROGRAM
"#,
    );

    assert!(
        has_initializer_constant_expression_error(&errors),
        "expected function-call initializer rejection, got {errors:?}"
    );
}

#[test]
fn noninteger_standard_function_call_initializer_is_rejected_as_nonconstant() {
    let errors = initializer_errors(
        r#"
PROGRAM Main
VAR
    derived : REAL := ABS(-1.0);
END_VAR
END_PROGRAM
"#,
    );

    assert!(
        has_initializer_constant_expression_error(&errors),
        "expected standard-function initializer rejection, got {errors:?}"
    );
}

#[test]
fn noninteger_constant_declaration_call_initializer_is_rejected_as_nonconstant() {
    let errors = initializer_errors(
        r#"
PROGRAM Main
VAR CONSTANT
    derived : REAL := ABS(-1.0);
END_VAR
END_PROGRAM
"#,
    );

    assert!(
        has_initializer_constant_expression_error(&errors),
        "expected constant-declaration call initializer rejection, got {errors:?}"
    );
    assert_eq!(errors.len(), 1, "expected only E202, got {errors:?}");
}

#[test]
fn undefined_noninteger_initializer_callee_reports_only_its_primary_resolution_error() {
    let errors = initializer_errors(
        r#"
PROGRAM Main
VAR
    derived : BOOL := Missing();
END_VAR
END_PROGRAM
"#,
    );

    assert_eq!(
        errors
            .iter()
            .filter(|diagnostic| diagnostic.code == DiagnosticCode::UndefinedFunction)
            .count(),
        1,
        "undefined initializer callee must emit exactly one primary diagnostic: {errors:?}"
    );
    assert!(
        !has_initializer_constant_expression_error(&errors),
        "undefined initializer callee emitted cascading E202: {errors:?}"
    );
    assert_eq!(
        errors.len(),
        1,
        "expected only the primary E103, got {errors:?}"
    );
}

#[test]
fn ambiguous_noninteger_initializer_callee_reports_only_its_primary_resolution_error() {
    let errors = initializer_errors(
        r#"
NAMESPACE A
FUNCTION Pick : BOOL
    Pick := TRUE;
END_FUNCTION
END_NAMESPACE
NAMESPACE B
FUNCTION Pick : BOOL
    Pick := FALSE;
END_FUNCTION
END_NAMESPACE
USING A;
USING B;
PROGRAM Main
VAR
    derived : BOOL := Pick();
END_VAR
END_PROGRAM
"#,
    );

    assert_eq!(
        errors
            .iter()
            .filter(|diagnostic| diagnostic.code == DiagnosticCode::CannotResolve)
            .count(),
        1,
        "ambiguous initializer callee must emit exactly one primary diagnostic: {errors:?}"
    );
    assert!(
        !has_initializer_constant_expression_error(&errors),
        "ambiguous initializer callee emitted cascading E202: {errors:?}"
    );
    assert_eq!(
        errors.len(),
        1,
        "expected only the primary E105, got {errors:?}"
    );
}
