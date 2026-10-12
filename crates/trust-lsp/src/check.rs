//! `trust-lsp check`: the editor's diagnostics for a whole project, from the command line.
//!
//! The project is loaded exactly as the language server loads a workspace folder
//! (`trust-lsp.toml`, include paths, libraries, vendor profile, `[diagnostics]` toggles
//! and severity overrides), and every file's diagnostics come from the same function that
//! answers `textDocument/diagnostic`. CI and batch tools therefore see what the editor
//! shows, without speaking LSP.
//!
//! ```text
//! trust-lsp check [--project DIR] [--format text|json] [--file PATH]... [--deny-warnings]
//! ```
//!
//! Exit status: 0 no errors, 1 errors (or warnings with `--deny-warnings`), 2 usage or
//! I/O problem.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::json;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Url};
use tower_lsp::{Client, LanguageServer, LspService};

use crate::handlers;
use crate::state::ServerState;

const USAGE: &str =
    "usage: trust-lsp check [--project DIR] [--format text|json] [--file PATH]... [--deny-warnings]

Prints the diagnostics the language server reports for every source file of the
project (configured by trust-lsp.toml in DIR, default: current directory).

  --project DIR      project root (default: .)
  --format FORMAT    text (default) or json
  --file PATH        report only this file (repeatable; the whole project is still
                     analyzed, so cross-file diagnostics stay correct)
  --deny-warnings    exit 1 on warnings too

Exit status: 0 no errors, 1 errors (or warnings with --deny-warnings), 2 usage error.
";

#[derive(Debug, PartialEq, Eq)]
enum Format {
    Text,
    Json,
}

#[derive(Debug)]
struct Options {
    project: PathBuf,
    format: Format,
    files: Vec<PathBuf>,
    deny_warnings: bool,
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        project: PathBuf::from("."),
        format: Format::Text,
        files: Vec::new(),
        deny_warnings: false,
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = |name: &str| {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "--project" => options.project = PathBuf::from(value("--project")?),
            "--format" => {
                options.format = match value("--format")?.as_str() {
                    "text" => Format::Text,
                    "json" => Format::Json,
                    other => return Err(format!("unknown format '{other}' (text, json)")),
                }
            }
            "--file" => options.files.push(PathBuf::from(value("--file")?)),
            "--deny-warnings" => options.deny_warnings = true,
            "-h" | "--help" => return Err(String::new()),
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    Ok(options)
}

/// A language server that is never driven: `index_workspace` needs a `Client` for
/// progress and log messages, which go nowhere in a command-line run.
struct Silent;

#[tower_lsp::async_trait]
impl LanguageServer for Silent {
    async fn initialize(
        &self,
        _: tower_lsp::lsp_types::InitializeParams,
    ) -> tower_lsp::jsonrpc::Result<tower_lsp::lsp_types::InitializeResult> {
        Ok(tower_lsp::lsp_types::InitializeResult::default())
    }

    async fn shutdown(&self) -> tower_lsp::jsonrpc::Result<()> {
        Ok(())
    }
}

fn silent_client() -> (Client, LspService<Silent>) {
    let captured = Arc::new(Mutex::new(None));
    let captured_clone = Arc::clone(&captured);
    let (service, socket) = LspService::new(move |client| {
        *captured_clone.lock().expect("lock client") = Some(client.clone());
        Silent
    });
    drop(socket);
    let client = captured
        .lock()
        .expect("lock client")
        .take()
        .expect("client created by LspService::new");
    (client, service)
}

struct Item {
    path: String,
    line: u32,
    column: u32,
    end_line: u32,
    end_column: u32,
    severity: &'static str,
    code: Option<String>,
    message: String,
}

fn severity_name(severity: Option<DiagnosticSeverity>) -> &'static str {
    match severity {
        Some(DiagnosticSeverity::WARNING) => "warning",
        Some(DiagnosticSeverity::INFORMATION) => "information",
        Some(DiagnosticSeverity::HINT) => "hint",
        _ => "error",
    }
}

fn code_text(code: &Option<NumberOrString>) -> Option<String> {
    match code {
        Some(NumberOrString::String(code)) => Some(code.clone()),
        Some(NumberOrString::Number(code)) => Some(code.to_string()),
        None => None,
    }
}

