//! Shared helpers for semantic tests.
#![allow(dead_code, unused_imports)]

pub use trust_hir::db::{Database, FileId, SemanticDatabase, SourceDatabase};
pub use trust_hir::diagnostics::{DiagnosticCode, DiagnosticSeverity};
pub use trust_hir::symbols::{ParamDirection, SymbolKind, Visibility};
pub use trust_hir::Type;

/// Helper to check diagnostics for a source file.
pub fn check_errors(source: &str) -> Vec<DiagnosticCode> {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    db.diagnostics(file)
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .map(|d| d.code)
        .collect()
}

/// Helper to assert no errors in source.
pub fn check_no_errors(source: &str) {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    let diagnostics = db
        .diagnostics(file)
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .cloned()
        .collect::<Vec<_>>();
    let errors = diagnostics.iter().map(|d| d.code).collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "Expected no errors, got: {:?}",
        diagnostics
    );
}

/// Helper to assert no errors across multiple source files.
pub fn check_no_errors_multi(sources: &[&str]) {
    let mut db = Database::new();
    for (idx, source) in sources.iter().enumerate() {
        db.set_source_text(FileId(idx as u32), (*source).to_string());
    }
    let errors = sources
        .iter()
        .enumerate()
        .flat_map(|(idx, _)| {
            db.diagnostics(FileId(idx as u32))
                .iter()
                .filter(|d| d.severity == DiagnosticSeverity::Error)
                .map(|d| d.code)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "Expected no errors across files, got: {:?}",
        errors
    );
}

/// Helper to assert a specific error is present.
pub fn check_has_error(source: &str, expected: DiagnosticCode) {
    let errors = check_errors(source);
    assert!(
        errors.contains(&expected),
        "Expected {:?} in {:?}",
        expected,
        errors
    );
}

/// Helper to check warnings for a source file.
pub fn check_warnings(source: &str) -> Vec<DiagnosticCode> {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    db.diagnostics(file)
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Warning)
        .map(|d| d.code)
        .collect()
}
