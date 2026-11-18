//! The compiler's messages of a Rust POU build (specification 48 RU-44): the build's cargo writes
//! each of the compiler's diagnostics as a JSON message (`--message-format json`), which gives the
//! build its report, in cargo's order, and the project's crate its errors and warnings, each where
//! it is in the crate's files.
//!
//! The build gives the compiler the crate's folder as `project` (`--remap-path-prefix`), so a
//! message names a file of the crate `project/src/lib.rs`; the report and the diagnostics name it as
//! the project does, `rust/src/lib.rs`.

use serde::Deserialize;

/// How grave a diagnostic of the project's crate is: an error fails the build, a warning does not
/// (RU-44).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustSeverity {
    /// A compiler error.
    Error,
    /// A compiler warning, which fails the build only when the crate denies it.
    Warning,
}

impl RustSeverity {
    /// Its name in a report: `error` or `warning`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
        }
    }
}

/// A compiler error or warning of the project's crate, at its place in the crate's files
/// (specification 48 RU-44).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustDiagnostic {
    /// Whether it is an error or a warning.
    pub severity: RustSeverity,
    /// The compiler's code: an error's (`E0308`) or a lint's (`unused_variables`); none for a
    /// diagnostic without one.
    pub code: Option<String>,
    /// The compiler's message.
    pub message: String,
    /// The crate's file, relative to the project, with `/` (`rust/src/lib.rs`).
    pub file: String,
    /// The line of its first character, 1-based.
    pub line: u32,
    /// The column of its first character, 1-based, in UTF-16 code units.
    pub column: u32,
    /// The line of its last character, 1-based.
    pub end_line: u32,
    /// The column after its last character, 1-based, in UTF-16 code units.
    pub end_column: u32,
    /// The compiler's text of it, as the report shows it.
    pub rendered: String,
}

/// What the build takes from its cargo's messages.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Messages {
    /// The compiler's text of every message, in cargo's order: the report, with the remarks that
    /// the stack bound reads (`-C remark=prolog-epilog`).
    pub(super) text: String,
    /// The errors and warnings of the project's crate that have a place in its files.
    pub(super) diagnostics: Vec<RustDiagnostic>,
}

/// The name the build gives the compiler for the crate's folder (`--remap-path-prefix`).
pub(super) const REMAPPED_CRATE: &str = "project";

/// One line of cargo's JSON messages, of which the build reads the compiler's.
#[derive(Deserialize)]
struct Line {
    reason: String,
    message: Option<Diagnostic>,
}

/// A diagnostic of the compiler, as it writes it in JSON.
#[derive(Deserialize)]
struct Diagnostic {
    message: String,
    code: Option<Code>,
    level: String,
    spans: Vec<Span>,
    rendered: Option<String>,
}

/// A diagnostic's code.
#[derive(Deserialize)]
struct Code {
    code: String,
}

/// Where a diagnostic lies: 1-based lines and columns, a column counted in characters, the end
/// column after the last character.
#[derive(Deserialize)]
struct Span {
    file_name: String,
    line_start: u32,
    line_end: u32,
    column_start: u32,
    column_end: u32,
    is_primary: bool,
    text: Vec<SpanLine>,
}

/// The text of a line a span covers.
#[derive(Deserialize)]
struct SpanLine {
    text: String,
}

/// The messages of cargo's JSON output `stdout`, the crate's files named as the project names them,
/// below `label`, the crate's folder in the project (`rust`).
pub(super) fn read(stdout: &[u8], label: &str) -> Messages {
    let mut messages = Messages::default();
    for line in String::from_utf8_lossy(stdout).lines() {
        let Ok(Line {
            reason,
            message: Some(diagnostic),
        }) = serde_json::from_str::<Line>(line)
        else {
            continue;
        };
        if reason != "compiler-message" {
            continue;
        }
        let rendered = in_the_project(
            diagnostic
                .rendered
                .as_deref()
                .unwrap_or(&diagnostic.message),
            label,
        );
        messages.text.push_str(&rendered);
        if !rendered.ends_with('\n') {
            messages.text.push('\n');
        }
        if let Some(found) = of_the_crate(&diagnostic, &rendered, label) {
            messages.diagnostics.push(found);
        }
    }
    messages
}

/// The diagnostic `diagnostic` of the crate, rendered as `rendered`, when it is an error or a
/// warning whose primary span lies in the crate's files.
fn of_the_crate(diagnostic: &Diagnostic, rendered: &str, label: &str) -> Option<RustDiagnostic> {
    let severity = match diagnostic.level.as_str() {
        "error" | "error: internal compiler error" => RustSeverity::Error,
        "warning" => RustSeverity::Warning,
        _ => return None,
    };
    let span = diagnostic.spans.iter().find(|span| span.is_primary)?;
    let file = crate_file(&span.file_name, label)?;
    Some(RustDiagnostic {
        severity,
        code: diagnostic.code.as_ref().map(|code| code.code.clone()),
        message: diagnostic.message.clone(),
        file,
        line: span.line_start,
        column: utf16_column(span.text.first(), span.column_start),
        end_line: span.line_end,
        end_column: utf16_column(span.text.last(), span.column_end),
        rendered: rendered.to_string(),
    })
}

/// The project's name of the compiler's file `file_name` when it lies in the crate's folder:
/// `project/src/lib.rs`, or `project\src\lib.rs` on Windows, is `<label>/src/lib.rs`, and
/// `src/lib.rs` when the crate is the project's own folder, `.`, as a Rust-first project's is.
fn crate_file(file_name: &str, label: &str) -> Option<String> {
    let rest = file_name
        .strip_prefix(REMAPPED_CRATE)?
        .strip_prefix(['/', '\\'])?
        .replace('\\', "/");
    Some(match label.trim_start_matches("./").trim_end_matches('/') {
        "" | "." => rest,
        label => format!("{label}/{rest}"),
    })
}

/// The 1-based column, in UTF-16 code units, of the compiler's character column `column` of the
/// line `line`; `column` itself when the compiler gave no text of the line. A column past the
/// line's text, as an end after its last character is, counts one unit per character beyond it.
fn utf16_column(line: Option<&SpanLine>, column: u32) -> u32 {
    let Some(line) = line else {
        return column;
    };
    let before = usize::try_from(column.saturating_sub(1)).unwrap_or(usize::MAX);
    let units: usize = line.text.chars().take(before).map(char::len_utf16).sum();
    let beyond = before.saturating_sub(line.text.chars().count());
    u32::try_from(units + beyond + 1).unwrap_or(u32::MAX)
}

/// `text` with the crate's files named as the project names them: the compiler's location lines,
/// `--> project/src/lib.rs:10:21` and `::: project/src/lib.rs:4:1`, name `<label>/src/lib.rs`.
fn in_the_project(text: &str, label: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let indent = line.len() - line.trim_start().len();
        let (head, rest) = line.split_at(indent);
        let renamed = ["--> ", "::: "].iter().find_map(|arrow| {
            let place = rest.strip_prefix(arrow)?;
            let end = place.find(':').unwrap_or(place.len());
            let file = crate_file(place[..end].trim_end(), label)?;
            Some(format!("{head}{arrow}{file}{}", &place[end..]))
        });
        out.push_str(renamed.as_deref().unwrap_or(line));
    }
    out
}

#[cfg(test)]
mod tests;
