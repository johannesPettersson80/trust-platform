//! MCP server binding for the Agent API v1 (specification 20 §8).
//!
//! This module is a transport adapter only: every tool call is forwarded, with
//! its raw JSON arguments, to [`AgentServer::execute`], the same dispatch
//! entry point that serves `trust-dev agent serve`. It adds no method, no
//! validation rule, and no error mapping of its own beyond the projection of
//! contract results onto MCP tool results. The normative binding rules live in
//! `docs/guides/AGENT_MCP_BINDING_V1.md`.

use std::borrow::Cow;
use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Context;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ErrorCode, ErrorData,
    Implementation, JsonObject, ListToolsResult, PaginatedRequestParams, ProtocolVersion,
    ServerCapabilities, ServerConfig, Tool,
};
use rmcp::service::{serve_server, RequestContext, RoleServer};
use rmcp::ServerHandler;
use schemars::{schema_for, JsonSchema};
use serde_json::{json, Value};

use super::{
    resolve_workspace_root, AgentCommandError, AgentServer, CommonProjectParams,
    HarnessAdvanceTimeParams, HarnessCycleParams, HarnessExecuteParams, HarnessGetOutputParams,
    HarnessLoadParams, HarnessRunUntilParams, HarnessSetInputParams, LspAstCanonicalizeParams,
    LspAstSimilarityParams, LspDiagnosticsParams, LspFormatParams, RuntimeReloadParams,
    RuntimeTestParams, WorkspaceReadParams, WorkspaceWriteParams,
};

/// The MCP protocol revision this server pins, per
/// `docs/guides/AGENT_MCP_BINDING_V1.md`.
const MCP_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion::V_2025_06_18;

const ERROR_METHOD_NOT_FOUND: i32 = -32601;

/// One row of the static tool table: the canonical Agent API method and the
/// tool description. The MCP tool name is the method with dots replaced by
/// underscores (`docs/guides/AGENT_MCP_BINDING_V1.md`, tool mapping).
struct AgentToolBinding {
    method: &'static str,
    description: &'static str,
}

/// The 1:1 projection of the Agent API methods onto MCP tools. This table and
/// `agent.describe`'s `methods` list MUST stay in lockstep; the parity test in
/// `mcp_contract_tests.rs` fails when they drift.
const TOOL_BINDINGS: &[AgentToolBinding] = &[
    AgentToolBinding {
        method: "agent.describe",
        description:
            "Describe the Agent API v1 contract: workspace root, transport, framing, and supported methods.",
    },
    AgentToolBinding {
        method: "workspace.read",
        description: "Read a file inside the workspace root.",
    },
    AgentToolBinding {
        method: "workspace.write",
        description: "Write a file inside the workspace root.",
    },
    AgentToolBinding {
        method: "workspace.project_info",
        description:
            "Return project-root, source-root, dependency, config, and runtime-orientation metadata.",
    },
    AgentToolBinding {
        method: "lsp.diagnostics",
        description:
            "Return machine-readable diagnostics for one ST file or a whole project source tree.",
    },
    AgentToolBinding {
        method: "lsp.ast_canonicalize",
        description:
            "Return the canonical normalized AST token stream and 5-gram fingerprints for one source.",
    },
    AgentToolBinding {
        method: "lsp.ast_similarity",
        description:
            "Compare two canonical AST token streams and return their similarity evidence.",
    },
    AgentToolBinding {
        method: "lsp.format",
        description: "Return a formatting preview for one ST file without mutating disk.",
    },
    AgentToolBinding {
        method: "runtime.build",
        description: "Reuse the `trust-runtime build --ci` payload path.",
    },
    AgentToolBinding {
        method: "runtime.compile_reload",
        description:
            "Run the full diagnose -> build -> reload loop and return a single structured result.",
    },
    AgentToolBinding {
        method: "runtime.validate",
        description: "Reuse the `trust-runtime validate --ci` payload path.",
    },
    AgentToolBinding {
        method: "runtime.test",
        description: "Reuse the `trust-dev test --output json` payload path.",
    },
    AgentToolBinding {
        method: "runtime.reload",
        description:
            "Rebuild the program image and send it with bytecode.reload to a running runtime control endpoint.",
    },
    AgentToolBinding {
        method: "harness.load",
        description: "Load inline/project sources into a deterministic in-process harness.",
    },
    AgentToolBinding {
        method: "harness.reload",
        description: "Reload harness sources while preserving retain semantics when supported.",
    },
    AgentToolBinding {
        method: "harness.execute",
        description:
            "Run one isolated deterministic fixture transaction and return structured step/assertion evidence.",
    },
    AgentToolBinding {
        method: "harness.cycle",
        description: "Advance cycles, optionally advancing virtual time first.",
    },
    AgentToolBinding {
        method: "harness.set_input",
        description: "Set a harness input/global/program variable.",
    },
    AgentToolBinding {
        method: "harness.get_output",
        description: "Read a harness output/global/program variable.",
    },
    AgentToolBinding {
        method: "harness.advance_time",
        description: "Advance virtual time without executing a cycle.",
    },
    AgentToolBinding {
        method: "harness.run_until",
        description: "Cycle until a named output matches an expected value.",
    },
];

