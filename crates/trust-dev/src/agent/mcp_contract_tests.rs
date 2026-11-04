//! Contract tests for the MCP binding (`docs/guides/AGENT_MCP_BINDING_V1.md`,
//! specification 20 §8). They prove the binding is a faithful transport
//! adapter: the tool table never drifts from `agent.describe`, arguments are
//! forwarded verbatim, results and contract error codes survive the mapping,
//! and the shared session stays stateful across tool calls.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use rmcp::model::{
    CallToolRequestParams, CallToolResult, JsonObject, ListToolsResult, ProtocolVersion,
};
use rmcp::service::{serve_client, serve_server, RoleClient, RunningService, ServiceError};
use serde_json::{json, Value};

use super::{
    binding_for_tool_name, input_schema, tool_definitions, tool_name_for_method, AgentMcpServer,
    TOOL_BINDINGS,
};
use crate::agent::AgentServer;

fn temp_workspace(tag: &str) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "trust-dev-mcp-{}-{}-{}",
        tag,
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir_all(&dir).expect("create temp workspace");
    dir
}

fn test_server(workspace_root: PathBuf) -> AgentMcpServer {
    // Resolve the workspace root exactly like the CLI entry points do, so the
    // canonical containment check sees the same root shape it sees in
    // production (on Windows, canonicalize adds the `\\?\` verbatim prefix).
    let resolved =
        super::resolve_workspace_root(Some(workspace_root)).expect("temp workspace root resolves");
    AgentMcpServer::new(Arc::new(Mutex::new(AgentServer::new(resolved))))
}

fn arguments(arguments: Value) -> JsonObject {
    serde_json::from_value::<JsonObject>(arguments).expect("test arguments are a JSON object")
}

/// The drift guard: the MCP tool table is a 1:1 projection of
/// `agent.describe.methods`, in the same order, mangled dot-to-underscore,
/// and every tool carries an object input schema and a non-empty description.
#[test]
fn mcp_tool_table_matches_agent_describe() {
    let mut server = AgentServer::new(PathBuf::from("."));
    let describe = server
        .execute("agent.describe", None)
        .expect("agent.describe dispatches");
    let methods: Vec<&str> = describe["methods"]
        .as_array()
        .expect("describe carries a methods array")
        .iter()
        .map(|method| method.as_str().expect("each method is a string"))
        .collect();

    let tools = tool_definitions();
    assert_eq!(
        tools.len(),
        methods.len(),
        "tool count must equal the agent.describe method count"
    );
    for (tool, method) in tools.iter().zip(methods.iter()) {
        assert_eq!(
            tool.name.as_ref(),
            tool_name_for_method(method),
            "tool name must be the mangled method name, in describe order"
        );
        let description = tool.description.as_deref().expect("tool has a description");
        assert!(!description.is_empty(), "tool description is non-empty");
        let schema = Value::Object((*tool.input_schema).clone());
        assert_eq!(
            schema["type"], "object",
            "input schema for {method} must be an object schema"
        );
    }

    // The mangled names satisfy the MCP tool-name pattern
    // ^[a-zA-Z0-9_-]{1,64}$ (docs/guides/AGENT_MCP_BINDING_V1.md).
    for name in tools.iter().map(|tool| tool.name.as_ref()) {
        assert!(
            !name.is_empty() && name.len() <= 64,
            "tool name '{name}' fits the MCP length bound"
        );
        assert!(
            name.chars()
                .all(|character| character.is_ascii_alphanumeric()
                    || character == '_'
                    || character == '-'),
            "tool name '{name}' uses only MCP-legal characters"
        );
    }
}

/// Dispatch resolves tools through the stored mapping, never by reverse
/// mangling; unknown tools stay unknown.
#[test]
fn tool_binding_lookup_uses_stored_mapping() {
    let binding = binding_for_tool_name("workspace_project_info").expect("mangled name resolves");
    assert_eq!(binding.method, "workspace.project_info");
    assert!(
        binding_for_tool_name("workspace.project_info").is_none(),
        "the canonical method name is not itself a tool name"
    );
    assert!(binding_for_tool_name("workspace.project").is_none());
    assert!(binding_for_tool_name("does_not_exist").is_none());
}

