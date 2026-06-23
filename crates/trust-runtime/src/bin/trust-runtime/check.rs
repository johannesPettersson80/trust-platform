//! Project check command (compile sources/config without writing artifacts).

use std::path::{Path, PathBuf};

use serde::Serialize;
use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::bundle_builder::{check_program_stbc, inspect_project_layout};

use crate::{ci, style};

#[derive(Debug, Clone, Serialize)]
struct CheckIssue {
    severity: &'static str,
    message: String,
    code: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    column: Option<u32>,
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
    bytecode_size: Option<usize>,
}

pub fn run_check(
    project: Option<PathBuf>,
    sources: Option<PathBuf>,
    json: bool,
    ci_mode: bool,
) -> anyhow::Result<()> {
    let project_root = match project {
        Some(path) => path,
        None => detect_bundle_path(None).unwrap_or(std::env::current_dir()?),
    };
    let response = build_check_response(&project_root, sources.as_deref());
    let print_json = json || ci_mode;

    if print_json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else if response.ok {
        println!("{}", style::success("Project check passed"));
        println!("Sources: {} file(s)", response.source_count);
        for path in response.sources.iter().take(5) {
            println!(" - {path}");
        }
        if response.source_count > 5 {
            println!(" - ... +{}", response.source_count - 5);
        }
    } else {
        eprintln!("{}", style::error("Project check failed"));
        for issue in &response.issues {
            let location = issue
                .file
                .as_deref()
                .map_or_else(String::new, |file| format!("{file}: "));
            eprintln!(" - {location}{}", issue.message);
        }
    }

    if response.ok {
        return Ok(());
    }

    std::process::exit(exit_code_for_issues(&response.issues));
}

fn build_check_response(project_root: &Path, sources_root: Option<&Path>) -> CheckProgramResponse {
    let mut issues = config_issues(project_root);
    let mut sources = Vec::new();
    let mut dependency_roots = Vec::new();
    let mut resolved_dependencies = Vec::new();
    let mut bytecode_size = None;

    match inspect_project_layout(project_root, sources_root) {
        Ok(layout) => {
            sources = paths_to_strings(layout.sources);
            dependency_roots = paths_to_strings(layout.dependency_roots);
            resolved_dependencies = layout.resolved_dependencies;
            match check_program_stbc(project_root, sources_root) {
                Ok(report) => {
                    bytecode_size = Some(report.bytecode_size);
                    sources = paths_to_strings(report.sources);
                    dependency_roots = paths_to_strings(report.dependency_roots);
                    resolved_dependencies = report.resolved_dependencies;
                }
                Err(error) => issues.push(issue(
                    "error",
                    format!("program compile failed: {error}"),
                    "compile",
                    None,
                )),
            }
        }
        Err(error) => issues.push(issue(
            "error",
            format!("project source layout is invalid: {error}"),
            "sources",
            None,
        )),
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
        bytecode_size,
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

fn issue(
    severity: &'static str,
    message: String,
    code: &'static str,
    file: Option<PathBuf>,
) -> CheckIssue {
    CheckIssue {
        severity,
        message,
        code,
        file: file.map(|path| path.display().to_string()),
        line: None,
        column: None,
    }
}

fn paths_to_strings(paths: Vec<PathBuf>) -> Vec<String> {
    paths
        .into_iter()
        .map(|path| path.display().to_string())
        .collect()
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
