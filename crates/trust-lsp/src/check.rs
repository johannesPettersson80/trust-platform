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
    native_path(path.strip_prefix(root).unwrap_or(&path))
}

/// A path as the platform writes it: native separators, and on Windows without the
/// verbatim prefix (`\\?\`) that canonicalization adds.
fn native_path(path: &Path) -> String {
    let text = path.display().to_string();
    if cfg!(windows) {
        strip_verbatim_prefix(&text)
    } else {
        text
    }
}

/// `\\?\C:\dir` is `C:\dir` and `\\?\UNC\server\share` is `\\server\share`; other verbatim
/// paths have no plain form and are kept.
fn strip_verbatim_prefix(text: &str) -> String {
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        let bytes = rest.as_bytes();
        if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            return rest.to_string();
        }
    }
    text.to_string()
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
    run_with_output(args, &mut std::io::stdout(), &mut std::io::stderr()).await
}

/// `run`, writing the report to `out` and messages to `err`.
async fn run_with_output(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    match check(args, out, err).await {
        Ok(status) => status,
        Err(message) => {
            // nothing more can be reported when stderr itself fails
            let _ = writeln!(err, "trust-lsp check: {message}");
            2
        }
    }
}

/// The check itself; `Err` is a usage or I/O problem (exit status 2).
async fn check(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Result<i32, String> {
    let options = match parse_args(args) {
        Ok(options) => options,
        Err(message) if message.is_empty() => {
            write!(out, "{USAGE}").map_err(|error| format!("write output: {error}"))?;
            return Ok(0);
        }
        Err(message) => return Err(format!("{message}\n\n{USAGE}")),
    };
    let root = options
        .project
        .canonicalize()
        .map_err(|error| format!("{}: {error}", options.project.display()))?;
    let root_uri = Url::from_directory_path(&root)
        .map_err(|()| format!("{}: not a directory path", native_path(&root)))?;
    let mut only: Vec<(PathBuf, &PathBuf)> = Vec::new();
    for file in &options.files {
        let path = if file.is_absolute() {
            file.clone()
        } else {
            root.join(file)
        };
        let path = path
            .canonicalize()
            .map_err(|error| format!("{}: {error}", file.display()))?;
        only.push((path, file));
    }

    let (client, _service) = silent_client();
    let state = ServerState::new();
    state.set_workspace_folders(vec![root_uri]);
    handlers::index_workspace(&client, &state).await;
    state.mark_index_first_pass_done();

    let mut documents = state.documents();
    documents.sort_by(|a, b| a.uri.as_str().cmp(b.uri.as_str()));
    if documents.is_empty() {
        return Err(format!(
            "no source files found in {} (see include_paths in trust-lsp.toml)",
            native_path(&root)
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
    // A requested file the project does not contain would otherwise pass unchecked.
    for (path, file) in &only {
        if !document_paths.iter().flatten().any(|known| known == path) {
            return Err(format!(
                "{}: not a source file of the project in {}",
                file.display(),
                native_path(&root)
            ));
        }
    }

    let mut items = Vec::new();
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
        items.extend(diagnostics.iter().map(|d| to_item(&root, &doc.uri, d)));
    }
    items.sort_by(|a, b| {
        (&a.path, a.line, a.column, a.severity, &a.code, &a.message)
            .cmp(&(&b.path, b.line, b.column, b.severity, &b.code, &b.message))
    });
    let errors = items.iter().filter(|i| i.severity == "error").count();
    let warnings = items.iter().filter(|i| i.severity == "warning").count();

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
            result.and_then(|()| {
                writeln!(
                    err,
                    "trust-lsp check: {errors} error(s), {warnings} warning(s) in {checked} file(s)"
                )
            })
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
                "project": native_path(&root),
                "files": checked,
                "errors": errors,
                "warnings": warnings,
                "diagnostics": diagnostics,
            });
            writeln!(out, "{payload}")
        }
    };
    written.map_err(|error| format!("write output: {error}"))?;
    if errors > 0 || (options.deny_warnings && warnings > 0) {
        Ok(1)
    } else {
        Ok(0)
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

    /// A two-file project in a fresh temporary directory, removed on drop.
    struct Project(PathBuf);

    impl Project {
        fn new(name: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("trust-lsp-check-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
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
            Self(root)
        }

        fn root(&self) -> &str {
            self.0.to_str().expect("utf-8 path")
        }
    }

    impl Drop for Project {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    async fn check_output(arguments: &[&str]) -> (i32, String, String) {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let status = run_with_output(&args(arguments), &mut out, &mut err).await;
        (
            status,
            String::from_utf8(out).expect("utf-8 stdout"),
            String::from_utf8(err).expect("utf-8 stderr"),
        )
    }

    #[tokio::test]
    async fn reports_project_diagnostics() {
        let project = Project::new("json");
        let (status, out, err) =
            check_output(&["--project", project.root(), "--format", "json"]).await;
        // `missing` is undefined; `Point` comes from the other file and resolves.
        assert_eq!(status, 1, "stderr: {err}");
        let payload: serde_json::Value = serde_json::from_str(&out).expect("one JSON object");
        let main = native_path(&Path::new("src").join("Main.st"));
        assert_eq!(payload["files"], 2);
        assert_eq!(payload["errors"], 1);
        assert_eq!(payload["warnings"], 1);
        assert_eq!(
            payload["diagnostics"],
            json!([
                {
                    "path": main,
                    "line": 1,
                    "column": 9,
                    "endLine": 1,
                    "endColumn": 13,
                    "severity": "warning",
                    "code": "W009",
                    "message": "unused program 'Main'",
                },
                {
                    // the editor's range of the name includes the space before it
                    "path": main,
                    "line": 3,
                    "column": 7,
                    "endLine": 3,
                    "endColumn": 15,
                    "severity": "error",
                    "code": "E101",
                    "message": "undefined identifier 'missing'",
                },
            ])
        );
        assert_eq!(err, "");
    }

    #[tokio::test]
    async fn text_output_lists_each_diagnostic_and_a_summary() {
        let project = Project::new("text");
        let (status, out, err) = check_output(&["--project", project.root()]).await;
        let main = native_path(&Path::new("src").join("Main.st"));
        assert_eq!(status, 1);
        assert_eq!(
            out,
            format!(
                "{main}:1:9: warning[W009]: unused program 'Main'\n\
                 {main}:3:7: error[E101]: undefined identifier 'missing'\n"
            )
        );
        assert_eq!(
            err,
            "trust-lsp check: 1 error(s), 1 warning(s) in 2 file(s)\n"
        );
    }

    #[tokio::test]
    async fn a_file_outside_the_project_is_a_usage_error() {
        let project = Project::new("outside");
        let outside =
            std::env::temp_dir().join(format!("trust-lsp-check-outside-{}.st", std::process::id()));
        std::fs::write(&outside, "PROGRAM Elsewhere\nEND_PROGRAM\n").expect("outside file");
        let outside_text = outside.to_str().expect("utf-8 path").to_string();
        let (status, out, err) =
            check_output(&["--project", project.root(), "--file", &outside_text]).await;
        let _ = std::fs::remove_file(&outside);
        assert_eq!(status, 2);
        assert_eq!(out, "");
        assert!(
            err.starts_with(&format!(
                "trust-lsp check: {outside_text}: not a source file of the project in "
            )),
            "stderr: {err}"
        );
    }

    #[tokio::test]
    async fn file_filter_reports_only_the_named_project_file() {
        let project = Project::new("filter");
        let (status, out, err) =
            check_output(&["--project", project.root(), "--file", "src/Types.st"]).await;
        assert_eq!(status, 0);
        assert_eq!(out, "");
        assert_eq!(
            err,
            "trust-lsp check: 0 error(s), 0 warning(s) in 1 file(s)\n"
        );
    }

    #[tokio::test]
    async fn a_project_without_source_files_is_a_usage_error() {
        let root =
            std::env::temp_dir().join(format!("trust-lsp-check-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("empty project");
        let (status, out, err) =
            check_output(&["--project", root.to_str().expect("utf-8 path")]).await;
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(status, 2);
        assert_eq!(out, "");
        assert!(
            err.starts_with("trust-lsp check: no source files found in "),
            "stderr: {err}"
        );
    }

    #[test]
    fn windows_paths_are_shown_without_the_verbatim_prefix() {
        assert_eq!(
            strip_verbatim_prefix(r"\\?\C:\work\plant"),
            r"C:\work\plant"
        );
        assert_eq!(
            strip_verbatim_prefix(r"\\?\UNC\server\share\plant"),
            r"\\server\share\plant"
        );
        assert_eq!(
            strip_verbatim_prefix(r"\\?\Volume{0}\plant"),
            r"\\?\Volume{0}\plant"
        );
        assert_eq!(strip_verbatim_prefix(r"src\Main.st"), r"src\Main.st");
        assert_eq!(
            native_path(&Path::new("src").join("Main.st")),
            Path::new("src").join("Main.st").display().to_string()
        );
    }
}