/// Schemas are advisory: they exist for every method and stay object-shaped,
/// including for `harness.load` whose fail-closed selector rule JSON Schema
/// cannot express (docs/guides/AGENT_MCP_BINDING_V1.md).
#[test]
fn every_method_has_an_object_schema() {
    for binding in TOOL_BINDINGS {
        let schema = Value::Object(input_schema(binding.method));
        assert_eq!(
            schema["type"], "object",
            "{} schema must be an object schema",
            binding.method
        );
    }
    let load = Value::Object(input_schema("harness.load"));
    let properties = load["properties"].as_object().expect("schema properties");
    for selector in ["inline_sources", "files", "project"] {
        assert!(
            properties.contains_key(selector),
            "harness.load schema still names the {selector} selector"
        );
    }
}

/// An in-process MCP session over rmcp's in-memory transport. The server task
/// holds its [`RunningService`] for the whole test: dropping it would close
/// the transport the moment the handshake returns. The abort on drop tears
/// both sides down.
struct McpMemorySession {
    client: RunningService<RoleClient, ()>,
    server_task: tokio::task::JoinHandle<()>,
}

impl McpMemorySession {
    async fn start(workspace_root: PathBuf) -> Self {
        let (server_transport, client_transport) = tokio::io::duplex(64 * 1024);
        let server_task = tokio::spawn(async move {
            let running = serve_server(test_server(workspace_root), server_transport)
                .await
                .expect("serve MCP server over memory transport");
            let _ = running.waiting().await;
        });
        let client = serve_client((), client_transport)
            .await
            .expect("initialize MCP client");
        Self {
            client,
            server_task,
        }
    }
}

impl Drop for McpMemorySession {
    fn drop(&mut self) {
        self.server_task.abort();
    }
}

/// Handshake: initialize negotiates the pinned revision and returns the
/// tools-only capability set and the trust-dev server info.
#[tokio::test]
async fn initialize_reports_pinned_revision_and_tools_capability() {
    let workspace = temp_workspace("handshake");
    let session = McpMemorySession::start(workspace.clone()).await;
    let info = session.client.peer_info().expect("initialize result");
    assert_eq!(info.protocol_version, ProtocolVersion::V_2025_06_18);
    assert!(
        info.capabilities.tools.is_some(),
        "server advertises the tools capability"
    );
    assert!(
        info.capabilities.resources.is_none() && info.capabilities.prompts.is_none(),
        "server advertises tools only"
    );
    let server_info = info.server_info.as_ref().expect("server info present");
    assert_eq!(server_info.name, "trust-dev");
    assert_eq!(server_info.version, env!("CARGO_PKG_VERSION"));

    drop(session);
    let _ = std::fs::remove_dir_all(workspace);
}

/// tools/list over the wire mirrors the in-process tool table.
#[tokio::test]
async fn list_tools_over_memory_transport_matches_table() {
    let workspace = temp_workspace("list");
    let session = McpMemorySession::start(workspace.clone()).await;
    let listed: ListToolsResult = session
        .client
        .peer()
        .list_tools(None)
        .await
        .expect("tools/list");
    let table = tool_definitions();
    assert_eq!(listed.tools.len(), table.len());
    for (listed_tool, table_tool) in listed.tools.iter().zip(table.iter()) {
        assert_eq!(listed_tool.name, table_tool.name);
    }
    drop(session);
    let _ = std::fs::remove_dir_all(workspace);
}

/// Success path: `workspace_write` then `workspace_read` round-trip through
/// raw forwarded arguments; `structuredContent` is the Agent API JSON result.
#[tokio::test]
async fn workspace_round_trip_returns_structured_content() {
    let workspace = temp_workspace("round-trip");
    let session = McpMemorySession::start(workspace.clone()).await;
    let client = &session.client;

    let written: CallToolResult = client
        .peer()
        .call_tool(
            CallToolRequestParams::new("workspace_write").with_arguments(arguments(json!({
                "path": "notes/hello.txt",
                "text": "hello",
                "create_parents": true,
            }))),
        )
        .await
        .expect("workspace_write succeeds");
    assert_eq!(
        written.is_error,
        Some(false),
        "workspace_write failed: {:?}",
        written.content
    );
    assert_eq!(
        written.structured_content,
        Some(json!({"path": "notes/hello.txt", "bytes_written": 5}))
    );
    let text = written.content.first().expect("text content block");
    let rendered = text.as_text().expect("text block").text.clone();
    assert!(
        rendered.contains("bytes_written"),
        "text block renders the result"
    );

    let read: CallToolResult = client
        .peer()
        .call_tool(
            CallToolRequestParams::new("workspace_read").with_arguments(arguments(json!({
                "path": "notes/hello.txt",
            }))),
        )
        .await
        .expect("workspace_read succeeds");
    assert_eq!(
        read.structured_content,
        Some(json!({"path": "notes/hello.txt", "text": "hello"}))
    );
    let _ = std::fs::remove_dir_all(workspace);
}

