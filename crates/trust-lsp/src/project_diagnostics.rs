//! A project's diagnostics as the editor shows them (specification 14 §7.7; specification 20 §7).
//! The project is loaded exactly as the language server loads a workspace folder (`trust-lsp.toml`,
//! include paths, libraries, vendor profile, `[diagnostics]` toggles and severity overrides), and
//! every file's diagnostics come from the same function that answers `textDocument/diagnostic`.
//! `trust-lsp check` prints them, and [`project_diagnostics`] gives them to other tools, the agent
//! API's `lsp.diagnostics` among them, so that every route reports the same findings for a project.

use std::path::{Component, Path, PathBuf};

use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Position, Url};

use crate::check::{native_path, silent_client};
use crate::document_text::{DocumentPosition, LineIndex};
use crate::handlers;
use crate::state::ServerState;

/// One diagnosed file: its address, the text it was diagnosed in, and its diagnostics.
pub(crate) struct FileDiagnostics {
    pub(crate) uri: Url,
    pub(crate) content: String,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

/// A project's diagnosed files, its configuration file among them when every source file was
/// diagnosed, and how many source files were diagnosed.
pub(crate) struct Analysis {
    pub(crate) files: Vec<FileDiagnostics>,
    pub(crate) checked: usize,
}

/// Loads the project at `root`, a canonical folder, as the language server loads a workspace
/// folder, and diagnoses its source files: every one, or only those that `only` names (canonical
/// paths, each with the name it was given), with `content` in place of the saved text of the file
/// whose canonical path it names. The project's configuration file is diagnosed too when every
/// source file is.
///
/// # Errors
///
/// A project without source files, and a named file that is not one of the project's.
pub(crate) async fn analyze(
    root: &Path,
    only: &[(PathBuf, String)],
    content: Option<(&Path, &str)>,
) -> Result<Analysis, String> {
    let root_uri = Url::from_directory_path(root)
        .map_err(|()| format!("{}: not a directory path", native_path(root)))?;
    let (client, _service) = silent_client();
    let state = ServerState::new();
    state.set_workspace_folders(vec![root_uri.clone()]);
    handlers::index_workspace(&client, &state).await;
    state.mark_index_first_pass_done();

    let mut documents = state.documents();
    documents.sort_by(|a, b| a.uri.as_str().cmp(b.uri.as_str()));
    if documents.is_empty() {
        return Err(format!(
            "no source files found in {} (see include_paths in trust-lsp.toml)",
            native_path(root)
        ));
    }
    let document_paths: Vec<Option<PathBuf>> = documents
        .iter()
        .map(|doc| {
            doc.uri
                .to_file_path()
                .ok()
                .map(|path| path.canonicalize().unwrap_or(path))
        })
        .collect();
    let position = |path: &Path| {
        document_paths
            .iter()
            .position(|known| known.as_deref() == Some(path))
    };
    // A requested file the project does not contain would otherwise pass unchecked.
    for (path, name) in only {
        if position(path).is_none() {
            return Err(format!(
                "{name}: not a source file of the project in {}",
                native_path(root)
            ));
        }
    }
    if let Some((path, text)) = content {
        let index = position(path).ok_or_else(|| {
            format!(
                "{}: not a source file of the project in {}",
                native_path(path),
                native_path(root)
            )
        })?;
        // As the editor opens a file with changes it has not saved.
        let document = &mut documents[index];
        document.file_id = state.open_document(
            document.uri.clone(),
            document.version.saturating_add(1),
            text.to_owned(),
        );
        text.clone_into(&mut document.content);
    }

    let mut files = Vec::new();
    let mut checked = 0usize;
    for (doc, path) in documents.iter().zip(&document_paths) {
        if !only.is_empty() {
            let Some(path) = path else {
                continue;
            };
            if !only.iter().any(|(wanted, _)| wanted == path) {
                continue;
            }
        }
        checked += 1;
        let diagnostics = handlers::collect_diagnostics_with_ticket(
            &state,
            &doc.uri,
            &doc.content,
            doc.file_id,
            None,
        );
        files.push(FileDiagnostics {
            uri: doc.uri.clone(),
            content: doc.content.clone(),
            diagnostics,
        });
    }
    // The project's configuration file, diagnosed as the editor diagnoses it, so that a broken
    // library or dependency configuration is reported as well (specification 14 §7.7). Naming
    // files narrows the report to them.
    if only.is_empty()
        && let Some(config_path) = state
            .workspace_config_for_uri(&root_uri)
            .and_then(|config| config.config_path)
        && let (Ok(content), Ok(uri)) = (
            std::fs::read_to_string(&config_path),
            Url::from_file_path(&config_path),
        )
    {
        let diagnostics = handlers::config_file_diagnostics(&state, &uri, &content);
        files.push(FileDiagnostics {
            uri,
            content,
            diagnostics,
        });
    }
    Ok(Analysis { files, checked })
}

/// The name of a diagnostic's severity, as `trust-lsp check` writes it.
pub(crate) fn severity_name(severity: Option<DiagnosticSeverity>) -> &'static str {
    match severity {
        Some(DiagnosticSeverity::WARNING) => "warning",
        Some(DiagnosticSeverity::INFORMATION) => "information",
        Some(DiagnosticSeverity::HINT) => "hint",
        _ => "error",
    }
}

