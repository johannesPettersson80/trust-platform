use super::*;

use crate::error::RuntimeError;

fn all_errors() -> Vec<(BoundaryError, &'static str)> {
    vec![
        (
            BoundaryError::UnresolvedName {
                path: "missing".into(),
            },
            "unresolved_name",
        ),
        (
            BoundaryError::UnboundProgram {
                program: "Main".into(),
            },
            "unbound_program",
        ),
        (
            BoundaryError::AmbiguousName {
                path: "value".into(),
                candidates: vec!["A.value".into(), "B.value".into()],
            },
            "ambiguous_name",
        ),
        (
            BoundaryError::UnsupportedPathSyntax {
                path: "f()".into(),
                reason: "calls are not paths".to_string(),
            },
            "unsupported_path_syntax",
        ),
        (
            BoundaryError::WrongKind {
                path: "items[9]".into(),
                expected: "in-range array index",
                actual: "out-of-bounds array index",
            },
            "wrong_kind",
        ),
        (
            BoundaryError::UndeclaredBinding {
                path: "typo".into(),
            },
            "undeclared_binding",
        ),
        (
            BoundaryError::InternalLockFailure {
                context: "poisoned state",
            },
            "internal_lock_failure",
        ),
        (
            BoundaryError::InternalFailure {
                context: "runtime evaluation",
            },
            "internal_failure",
        ),
    ]
}

#[test]
fn boundary_error_contract_codes_are_stable_for_every_variant() {
    for (error, code) in all_errors() {
        assert_eq!(error.code(), code, "{error:?}");
    }
}

#[test]
fn boundary_error_contract_paths_cover_only_path_bearing_variants() {
    let expected = [
        Some("missing"),
        Some("Main"),
        Some("value"),
        Some("f()"),
        Some("items[9]"),
        Some("typo"),
        None,
        None,
    ];
    for ((error, _), path) in all_errors().into_iter().zip(expected) {
        assert_eq!(error.path(), path, "{error:?}");
    }
}

#[test]
fn boundary_error_contract_candidates_exist_only_for_ambiguity() {
    for (error, _) in all_errors() {
        if matches!(error, BoundaryError::AmbiguousName { .. }) {
            assert_eq!(error.candidates(), ["A.value", "B.value"]);
        } else {
            assert!(error.candidates().is_empty(), "{error:?}");
        }
    }
}

#[test]
fn boundary_error_contract_unresolved_display_names_path() {
    let error = BoundaryError::UnresolvedName {
        path: "Cell.output".into(),
    };
    assert_eq!(
        error.to_string(),
        "boundary path 'Cell.output' did not resolve to a declared value"
    );
}

#[test]
fn boundary_error_contract_unbound_program_display_names_program() {
    let error = BoundaryError::UnboundProgram {
        program: "MainTask".into(),
    };
    assert_eq!(
        error.to_string(),
        "PROGRAM 'MainTask' is declared but not bound by the CONFIGURATION"
    );
}

#[test]
fn boundary_error_contract_ambiguous_display_preserves_candidate_order() {
    let error = BoundaryError::AmbiguousName {
        path: "speed".into(),
        candidates: vec!["LineB.speed".into(), "LineA.speed".into()],
    };
    assert_eq!(
        error.to_string(),
        "boundary path 'speed' is ambiguous; candidates: LineB.speed, LineA.speed"
    );
}

#[test]
fn boundary_error_contract_unsupported_display_includes_parser_reason() {
    let error = BoundaryError::UnsupportedPathSyntax {
        path: "value + 1".into(),
        reason: "not an assignment path".to_string(),
    };
    assert_eq!(
        error.to_string(),
        "boundary path 'value + 1' uses unsupported syntax: not an assignment path"
    );
}

#[test]
fn boundary_error_contract_wrong_kind_display_names_expected_and_actual() {
    let error = BoundaryError::WrongKind {
        path: "ref^".into(),
        expected: "non-null reference",
        actual: "null reference",
    };
    assert_eq!(
        error.to_string(),
        "boundary path 'ref^' expected non-null reference but resolved to null reference"
    );
}

#[test]
fn boundary_error_contract_binding_and_internal_displays_are_distinct() {
    assert_eq!(
        BoundaryError::UndeclaredBinding {
            path: "motor".into()
        }
        .to_string(),
        "direct binding target 'motor' is not declared"
    );
    assert_eq!(
        BoundaryError::InternalLockFailure {
            context: "runtime snapshot"
        }
        .to_string(),
        "internal lock failure while resolving boundary path: runtime snapshot"
    );
    assert_eq!(
        BoundaryError::InternalFailure {
            context: "runtime evaluation"
        }
        .to_string(),
        "internal boundary failure: runtime evaluation"
    );
}

#[test]
fn boundary_error_contract_implements_std_error_without_altering_display() {
    let error = BoundaryError::UnresolvedName {
        path: "unknown".into(),
    };
    let as_error: &dyn std::error::Error = &error;
    assert_eq!(as_error.to_string(), error.to_string());
}

#[test]
fn boundary_error_contract_runtime_undefined_variable_preserves_runtime_name() {
    let error = BoundaryError::from_runtime(
        "caller.path",
        RuntimeError::UndefinedVariable("actual_name".into()),
    );
    assert_eq!(
        error,
        BoundaryError::UnresolvedName {
            path: "actual_name".into()
        }
    );
}

#[test]
fn boundary_error_contract_runtime_undefined_program_maps_to_unbound_program() {
    let error =
        BoundaryError::from_runtime("ignored", RuntimeError::UndefinedProgram("Logic".into()));
    assert_eq!(
        error,
        BoundaryError::UnboundProgram {
            program: "Logic".into()
        }
    );
}

#[test]
fn boundary_error_contract_runtime_undefined_field_uses_complete_caller_path() {
    let error = BoundaryError::from_runtime(
        "payload.missing",
        RuntimeError::UndefinedField("missing".into()),
    );
    assert_eq!(
        error,
        BoundaryError::UnresolvedName {
            path: "payload.missing".into()
        }
    );
}

#[test]
fn boundary_error_contract_runtime_type_mismatch_is_wrong_kind() {
    let error = BoundaryError::from_runtime("value", RuntimeError::TypeMismatch);
    assert_eq!(
        error,
        BoundaryError::WrongKind {
            path: "value".into(),
            expected: "observable value",
            actual: "incompatible runtime value"
        }
    );
}

#[test]
fn boundary_error_contract_runtime_null_reference_is_wrong_kind() {
    let error = BoundaryError::from_runtime("pointer^", RuntimeError::NullReference);
    assert_eq!(
        error,
        BoundaryError::WrongKind {
            path: "pointer^".into(),
            expected: "non-null reference",
            actual: "null reference"
        }
    );
}

#[test]
fn boundary_error_contract_runtime_index_failure_is_wrong_kind() {
    let error = BoundaryError::from_runtime(
        "values[4]",
        RuntimeError::IndexOutOfBounds {
            index: 4,
            lower: 1,
            upper: 3,
        },
    );
    assert_eq!(
        error,
        BoundaryError::WrongKind {
            path: "values[4]".into(),
            expected: "in-range array index",
            actual: "out-of-bounds array index"
        }
    );
}

#[test]
fn boundary_error_contract_other_runtime_failures_are_redacted() {
    let error = BoundaryError::from_runtime("secret", RuntimeError::DivisionByZero);
    assert_eq!(
        error,
        BoundaryError::InternalFailure {
            context: "boundary runtime evaluation"
        }
    );
    assert_eq!(error.path(), None);
}
