use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::Context;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use trust_runtime::bundle_builder::collect_project_source_files;
use trust_runtime::harness::{
    decode_json_value, encode_json_value, HarnessAutomation, HarnessAutomationError,
};
use trust_runtime::RestartMode;

#[path = "agent/harness.rs"]
mod harness;
const JSON_RPC_VERSION: &str = "2.0";
const ERROR_PATH_OUTSIDE_WORKSPACE: i32 = -32001;
const ERROR_IO: i32 = -32002;
const ERROR_HARNESS_NOT_LOADED: i32 = -32003;
const ERROR_RUN_UNTIL_TIMEOUT: i32 = -32004;

pub fn run_agent_serve(project: Option<PathBuf>) -> anyhow::Result<()> {
    let mut server = AgentServer::new(resolve_workspace_root(project)?);
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line.context("read agent request")?;
        if line.trim().is_empty() {
            continue;
        }
        let response = server.handle_line(&line);
        writeln!(out, "{}", serde_json::to_string(&response)?)?;
        out.flush()?;
    }

    Ok(())
}

fn resolve_workspace_root(project: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    let root = match project {
        Some(path) => path,
        None => std::env::current_dir().context("resolve current directory for agent serve")?,
    };
    if !root.exists() {
        anyhow::bail!("workspace root '{}' does not exist", root.display());
    }
    if !root.is_dir() {
        anyhow::bail!("workspace root '{}' is not a directory", root.display());
    }
    root.canonicalize()
        .with_context(|| format!("canonicalize workspace root '{}'", root.display()))
}

struct AgentServer {
    workspace_root: PathBuf,
    harness: HarnessAutomation,
}