fn display_path(root: &Path, uri: &Url) -> String {
    let path = uri
        .to_file_path()
        .unwrap_or_else(|_| PathBuf::from(uri.path()));
    // `root` is canonical; on Windows that is the verbatim form (`\\?\C:\...`), which a
    // path from a file URI never has, so canonicalize this side too before stripping.
    let path = path.canonicalize().unwrap_or(path);
    path.strip_prefix(root)
        .unwrap_or(&path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn to_item(root: &Path, uri: &Url, diagnostic: &Diagnostic) -> Item {
    let range = diagnostic.range;
    Item {
        path: display_path(root, uri),
        line: range.start.line + 1,
        column: range.start.character + 1,
        end_line: range.end.line + 1,
        end_column: range.end.character + 1,
        severity: severity_name(diagnostic.severity),
        code: code_text(&diagnostic.code),
        message: diagnostic.message.clone(),
    }
}

/// Runs `trust-lsp check` with the arguments after `check`; returns the exit status.
pub async fn run(args: &[String]) -> i32 {
    let options = match parse_args(args) {
        Ok(options) => options,
        Err(message) => {
            if message.is_empty() {
                print!("{USAGE}");
                return 0;
            }
            eprintln!("trust-lsp check: {message}\n\n{USAGE}");
            return 2;
        }
    };
    let root = match options.project.canonicalize() {
        Ok(root) => root,
        Err(error) => {
            eprintln!("trust-lsp check: {}: {error}", options.project.display());
            return 2;
        }
    };
    let Ok(root_uri) = Url::from_directory_path(&root) else {
        eprintln!("trust-lsp check: {}: not a directory path", root.display());
        return 2;
    };
    let mut only: Vec<PathBuf> = Vec::new();
    for file in &options.files {
        let path = if file.is_absolute() {
            file.clone()
        } else {
            root.join(file)
        };
        match path.canonicalize() {
            Ok(path) => only.push(path),
            Err(error) => {
                eprintln!("trust-lsp check: {}: {error}", file.display());
                return 2;
            }
        }
    }

    let (client, _service) = silent_client();
    let state = ServerState::new();
    state.set_workspace_folders(vec![root_uri]);
    handlers::index_workspace(&client, &state).await;
    state.mark_index_first_pass_done();

    let mut documents = state.documents();
    documents.sort_by(|a, b| a.uri.as_str().cmp(b.uri.as_str()));
    let mut items = Vec::new();
    let mut checked = 0usize;
    for doc in &documents {
        if !only.is_empty() {
            let Ok(path) = doc.uri.to_file_path() else {
                continue;
            };
            let path = path.canonicalize().unwrap_or(path);
            if !only.contains(&path) {
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
        items.extend(diagnostics.iter().map(|d| to_item(&root, &doc.uri, d)));
    }
    items.sort_by(|a, b| {
        (&a.path, a.line, a.column, a.severity, &a.code, &a.message)
            .cmp(&(&b.path, b.line, b.column, b.severity, &b.code, &b.message))
    });
    let errors = items.iter().filter(|i| i.severity == "error").count();
    let warnings = items.iter().filter(|i| i.severity == "warning").count();

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let written = match options.format {
        Format::Text => {
            let mut result = Ok(());
            for item in &items {
                let code = item
                    .code
                    .as_deref()
                    .map(|code| format!("[{code}]"))
                    .unwrap_or_default();
                result = result.and_then(|()| {
                    writeln!(
                        out,
                        "{}:{}:{}: {}{}: {}",
                        item.path, item.line, item.column, item.severity, code, item.message
                    )
                });
            }
            eprintln!(
                "trust-lsp check: {errors} error(s), {warnings} warning(s) in {checked} file(s)"
            );
            result
        }
        Format::Json => {
            let diagnostics: Vec<_> = items
                .iter()
                .map(|item| {
                    json!({
                        "path": item.path,
                        "line": item.line,
                        "column": item.column,
                        "endLine": item.end_line,
                        "endColumn": item.end_column,
                        "severity": item.severity,
                        "code": item.code,
                        "message": item.message,
                    })
                })
                .collect();
            let payload = json!({
                "version": 1,
                "project": root.display().to_string(),
                "files": checked,
                "errors": errors,
                "warnings": warnings,
                "diagnostics": diagnostics,
            });
            writeln!(out, "{payload}")
        }
    };
    if let Err(error) = written {
        eprintln!("trust-lsp check: write output: {error}");
        return 2;
    }
    if errors > 0 || (options.deny_warnings && warnings > 0) {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_options() {
        let o = parse_args(&args(&[
            "--project",
            "p",
            "--format",
            "json",
            "--file",
            "src/a.st",
            "--file",
            "src/b.st",
            "--deny-warnings",
        ]))
        .expect("valid arguments");
        assert_eq!(o.project, PathBuf::from("p"));
        assert_eq!(o.format, Format::Json);
        assert_eq!(o.files.len(), 2);
        assert!(o.deny_warnings);
    }

    #[test]
    fn rejects_unknown_arguments() {
        assert!(parse_args(&args(&["--bogus"])).is_err());
        assert!(parse_args(&args(&["--format", "xml"])).is_err());
        assert!(parse_args(&args(&["--project"])).is_err());
    }

    #[tokio::test]
    async fn reports_project_diagnostics() {
        let root = std::env::temp_dir().join(format!("trust-lsp-check-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        std::fs::create_dir_all(root.join("src")).expect("src dir");
        std::fs::write(
            root.join("trust-lsp.toml"),
            "[project]\ninclude_paths = [\"src\"]\n",
        )
        .expect("config");
        std::fs::write(
            root.join("src/Types.st"),
            "TYPE Point : STRUCT x : INT; END_STRUCT END_TYPE\n",
        )
        .expect("types");
        std::fs::write(
            root.join("src/Main.st"),
            "PROGRAM Main\nVAR p : Point; END_VAR\np.x := missing;\nEND_PROGRAM\n",
        )
        .expect("main");
        let status = run(&args(&[
            "--project",
            root.to_str().expect("utf-8 path"),
            "--format",
            "json",
        ]))
        .await;
        // `missing` is undefined; `Point` comes from the other file and must resolve.
        assert_eq!(status, 1);
    }
}
