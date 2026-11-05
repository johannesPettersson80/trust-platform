//! The Agent API v1 as MCP tools, served through the shared MCP connection layer
//! (`trust_lsp::mcp`, specification 20 §8). This module is a tool provider only: every
//! call is forwarded, with its raw JSON arguments, to [`AgentServer::execute`], the same
//! dispatch entry point that serves `trust-dev agent serve`. It adds no method, no
//! validation rule, and no error mapping of its own beyond the projection of contract
//! results onto MCP tool results. The normative binding rules live in
//! `docs/guides/AGENT_MCP_BINDING_V1.md`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use schemars::{schema_for, JsonSchema};
use serde_json::{json, Map, Value};

use trust_lsp::mcp::{Limits, ServerInfo, ToolProvider, Unavailable};

use super::{
    resolve_workspace_root, AgentCommandError, AgentServer, CommonProjectParams,
    HarnessAdvanceTimeParams, HarnessCycleParams, HarnessExecuteParams, HarnessGetOutputParams,
    HarnessLoadParams, HarnessRunUntilParams, HarnessSetInputParams, LspAstCanonicalizeParams,
    LspAstSimilarityParams, LspDiagnosticsParams, LspFormatParams, RuntimeCompileReloadParams,
    RuntimeIssueCapParams, RuntimeReloadParams, RuntimeTestParams, WorkspaceReadParams,
    WorkspaceWriteParams,
};

/// One row of the static tool table: the canonical Agent API method and the tool
/// description. The MCP tool name is the method with dots replaced by underscores
/// (`docs/guides/AGENT_MCP_BINDING_V1.md`, tool mapping).
struct AgentToolBinding {
    method: &'static str,
    description: &'static str,
}

/// The 1:1 projection of the Agent API methods onto MCP tools. This table and
/// `agent.describe`'s `methods` list MUST stay in lockstep; the parity test in
/// `mcp_provider_tests.rs` fails when they drift.
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
            "Return machine-readable diagnostics for one ST file or a whole project source tree. Supports opt-in paging: pass page and page_size together to bound each response (see totalIssues/totalPages); omit both for the full list.",
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
        description: "Reuse the `trust-runtime build --ci` payload path. Optional issues_limit caps the returned issues list (see issuesTruncated/totalIssues) and bounds a failed build's raw error text on large projects.",
    },
    AgentToolBinding {
        method: "runtime.compile_reload",
        description:
            "Run the full diagnose -> build -> reload loop and return a single structured result. Optional issues_limit caps the returned issues list (see issuesTruncated/totalIssues) and bounds a reported build failure's raw error text on large projects.",
    },
    AgentToolBinding {
        method: "runtime.validate",
        description: "Reuse the `trust-runtime validate --ci` payload path. Optional issues_limit bounds a failed validation's raw error text on large projects.",
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

/// MCP tool names MUST match `^[a-zA-Z0-9_-]{1,64}$`; the Agent API's `.` is not a legal
/// character, so it becomes `_`.
fn tool_name_for_method(method: &str) -> String {
    method.replace('.', "_")
}

fn binding_for_tool_name(tool_name: &str) -> Option<&'static AgentToolBinding> {
    TOOL_BINDINGS
        .iter()
        .find(|binding| tool_name_for_method(binding.method) == tool_name)
}

/// The advisory JSON Schema for one method's params, generated from the Agent API
/// parameter structs. JSON Schema cannot express every contract rule (see the guide's
/// "Input Schemas Are Advisory"); enforcement stays in `AgentServer`.
fn input_schema(method: &str) -> Value {
    fn generated_schema<T: JsonSchema>() -> Value {
        serde_json::to_value(schema_for!(T)).expect("generated schema serializes")
    }
    match method {
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
        "runtime.build" => generated_schema::<RuntimeIssueCapParams>(),
        "runtime.compile_reload" => generated_schema::<RuntimeCompileReloadParams>(),
        "runtime.validate" => generated_schema::<RuntimeIssueCapParams>(),
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
    }
}

/// The tools as `tools/list` answers them, built once on first use.
fn tool_definitions() -> &'static Vec<Value> {
    static TOOLS: OnceLock<Vec<Value>> = OnceLock::new();
    TOOLS.get_or_init(|| {
        TOOL_BINDINGS
            .iter()
            .map(|binding| {
                json!({
                    "name": tool_name_for_method(binding.method),
                    "description": binding.description,
                    "inputSchema": input_schema(binding.method),
                })
            })
            .collect()
    })
}

/// What the server says of itself (specification 20 §8).
const INFO: ServerInfo = ServerInfo {
    name: "trust-dev-agent",
    version: "1.0.0",
    instructions: "Serve one truST workspace over the Agent API v1 contract: read and write sources, diagnose with lsp.diagnostics (page large projects), run the runtime loop with runtime.compile_reload (set issues_limit on large projects), and exercise deterministic behavior through the harness tools. Sequential serving: one instance holds one harness session.",
    unavailable: "The agent server could not answer the call.",
};

