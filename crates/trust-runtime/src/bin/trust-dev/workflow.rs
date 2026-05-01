use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::Context;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use glob::glob;
use serde::Serialize;
use serde_json::{json, Value as JsonValue};
use toml::Value as TomlValue;
use trust_runtime::bundle_builder::{inspect_project_layout, resolve_sources_root};
use trust_runtime::config::{IoConfig, RuntimeConfig, WebAuthMode};
use trust_runtime::web::ide::{IdeRole, WebIdeState};
use trust_wasm_analysis::{canonical_ast_similarity, canonical_ast_summary, DiagnosticItem};

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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticsSummary {
    target: String,
    errors: usize,
    warnings: usize,
    issues: Vec<AgentIssue>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompileReloadResult {
    target: String,
    dirty: bool,
    errors: usize,
    warnings: usize,
    issues: Vec<AgentIssue>,
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
    project_root: &Path,
    sources_root: Option<&Path>,
    path: Option<&str>,
    content: Option<String>,
) -> anyhow::Result<JsonValue> {
    if content.is_some() && path.is_none() {
        anyhow::bail!("content overrides require a file path");
    }
    let summary = collect_diagnostics_summary(project_root, sources_root, path, content)?;
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
        .map_err(anyhow::Error::new)
        .context("format structured text source")?;
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
    project_root: &Path,
    sources_root: Option<&Path>,
    endpoint: Option<String>,
    token: Option<String>,
) -> anyhow::Result<JsonValue> {
    let summary = collect_diagnostics_summary(project_root, sources_root, None, None)?;
    if summary.errors > 0 {
        return serde_json::to_value(CompileReloadResult {
            target: project_root.display().to_string(),
            dirty: false,
            errors: summary.errors,
            warnings: summary.warnings,
            issues: summary.issues,
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
                errors: summary.errors,
                warnings: summary.warnings,
                issues: summary.issues,
                runtime_status: "error",
                runtime_message: Some(format!("Build failed: {error}")),
                build: None,
                reload: None,
            })
            .context("serialize build-failure compile_reload payload");
        }
    };

    let program_path = project_root.join("program.stbc");
    let bytecode = match std::fs::read(&program_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return serde_json::to_value(CompileReloadResult {
                target: project_root.display().to_string(),
                dirty: false,
                errors: summary.errors,
                warnings: summary.warnings,
                issues: summary.issues,
                runtime_status: "error",
                runtime_message: Some(format!(
                    "Reload failed: unable to read '{}': {error}",
                    program_path.display()
                )),
                build: Some(build),
                reload: None,
            })
            .context("serialize bytecode-read-failure compile_reload payload");
        }
    };

    let reload = match crate::ctl::call_control_request(
        Some(project_root.to_path_buf()),
        endpoint,
        token,
        "bytecode.reload",
        Some(json!({
            "bytes": BASE64_STANDARD.encode(bytecode),
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
                errors: summary.errors,
                warnings: summary.warnings,
                issues: summary.issues,
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
        errors: summary.errors,
        warnings: summary.warnings,
        issues: summary.issues,
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
    let program_path = project_root.join("program.stbc");
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
            "programStbc": {
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

fn collect_diagnostics_summary(
    project_root: &Path,
    sources_root: Option<&Path>,
    path: Option<&str>,
    content: Option<String>,
) -> anyhow::Result<DiagnosticsSummary> {
    let ide = WebIdeState::new(Some(project_root.to_path_buf()));
    let session = ide
        .create_session(IdeRole::Viewer)
        .context("create IDE session for diagnostics")?;

    let project_root = canonicalize_or_self(project_root);
    let mut issues = Vec::new();
    let target = if let Some(path) = path {
        let diagnostics = ide
            .diagnostics(&session.token, path, content)
            .map_err(anyhow::Error::new)
            .with_context(|| format!("collect diagnostics for '{path}'"))?;
        let file = project_root.join(path).display().to_string();
        issues.extend(
            diagnostics
                .into_iter()
                .map(|item| map_diagnostic(path, &file, item)),
        );
        file
    } else {
        let paths = collect_project_source_paths(&project_root, sources_root)?;
        for path in &paths {
            let file = project_root.join(path).display().to_string();
            let diagnostics = ide
                .diagnostics(&session.token, path, None)
                .map_err(anyhow::Error::new)
                .with_context(|| format!("collect diagnostics for '{path}'"))?;
            issues.extend(
                diagnostics
                    .into_iter()
                    .map(|item| map_diagnostic(path.as_str(), &file, item)),
            );
        }
        project_root.display().to_string()
    };

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

    let errors = issues
        .iter()
        .filter(|issue| issue.severity == "error")
        .count();
    let warnings = issues
        .iter()
        .filter(|issue| issue.severity == "warning")
        .count();

    Ok(DiagnosticsSummary {
        target,
        errors,
        warnings,
        issues,
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

fn collect_project_source_paths(
    project_root: &Path,
    sources_root: Option<&Path>,
) -> anyhow::Result<Vec<String>> {
    let project_root = canonicalize_or_self(project_root);
    let sources_root = canonicalize_or_self(&resolve_sources_root(&project_root, sources_root)?);
    let patterns = ["**/*.st", "**/*.ST", "**/*.pou", "**/*.POU"];
    let mut paths = BTreeSet::new();

    for pattern in patterns {
        for entry in glob(&format!("{}/{}", sources_root.display(), pattern))
            .with_context(|| format!("glob project sources under {}", sources_root.display()))?
        {
            let path = canonicalize_or_self(&entry?);
            if !path.is_file() {
                continue;
            }
            let relative = path.strip_prefix(&project_root).with_context(|| {
                format!(
                    "source '{}' does not live under project root '{}'",
                    path.display(),
                    project_root.display()
                )
            })?;
            paths.insert(relative.to_string_lossy().replace('\\', "/"));
        }
    }

    Ok(paths.into_iter().collect())
}

fn map_diagnostic(path: &str, file: &str, diagnostic: DiagnosticItem) -> AgentIssue {
    let severity = diagnostic.severity.to_ascii_lowercase();
    let code = (!diagnostic.code.trim().is_empty()).then_some(diagnostic.code);
    AgentIssue {
        path: path.to_string(),
        file: file.to_string(),
        line: diagnostic.range.start.line + 1,
        column: diagnostic.range.start.character + 1,
        end_line: diagnostic.range.end.line + 1,
        end_column: diagnostic.range.end.character + 1,
        span: AgentIssueSpan {
            start: diagnostic.span.start,
            end: diagnostic.span.end,
        },
        severity,
        message: diagnostic.message,
        code,
        source: "trust-analysis",
        related: diagnostic
            .related
            .into_iter()
            .map(|item| AgentIssueRelated {
                line: item.range.start.line + 1,
                column: item.range.start.character + 1,
                end_line: item.range.end.line + 1,
                end_column: item.range.end.character + 1,
                span: AgentIssueSpan {
                    start: item.span.start,
                    end: item.span.end,
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
            "executionBackend": config.execution_backend.as_str(),
            "controlEndpoint": config.control_endpoint.as_str(),
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
