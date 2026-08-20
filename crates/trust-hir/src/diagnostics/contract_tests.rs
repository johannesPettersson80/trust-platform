use super::*;

fn range(start: u32, end: u32) -> TextRange {
    TextRange::new(start.into(), end.into())
}

fn registered_codes() -> Vec<(DiagnosticCode, &'static str, DiagnosticSeverity)> {
    use DiagnosticCode::*;
    use DiagnosticSeverity::{Error, Hint, Warning};

    vec![
        (UnexpectedToken, "E001", Error),
        (MissingToken, "E002", Error),
        (UnclosedBlock, "E003", Error),
        (UndefinedVariable, "E101", Error),
        (UndefinedType, "E102", Error),
        (UndefinedFunction, "E103", Error),
        (DuplicateDeclaration, "E104", Error),
        (CannotResolve, "E105", Error),
        (InvalidIdentifier, "E106", Error),
        (UndefinedField, "E107", Error),
        (DuplicateField, "E108", Error),
        (TypeMismatch, "E201", Error),
        (InvalidOperation, "E202", Error),
        (IncompatibleAssignment, "E203", Error),
        (WrongArgumentCount, "E204", Error),
        (InvalidArgumentType, "E205", Error),
        (MissingReturn, "E206", Error),
        (InvalidReturnType, "E207", Error),
        (InvalidAssignmentTarget, "E301", Error),
        (ConstantModification, "E302", Error),
        (InvalidArrayIndex, "E303", Error),
        (OutOfRange, "E304", Error),
        (CyclicDependency, "E305", Error),
        (InvalidTaskConfig, "E306", Error),
        (UnknownTask, "E307", Error),
        (InvalidOpenOtAttribute, "E308", Error),
        (UnusedVariable, "W001", Warning),
        (UnusedParameter, "W002", Warning),
        (UnreachableCode, "W003", Warning),
        (MissingElse, "W004", Warning),
        (ImplicitConversion, "W005", Warning),
        (ShadowedVariable, "W006", Warning),
        (Deprecated, "W007", Warning),
        (HighComplexity, "W008", Warning),
        (UnusedPou, "W009", Warning),
        (NondeterministicTimeDate, "W010", Warning),
        (NondeterministicIo, "W011", Warning),
        (SharedGlobalTaskHazard, "W012", Warning),
        (FloatingPointEquality, "W013", Warning),
        (LiteralDivisionByZero, "W014", Warning),
        (ArrayInitializerCardinality, "W015", Warning),
        (Simplification, "I001", Hint),
        (StyleSuggestion, "I002", Hint),
    ]
}

#[test]
fn complete_registered_code_table_has_stable_codes_and_severities() {
    for (code, expected_text, expected_severity) in registered_codes() {
        assert_eq!(code.code(), expected_text, "{code:?}");
        assert_eq!(code.severity(), expected_severity, "{code:?}");
    }
}

#[test]
fn every_registered_string_code_is_unique() {
    let cases = registered_codes();
    let unique = cases
        .iter()
        .map(|(_, text, _)| *text)
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(unique.len(), cases.len());
}

#[test]
fn code_prefix_matches_default_severity_family() {
    for (code, text, severity) in registered_codes() {
        let expected_prefix = match severity {
            DiagnosticSeverity::Error => 'E',
            DiagnosticSeverity::Warning => 'W',
            DiagnosticSeverity::Info | DiagnosticSeverity::Hint => 'I',
        };
        assert_eq!(text.chars().next(), Some(expected_prefix), "{code:?}");
    }
}

#[test]
fn new_uses_code_default_while_explicit_constructors_force_severity() {
    let default_error = Diagnostic::new(DiagnosticCode::TypeMismatch, range(1, 2), "default");
    let default_hint = Diagnostic::new(DiagnosticCode::Simplification, range(1, 2), "default");
    let forced_error = Diagnostic::error(DiagnosticCode::UnusedVariable, range(1, 2), "forced");
    let forced_warning = Diagnostic::warning(DiagnosticCode::TypeMismatch, range(1, 2), "forced");

    assert_eq!(default_error.severity, DiagnosticSeverity::Error);
    assert_eq!(default_hint.severity, DiagnosticSeverity::Hint);
    assert_eq!(forced_error.severity, DiagnosticSeverity::Error);
    assert_eq!(forced_warning.severity, DiagnosticSeverity::Warning);
}