/// Error paths stay inside error tool results: the text block carries the
/// contract-stable `{code, message, data}` JSON. An unknown tool is a JSON-RPC
/// protocol error, never a tool result.
#[tokio::test]
async fn contract_errors_map_to_error_tool_results() {
    let workspace = temp_workspace("errors");
    let session = McpMemorySession::start(workspace.clone()).await;
    let client = &session.client;

    let escaped: CallToolResult = client
        .peer()
        .call_tool(
            CallToolRequestParams::new("workspace_write").with_arguments(arguments(json!({
                "path": "../../escape.st",
                "text": "nope",
            }))),
        )
        .await
        .expect("path escape is an error result, not a protocol failure");
    assert_eq!(escaped.is_error, Some(true));
    assert!(escaped.structured_content.is_none());
    let payload: Value = serde_json::from_str(
        escaped
            .content
            .first()
            .expect("error text")
            .as_text()
            .expect("text block")
            .text
            .as_str(),
    )
    .expect("error text is the contract JSON object");
    assert_eq!(payload["code"], -32001);
    assert_eq!(payload["data"]["kind"], "path_outside_workspace");

    let invalid: CallToolResult = client
        .peer()
        .call_tool(
            CallToolRequestParams::new("workspace_read").with_arguments(arguments(json!({}))),
        )
        .await
        .expect("invalid params is an error result, not a protocol failure");
    assert_eq!(invalid.is_error, Some(true));
    let payload: Value = serde_json::from_str(
        invalid
            .content
            .first()
            .expect("error text")
            .as_text()
            .expect("text block")
            .text
            .as_str(),
    )
    .expect("error text is the contract JSON object");
    assert_eq!(payload["code"], -32602);

    let unknown = client
        .peer()
        .call_tool(CallToolRequestParams::new("nonexistent_method"))
        .await
        .expect_err("unknown tool is a JSON-RPC protocol error");
    let ServiceError::McpError(error) = unknown else {
        panic!("unknown tool must fail with an MCP protocol error, got {unknown:?}");
    };
    assert_eq!(error.code.0, -32601);
    let _ = std::fs::remove_dir_all(workspace);
}

/// The harness session is shared per server instance: `harness_load` then
/// `harness_cycle` advance one and the same session.
#[tokio::test]
async fn harness_session_is_shared_across_tool_calls() {
    let workspace = temp_workspace("harness");
    let session = McpMemorySession::start(workspace.clone()).await;
    let client = &session.client;

    let loaded: CallToolResult = client
        .peer()
        .call_tool(
            CallToolRequestParams::new("harness_load").with_arguments(arguments(json!({
                "inline_sources": [{"text": "PROGRAM Main\nEND_PROGRAM\n"}],
            }))),
        )
        .await
        .expect("harness_load succeeds");
    assert_eq!(
        loaded.is_error,
        Some(false),
        "harness_load failed: {:?}",
        loaded.content
    );
    let load_payload = loaded.structured_content.expect("load result");
    let cycles_after_load = load_payload["cycle_count"].as_u64().expect("cycle count");

    let cycled: CallToolResult = client
        .peer()
        .call_tool(
            CallToolRequestParams::new("harness_cycle").with_arguments(arguments(json!({
                "count": 2,
            }))),
        )
        .await
        .expect("harness_cycle succeeds");
    let cycle_payload = cycled.structured_content.expect("cycle result");
    let cycles_after_step = cycle_payload["cycle_count"].as_u64().expect("cycle count");
    assert_eq!(
        cycles_after_step,
        cycles_after_load + 2,
        "harness_cycle continues the session harness_load created"
    );
    let _ = std::fs::remove_dir_all(workspace);
}
