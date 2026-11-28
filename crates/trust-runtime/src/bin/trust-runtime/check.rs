//! Project check command (compile sources/config without writing artifacts).

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use serde::Serialize;
use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::bundle_builder::{check_program, inspect_project_layout};
use trust_runtime::harness::{CompileError, SourceDiagnostic};
use trust_runtime::native::{NativeError, RustDiagnostic};

use crate::{ci, style};

/// One finding of a check or a failed build (specification 11 §6.8.1c): a compile diagnostic
/// carries the compiler's code and where it is, its 1-based lines and columns; a diagnostic of the
/// project's Rust crate also names its source, `truST (rustc)` (specification 48 RU-44).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct CheckIssue {
    severity: &'static str,
    message: String,
    code: Cow<'static, str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    column: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_column: Option<u32>,
}

#[derive(Debug, Serialize)]
struct CheckProgramResponse {
    version: u32,
    command: &'static str,
    ok: bool,
    status: &'static str,
    project: String,
    errors: usize,
    warnings: usize,
    issues: Vec<CheckIssue>,
    source_count: usize,
    sources: Vec<String>,
    dependency_roots: Vec<String>,
    resolved_dependencies: Vec<String>,
}

pub fn run_check(
    project: Option<PathBuf>,
    sources: Vec<PathBuf>,
    json: bool,
    ci_mode: bool,
) -> anyhow::Result<()> {
    let project_root = match project {
        Some(path) => path,
        None => detect_bundle_path(None).unwrap_or(std::env::current_dir()?),
    };
    let response = build_check_response(&project_root, &sources);
    let print_json = json || ci_mode;

    if print_json {
        outln!("{}", serde_json::to_string_pretty(&response)?);
    } else if response.ok {
        outln!("{}", style::success("Project check passed"));
        outln!("Sources: {} file(s)", response.source_count);
        for path in response.sources.iter().take(5) {
            outln!(" - {path}");
        }
        if response.source_count > 5 {
            outln!(" - ... +{}", response.source_count - 5);
        }
        // A passed check's warnings, such as the Rust crate's (specification 11 §6.8.1c).
        if !response.issues.is_empty() {
            outln!("Warnings: {}", response.issues.len());
            for issue in &response.issues {
                outln!(" - {}{}", issue.location(), issue.message);
            }
        }
    } else {
        eprintln!("{}", style::error("Project check failed"));
        for issue in &response.issues {
            eprintln!(" - {}{}", issue.location(), issue.message);
        }
    }

    if response.ok {
        return Ok(());
    }

    std::process::exit(exit_code_for_issues(&response.issues));
}

fn build_check_response(project_root: &Path, source_folders: &[PathBuf]) -> CheckProgramResponse {
    let mut issues = config_issues(project_root);
    let mut sources = Vec::new();
    let mut dependency_roots = Vec::new();
    let mut resolved_dependencies = Vec::new();

    match inspect_project_layout(project_root, source_folders) {
        Ok(layout) => {
            sources = paths_to_strings(layout.sources);
            dependency_roots = paths_to_strings(layout.dependency_roots);
            resolved_dependencies = layout.resolved_dependencies;
            match check_program(project_root, source_folders) {
                Ok(report) => {
                    if let Some(problem) = resource_problem(project_root, &report.resources) {
                        issues.push(issue(
                            "error",
                            problem,
                            "config.resource",
                            Some(project_root.join("runtime.toml")),
                        ));
                    }
                    // The Rust crate's warnings, where they are (specification 11 §6.8.1c).
                    issues.extend(report.crate_warnings.iter().map(rust_issue));
                    sources = paths_to_strings(report.sources);
                    dependency_roots = paths_to_strings(report.dependency_roots);
                    resolved_dependencies = report.resolved_dependencies;
                }
                Err(error) => match compile_issues(&error) {
                    Some(found) => issues.extend(found),
                    None => issues.push(match toolchain_failure(&error) {
                        // A project with a Rust crate is checked with the toolchain's cargo.
                        Some(message) => issue("error", message.to_string(), "toolchain", None),
                        None => issue(
                            "error",
                            format!("program compile failed: {error}"),
                            "compile",
                            None,
                        ),
                    }),
                },
            }
        }
        Err(error) => issues.push(issue(
            "error",
            format!("project source layout is invalid: {error}"),
            "sources",
            None,
        )),
    }

    // The program must not use the project's identify output (specification 11, "Identify", load):
    // `run` refuses such a project, so `check` reports it. A project with other errors is not asked.
    if issues.iter().all(|issue| issue.severity != "error")
        && let Err(error) =
            trust_runtime::bundle_builder::refuse_identify_output_use(project_root, source_folders)
    {
        issues.push(issue(
            "error",
            identify_message(error),
            "config.identify",
            Some(project_root.join("runtime.toml")),
        ));
    }

    let errors = issues
        .iter()
        .filter(|issue| issue.severity == "error")
        .count();
    let warnings = issues
        .iter()
        .filter(|issue| issue.severity == "warning")
        .count();
    let ok = errors == 0;
    CheckProgramResponse {
        version: 1,
        command: "check",
        ok,
        status: if ok { "ok" } else { "failed" },
        project: project_root.display().to_string(),
        errors,
        warnings,
        issues,
        source_count: sources.len(),
        sources,
        dependency_roots,
        resolved_dependencies,
    }
}