/// A diagnostic's code as text.
pub(crate) fn code_text(code: Option<&NumberOrString>) -> Option<String> {
    match code {
        Some(NumberOrString::String(code)) => Some(code.clone()),
        Some(NumberOrString::Number(code)) => Some(code.to_string()),
        None => None,
    }
}

/// One finding in a file of a project, as `trust-lsp check` reports it, in the coordinates of the
/// agent API (specification 20 §7): one-based lines and columns, the columns in UTF-16 code units
/// as the editor counts them, and zero-based UTF-8 byte offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// The file, relative to the project with `/` between its folders; the full path for a file
    /// outside the project, such as a library that the configuration names elsewhere.
    pub path: String,
    /// The file's full path.
    pub file: PathBuf,
    /// Where the finding starts.
    pub line: u32,
    /// Where the finding starts.
    pub column: u32,
    /// Where the finding ends.
    pub end_line: u32,
    /// Where the finding ends.
    pub end_column: u32,
    /// The byte offset where the finding starts.
    pub start: u32,
    /// The byte offset where the finding ends.
    pub end: u32,
    /// `error`, `warning`, `information` or `hint`.
    pub severity: &'static str,
    /// The diagnostic's code, such as `E203`.
    pub code: Option<String>,
    /// What the finding says.
    pub message: String,
    /// The places in the same file that the finding names as related.
    pub related: Vec<RelatedFinding>,
}

/// A place in the finding's file that a finding names as related, in the finding's coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedFinding {
    /// Where the place starts.
    pub line: u32,
    /// Where the place starts.
    pub column: u32,
    /// Where the place ends.
    pub end_line: u32,
    /// Where the place ends.
    pub end_column: u32,
    /// The byte offset where the place starts.
    pub start: u32,
    /// The byte offset where the place ends.
    pub end: u32,
    /// What the finding says of the place.
    pub message: String,
}

/// The findings of the project at `root` as `trust-lsp check` reports them: those of every source
/// file and of the configuration file, or of only the source file that `path` names (relative to
/// `root`, or full), with `content` in place of that file's saved text. They are sorted by file,
/// position, severity, code and message.
///
/// # Errors
///
/// A folder or file that cannot be found, a project without source files, a `path` that is not a
/// source file of the project, `content` without `path`, and an analysis that cannot start.
pub fn project_diagnostics(
    root: &Path,
    path: Option<&Path>,
    content: Option<&str>,
) -> Result<Vec<Finding>, String> {
    if content.is_some() && path.is_none() {
        return Err("content needs the path of the file it replaces".to_owned());
    }
    let root = root
        .canonicalize()
        .map_err(|error| format!("{}: {error}", root.display()))?;
    let named = path
        .map(|path| {
            let full = if path.is_absolute() {
                path.to_path_buf()
            } else {
                root.join(path)
            };
            full.canonicalize()
                .map(|full| (full, path.display().to_string()))
                .map_err(|error| format!("{}: {error}", path.display()))
        })
        .transpose()?;
    let only: Vec<(PathBuf, String)> = named.iter().cloned().collect();
    let replaced = named
        .as_ref()
        .zip(content)
        .map(|((full, _), text)| (full.as_path(), text));
    let analysis = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("start the analysis: {error}"))?
        .block_on(analyze(&root, &only, replaced))?;

    let mut findings = Vec::new();
    for file in &analysis.files {
        let full = file
            .uri
            .to_file_path()
            .unwrap_or_else(|_| PathBuf::from(file.uri.path()));
        let full = full.canonicalize().unwrap_or(full);
        let path = slashed(full.strip_prefix(&root).unwrap_or(&full));
        let index = LineIndex::new(&file.content);
        let offset = |position: Position| {
            index
                .position_to_offset(DocumentPosition {
                    line: position.line,
                    character: position.character,
                })
                .unwrap_or(file.content.len())
                .try_into()
                .unwrap_or(u32::MAX)
        };
        for diagnostic in &file.diagnostics {
            let range = diagnostic.range;
            let related = diagnostic
                .related_information
                .iter()
                .flatten()
                .filter(|related| related.location.uri == file.uri)
                .map(|related| {
                    let range = related.location.range;
                    RelatedFinding {
                        line: range.start.line + 1,
                        column: range.start.character + 1,
                        end_line: range.end.line + 1,
                        end_column: range.end.character + 1,
                        start: offset(range.start),
                        end: offset(range.end),
                        message: related.message.clone(),
                    }
                })
                .collect();
            findings.push(Finding {
                path: path.clone(),
                file: full.clone(),
                line: range.start.line + 1,
                column: range.start.character + 1,
                end_line: range.end.line + 1,
                end_column: range.end.character + 1,
                start: offset(range.start),
                end: offset(range.end),
                severity: severity_name(diagnostic.severity),
                code: code_text(diagnostic.code.as_ref()),
                message: diagnostic.message.clone(),
                related,
            });
        }
    }
    findings.sort_by(|a, b| {
        (&a.path, a.line, a.column, a.severity, &a.code, &a.message)
            .cmp(&(&b.path, b.line, b.column, b.severity, &b.code, &b.message))
    });
    Ok(findings)
}

/// A path with `/` between its parts, whatever the platform writes.
fn slashed(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            Component::RootDir => Some(String::new()),
            Component::Prefix(prefix) => Some(prefix.as_os_str().to_string_lossy().into_owned()),
            Component::CurDir | Component::ParentDir => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
#[path = "project_diagnostics_tests.rs"]
mod tests;