impl AgentServer {
    fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
            harness: HarnessAutomation::new(),
        }
    }

    fn handle_line(&mut self, line: &str) -> JsonRpcResponse {
        match serde_json::from_str::<JsonRpcRequest>(line) {
            Ok(request) => self.handle_request(request),
            Err(error) => JsonRpcResponse::error(
                JsonValue::Null,
                JsonRpcError::new(-32700, format!("Parse error: {error}"), None),
            ),
        }
    }

    fn handle_request(&mut self, request: JsonRpcRequest) -> JsonRpcResponse {
        if request.jsonrpc != JSON_RPC_VERSION {
            return JsonRpcResponse::error(
                request.id,
                JsonRpcError::new(
                    -32600,
                    format!("Unsupported jsonrpc version '{}'", request.jsonrpc),
                    None,
                ),
            );
        }

        match self.execute(&request.method, request.params) {
            Ok(result) => JsonRpcResponse::success(request.id, result),
            Err(error) => JsonRpcResponse::error(request.id, error.into()),
        }
    }

    fn execute(
        &mut self,
        method: &str,
        params: Option<JsonValue>,
    ) -> Result<JsonValue, AgentCommandError> {
        match method {
            "agent.describe" => Ok(json!({
                "workspace_root": self.workspace_root.display().to_string(),
                "framing": "jsonl",
                "transport": "stdio",
                "methods": [
                    "agent.describe",
                    "workspace.read",
                    "workspace.write",
                    "workspace.project_info",
                    "lsp.diagnostics",
                    "lsp.ast_canonicalize",
                    "lsp.ast_similarity",
                    "lsp.format",
                    "runtime.build",
                    "runtime.compile_reload",
                    "runtime.validate",
                    "runtime.test",
                    "runtime.reload",
                    "harness.load",
                    "harness.reload",
                    "harness.execute",
                    "harness.cycle",
                    "harness.set_input",
                    "harness.get_output",
                    "harness.advance_time",
                    "harness.run_until",
                ],
                "notes": [
                    "Transport is stdio JSON-RPC only in v1.",
                    "Network listeners are intentionally unsupported in this initial agent surface.",
                    "lsp.diagnostics and lsp.format reuse the in-process Web IDE analysis/formatting services.",
                    "lsp.ast_canonicalize and lsp.ast_similarity expose the canonical AST 5-gram normalization path used by contamination and dedup tooling.",
                    "runtime.compile_reload returns diagnostics plus build/reload status for iterative repair loops.",
                    "runtime.reload rebuilds program.stbc and sends bytecode.reload to the configured control endpoint.",
                    "harness.execute runs a fresh deterministic fixture session and reports pass/fail plus reduced failure context for benchmark/datagen callers.",
                    "Source-aware attached-session reload flows remain future work.",
                ],
            })),
            "workspace.read" => self.workspace_read(parse_params(params)?),
            "workspace.write" => self.workspace_write(parse_params(params)?),
            "workspace.project_info" => self.workspace_project_info(parse_optional_params(params)?),
            "lsp.diagnostics" => self.lsp_diagnostics(parse_optional_params(params)?),
            "lsp.ast_canonicalize" => self.lsp_ast_canonicalize(parse_optional_params(params)?),
            "lsp.ast_similarity" => self.lsp_ast_similarity(parse_optional_params(params)?),
            "lsp.format" => self.lsp_format(parse_params(params)?),
            "runtime.build" => self.runtime_build(parse_optional_params(params)?),
            "runtime.compile_reload" => self.runtime_compile_reload(parse_optional_params(params)?),
            "runtime.validate" => self.runtime_validate(parse_optional_params(params)?),
            "runtime.test" => self.runtime_test(parse_optional_params(params)?),
            "runtime.reload" => self.runtime_reload(parse_optional_params(params)?),
            "harness.load" => self.harness_load(parse_optional_params(params)?),
            "harness.reload" => self.harness_reload(parse_optional_params(params)?),
            "harness.execute" => self.harness_execute(parse_optional_params(params)?),
            "harness.cycle" => self.harness_cycle(parse_optional_params(params)?),
            "harness.set_input" => self.harness_set_input(parse_params(params)?),
            "harness.get_output" => self.harness_get_output(parse_params(params)?),
            "harness.advance_time" => self.harness_advance_time(parse_params(params)?),
            "harness.run_until" => self.harness_run_until(parse_params(params)?),
            _ => Err(AgentCommandError::method_not_found(method)),
        }
    }

    fn workspace_read(&self, params: WorkspaceReadParams) -> Result<JsonValue, AgentCommandError> {
        let relative_path = normalize_workspace_path(params.path.as_str())?;
        let full_path = self.workspace_root.join(&relative_path);
        let text = fs::read_to_string(&full_path).map_err(|error| {
            AgentCommandError::io(
                format!("failed to read '{}': {error}", full_path.display()),
                json!({
                    "path": relative_path.display().to_string(),
                }),
            )
        })?;
        Ok(json!({
            "path": relative_path.display().to_string(),
            "text": text,
        }))
    }

    fn workspace_write(
        &self,
        params: WorkspaceWriteParams,
    ) -> Result<JsonValue, AgentCommandError> {
        let relative_path = normalize_workspace_path(params.path.as_str())?;
        let full_path = self.workspace_root.join(&relative_path);
        if params.create_parents {
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    AgentCommandError::io(
                        format!(
                            "failed to create parent directories for '{}': {error}",
                            full_path.display()
                        ),
                        json!({
                            "path": relative_path.display().to_string(),
                        }),
                    )
                })?;
            }
        }
        fs::write(&full_path, params.text.as_bytes()).map_err(|error| {
            AgentCommandError::io(
                format!("failed to write '{}': {error}", full_path.display()),
                json!({
                    "path": relative_path.display().to_string(),
                }),
            )
        })?;
        Ok(json!({
            "path": relative_path.display().to_string(),
            "bytes_written": params.text.len(),
        }))
    }

    fn workspace_project_info(
        &self,
        params: CommonProjectParams,
    ) -> Result<JsonValue, AgentCommandError> {
        let project_root = self.resolve_project_root(params.project.as_deref())?;
        let sources_root = params
            .sources_root
            .as_deref()
            .map(|path| self.resolve_project_subpath(&project_root, path))
            .transpose()?;
        crate::workflow::project_info_payload(&project_root, sources_root.as_deref())
            .map_err(AgentCommandError::from_anyhow)
    }

    fn runtime_build(&self, params: CommonProjectParams) -> Result<JsonValue, AgentCommandError> {
        let project_root = self.resolve_project_root(params.project.as_deref())?;
        let sources_root = params
            .sources_root
            .as_deref()
            .map(|path| self.resolve_project_subpath(&project_root, path))
            .transpose()?;
        crate::build::build_json_payload(Some(project_root), sources_root)
            .map_err(AgentCommandError::from_anyhow)
    }

    fn lsp_diagnostics(
        &self,
        params: LspDiagnosticsParams,
    ) -> Result<JsonValue, AgentCommandError> {
        let project_root = self.resolve_project_root(params.project.as_deref())?;
        let sources_root = params
            .sources_root
            .as_deref()
            .map(|path| self.resolve_project_subpath(&project_root, path))
            .transpose()?;
        let path = params
            .path
            .as_deref()
            .map(normalize_workspace_path)
            .transpose()?
            .map(|path| path.to_string_lossy().replace('\\', "/"));
        crate::workflow::diagnostics_payload(
            &project_root,
            sources_root.as_deref(),
            path.as_deref(),
            params.content,
        )
        .map_err(AgentCommandError::from_anyhow)
    }

    fn lsp_format(&self, params: LspFormatParams) -> Result<JsonValue, AgentCommandError> {
        let project_root = self.resolve_project_root(params.project.as_deref())?;
        let path = normalize_workspace_path(&params.path)?;
        crate::workflow::format_payload(
            &project_root,
            path.to_string_lossy().replace('\\', "/").as_str(),
            params.content,
        )
        .map_err(AgentCommandError::from_anyhow)
    }

    fn lsp_ast_canonicalize(
        &self,
        params: LspAstCanonicalizeParams,
    ) -> Result<JsonValue, AgentCommandError> {
        let project_root = self.resolve_project_root(params.project.as_deref())?;
        let path = params
            .path
            .as_deref()
            .map(normalize_workspace_path)
            .transpose()?
            .map(|path| path.to_string_lossy().replace('\\', "/"));
        crate::workflow::ast_canonicalize_payload(&project_root, path.as_deref(), params.content)
            .map_err(AgentCommandError::from_anyhow)
    }

    fn lsp_ast_similarity(
        &self,
        params: LspAstSimilarityParams,
    ) -> Result<JsonValue, AgentCommandError> {
        let project_root = self.resolve_project_root(params.project.as_deref())?;
        let left_path = params
            .left_path
            .as_deref()
            .map(normalize_workspace_path)
            .transpose()?
            .map(|path| path.to_string_lossy().replace('\\', "/"));
        let right_path = params
            .right_path
            .as_deref()
            .map(normalize_workspace_path)
            .transpose()?
            .map(|path| path.to_string_lossy().replace('\\', "/"));
        crate::workflow::ast_similarity_payload(
            &project_root,
            left_path.as_deref(),
            params.left_content,
            right_path.as_deref(),
            params.right_content,
        )
        .map_err(AgentCommandError::from_anyhow)
    }

    fn runtime_validate(
        &self,
        params: CommonProjectParams,
    ) -> Result<JsonValue, AgentCommandError> {
        let project_root = self.resolve_project_root(params.project.as_deref())?;
        crate::run::validate_json_payload(project_root).map_err(AgentCommandError::from_anyhow)
    }

    fn runtime_test(&self, params: RuntimeTestParams) -> Result<JsonValue, AgentCommandError> {
        let project_root = self.resolve_project_root(params.project.as_deref())?;
        crate::test::run_test_json(
            Some(project_root),
            params.filter,
            params.list,
            params.timeout_seconds,
        )
        .map_err(AgentCommandError::from_anyhow)
    }

    fn runtime_compile_reload(
        &self,
        params: RuntimeReloadParams,
    ) -> Result<JsonValue, AgentCommandError> {
        let project_root = self.resolve_project_root(params.project.as_deref())?;
        let sources_root = params
            .sources_root
            .as_deref()
            .map(|path| self.resolve_project_subpath(&project_root, path))
            .transpose()?;
        crate::workflow::compile_reload_payload(
            &project_root,
            sources_root.as_deref(),
            params.endpoint,
            params.token,
        )
        .map_err(AgentCommandError::from_anyhow)
    }

    fn runtime_reload(&self, params: RuntimeReloadParams) -> Result<JsonValue, AgentCommandError> {
        let project_root = self.resolve_project_root(params.project.as_deref())?;
        let sources_root = params
            .sources_root
            .as_deref()
            .map(|path| self.resolve_project_subpath(&project_root, path))
            .transpose()?;
        let build = crate::build::build_json_payload(Some(project_root.clone()), sources_root)
            .map_err(AgentCommandError::from_anyhow)?;
        let program_path = project_root.join("program.stbc");
        let bytecode = fs::read(&program_path).map_err(|error| {
            AgentCommandError::io(
                format!("failed to read '{}': {error}", program_path.display()),
                json!({
                    "path": program_path.display().to_string(),
                }),
            )
        })?;
        let control = crate::ctl::call_control_request(
            Some(project_root),
            params.endpoint,
            params.token,
            "bytecode.reload",
            Some(json!({
                "bytes": BASE64_STANDARD.encode(bytecode),
            })),
        )
        .map_err(AgentCommandError::from_anyhow)?;
        Ok(json!({
            "build": build,
            "reload": {
                "endpoint": control.endpoint,
                "result": control.result,
                "response": control.raw_response,
            }
        }))
    }
}