/// The runtime runs the configuration's resource that runtime.toml names, else a legacy
/// placeholder; a name the program lacks stops it at start, so the check reports it now.
/// A program without a CONFIGURATION runs under the name its project gives it (specification 11
/// §6.8.1d, specification 44 §5.2), as `trust_runtime::native::check_resource` admits it.
fn resource_problem(project_root: &Path, resources: &[String]) -> Option<String> {
    let config =
        trust_runtime::config::RuntimeConfig::load(project_root.join("runtime.toml")).ok()?;
    let name = config.resource_name.as_str();
    let legacy = resources.is_empty()
        || (resources.len() == 1 && resources[0].eq_ignore_ascii_case("RESOURCE"));
    if legacy
        || resources
            .iter()
            .any(|resource| resource.eq_ignore_ascii_case(name))
    {
        return None;
    }
    let declared: Vec<&str> = resources.iter().map(String::as_str).collect();
    Some(crate::run::resource_mismatch(name, &declared))
}

/// The text of a refused identify output: the refusal of the load check, without the error kind.
fn identify_message(error: trust_runtime::error::RuntimeError) -> String {
    match error {
        trust_runtime::error::RuntimeError::InvalidConfig(text) => text.to_string(),
        other => other.to_string(),
    }
}

fn config_issues(project_root: &Path) -> Vec<CheckIssue> {
    let mut issues = Vec::new();
    validate_required_config(
        project_root.join("runtime.toml"),
        "config.runtime",
        "runtime.toml",
        trust_runtime::config::validate_runtime_toml_text,
        &mut issues,
    );
    validate_required_config(
        project_root.join("io.toml"),
        "config.io",
        "io.toml",
        trust_runtime::config::validate_io_toml_text,
        &mut issues,
    );
    validate_optional_ads_config(project_root, &mut issues);
    validate_optional_opcua_client_config(project_root, &mut issues);
    issues
}

fn validate_required_config(
    path: PathBuf,
    code: &'static str,
    display_name: &'static str,
    validate: fn(&str) -> Result<(), trust_runtime::error::RuntimeError>,
    issues: &mut Vec<CheckIssue>,
) {
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            if let Err(error) = validate(&text) {
                issues.push(issue(
                    "error",
                    format!("{display_name} is invalid: {error}"),
                    code,
                    Some(path),
                ));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            issues.push(issue(
                "error",
                format!("missing {display_name}"),
                code,
                Some(path),
            ));
        }
        Err(error) => issues.push(issue(
            "error",
            format!("failed to read {display_name}: {error}"),
            code,
            Some(path),
        )),
    }
}

fn validate_optional_ads_config(project_root: &Path, issues: &mut Vec<CheckIssue>) {
    let path = project_root.join("ads.toml");
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            if let Err(error) = trust_runtime::ads::parse_ads_toml(&text) {
                issues.push(issue(
                    "error",
                    format!("ads.toml is invalid: {error}"),
                    "config.ads",
                    Some(path),
                ));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => issues.push(issue(
            "error",
            format!("failed to read ads.toml: {error}"),
            "config.ads",
            Some(path),
        )),
    }
}

fn validate_optional_opcua_client_config(project_root: &Path, issues: &mut Vec<CheckIssue>) {
    let path = project_root.join("opcua_client.toml");
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            if let Err(error) = trust_runtime::config::validate_opcua_client_toml_text(&text) {
                issues.push(issue(
                    "error",
                    format!("opcua_client.toml is invalid: {error}"),
                    "config.opcua_client",
                    Some(path),
                ));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => issues.push(issue(
            "error",
            format!("failed to read opcua_client.toml: {error}"),
            "config.opcua_client",
            Some(path),
        )),
    }
}

fn issue(
    severity: &'static str,
    message: String,
    code: &'static str,
    file: Option<PathBuf>,
) -> CheckIssue {
    CheckIssue {
        severity,
        message,
        code: Cow::Borrowed(code),
        source: None,
        file: file.map(|path| path.display().to_string()),
        line: None,
        column: None,
        end_line: None,
        end_column: None,
    }
}

impl CheckIssue {
    /// Whether the issue is an error.
    pub(crate) fn is_error(&self) -> bool {
        self.severity == "error"
    }

    /// Where the issue is, as a person reads it before its message: `file:line:column: `, or
    /// `file: ` without a position, or nothing.
    pub(crate) fn location(&self) -> String {
        match (self.file.as_deref(), self.line, self.column) {
            (Some(file), Some(line), Some(column)) => format!("{file}:{line}:{column}: "),
            (Some(file), _, _) => format!("{file}: "),
            (None, _, _) => String::new(),
        }
    }
}

