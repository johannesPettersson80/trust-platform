//! The diagnostics of a Simulator build (specification 25 §6.10, specification 48 RU-44): after each
//! build of a launch or of `stReload`, the adapter sends the custom event `trustBuildDiagnostics`
//! with the compiler's errors and warnings of the project's Rust crate, each at its file's absolute
//! path, which the extension shows in Problems in place of the last build's.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use trust_runtime::native::RustDiagnostic;

/// The custom event of a build's diagnostics.
pub(super) const BUILD_DIAGNOSTICS_EVENT: &str = "trustBuildDiagnostics";

/// The source the extension gives a diagnostic of the project's Rust crate.
pub(super) const RUST_SOURCE: &str = "truST (rustc)";

/// The event's body for the diagnostics `diagnostics` of a build of the project at `root`: the
/// source and each diagnostic, at the absolute path of its file, 1-based, as the build gives it.
pub(super) fn body(root: &Path, diagnostics: &[RustDiagnostic]) -> Value {
    let diagnostics: Vec<Value> = diagnostics
        .iter()
        .map(|diagnostic| {
            json!({
                "path": path_in(root, &diagnostic.file).display().to_string(),
                "severity": diagnostic.severity.name(),
                "code": diagnostic.code,
                "message": diagnostic.message,
                "line": diagnostic.line,
                "column": diagnostic.column,
                "end_line": diagnostic.end_line,
                "end_column": diagnostic.end_column,
            })
        })
        .collect();
    json!({ "source": RUST_SOURCE, "diagnostics": diagnostics })
}

/// The absolute path of `file`, relative to the project with `/`, in the project at `root`.
fn path_in(root: &Path, file: &str) -> PathBuf {
    let root = std::path::absolute(root).unwrap_or_else(|_| root.to_path_buf());
    file.split('/')
        .filter(|part| !part.is_empty())
        .fold(root, |path, part| path.join(part))
}

#[cfg(test)]
mod tests {
    use super::*;
    use trust_runtime::native::RustSeverity;

    #[test]
    fn a_diagnostic_is_at_its_files_absolute_path_with_the_builds_place() {
        // Specification 25 §6.10: each diagnostic has its file's absolute path, so the extension
        // opens it without knowing the project's folder, and the build's 1-based place. Catches
        // a path left relative (`rust/src/lib.rs`), which VS Code resolves against nothing, and a
        // place or code lost on the way.
        let root = Path::new("project-root");
        let diagnostic = RustDiagnostic {
            severity: RustSeverity::Error,
            code: Some("E0308".to_string()),
            message: "mismatched types".to_string(),
            file: "rust/src/lib.rs".to_string(),
            line: 10,
            column: 21,
            end_line: 10,
            end_column: 27,
            rendered: String::new(),
        };
        let body = body(root, &[diagnostic]);
        let expected_path = std::path::absolute(root)
            .expect("the folder's absolute path")
            .join("rust")
            .join("src")
            .join("lib.rs");
        assert_eq!(
            body,
            json!({
                "source": "truST (rustc)",
                "diagnostics": [{
                    "path": expected_path.display().to_string(),
                    "severity": "error",
                    "code": "E0308",
                    "message": "mismatched types",
                    "line": 10,
                    "column": 21,
                    "end_line": 10,
                    "end_column": 27,
                }],
            })
        );
    }

    #[test]
    fn a_build_without_diagnostics_sends_an_empty_list() {
        // Specification 25 §6.10: a build without diagnostics clears the last build's in
        // Problems, so the event still goes, with no diagnostics. Catches a build that sends
        // nothing and leaves the last build's errors after the crate was fixed.
        assert_eq!(
            body(Path::new("p"), &[]),
            json!({ "source": "truST (rustc)", "diagnostics": [] })
        );
    }
}
