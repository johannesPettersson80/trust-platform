//! External diagnostics ingestion for custom linters.

use serde::Deserialize;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range, Url};

use crate::config::ProjectConfig;
use crate::state::path_to_uri;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ExternalDiagnosticsFile {
    List(Vec<ExternalDiagnosticEntry>),
    Wrapper {
        diagnostics: Vec<ExternalDiagnosticEntry>,
    },
}

#[derive(Debug, Deserialize)]
struct ExternalDiagnosticEntry {
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    uri: Option<String>,
    range: ExternalRange,
    #[serde(default)]
    severity: Option<ExternalSeverity>,
    #[serde(default)]
    code: Option<String>,
    message: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    fix: Option<ExternalFix>,
}

#[derive(Debug, Deserialize)]
struct ExternalRange {
    start: ExternalPosition,
    end: ExternalPosition,
}

#[derive(Debug, Deserialize)]
struct ExternalPosition {
    line: u32,
    character: u32,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ExternalSeverity {
    String(String),
    Number(u8),
}

#[derive(Debug, Deserialize)]
struct ExternalFix {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    range: Option<ExternalRange>,
    new_text: String,
}

#[derive(Debug, Deserialize, serde::Serialize)]
pub(crate) struct ExternalFixData {
    pub title: Option<String>,
    pub range: Option<Range>,
    #[serde(rename = "newText")]
    pub new_text: String,
}

pub(crate) fn collect_external_diagnostics(config: &ProjectConfig, uri: &Url) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for path in &config.diagnostic_external_paths {
        if let Ok(contents) = fs::read_to_string(path) {
            if let Ok(parsed) = serde_json::from_str::<ExternalDiagnosticsFile>(&contents) {
                let entries = match parsed {
                    ExternalDiagnosticsFile::List(list) => list,
                    ExternalDiagnosticsFile::Wrapper { diagnostics } => diagnostics,
                };
                for entry in entries {
                    if let Some(target_uri) = entry_uri(&config.root, &entry) {
                        if &target_uri == uri {
                            diagnostics.push(entry_to_lsp(entry));
                        }
                    }
                }
            }
        }
    }
    diagnostics
}

fn entry_uri(root: &Path, entry: &ExternalDiagnosticEntry) -> Option<Url> {
    if let Some(uri) = entry.uri.as_ref() {
        return Url::parse(uri).ok();
    }
    let path = entry.path.as_ref()?;
    let path = resolve_path(root, path);
    path_to_uri(&path)
}

fn resolve_path(root: &Path, entry: &str) -> PathBuf {
    let path = PathBuf::from(entry);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn entry_to_lsp(entry: ExternalDiagnosticEntry) -> Diagnostic {
    let range = entry.range.into_lsp();
    let severity = entry
        .severity
        .as_ref()
        .and_then(|severity| severity.to_lsp())
        .or(Some(DiagnosticSeverity::WARNING));
    let mut diagnostic = Diagnostic {
        range,
        severity,
        code: entry.code.map(NumberOrString::String),
        source: Some(entry.source.unwrap_or_else(|| "external".to_string())),
        message: entry.message,
        ..Default::default()
    };

    if let Some(fix) = entry.fix {
        let fix_data = ExternalFixData {
            title: fix.title,
            range: fix.range.map(|range| range.into_lsp()),
            new_text: fix.new_text,
        };
        diagnostic.data = Some(json!({ "externalFix": fix_data }));
    }

    diagnostic
}

impl ExternalRange {
    fn into_lsp(self) -> Range {
        Range {
            start: self.start.into_lsp(),
            end: self.end.into_lsp(),
        }
    }
}

impl ExternalPosition {
    fn into_lsp(self) -> Position {
        Position {
            line: self.line,
            character: self.character,
        }
    }
}

impl ExternalSeverity {
    fn to_lsp(&self) -> Option<DiagnosticSeverity> {
        match self {
            ExternalSeverity::String(value) => match value.trim().to_ascii_lowercase().as_str() {
                "error" => Some(DiagnosticSeverity::ERROR),
                "warning" => Some(DiagnosticSeverity::WARNING),
                "info" | "information" => Some(DiagnosticSeverity::INFORMATION),
                "hint" => Some(DiagnosticSeverity::HINT),
                _ => None,
            },
            ExternalSeverity::Number(value) => match value {
                1 => Some(DiagnosticSeverity::ERROR),
                2 => Some(DiagnosticSeverity::WARNING),
                3 => Some(DiagnosticSeverity::INFORMATION),
                4 => Some(DiagnosticSeverity::HINT),
                _ => None,
            },
        }
    }
}

#[cfg(test)]
#[path = "external_diagnostics/contract_tests.rs"]
mod contract_tests;
