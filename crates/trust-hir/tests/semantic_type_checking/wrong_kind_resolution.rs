use crate::common::*;
use trust_hir::diagnostics::Diagnostic;

fn error_diagnostics(source: &str) -> Vec<Diagnostic> {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    db.diagnostics(file)
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .cloned()
        .collect()
}

fn error_diagnostics_multi(sources: &[&str], target_file: FileId) -> Vec<Diagnostic> {
    let mut db = Database::new();
    for (idx, source) in sources.iter().enumerate() {
        db.set_source_text(FileId(idx as u32), (*source).to_string());
    }
    db.diagnostics(target_file)
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .cloned()
        .collect()
}

fn assert_exact_error_without_wrong_reason(
    source: &str,
    expected_code: DiagnosticCode,
    expected_message: &str,
    forbidden_codes: &[DiagnosticCode],
) {
    let errors = error_diagnostics(source);
    assert!(
        errors.iter().any(|diagnostic| {
            diagnostic.code == expected_code && diagnostic.message == expected_message
        }),
        "expected {expected_code:?} with message {expected_message:?}, got {errors:?}"
    );
    for forbidden in forbidden_codes {
        assert!(
            !errors
                .iter()
                .any(|diagnostic| diagnostic.code == *forbidden),
            "wrong-kind error must not degrade into {forbidden:?}; got {errors:?}"
        );
    }
}

fn assert_exact_project_error_without_wrong_reason(
    sources: &[&str],
    target_file: FileId,
    expected_code: DiagnosticCode,
    expected_message: &str,
    forbidden_codes: &[DiagnosticCode],
) {
    let errors = error_diagnostics_multi(sources, target_file);
    assert!(
        errors.iter().any(|diagnostic| {
            diagnostic.code == expected_code && diagnostic.message == expected_message
        }),
        "expected {expected_code:?} with message {expected_message:?}, got {errors:?}"
    );
    for forbidden in forbidden_codes {
        assert!(
            !errors
                .iter()
                .any(|diagnostic| diagnostic.code == *forbidden),
            "project wrong-kind error must not degrade into {forbidden:?}; got {errors:?}"
        );
    }
}

#[test]
fn value_used_as_type_reports_wrong_kind_not_undefined_type() {
    assert_exact_error_without_wrong_reason(
        r#"
PROGRAM Test
    VAR
        AliasValue : DINT;
        x : AliasValue;
    END_VAR
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
        "identifier 'AliasValue' is a variable, not a type",
        &[DiagnosticCode::UndefinedType, DiagnosticCode::TypeMismatch],
    );
}

#[test]
fn type_used_as_value_reports_wrong_kind_not_undefined_variable() {
    assert_exact_error_without_wrong_reason(
        r#"
TYPE MyInt : DINT;
END_TYPE

PROGRAM Test
    VAR x : DINT; END_VAR
    x := MyInt;
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
        "type 'MyInt' cannot be used as a value",
        &[
            DiagnosticCode::UndefinedVariable,
            DiagnosticCode::CannotResolve,
            DiagnosticCode::TypeMismatch,
        ],
    );
}

#[test]
fn namespace_used_as_callable_reports_not_callable_not_cannot_resolve() {
    assert_exact_error_without_wrong_reason(
        r#"
NAMESPACE Factory
END_NAMESPACE

PROGRAM Test
    VAR x : DINT; END_VAR
    x := Factory();
END_PROGRAM
"#,
        DiagnosticCode::UndefinedFunction,
        "'Factory' is not callable",
        &[
            DiagnosticCode::CannotResolve,
            DiagnosticCode::UndefinedVariable,
            DiagnosticCode::InvalidArgumentType,
        ],
    );
}

#[test]
fn callable_used_as_variable_reports_wrong_kind_not_undefined_variable() {
    assert_exact_error_without_wrong_reason(
        r#"
FUNCTION AddOne : DINT
    VAR_INPUT x : DINT; END_VAR
    AddOne := x + 1;
END_FUNCTION

PROGRAM Test
    VAR x : DINT; END_VAR
    x := AddOne;
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
        "function 'AddOne' cannot be used as a value",
        &[
            DiagnosticCode::UndefinedVariable,
            DiagnosticCode::CannotResolve,
            DiagnosticCode::TypeMismatch,
        ],
    );
}

#[test]
fn parenthesized_value_used_as_callable_reports_not_callable() {
    assert_exact_error_without_wrong_reason(
        r#"
PROGRAM Test
VAR
    x : DINT;
    y : DINT;
END_VAR
y := (x)();
END_PROGRAM
"#,
        DiagnosticCode::UndefinedFunction,
        "expression is not callable",
        &[
            DiagnosticCode::CannotResolve,
            DiagnosticCode::UndefinedVariable,
            DiagnosticCode::TypeMismatch,
        ],
    );
}

#[test]
fn imported_namespaced_function_used_as_type_reports_wrong_kind_not_undefined_type() {
    assert_exact_project_error_without_wrong_reason(
        &[
            r#"
NAMESPACE Library
FUNCTION MakeValue : DINT
    MakeValue := 1;
END_FUNCTION
END_NAMESPACE
"#,
            r#"
PROGRAM Main
VAR
    x : Library.MakeValue;
END_VAR
END_PROGRAM
"#,
        ],
        FileId(1),
        DiagnosticCode::InvalidOperation,
        "identifier 'MakeValue' is a function, not a type",
        &[DiagnosticCode::UndefinedType, DiagnosticCode::CannotResolve],
    );
}

#[test]
fn imported_namespaced_type_used_as_value_reports_wrong_kind_not_undefined_variable() {
    assert_exact_project_error_without_wrong_reason(
        &[
            r#"
NAMESPACE Library
TYPE Widget : DINT;
END_TYPE
END_NAMESPACE
"#,
            r#"
PROGRAM Main
VAR
    x : DINT;
END_VAR
x := Library.Widget;
END_PROGRAM
"#,
        ],
        FileId(1),
        DiagnosticCode::InvalidOperation,
        "type 'Widget' cannot be used as a value",
        &[
            DiagnosticCode::UndefinedVariable,
            DiagnosticCode::CannotResolve,
            DiagnosticCode::TypeMismatch,
        ],
    );
}

#[test]
fn imported_namespace_used_as_callable_reports_not_callable_not_cannot_resolve() {
    assert_exact_project_error_without_wrong_reason(
        &[
            r#"
NAMESPACE Library
FUNCTION MakeValue : DINT
    MakeValue := 1;
END_FUNCTION
END_NAMESPACE
"#,
            r#"
PROGRAM Main
VAR
    x : DINT;
END_VAR
x := Library();
END_PROGRAM
"#,
        ],
        FileId(1),
        DiagnosticCode::UndefinedFunction,
        "'Library' is not callable",
        &[
            DiagnosticCode::CannotResolve,
            DiagnosticCode::UndefinedVariable,
            DiagnosticCode::InvalidArgumentType,
        ],
    );
}