fn parse_params<T>(params: Option<JsonValue>) -> Result<T, AgentCommandError>
where
    T: for<'de> Deserialize<'de>,
{
    let params = params.ok_or_else(|| {
        AgentCommandError::invalid_params("Request params are required for this method.")
    })?;
    serde_json::from_value(params)
        .map_err(|error| AgentCommandError::invalid_params(format!("Invalid params: {error}")))
}

fn parse_optional_params<T>(params: Option<JsonValue>) -> Result<T, AgentCommandError>
where
    T: for<'de> Deserialize<'de> + Default,
{
    match params {
        Some(params) => serde_json::from_value(params)
            .map_err(|error| AgentCommandError::invalid_params(format!("Invalid params: {error}"))),
        None => Ok(T::default()),
    }
}

fn normalize_workspace_path(path: &str) -> Result<PathBuf, AgentCommandError> {
    if path.trim().is_empty() {
        return Err(AgentCommandError::invalid_params("Path must not be empty."));
    }

    let candidate = Path::new(path);
    if candidate.is_absolute() {
        return Err(AgentCommandError::path_outside_workspace(path));
    }

    let mut normalized = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(AgentCommandError::path_outside_workspace(path));
                }
            }
            Component::Prefix(_) | Component::RootDir => {
                return Err(AgentCommandError::path_outside_workspace(path));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(AgentCommandError::invalid_params(
            "Path must resolve to a file inside the workspace root.",
        ));
    }

    Ok(normalized)
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    #[serde(default)]
    id: JsonValue,
    method: String,
    params: Option<JsonValue>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: JsonValue,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    fn success(id: JsonValue, result: JsonValue) -> Self {
        Self {
            jsonrpc: JSON_RPC_VERSION,
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: JsonValue, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: JSON_RPC_VERSION,
            id,
            result: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<JsonValue>,
}

impl JsonRpcError {
    fn new(code: i32, message: String, data: Option<JsonValue>) -> Self {
        Self {
            code,
            message,
            data,
        }
    }
}

#[derive(Debug)]
struct AgentCommandError {
    code: i32,
    message: String,
    data: Option<JsonValue>,
}

impl AgentCommandError {
    fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
            data: None,
        }
    }

    fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("Method '{method}' is not available."),
            data: None,
        }
    }

    fn path_outside_workspace(path: &str) -> Self {
        Self {
            code: ERROR_PATH_OUTSIDE_WORKSPACE,
            message: format!("Path '{path}' resolves outside the workspace root."),
            data: Some(json!({
                "path": path,
                "kind": "path_outside_workspace",
            })),
        }
    }

    fn io(message: String, data: JsonValue) -> Self {
        Self {
            code: ERROR_IO,
            message,
            data: Some(data),
        }
    }

    fn from_anyhow(error: anyhow::Error) -> Self {
        Self {
            code: ERROR_IO,
            message: error.to_string(),
            data: None,
        }
    }
}