/// MCP tool names MUST match `^[a-zA-Z0-9_-]{1,64}$`; the Agent API's `.` is
/// not a legal character, so it becomes `_`.
fn tool_name_for_method(method: &str) -> String {
    method.replace('.', "_")
}

fn binding_for_tool_name(tool_name: &str) -> Option<&'static AgentToolBinding> {
    TOOL_BINDINGS
        .iter()
        .find(|binding| tool_name_for_method(binding.method) == tool_name)
}

/// The advisory JSON Schema for one method's params, generated from the
/// Agent API parameter structs. JSON Schema cannot express every contract
/// rule (see the guide's "Input Schemas Are Advisory"); enforcement stays in
/// `AgentServer`.
fn input_schema(method: &str) -> JsonObject {
    fn generated_schema<T: JsonSchema>() -> Value {
        serde_json::to_value(schema_for!(T)).expect("generated schema serializes")
    }
    let value: Value = match method {
        "agent.describe" => json!({
            "type": "object",
            "properties": {},
        }),
        "workspace.read" => generated_schema::<WorkspaceReadParams>(),
        "workspace.write" => generated_schema::<WorkspaceWriteParams>(),
        "workspace.project_info" => generated_schema::<CommonProjectParams>(),
        "lsp.diagnostics" => generated_schema::<LspDiagnosticsParams>(),
        "lsp.ast_canonicalize" => generated_schema::<LspAstCanonicalizeParams>(),
        "lsp.ast_similarity" => generated_schema::<LspAstSimilarityParams>(),
        "lsp.format" => generated_schema::<LspFormatParams>(),
        "runtime.build" => generated_schema::<CommonProjectParams>(),
        "runtime.compile_reload" => generated_schema::<RuntimeReloadParams>(),
        "runtime.validate" => generated_schema::<CommonProjectParams>(),
        "runtime.test" => generated_schema::<RuntimeTestParams>(),
        "runtime.reload" => generated_schema::<RuntimeReloadParams>(),
        "harness.load" => generated_schema::<HarnessLoadParams>(),
        "harness.reload" => generated_schema::<HarnessLoadParams>(),
        "harness.execute" => generated_schema::<HarnessExecuteParams>(),
        "harness.cycle" => generated_schema::<HarnessCycleParams>(),
        "harness.set_input" => generated_schema::<HarnessSetInputParams>(),
        "harness.get_output" => generated_schema::<HarnessGetOutputParams>(),
        "harness.advance_time" => generated_schema::<HarnessAdvanceTimeParams>(),
        "harness.run_until" => generated_schema::<HarnessRunUntilParams>(),
        _ => unreachable!("tool table methods are exhaustive"),
    };
    match value {
        Value::Object(map) => map,
        _ => unreachable!("struct schemas serialize to JSON objects"),
    }
}

fn tool_definitions() -> Vec<Tool> {
    TOOL_BINDINGS
        .iter()
        .map(|binding| {
            Tool::new(
                tool_name_for_method(binding.method),
                binding.description,
                input_schema(binding.method),
            )
        })
        .collect()
}

