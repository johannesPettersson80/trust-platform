//! Contract tests for the MCP binding (`docs/guides/AGENT_MCP_BINDING_V1.md`,
//! specification 20 §8), at the tool-provider level. The connection layer's own
//! transport tests live in `trust-lsp::mcp`; these tests prove the agent provider
//! is a faithful adapter: the tool table never drifts from `agent.describe`,
//! arguments are forwarded verbatim, results and contract error codes survive the
//! mapping, and the shared session stays stateful across tool calls.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use trust_lsp::mcp::ToolProvider;

use super::{
    binding_for_tool_name, input_schema, tool_definitions, tool_name_for_method, AgentToolProvider,
    TOOL_BINDINGS,
};
use crate::agent::{resolve_workspace_root, AgentServer};

fn temp_workspace(tag: &str) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "trust-dev-mcp-provider-{}-{}-{}",
        tag,
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir_all(&dir).expect("create temp workspace");
    dir
}

/// The provider of one test workspace. The root resolves exactly like the CLI
/// entry points resolve it, so the canonical containment check sees the same
/// root shape it sees in production (on Windows, canonicalize adds the `\\?\`
/// verbatim prefix).
fn test_provider(workspace_root: &Path) -> AgentToolProvider {
    let resolved =
        resolve_workspace_root(Some(workspace_root.to_path_buf())).expect("workspace resolves");
    AgentToolProvider {
        agent: Arc::new(Mutex::new(AgentServer::new(resolved))),
    }
}

fn tool_call(provider: &AgentToolProvider, tool: &str, arguments: Value) -> Value {
    let arguments = match arguments {
        Value::Object(arguments) => arguments,
        other => panic!("test arguments must be an object, got {other}"),
    };
    let params = serde_json::from_value::<serde_json::Map<String, Value>>(json!({
        "name": tool,
        "arguments": Value::Object(arguments),
    }))
    .expect("call params");
    let (name, arguments) = provider
        .check(&params)
        .unwrap_or_else(|reason| panic!("{tool} passes check: {reason}"));
    provider
        .call("test-1", &name, arguments)
        .expect("the provider answers the call")
}

fn error_payload(result: &Value) -> Value {
    assert_eq!(
        result["isError"], true,
        "an error result is flagged: {result:#}"
    );
    let text = result["content"][0]["text"]
        .as_str()
        .expect("the error text block");
    serde_json::from_str(text).expect("the error text is the contract JSON object")
}

/// The drift guard: the MCP tool table is a 1:1 projection of
/// `agent.describe.methods`, in the same order, mangled dot-to-underscore, and
/// every tool carries an object input schema and a non-empty description.
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
            tool["name"],
            json!(tool_name_for_method(method)),
            "tool name must be the mangled method name, in describe order"
        );
        let description = tool["description"].as_str().expect("tool description");
        assert!(!description.is_empty(), "tool description is non-empty");
        let schema = &tool["inputSchema"];
        assert_eq!(
            schema["type"],
            json!("object"),
            "input schema for {method} must be an object schema"
        );
    }

    // The mangled names satisfy the MCP tool-name pattern
    // ^[a-zA-Z0-9_-]{1,64}$ (docs/guides/AGENT_MCP_BINDING_V1.md).
    for name in tools.iter().map(|tool| tool["name"].as_str().unwrap()) {
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
        let schema = input_schema(binding.method);
        assert_eq!(
            schema["type"],
            json!("object"),
            "{} schema must be an object schema",
            binding.method
        );
    }
    let load = input_schema("harness.load");
    let properties = load["properties"].as_object().expect("schema properties");
    for selector in ["inline_sources", "files", "project"] {
        assert!(
            properties.contains_key(selector),
            "harness.load schema still names the {selector} selector"
        );
    }
}

/// The contract's opt-in response-bounding parameters surface in the advisory
/// schemas: paging on `lsp_diagnostics`, the `issues_limit` cap on the three
/// runtime transaction tools. A flattened struct must not lose its parent
/// fields, and the shared `runtime.reload` tool must not gain a cap it does
/// not honour.
#[test]
fn response_bounding_params_surface_in_the_advisory_schemas() {
    let diagnostics = input_schema("lsp.diagnostics");
    let properties = diagnostics["properties"]
        .as_object()
        .expect("schema properties");
    for param in ["page", "page_size"] {
        assert!(
            properties.contains_key(param),
            "lsp.diagnostics schema names the {param} paging parameter"
        );
    }

    for method in [
        "runtime.build",
        "runtime.validate",
        "runtime.compile_reload",
    ] {
        let schema = input_schema(method);
        let properties = schema["properties"].as_object().expect("schema properties");
        assert!(
            properties.contains_key("issues_limit"),
            "{method} schema names the issues_limit cap"
        );
        assert!(
            properties.contains_key("project"),
            "{method} schema keeps the flattened project parameter"
        );
    }

    let reload = input_schema("runtime.reload");
    let properties = reload["properties"].as_object().expect("schema properties");
    assert!(
        !properties.contains_key("issues_limit"),
        "runtime.reload carries no issues_limit: its payload has no top-level issues list"
    );
}

