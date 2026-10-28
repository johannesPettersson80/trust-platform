//! The sidebar's gates (specification 25 §6.6): whether Start, Update and Debug may run, from the
//! errors VS Code shows in Problems and the last Compile.
//!
//! An error in a Structured Text source (`.st`, `.pou`) blocks with `Fix N errors to <verb>.`; an
//! error in `runtime.toml` or `trust-lsp.toml` blocks with `Fix runtime.toml to <verb>.`. Without
//! errors in Problems, a failed Compile blocks the same way, by the kind of its error issues. Debug
//! is also unavailable while the target is unreachable or starting.

use std::path::Path;

use serde::Deserialize;
use serde_json::{json, Value};

use super::decode;

#[derive(Deserialize)]
struct Args {
    /// The error count of every file VS Code shows diagnostics for.
    #[serde(default)]
    diagnostics: Vec<DiagnosticCount>,
    /// The last Compile, when the extension knows one.
    #[serde(default)]
    compile: Option<CompileArgs>,
    /// The selected target, for Debug.
    #[serde(default)]
    target: Option<TargetArgs>,
}

#[derive(Deserialize)]
struct DiagnosticCount {
    path: String,
    errors: usize,
}

#[derive(Deserialize)]
struct CompileArgs {
    /// `unknown`, `dirty`, `clean` or `failed`.
    state: String,
    /// The report of `trust-runtime check --json`, for a failed Compile.
    #[serde(default)]
    report: Option<CheckReport>,
}

#[derive(Deserialize)]
struct CheckReport {
    #[serde(default)]
    errors: Option<usize>,
    #[serde(default)]
    issues: Vec<CheckIssue>,
}

#[derive(Deserialize)]
struct CheckIssue {
    #[serde(default)]
    severity: String,
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    code: Option<String>,
}

#[derive(Deserialize)]
struct TargetArgs {
    /// The target's status: `running`, `stopped`, `starting`, `unreachable`, …
    #[serde(default)]
    status: String,
}

/// The errors Problems shows, split by kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct ErrorCounts {
    source: usize,
    config: usize,
}

impl ErrorCounts {
    fn total(self) -> usize {
        self.source + self.config
    }
}

/// The verbs a gate names.
const VERBS: [&str; 3] = ["start", "update", "debug"];

/// `{ ok, validity, compile, reasons, debug }`.
pub(super) fn answer(argument: Value) -> Result<Value, String> {
    let args: Args = decode(argument)?;
    let problems = problem_counts(&args.diagnostics);
    let failed_compile = args
        .compile
        .as_ref()
        .filter(|compile| compile.state == "failed")
        .map(|compile| compile_counts(compile.report.as_ref()));
    let reasons: serde_json::Map<String, Value> = VERBS
        .iter()
        .map(|verb| {
            (
                (*verb).to_owned(),
                json!(gate_reason(problems, failed_compile, verb)),
            )
        })
        .collect();
    let debug_reason = gate_reason(problems, failed_compile, "debug");
    let target_status = args
        .target
        .as_ref()
        .map_or("", |target| target.status.as_str());
    let debug_enabled =
        debug_reason.is_none() && !matches!(target_status, "unreachable" | "starting");
    let debug_title = debug_reason.clone().unwrap_or_else(|| {
        if debug_enabled {
            "Start debugging".to_owned()
        } else {
            "Debug is unavailable until the target is reachable.".to_owned()
        }
    });
    Ok(json!({
        "ok": true,
        "validity": {
            "ok": problems.total() == 0,
            "label": validity_label(problems.total()),
            "errors": problems.total(),
            "sourceErrors": problems.source,
            "configErrors": problems.config,
        },
        "compile": failed_compile.map(|(errors, counts)| json!({
            "errors": errors,
            "sourceErrors": counts.source,
            "configErrors": counts.config,
        })),
        "reasons": reasons,
        "debug": { "enabled": debug_enabled, "title": debug_title },
    }))
}

/// The errors of the Structured Text sources and of the project settings among `diagnostics`;
/// other files do not gate.
fn problem_counts(diagnostics: &[DiagnosticCount]) -> ErrorCounts {
    let mut counts = ErrorCounts::default();
    for diagnostic in diagnostics {
        if is_settings_file(&diagnostic.path) {
            counts.config += diagnostic.errors;
        } else if is_source_file(&diagnostic.path) {
            counts.source += diagnostic.errors;
        }
    }
    counts
}

/// The error count of a failed Compile and its error issues by kind. An issue about a settings
/// file, or whose code names the configuration, is a settings error.
fn compile_counts(report: Option<&CheckReport>) -> (usize, ErrorCounts) {
    let Some(report) = report else {
        return (1, ErrorCounts::default());
    };
    let mut counts = ErrorCounts::default();
    for issue in &report.issues {
        if !issue.severity.eq_ignore_ascii_case("error") {
            continue;
        }
        let settings = issue.file.as_deref().is_some_and(is_settings_file)
            || issue
                .code
                .as_deref()
                .is_some_and(|code| code.to_ascii_lowercase().contains("config"));
        if settings {
            counts.config += 1;
        } else {
            counts.source += 1;
        }
    }
    let errors = report.errors.unwrap_or(report.issues.len());
    (errors, counts)
}

/// Why `verb` may not run, or `None`. Problems decide first; a failed Compile decides when Problems
/// shows no error.
fn gate_reason(
    problems: ErrorCounts,
    failed_compile: Option<(usize, ErrorCounts)>,
    verb: &str,
) -> Option<String> {
    if problems.total() > 0 {
        return Some(if problems.config > 0 {
            settings_reason(verb)
        } else {
            source_reason(problems.source, verb)
        });
    }
    let (errors, counts) = failed_compile?;
    Some(if counts.config > 0 {
        settings_reason(verb)
    } else {
        source_reason(errors, verb)
    })
}

fn settings_reason(verb: &str) -> String {
    format!("Fix runtime.toml to {verb}.")
}

fn source_reason(errors: usize, verb: &str) -> String {
    let count = errors.max(1);
    format!("Fix {count} {} to {verb}.", plural(count, "error"))
}

fn validity_label(errors: usize) -> String {
    if errors == 0 {
        "No known errors".to_owned()
    } else {
        format!("{errors} {} — see Problems", plural(errors, "error"))
    }
}

fn plural(count: usize, word: &str) -> String {
    if count == 1 {
        word.to_owned()
    } else {
        format!("{word}s")
    }
}

/// `runtime.toml` or `trust-lsp.toml`, in any letter case.
fn is_settings_file(path: &str) -> bool {
    file_name(path).is_some_and(|name| {
        name.eq_ignore_ascii_case("runtime.toml") || name.eq_ignore_ascii_case("trust-lsp.toml")
    })
}

/// A Structured Text source: `.st` or `.pou`, in any letter case.
fn is_source_file(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("st") || extension.eq_ignore_ascii_case("pou")
        })
}

/// The last part of `path`, with either separator.
fn file_name(path: &str) -> Option<&str> {
    path.rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
}

#[cfg(test)]
#[path = "compile_gate_tests.rs"]
mod tests;