/// The MCP front of one Agent API server instance. Requests are served
/// sequentially: every tool call locks the shared [`AgentServer`] for the
/// duration of its dispatch, so one instance exposes exactly one stateful
/// harness session and later calls observe earlier calls' effects.
struct AgentMcpServer {
    agent: Arc<Mutex<AgentServer>>,
}

impl AgentMcpServer {
    fn new(agent: Arc<Mutex<AgentServer>>) -> Self {
        Self { agent }
    }

    /// Dispatch one tool call through the shared [`AgentServer`] on the
    /// blocking thread pool. Builds and tests take seconds; the async worker
    /// thread must not park on them.
    async fn dispatch(
        &self,
        method: &'static str,
        params: Option<Value>,
    ) -> Result<Value, AgentCommandError> {
        let agent = Arc::clone(&self.agent);
        tokio::task::spawn_blocking(move || {
            let mut server = agent.lock().expect("agent server mutex poisoned");
            server.execute(method, params)
        })
        .await
        .expect("agent dispatch task panicked")
    }

    /// Project an Agent API result onto an MCP tool result
    /// (`docs/guides/AGENT_MCP_BINDING_V1.md`, result and error mapping).
    fn project_result(result: Value) -> CallToolResponse {
        let pretty = serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string());
        let mut call = CallToolResult::success(vec![ContentBlock::text(pretty)]);
        call.structured_content = Some(result);
        CallToolResponse::from(call)
    }

    /// Project an Agent API error onto an error tool result whose text is the
    /// contract-stable JSON `{code, message, data}` object.
    fn project_error(error: AgentCommandError) -> CallToolResponse {
        let mut payload = json!({
            "code": error.code,
            "message": error.message,
        });
        if let Some(data) = error.data {
            payload["data"] = data;
        }
        let text = serde_json::to_string(&payload).expect("error payload serializes");
        CallToolResponse::from(CallToolResult::error(vec![ContentBlock::text(text)]))
    }
}

impl ServerHandler for AgentMcpServer {
    fn get_info(&self) -> ServerConfig {
        let mut info = ServerConfig::new(ServerCapabilities::builder().enable_tools().build());
        info.protocol_version = MCP_PROTOCOL_VERSION;
        info.server_info = Implementation::new("trust-dev", env!("CARGO_PKG_VERSION"));
        info
    }

    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(&[MCP_PROTOCOL_VERSION])
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + Send + '_ {
        std::future::ready(Ok(ListToolsResult::with_all_items(tool_definitions())))
    }

    fn call_tool<'a>(
        &'a self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResponse, ErrorData>> + Send + 'a {
        Box::pin(async move {
            let Some(binding) = binding_for_tool_name(&request.name) else {
                return Err(ErrorData::new(
                    ErrorCode::METHOD_NOT_FOUND,
                    format!(
                        "Tool '{}' does not map to an Agent API method; see tools/list.",
                        request.name
                    ),
                    None,
                ));
            };
            let params = request.arguments.map(Value::Object);
            match self.dispatch(binding.method, params).await {
                Ok(result) => Ok(Self::project_result(result)),
                Err(error) if error.code == ERROR_METHOD_NOT_FOUND => Err(ErrorData::new(
                    ErrorCode::METHOD_NOT_FOUND,
                    error.message,
                    error.data,
                )),
                Err(error) => Ok(Self::project_error(error)),
            }
        })
    }
}

/// Serve the Agent API as an MCP server on stdio
/// (`trust-dev agent serve-mcp`).
pub fn run_agent_serve_mcp(project: Option<PathBuf>) -> anyhow::Result<()> {
    let workspace_root = resolve_workspace_root(project)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build tokio runtime for agent MCP serve")?;
    runtime.block_on(async {
        let server = AgentMcpServer::new(Arc::new(Mutex::new(AgentServer::new(workspace_root))));
        let running = serve_server(server, rmcp::transport::stdio())
            .await
            .context("serve agent MCP server on stdio")?;
        running
            .waiting()
            .await
            .context("agent MCP server stopped with error")?;
        anyhow::Ok(())
    })
}

#[cfg(test)]
#[path = "mcp_contract_tests.rs"]
mod mcp_contract_tests;