/// Success path: `workspace_write` then `workspace_read` round-trip through
/// raw forwarded arguments; `structuredContent` is the Agent API JSON result
/// and the text block renders it.
#[test]
fn workspace_round_trip_returns_structured_content() {
    let workspace = temp_workspace("round-trip");
    let provider = test_provider(&workspace);

    let written = tool_call(
        &provider,
        "workspace_write",
        json!({
            "path": "notes/hello.txt",
            "text": "hello",
            "create_parents": true,
        }),
    );
    assert_eq!(written["isError"], json!(null), "success is not flagged");
    assert_eq!(
        written["structuredContent"],
        json!({"path": "notes/hello.txt", "bytes_written": 5})
    );
    let text = written["content"][0]["text"].as_str().expect("text block");
    assert!(
        text.contains("bytes_written"),
        "the text block renders the result: {text}"
    );

    let read = tool_call(
        &provider,
        "workspace_read",
        json!({ "path": "notes/hello.txt" }),
    );
    assert_eq!(
        read["structuredContent"],
        json!({"path": "notes/hello.txt", "text": "hello"})
    );
    let _ = std::fs::remove_dir_all(workspace);
}

/// Error paths stay inside error tool results: the text block carries the
/// contract-stable `{code, message, data}` JSON and the connection stays open
/// (the next call still answers).
#[test]
fn contract_errors_map_to_error_tool_results() {
    let workspace = temp_workspace("errors");
    let provider = test_provider(&workspace);

    // A file the surviving-connection call can read afterwards.
    tool_call(
        &provider,
        "workspace_write",
        json!({
            "path": "notes/hello.txt",
            "text": "hello",
            "create_parents": true,
        }),
    );

    let escaped = tool_call(
        &provider,
        "workspace_write",
        json!({
            "path": "../../escape.st",
            "text": "nope",
        }),
    );
    let payload = error_payload(&escaped);
    assert_eq!(payload["code"], -32001);
    assert_eq!(payload["data"]["kind"], "path_outside_workspace");

    let invalid = tool_call(&provider, "workspace_read", json!({}));
    let payload = error_payload(&invalid);
    assert_eq!(payload["code"], -32602);

    // The connection survives the error results.
    let after = tool_call(
        &provider,
        "workspace_read",
        json!({ "path": "notes/hello.txt" }),
    );
    assert!(after.get("isError").is_none(), "{after:#}");
    let _ = std::fs::remove_dir_all(workspace);
}

/// The layer's `check` refuses an unknown tool name and non-object arguments;
/// both are the connection's -32602, never a tool result.
#[test]
fn check_refuses_unknown_tools_and_non_object_arguments() {
    let workspace = temp_workspace("check");
    let provider = test_provider(&workspace);

    let unknown: serde_json::Map<String, Value> =
        serde_json::from_value(json!({ "name": "nonexistent_method" })).expect("params object");
    assert!(provider.check(&unknown).is_err(), "unknown tool refused");

    let not_object: serde_json::Map<String, Value> = serde_json::from_value(json!({
        "name": "workspace_read",
        "arguments": [1, 2],
    }))
    .expect("params object");
    assert!(
        provider.check(&not_object).is_err(),
        "non-object arguments refused"
    );
    let _ = std::fs::remove_dir_all(workspace);
}

/// The harness session is shared per provider instance: `harness_load` then
/// `harness_cycle` advance one and the same session.
#[test]
fn harness_session_is_shared_across_tool_calls() {
    let workspace = temp_workspace("harness");
    let provider = test_provider(&workspace);

    let loaded = tool_call(
        &provider,
        "harness_load",
        json!({
            "inline_sources": [{"text": "PROGRAM Main\nEND_PROGRAM\n"}],
        }),
    );
    let load_payload = loaded["structuredContent"]
        .as_object()
        .expect("load result");
    let cycles_after_load = load_payload["cycle_count"].as_u64().expect("cycle count");

    let cycled = tool_call(&provider, "harness_cycle", json!({ "count": 2 }));
    let cycle_payload = cycled["structuredContent"]
        .as_object()
        .expect("cycle result");
    let cycles_after_step = cycle_payload["cycle_count"].as_u64().expect("cycle count");
    assert_eq!(
        cycles_after_step,
        cycles_after_load + 2,
        "harness_cycle continues the session harness_load created"
    );
    let _ = std::fs::remove_dir_all(workspace);
}

/// A dispatch that panicked poisons the server mutex, and every later call
/// answers an error tool result that says the session must be restarted — not
/// the connection layer's generic failure answer, which one failed call would
/// be indistinguishable from (specification 20 §8).
#[test]
fn a_panicked_dispatch_marks_every_later_call_as_a_dead_session() {
    let workspace = temp_workspace("poisoned");
    let resolved = resolve_workspace_root(Some(workspace.clone())).expect("workspace resolves");
    let agent = Arc::new(Mutex::new(AgentServer::new(resolved)));
    let provider = AgentToolProvider {
        agent: Arc::clone(&agent),
    };

    // End the session the way a panicking dispatch ends it: the mutex is
    // dropped by an unwinding panic, which poisons it.
    let poisoner = Arc::clone(&agent);
    let _ = std::thread::spawn(move || {
        let _held = poisoner.lock().expect("lock before the panic");
        panic!("the dispatch panicked while holding the server");
    })
    .join();

    let dead = tool_call(&provider, "agent_describe", json!({}));
    let payload = error_payload(&dead);
    assert_eq!(payload["code"], -32002, "{payload:#}");
    assert_eq!(
        payload["data"]["kind"], "session_ended",
        "the payload carries the machine-readable kind: {payload:#}"
    );
    let message = payload["message"].as_str().expect("restart message");
    assert!(
        message.contains("Restart 'trust-dev agent serve-mcp'"),
        "the message names the restart action: {message}"
    );
    assert!(
        message.contains("harness session state is lost"),
        "the message is honest about the lost session state: {message}"
    );
    let _ = std::fs::remove_dir_all(workspace);
}