#[test]
fn constructors_preserve_code_range_message_and_empty_related_list() {
    let diagnostic = Diagnostic::new(
        DiagnosticCode::CannotResolve,
        range(12, 34),
        "cannot resolve value",
    );

    assert_eq!(diagnostic.code, DiagnosticCode::CannotResolve);
    assert_eq!(diagnostic.range, range(12, 34));
    assert_eq!(diagnostic.message, "cannot resolve value");
    assert!(diagnostic.related.is_empty());
}

#[test]
fn related_information_appends_in_call_order() {
    let diagnostic = Diagnostic::error(DiagnosticCode::DuplicateDeclaration, range(10, 20), "dup")
        .with_related(range(1, 2), "first")
        .with_related(range(3, 4), "second")
        .with_related(range(5, 6), "third");

    assert_eq!(
        diagnostic.related,
        vec![
            RelatedInfo {
                range: range(1, 2),
                message: "first".to_owned(),
            },
            RelatedInfo {
                range: range(3, 4),
                message: "second".to_owned(),
            },
            RelatedInfo {
                range: range(5, 6),
                message: "third".to_owned(),
            },
        ]
    );
}

#[test]
fn display_is_exact_for_all_four_explicit_severities() {
    for (severity, label) in [
        (DiagnosticSeverity::Error, "error"),
        (DiagnosticSeverity::Warning, "warning"),
        (DiagnosticSeverity::Info, "info"),
        (DiagnosticSeverity::Hint, "hint"),
    ] {
        let diagnostic = Diagnostic {
            code: DiagnosticCode::StyleSuggestion,
            severity,
            range: range(7, 19),
            message: "message".to_owned(),
            related: Vec::new(),
        };

        assert_eq!(
            diagnostic.to_string(),
            format!("{label}[I002]: message (at 7..19)")
        );
    }
}

#[test]
fn is_error_observes_explicit_severity_not_code_family() {
    let forced_error =
        Diagnostic::error(DiagnosticCode::UnusedVariable, range(0, 1), "forced error");
    let forced_warning =
        Diagnostic::warning(DiagnosticCode::TypeMismatch, range(0, 1), "forced warning");

    assert!(forced_error.is_error());
    assert!(!forced_warning.is_error());
}

#[test]
fn builder_deduplicates_only_exact_diagnostics() {
    let base = Diagnostic::error(DiagnosticCode::TypeMismatch, range(1, 2), "base");
    let changed_message = Diagnostic::error(DiagnosticCode::TypeMismatch, range(1, 2), "other");
    let changed_range = Diagnostic::error(DiagnosticCode::TypeMismatch, range(2, 3), "base");
    let changed_related = base.clone().with_related(range(0, 1), "context");
    let mut builder = DiagnosticBuilder::new();

    builder.add(base.clone());
    builder.add(base);
    builder.add(changed_message);
    builder.add(changed_range);
    builder.add(changed_related);

    assert_eq!(builder.finish().len(), 4);
}

#[test]
fn builder_preserves_first_insertion_order_across_helpers() {
    let mut builder = DiagnosticBuilder::new();
    builder.warning(DiagnosticCode::UnusedVariable, range(30, 31), "third");
    builder.error(DiagnosticCode::TypeMismatch, range(10, 11), "first");
    builder.add(Diagnostic::new(
        DiagnosticCode::StyleSuggestion,
        range(20, 21),
        "second",
    ));

    let diagnostics = builder.finish();

    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>(),
        vec!["third", "first", "second"]
    );
}

#[test]
fn builder_has_errors_tracks_explicit_error_presence() {
    let mut builder = DiagnosticBuilder::new();
    assert!(!builder.has_errors());

    builder.warning(DiagnosticCode::TypeMismatch, range(0, 1), "forced warning");
    assert!(!builder.has_errors());

    builder.error(DiagnosticCode::UnusedVariable, range(0, 1), "forced error");
    assert!(builder.has_errors());
}

#[test]
fn empty_builder_finishes_to_empty_vector() {
    assert!(DiagnosticBuilder::new().finish().is_empty());
}