impl From<AgentCommandError> for JsonRpcError {
    fn from(value: AgentCommandError) -> Self {
        Self::new(value.code, value.message, value.data)
    }
}

impl From<HarnessAutomationError> for AgentCommandError {
    fn from(value: HarnessAutomationError) -> Self {
        match value {
            HarnessAutomationError::NotLoaded => Self {
                code: ERROR_HARNESS_NOT_LOADED,
                message: "Harness is not loaded. Call harness.load first.".to_string(),
                data: None,
            },
            HarnessAutomationError::InvalidArgument(message) => Self::invalid_params(message),
            HarnessAutomationError::Compile(message) | HarnessAutomationError::Runtime(message) => {
                Self {
                    code: ERROR_IO,
                    message,
                    data: None,
                }
            }
            HarnessAutomationError::RuntimeCycle { message, errors } => Self {
                code: ERROR_IO,
                message,
                data: Some(json!({ "errors": errors })),
            },
            HarnessAutomationError::Boundary(error) => Self {
                code: ERROR_IO,
                message: error.to_string(),
                data: Some(json!({
                    "kind": error.code(),
                    "path": error.path(),
                    "candidates": error.candidates().iter().map(|candidate| candidate.as_str()).collect::<Vec<_>>(),
                })),
            },
            HarnessAutomationError::RunUntilTimeout {
                name,
                max_cycles,
                expected,
            } => Self {
                code: ERROR_RUN_UNTIL_TIMEOUT,
                message: format!(
                    "run_until exceeded {max_cycles} cycles before '{name}' matched the expected value."
                ),
                data: Some(json!({
                    "name": name,
                    "max_cycles": max_cycles,
                    "expected": encode_json_value(&expected),
                })),
            },
        }
    }
}