/// The issues of a compile that failed on its sources' diagnostics, one each, where it is, or on
/// constructs the build does not compile yet, one each, at the construct's place when the build
/// knows it (specification 11 §6.8.1c), or on the errors of the project's Rust crate, with its
/// warnings, each where it is (specification 48 RU-44); `None` for a failure that names no place
/// in the sources, the one construct without a place among them.
pub(crate) fn compile_issues(error: &anyhow::Error) -> Option<Vec<CheckIssue>> {
    let crate_failure = error
        .chain()
        .find_map(|cause| match cause.downcast_ref::<NativeError>() {
            Some(NativeError::Rust(failure)) => Some(failure),
            _ => None,
        });
    if let Some(failure) = crate_failure {
        return Some(failure.diagnostics.iter().map(rust_issue).collect());
    }
    let diagnostics = error.chain().find_map(|cause| {
        let compile = match cause.downcast_ref::<NativeError>() {
            Some(NativeError::Sources(compile)) => Some(compile),
            _ => cause.downcast_ref::<CompileError>(),
        };
        compile.map(CompileError::diagnostics)
    });
    if let Some(diagnostics) = diagnostics {
        return (!diagnostics.is_empty())
            .then(|| diagnostics.iter().map(diagnostic_issue).collect());
    }
    let refused: Vec<&NativeError> =
        error
            .chain()
            .find_map(|cause| match cause.downcast_ref::<NativeError>() {
                Some(NativeError::AllUnsupported(each)) => Some(each.iter().collect()),
                Some(one @ NativeError::Unsupported { .. }) => Some(vec![one]),
                _ => None,
            })?;
    if let [NativeError::Unsupported { at: None, .. }] = refused.as_slice() {
        return None;
    }
    Some(refused.into_iter().map(refusal_issue).collect())
}

/// The issue of a construct the build does not compile yet, at its place when the build knows it.
fn refusal_issue(refusal: &NativeError) -> CheckIssue {
    let place = match refusal {
        NativeError::Unsupported { at, .. } => at.as_ref(),
        _ => None,
    };
    CheckIssue {
        severity: "error",
        message: format!("program compile failed: {refusal}"),
        code: Cow::Borrowed("compile"),
        source: None,
        file: place.map(|place| place.file.clone()),
        line: place.map(|place| place.line),
        column: place.map(|place| place.column),
        end_line: None,
        end_column: None,
    }
}

/// The source of a diagnostic of the project's Rust crate (specification 48 RU-44).
pub(crate) const RUST_SOURCE: &str = "truST (rustc)";

/// The issue of an error or warning of the project's Rust crate, where it is in the crate's files,
/// with the compiler's code, or `rustc` for a diagnostic without one (specification 48 RU-44).
pub(crate) fn rust_issue(diagnostic: &RustDiagnostic) -> CheckIssue {
    CheckIssue {
        severity: diagnostic.severity.name(),
        message: diagnostic.message.clone(),
        code: diagnostic
            .code
            .clone()
            .map_or(Cow::Borrowed("rustc"), Cow::Owned),
        source: Some(RUST_SOURCE),
        file: Some(diagnostic.file.clone()),
        line: Some(diagnostic.line),
        column: Some(diagnostic.column),
        end_line: Some(diagnostic.end_line),
        end_column: Some(diagnostic.end_column),
    }
}

fn diagnostic_issue(diagnostic: &SourceDiagnostic) -> CheckIssue {
    CheckIssue {
        severity: diagnostic.severity,
        message: diagnostic.message.clone(),
        code: Cow::Borrowed(diagnostic.code),
        source: None,
        file: diagnostic.file.clone(),
        line: Some(diagnostic.line),
        column: Some(diagnostic.column),
        end_line: Some(diagnostic.end_line),
        end_column: Some(diagnostic.end_column),
    }
}

fn paths_to_strings(paths: Vec<PathBuf>) -> Vec<String> {
    paths
        .into_iter()
        .map(|path| path.display().to_string())
        .collect()
}

/// The toolchain's failure among `error`'s causes: a project with a Rust crate checked without a
/// toolchain that has cargo and the SDK's sources, or a cargo run that failed without the crate's
/// errors (specification 11 §6.8.1c).
fn toolchain_failure(error: &anyhow::Error) -> Option<&str> {
    error
        .chain()
        .find_map(|cause| match cause.downcast_ref::<NativeError>() {
            Some(NativeError::Toolchain(message) | NativeError::ToolchainNotInstalled(message)) => {
                Some(message.as_str())
            }
            _ => None,
        })
}

fn exit_code_for_issues(issues: &[CheckIssue]) -> i32 {
    if issues
        .iter()
        .any(|issue| issue.code.starts_with("config.") || issue.code == "sources")
    {
        return ci::EXIT_INVALID_CONFIG;
    }
    ci::EXIT_BUILD_FAILED
}
