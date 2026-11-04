use std::path::{Path, PathBuf};

use anyhow::Context;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use serde::Serialize;
use serde_json::{json, Value as JsonValue};
use toml::Value as TomlValue;
use trust_runtime::bundle_builder::inspect_project_layout;
use trust_runtime::config::{IoConfig, RuntimeConfig, WebAuthMode};
use trust_runtime::web::ide::{IdeRole, WebIdeState};
use trust_wasm_analysis::{canonical_ast_similarity, canonical_ast_summary};

#[cfg(test)]
#[path = "workflow/contract_tests.rs"]
mod contract_tests;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentIssueSpan {
    start: u32,
    end: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentIssue {
    path: String,
    file: String,
    line: u32,
    column: u32,
    end_line: u32,
    end_column: u32,
    span: AgentIssueSpan,
    severity: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
    source: &'static str,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    related: Vec<AgentIssueRelated>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentIssueRelated {
    line: u32,
    column: u32,
    end_line: u32,
    end_column: u32,
    span: AgentIssueSpan,
    message: String,
}

/// Which page of the listed issues a request asks for (specification 20 §7): `number` from 1,
/// `size` issues on each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IssuePage {
    pub(crate) number: usize,
    pub(crate) size: usize,
}

/// The issues a request lists and the counts over all of them; on a page, the page's issues and
/// the page's place among all of them (specification 20 §7).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct IssueListing {
    errors: usize,
    warnings: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page_size: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_issues: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_pages: Option<usize>,
    issues: Vec<AgentIssue>,
}