/// The connection limits of the agent server (specification 20 §8). Calls are served
/// sequentially and builds can take minutes, so the call time is generous; the frame
/// bounds stay honest because lsp.diagnostics pages and the runtime transaction methods
/// take issues_limit.
const LIMITS: Limits = Limits {
    line_bytes: 16 * 1024 * 1024,
    active_calls: 1,
    call_time: std::time::Duration::from_secs(15 * 60),
    answer_time: std::time::Duration::from_secs(15 * 60),
};

/// The MCP front of one Agent API server instance. Calls are served sequentially:
/// every tool call locks the shared [`AgentServer`] for the duration of its dispatch,
/// so one instance exposes exactly one stateful harness session and later calls
/// observe earlier calls' effects (specification 20 §8).
struct AgentToolProvider {
    agent: Arc<Mutex<AgentServer>>,
}

impl ToolProvider for AgentToolProvider {
    fn info(&self) -> ServerInfo {
        INFO
    }

    fn tools(&self) -> &[Value] {
        tool_definitions()
    }

    /// A known tool name and an object of arguments, or the reason the server answers
    /// -32602. The arguments pass through untouched; their validation is the contract's,
    /// inside `AgentServer`.
    fn check(
        &self,
        params: &Map<String, Value>,
    ) -> Result<(String, Map<String, Value>), &'static str> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| binding_for_tool_name(name).is_some())
            .ok_or("Unknown tool; see tools/list for the Agent API v1 tools.")?;
        let Some(Value::Object(arguments)) = params.get("arguments") else {
            return Err("Tool arguments must be an object.");
        };
        Ok((name.to_owned(), arguments.clone()))
    }

    /// Dispatch one call through the shared [`AgentServer`], sequentially: the lock
    /// spans the whole dispatch, so builds and tests run one at a time and later calls
    /// observe earlier calls' effects. A contract error becomes an error tool result
    /// whose text carries the contract-stable `{code, message, data}` object, so the
    /// connection stays open (specification 20 §8); only a poisoned server mutex — a
    /// call that panicked — is the connection layer's failure answer.
    fn call(
        &self,
        _id: &str,
        name: &str,
        arguments: Map<String, Value>,
    ) -> Result<Value, Unavailable> {
        let method = binding_for_tool_name(name)
            .map(|binding| binding.method)
            .expect("check() admits only known tool names");
        let params = Value::Object(arguments);
        let dispatch = match self.agent.lock() {
            Ok(mut server) => server.execute(method, Some(params)),
            Err(_) => return Err(Unavailable),
        };
        match dispatch {
            Ok(result) => Ok(Self::result(result)),
            Err(error) => Ok(Self::error_result(error)),
        }
    }

    /// Sequential v1 serving cannot interrupt a running dispatch; the call answers
    /// through `call` when it ends. The layer cancels a call at most once, so the
    /// honest v1 answer is a no-op (specification 20 §8).
    fn cancel(&self, _id: &str) {}
}

impl AgentToolProvider {
    /// Project an Agent API result onto the MCP tool result the connection layer sends
    /// (`docs/guides/AGENT_MCP_BINDING_V1.md`, result and error mapping): the payload
    /// as `structuredContent`, and its pretty form as the text block a client shows.
    fn result(result: Value) -> Value {
        let pretty = serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string());
        json!({
            "content": [{ "type": "text", "text": pretty }],
            "structuredContent": result,
        })
    }

    /// Project an Agent API error onto an error tool result whose text is the
    /// contract-stable JSON `{code, message, data}` object. The contract's error codes
    /// survive in the payload (specification 20 §8).
    fn error_result(error: AgentCommandError) -> Value {
        let mut payload = json!({
            "code": error.code,
            "message": error.message,
        });
        if let Some(data) = error.data {
            payload["data"] = data;
        }
        let text = serde_json::to_string(&payload).expect("error payload serializes");
        json!({
            "content": [{ "type": "text", "text": text }],
            "isError": true,
        })
    }
}

/// Serve the Agent API as an MCP server on standard input and standard output
/// (`trust-dev agent serve-mcp`, specification 20 §8).
pub fn run_agent_serve_mcp(project: Option<PathBuf>) -> anyhow::Result<()> {
    let workspace_root = resolve_workspace_root(project)?;
    let provider = AgentToolProvider {
        agent: Arc::new(Mutex::new(AgentServer::new(workspace_root))),
    };
    let server =
        trust_lsp::mcp::Server::start(Arc::new(provider), LIMITS, Box::new(std::io::stdout()))
            .map_err(|error| anyhow::anyhow!("start agent MCP server: {error:?}"))?;
    server.serve(&mut std::io::stdin().lock());
    Ok(())
}

#[cfg(test)]
#[path = "mcp_provider_tests.rs"]
mod mcp_provider_tests;