#[derive(Debug, Deserialize)]
struct WorkspaceReadParams {
    path: String,
}

#[derive(Debug, Deserialize)]
struct WorkspaceWriteParams {
    path: String,
    text: String,
    #[serde(default = "default_true")]
    create_parents: bool,
}

#[derive(Debug, Default, Deserialize)]
struct CommonProjectParams {
    project: Option<String>,
    sources_root: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct LspDiagnosticsParams {
    project: Option<String>,
    sources_root: Option<String>,
    path: Option<String>,
    content: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct LspAstCanonicalizeParams {
    project: Option<String>,
    path: Option<String>,
    content: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct LspAstSimilarityParams {
    project: Option<String>,
    left_path: Option<String>,
    left_content: Option<String>,
    right_path: Option<String>,
    right_content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LspFormatParams {
    project: Option<String>,
    path: String,
    content: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RuntimeTestParams {
    project: Option<String>,
    filter: Option<String>,
    #[serde(default)]
    list: bool,
    #[serde(default = "default_timeout_seconds")]
    timeout_seconds: u64,
}

#[derive(Debug, Default, Deserialize)]
struct RuntimeReloadParams {
    project: Option<String>,
    sources_root: Option<String>,
    endpoint: Option<String>,
    token: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct HarnessLoadParams {
    project: Option<String>,
    files: Option<Vec<String>>,
    inline_sources: Option<Vec<InlineSource>>,
}

#[derive(Debug, Default, Deserialize)]
struct HarnessExecuteParams {
    #[serde(flatten)]
    load: HarnessLoadParams,
    #[serde(default)]
    steps: Vec<HarnessExecuteStep>,
    #[serde(default)]
    assertions: Vec<HarnessAssertion>,
    #[serde(default)]
    watch: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct InlineSource {
    #[allow(dead_code)]
    path: Option<String>,
    text: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum HarnessExecuteStep {
    SetInput {
        name: String,
        value: JsonValue,
    },
    SetAccess {
        name: String,
        value: JsonValue,
    },
    BindDirect {
        name: String,
        address: String,
    },
    SetDirectInput {
        address: String,
        value: JsonValue,
    },
    AdvanceTime {
        duration_ms: i64,
    },
    Cycle {
        #[serde(default = "default_cycle_count")]
        count: u32,
        dt_ms: Option<i64>,
    },
    RunUntil {
        name: String,
        equals: JsonValue,
        max_cycles: Option<u64>,
        dt_ms: Option<i64>,
    },
    Restart {
        mode: Option<String>,
    },
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum HarnessAssertion {
    OutputEquals { name: String, equals: JsonValue },
    AccessEquals { name: String, equals: JsonValue },
    DirectOutputEquals { address: String, equals: JsonValue },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HarnessExecuteResult {
    source_count: usize,
    status: &'static str,
    passed: bool,
    steps_run: usize,
    assertions: HarnessExecuteAssertionSummary,
    watch_snapshot: JsonValue,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    failures: Vec<HarnessExecuteFailure>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HarnessExecuteAssertionSummary {
    total: usize,
    evaluated: usize,
    passed: usize,
    failed: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HarnessExecuteFailure {
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    step_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    step: Option<HarnessExecuteStep>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assertion: Option<HarnessAssertion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    actual: Option<JsonValue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    errors: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct HarnessCycleParams {
    #[serde(default = "default_cycle_count")]
    count: u32,
    dt_ms: Option<i64>,
    #[serde(default)]
    watch: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct HarnessSetInputParams {
    name: String,
    value: JsonValue,
}

#[derive(Debug, Deserialize)]
struct HarnessGetOutputParams {
    name: String,
}

#[derive(Debug, Deserialize)]
struct HarnessAdvanceTimeParams {
    duration_ms: i64,
}

#[derive(Debug, Deserialize)]
struct HarnessRunUntilParams {
    name: String,
    equals: JsonValue,
    max_cycles: Option<u64>,
    dt_ms: Option<i64>,
    #[serde(default)]
    watch: Vec<String>,
}

fn default_true() -> bool {
    true
}

fn default_timeout_seconds() -> u64 {
    5
}

fn default_cycle_count() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    use super::normalize_workspace_path;
    use serde_json::json;
    use std::path::PathBuf;
    use trust_runtime::harness::{decode_json_value, encode_json_value};
    use trust_runtime::value::Value;

    #[test]
    fn normalize_workspace_path_rejects_parent_escape() {
        let error = normalize_workspace_path("../../secret.st").expect_err("escape should fail");
        assert_eq!(error.code, super::ERROR_PATH_OUTSIDE_WORKSPACE);
    }

    #[test]
    fn normalize_workspace_path_collapses_current_dir_segments() {
        let path = normalize_workspace_path("./src/../src/main.st").expect("normalize path");
        assert_eq!(path, PathBuf::from("src/main.st"));
    }

    #[test]
    fn decode_json_value_supports_typed_scalars() {
        assert_eq!(
            decode_json_value(&json!({"type": "BOOL", "value": true})).expect("bool"),
            Value::Bool(true)
        );
        assert_eq!(
            decode_json_value(&json!({"type": "INT", "value": 4})).expect("int"),
            Value::Int(4)
        );
        assert_eq!(
            decode_json_value(&json!({"type": "STRING", "value": "hello"})).expect("string"),
            Value::String("hello".into())
        );
    }

    #[test]
    fn encode_value_emits_typed_payload() {
        assert_eq!(
            encode_json_value(&Value::Bool(true)),
            json!({"type": "BOOL", "value": true})
        );
    }
}