impl IssueListing {
    /// `issues` sorted by file, position, severity, code and message, counted, and cut to `page`
    /// when it is given. There is always at least one page; one past the last is empty.
    fn new(mut issues: Vec<AgentIssue>, page: Option<IssuePage>) -> Self {
        issues.sort_by(|a, b| {
            (
                &a.file,
                a.line,
                a.column,
                a.severity.as_str(),
                a.code.as_deref().unwrap_or(""),
                a.message.as_str(),
            )
                .cmp(&(
                    &b.file,
                    b.line,
                    b.column,
                    b.severity.as_str(),
                    b.code.as_deref().unwrap_or(""),
                    b.message.as_str(),
                ))
        });
        let count = |severity: &str| {
            issues
                .iter()
                .filter(|issue| issue.severity == severity)
                .count()
        };
        let (errors, warnings, total_issues) = (count("error"), count("warning"), issues.len());
        let Some(IssuePage { number, size }) = page else {
            return Self {
                errors,
                warnings,
                page: None,
                page_size: None,
                total_issues: None,
                total_pages: None,
                issues,
            };
        };
        Self {
            errors,
            warnings,
            page: Some(number),
            page_size: Some(size),
            total_issues: Some(total_issues),
            total_pages: Some(total_issues.div_ceil(size).max(1)),
            issues: issues
                .into_iter()
                .skip(number.saturating_sub(1).saturating_mul(size))
                .take(size)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticsSummary {
    target: String,
    #[serde(flatten)]
    listing: IssueListing,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompileReloadResult {
    target: String,
    dirty: bool,
    #[serde(flatten)]
    listing: IssueListing,
    runtime_status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    runtime_message: Option<String>,
    build: Option<JsonValue>,
    reload: Option<JsonValue>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FormatPreview {
    path: String,
    file: String,
    content: String,
    changed: bool,
}

pub(crate) fn diagnostics_payload(
    workspace_root: &Path,
    project_root: &Path,
    sources_root: Option<&Path>,
    path: Option<&str>,
    content: Option<String>,
    page: Option<IssuePage>,
) -> anyhow::Result<JsonValue> {
    if content.is_some() && path.is_none() {
        anyhow::bail!("content overrides require a file path");
    }
    let summary = collect_diagnostics_summary(
        workspace_root,
        project_root,
        sources_root,
        path,
        content,
        page,
    )?;
    serde_json::to_value(summary).context("serialize diagnostics payload")
}

pub(crate) fn ast_canonicalize_payload(
    project_root: &Path,
    path: Option<&str>,
    content: Option<String>,
) -> anyhow::Result<JsonValue> {
    let source = read_analysis_source(project_root, path, content, "canonical AST")?;
    serde_json::to_value(canonical_ast_summary(&source)).context("serialize canonical AST payload")
}

pub(crate) fn ast_similarity_payload(
    project_root: &Path,
    left_path: Option<&str>,
    left_content: Option<String>,
    right_path: Option<&str>,
    right_content: Option<String>,
) -> anyhow::Result<JsonValue> {
    let left = read_analysis_source(project_root, left_path, left_content, "left AST input")?;
    let right = read_analysis_source(project_root, right_path, right_content, "right AST input")?;
    serde_json::to_value(canonical_ast_similarity(&left, &right))
        .context("serialize AST similarity payload")
}

pub(crate) fn format_payload(
    project_root: &Path,
    path: &str,
    content: Option<String>,
) -> anyhow::Result<JsonValue> {
    let ide = WebIdeState::new(Some(project_root.to_path_buf()));
    let session = ide
        .create_session(IdeRole::Viewer)
        .context("create IDE session for formatting")?;
    let formatted = ide
        .format_source(&session.token, path, content)
        .map_err(|error| anyhow::anyhow!("format structured text source: {error}"))?;
    let file_path = project_root.join(&formatted.path);
    serde_json::to_value(FormatPreview {
        path: formatted.path,
        file: file_path.display().to_string(),
        content: formatted.content,
        changed: formatted.changed,
    })
    .context("serialize format payload")
}

pub(crate) fn compile_reload_payload(
    workspace_root: &Path,
    project_root: &Path,
    sources_root: Option<&Path>,
    endpoint: Option<String>,
    token: Option<String>,
) -> anyhow::Result<JsonValue> {
    let summary =
        collect_diagnostics_summary(workspace_root, project_root, sources_root, None, None, None)?;
    if summary.listing.errors > 0 {
        return serde_json::to_value(CompileReloadResult {
            target: project_root.display().to_string(),
            dirty: false,
            listing: summary.listing,
            runtime_status: "skipped",
            runtime_message: Some("Build and reload blocked by diagnostics.".to_string()),
            build: None,
            reload: None,
        })
        .context("serialize blocked compile_reload payload");
    }

    let build = match crate::build::build_json_payload(
        Some(project_root.to_path_buf()),
        sources_root.map(Path::to_path_buf),
    ) {
        Ok(payload) => payload,
        Err(error) => {
            return serde_json::to_value(CompileReloadResult {
                target: project_root.display().to_string(),
                dirty: false,
                listing: summary.listing,
                runtime_status: "error",
                runtime_message: Some(format!("Build failed: {error}")),
                build: None,
                reload: None,
            })
            .context("serialize build-failure compile_reload payload");
        }
    };

    let program_path = project_root.join(trust_runtime::native::PROGRAM_IMAGE_FILE);
    let image = match std::fs::read(&program_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return serde_json::to_value(CompileReloadResult {
                target: project_root.display().to_string(),
                dirty: false,
                listing: summary.listing,
                runtime_status: "error",
                runtime_message: Some(format!(
                    "Reload failed: unable to read '{}': {error}",
                    program_path.display()
                )),
                build: Some(build),
                reload: None,
            })
            .context("serialize image-read-failure compile_reload payload");
        }
    };

    let reload = match crate::ctl::call_control_request(
        Some(project_root.to_path_buf()),
        endpoint,
        token,
        "bytecode.reload",
        Some(json!({
            "bytes": BASE64_STANDARD.encode(image),
        })),
    ) {
        Ok(control) => json!({
            "endpoint": control.endpoint,
            "result": control.result,
            "response": control.raw_response,
        }),
        Err(error) => {
            return serde_json::to_value(CompileReloadResult {
                target: project_root.display().to_string(),
                dirty: false,
                listing: summary.listing,
                runtime_status: "error",
                runtime_message: Some(format!("Reload failed: {error}")),
                build: Some(build),
                reload: None,
            })
            .context("serialize reload-failure compile_reload payload");
        }
    };

    serde_json::to_value(CompileReloadResult {
        target: project_root.display().to_string(),
        dirty: false,
        listing: summary.listing,
        runtime_status: "ok",
        runtime_message: Some("Runtime reload succeeded.".to_string()),
        build: Some(build),
        reload: Some(reload),
    })
    .context("serialize compile_reload payload")
}

pub(crate) fn project_info_payload(
    project_root: &Path,
    sources_root: Option<&Path>,
) -> anyhow::Result<JsonValue> {
    let inspection = inspect_project_layout(project_root, sources_root)?;
    let project_root = canonicalize_or_self(project_root);
    let runtime_path = project_root.join("runtime.toml");
    let io_path = project_root.join("io.toml");
    let simulation_path = project_root.join("simulation.toml");
    let program_path = project_root.join(trust_runtime::native::PROGRAM_IMAGE_FILE);
    let manifest_path = inspection
        .manifest_path
        .clone()
        .unwrap_or_else(|| project_root.join("trust-lsp.toml"));

    let source_root_display = canonicalize_or_self(&inspection.sources_root)
        .display()
        .to_string();
    let sources = inspection
        .sources
        .iter()
        .map(|path| display_relative_or_absolute(&project_root, path))
        .collect::<Vec<_>>();
    let dependency_roots = inspection
        .dependency_roots
        .iter()
        .map(|path| canonicalize_or_self(path).display().to_string())
        .collect::<Vec<_>>();

    let vendor_profile = match inspection.manifest_path.as_deref() {
        Some(path) => read_vendor_profile(path)?,
        None => None,
    };
    let runtime = read_runtime_summary(&runtime_path);
    let io = read_io_summary(&io_path);

    Ok(json!({
        "version": 1,
        "project": project_root.display().to_string(),
        "sourcesRoot": source_root_display,
        "sourceCount": sources.len(),
        "sources": sources,
        "dependencyRoots": dependency_roots,
        "resolvedDependencies": inspection.resolved_dependencies,
        "files": {
            "runtimeToml": {
                "path": runtime_path.display().to_string(),
                "exists": runtime_path.is_file(),
            },
            "ioToml": {
                "path": io_path.display().to_string(),
                "exists": io_path.is_file(),
            },
            "simulationToml": {
                "path": simulation_path.display().to_string(),
                "exists": simulation_path.is_file(),
            },
            "trustLspToml": {
                "path": manifest_path.display().to_string(),
                "exists": inspection.manifest_path.is_some(),
            },
            "programImage": {
                "path": program_path.display().to_string(),
                "exists": program_path.is_file(),
            },
        },
        "lsp": {
            "manifestPath": inspection
                .manifest_path
                .as_ref()
                .map(|path| path.display().to_string()),
            "vendorProfile": vendor_profile,
        },
        "runtime": runtime,
        "io": io,
    }))
}

/// The diagnostics of the project at `project_root`, or of its file `path` with `content` in place
/// of the saved text, as the language server reports them: the same findings as `trust-lsp check`,
/// from the project's `trust-lsp.toml` (specification 20 §7). Only the files inside
/// `workspace_root`, and under `sources_root` when it is given, are listed: a library that the
/// configuration names outside the workspace is analyzed but not listed (specification 20 §3).
fn collect_diagnostics_summary(
    workspace_root: &Path,
    project_root: &Path,
    sources_root: Option<&Path>,
    path: Option<&str>,
    content: Option<String>,
    page: Option<IssuePage>,
) -> anyhow::Result<DiagnosticsSummary> {
    let project_root = canonicalize_or_self(project_root);
    let workspace_root = canonicalize_or_self(workspace_root);
    // `sources_root` narrows a whole-project listing; with `path`, only that file is listed.
    let sources_root = match (path, sources_root) {
        (None, Some(root)) if !root.is_dir() => {
            anyhow::bail!("sources directory not found: {}", root.display())
        }
        (None, Some(root)) => Some(canonicalize_or_self(root)),
        _ => None,
    };
    let analysis = trust_lsp::project_diagnostics(&project_root, path.map(Path::new), content.as_deref())
            // The reason names the file or the project.
            .map_err(|reason| match path {
                Some(_) => anyhow::anyhow!("collect diagnostics: {reason}"),
                None => anyhow::anyhow!("collect project diagnostics: {reason}"),
            })?;
    // A folder that the language server does not analyze never answers as clean.
    if let Some(root) = &sources_root
        && !analysis.files.iter().any(|file| file.starts_with(root))
    {
        anyhow::bail!(
            "sources directory {} holds none of the project's source files (the include_paths of \
             trust-lsp.toml decide them)",
            root.display()
        );
    }
    let issues = analysis
        .findings
        .into_iter()
        .filter(|finding| finding.file.starts_with(&workspace_root))
        .filter(|finding| {
            sources_root
                .as_ref()
                .is_none_or(|root| finding.file.starts_with(root))
        })
        .map(map_finding)
        .collect();
    let target = match path {
        Some(path) => project_root.join(path).display().to_string(),
        None => project_root.display().to_string(),
    };
    Ok(DiagnosticsSummary {
        target,
        listing: IssueListing::new(issues, page),
    })
}

fn read_analysis_source(
    project_root: &Path,
    path: Option<&str>,
    content: Option<String>,
    label: &str,
) -> anyhow::Result<String> {
    if let Some(content) = content {
        return Ok(content);
    }

    let Some(path) = path else {
        anyhow::bail!("{label} requires either inline content or a project-relative path");
    };

    let full_path = canonicalize_or_self(project_root).join(path);
    std::fs::read_to_string(&full_path)
        .with_context(|| format!("read {label} source '{}'", full_path.display()))
}

/// A finding of the language server as an agent issue: the same coordinates (one-based lines and
/// columns in UTF-16 code units, zero-based UTF-8 byte offsets) and its related places.
fn map_finding(finding: trust_lsp::Finding) -> AgentIssue {
    AgentIssue {
        path: finding.path,
        file: finding.file.display().to_string(),
        line: finding.line,
        column: finding.column,
        end_line: finding.end_line,
        end_column: finding.end_column,
        span: AgentIssueSpan {
            start: finding.start,
            end: finding.end,
        },
        severity: finding.severity.to_owned(),
        message: finding.message,
        code: finding.code.filter(|code| !code.trim().is_empty()),
        source: "trust-lsp",
        related: finding
            .related
            .into_iter()
            .map(|item| AgentIssueRelated {
                line: item.line,
                column: item.column,
                end_line: item.end_line,
                end_column: item.end_column,
                span: AgentIssueSpan {
                    start: item.start,
                    end: item.end,
                },
                message: item.message,
            })
            .collect(),
    }
}

fn read_vendor_profile(path: &Path) -> anyhow::Result<Option<String>> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read project manifest '{}'", path.display()))?;
    let parsed = toml::from_str::<TomlValue>(&text)
        .with_context(|| format!("parse project manifest '{}'", path.display()))?;
    Ok(parsed
        .get("project")
        .and_then(TomlValue::as_table)
        .and_then(|section| section.get("vendor_profile"))
        .and_then(TomlValue::as_str)
        .map(ToOwned::to_owned))
}

fn read_runtime_summary(path: &Path) -> JsonValue {
    if !path.is_file() {
        return json!({
            "available": false,
            "path": path.display().to_string(),
        });
    }
    match RuntimeConfig::load(path) {
        Ok(config) => json!({
            "available": true,
            "path": path.display().to_string(),
            "executionBackend": trust_runtime::execution_backend::EXECUTION_BACKEND,
            "controlEndpoint": config.control_endpoint.as_deref(),
            "hasControlToken": config.control_auth_token.is_some(),
            "controlDebugEnabled": config.control_debug_enabled,
            "web": {
                "enabled": config.web.enabled,
                "listen": config.web.listen.as_str(),
                "auth": match config.web.auth {
                    WebAuthMode::Local => "local",
                    WebAuthMode::Token => "token",
                },
                "tls": config.web.tls,
            },
            "discovery": {
                "enabled": config.discovery.enabled,
                "serviceName": config.discovery.service_name.as_str(),
                "advertise": config.discovery.advertise,
            },
            "mesh": {
                "enabled": config.mesh.enabled,
                "role": config.mesh.role.as_str(),
                "listen": config.mesh.listen.as_str(),
                "connect": config.mesh.connect.iter().map(|value| value.as_str()).collect::<Vec<_>>(),
            },
            "runtimeCloudProfile": config.runtime_cloud_profile.as_str(),
        }),
        Err(error) => json!({
            "available": true,
            "path": path.display().to_string(),
            "parseError": error.to_string(),
        }),
    }
}

fn read_io_summary(path: &Path) -> JsonValue {
    if !path.is_file() {
        return json!({
            "available": false,
            "path": path.display().to_string(),
        });
    }
    match IoConfig::load(path) {
        Ok(config) => json!({
            "available": true,
            "path": path.display().to_string(),
            "driverCount": config.drivers.len(),
            "drivers": config.drivers.iter().map(|driver| driver.name.as_str()).collect::<Vec<_>>(),
            "safeStateCount": config.safe_state.outputs.len(),
        }),
        Err(error) => json!({
            "available": true,
            "path": path.display().to_string(),
            "parseError": error.to_string(),
        }),
    }
}

fn display_relative_or_absolute(project_root: &Path, path: &Path) -> String {
    let normalized = canonicalize_or_self(path);
    normalized
        .strip_prefix(project_root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| normalized.display().to_string())
}

fn canonicalize_or_self(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
