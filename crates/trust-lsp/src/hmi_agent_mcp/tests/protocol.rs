//! The protocol rules of the stdio server (specification 43, §3.1): message form, request ids,
//! `initialize`, `ping`, `tools/list`, unknown methods, notifications and standard output.

use std::time::Duration;

use serde_json::{json, Value};

use super::super::bridge::TOOLS;
use super::super::Limits;
use super::{cancel, ping, refusal, Calls, FakeBridge, Session, CAPABILITY};

/// The instructions of §3.1, word for word.
const INSTRUCTIONS: &str = "Use the connected VS Code project's HMI tools. Inspect context and the visual library before editing. Render and inspect proposals, validate bindings, and review changes before applying. Authoring is separate from deployment and equipment operation.";

/// Catches: a line that is not JSON (or not UTF-8) going unanswered or being answered with another
/// code than the parse error.
#[test]
fn a_line_that_is_not_json_is_a_parse_error() {
    let bridge = FakeBridge::start(Calls::Never);
    let session = Session::start(&bridge, Limits::PRODUCT);
    let lines: [&[u8]; 4] = [b"{", b"", b"{\"jsonrpc\":\"2.0\",\"id\":1", b"\xff\xfe"];
    for line in lines {
        let mut bytes = line.to_vec();
        bytes.push(b'\n');
        session.send_raw(&bytes);
        assert_eq!(
            session.next(),
            refusal(Value::Null, -32700, "Invalid JSON."),
            "{}",
            String::from_utf8_lossy(line)
        );
    }
    session.close();
}

/// Catches: a message that is not a JSON-RPC 2.0 request being run, ignored silently, or answered
/// without the id it carries. §3.1 answers -32600 here; the TypeScript client answered such a
/// message with -32000 or not at all, because its bridge refused it with HTTP 400.
#[test]
fn a_message_that_is_not_a_json_rpc_request_is_invalid() {
    let bridge = FakeBridge::start(Calls::Never);
    let session = Session::start(&bridge, Limits::PRODUCT);
    for (message, id) in [
        (json!([]), Value::Null),
        (json!([ping(json!(1))]), Value::Null),
        (json!(1), Value::Null),
        (
            json!({"jsonrpc": "1.0", "id": 3, "method": "ping"}),
            json!(3),
        ),
        (json!({"id": 4, "method": "ping"}), json!(4)),
        (json!({"jsonrpc": "2.0", "id": 5}), json!(5)),
        (
            json!({"jsonrpc": "2.0", "id": "six", "method": 6}),
            json!("six"),
        ),
        (json!({"jsonrpc": "2.0", "method": 7}), Value::Null),
        (
            json!({"jsonrpc": "2.0", "id": true, "method": 8}),
            Value::Null,
        ),
    ] {
        assert_eq!(
            session.ask(&message),
            refusal(id, -32600, "Invalid request."),
            "{message}"
        );
    }
    session.close();
}

/// Catches: a request whose id cannot be answered being run, or a string or fractional number id
/// being refused.
#[test]
fn a_request_id_is_a_string_or_a_number() {
    let bridge = FakeBridge::start(Calls::Never);
    let session = Session::start(&bridge, Limits::PRODUCT);
    for id in [Value::Null, json!(true), json!({"n": 1}), json!([1])] {
        assert_eq!(
            session.ask(&ping(id.clone())),
            refusal(Value::Null, -32600, "Invalid request id."),
            "{id}"
        );
    }
    for id in [json!("a"), json!(1.5), json!(-2), json!(0)] {
        assert_eq!(
            session.ask(&ping(id.clone())),
            json!({"jsonrpc": "2.0", "id": id, "result": {}}),
        );
    }
    session.close();
}

/// Catches: answering a protocol version the client did not ask for although the server speaks
/// it, answering one the server does not speak, or another server identity.
#[test]
fn initialize_answers_the_clients_version_when_supported_and_the_newest_otherwise() {
    let bridge = FakeBridge::start(Calls::Never);
    let session = Session::start(&bridge, Limits::PRODUCT);
    for (requested, answered) in [
        (json!("2025-11-25"), "2025-11-25"),
        (json!("2025-06-18"), "2025-06-18"),
        (json!("2025-03-26"), "2025-03-26"),
        (json!("2024-11-05"), "2024-11-05"),
        (json!("2024-10-07"), "2025-11-25"),
        (json!("2026-01-01"), "2025-11-25"),
        (json!(20_250_618), "2025-11-25"),
        (Value::Null, "2025-11-25"),
    ] {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {"protocolVersion": requested, "capabilities": {}, "clientInfo": {"name": "fixture", "version": "1"}}
        });
        assert_eq!(
            session.ask(&request),
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "protocolVersion": answered,
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "trust-hmi-authoring", "version": "1.0.0"},
                    "instructions": INSTRUCTIONS
                }
            }),
            "{requested}"
        );
    }
    let without_params = json!({"jsonrpc": "2.0", "id": 2, "method": "initialize"});
    assert_eq!(
        session.ask(&without_params)["result"]["protocolVersion"],
        "2025-11-25"
    );
    session.close();
}

/// Catches: a ping going unanswered, or answered with data.
#[test]
fn ping_answers_an_empty_result() {
    let bridge = FakeBridge::start(Calls::Never);
    let session = Session::start(&bridge, Limits::PRODUCT);
    assert_eq!(
        session.ask(&ping(json!(1))),
        json!({"jsonrpc": "2.0", "id": 1, "result": {}})
    );
    session.close();
}

/// Catches: a method the server does not offer being answered as if it existed.
#[test]
fn an_unknown_method_is_refused() {
    let bridge = FakeBridge::start(Calls::Never);
    let session = Session::start(&bridge, Limits::PRODUCT);
    for method in ["resources/list", "prompts/get", "tools/run", "Ping"] {
        let request = json!({"jsonrpc": "2.0", "id": 9, "method": method});
        assert_eq!(
            session.ask(&request),
            refusal(json!(9), -32601, "Unknown method."),
            "{method}"
        );
    }
    session.close();
}

/// Catches: a notification being answered (JSON-RPC forbids it), whatever it names.
#[test]
fn notifications_are_never_answered() {
    let bridge = FakeBridge::start(Calls::Never);
    let session = Session::start(&bridge, Limits::PRODUCT);
    session.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    session.send(&cancel(json!(99)));
    session.send(&json!({"jsonrpc": "2.0", "method": "ping"}));
    session.send(&json!({"jsonrpc": "2.0", "method": "tools/call", "params": {"name": "hmi_get_context", "arguments": {}}}));
    assert_eq!(
        session.ask(&ping(json!(10))),
        json!({"jsonrpc": "2.0", "id": 10, "result": {}}),
        "the first answer is the ping's"
    );
    assert!(session.silent_for(Duration::from_millis(200)));
    assert!(bridge.received_on("/mcp/call").is_empty());
    session.close();
}

/// The tool `name` as the server lists it: the loopback's description and schema, and the
/// annotations of specification 43 §3.1.
fn listed_tool(name: &str, description: &str, read_only: bool, destructive: bool) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {"type": "object"},
        "annotations": {
            "readOnlyHint": read_only,
            "destructiveHint": destructive,
            "openWorldHint": false,
        },
    })
}

/// Catches: a second definition of the tools in Rust, a list that differs from the loopback's
/// names, descriptions or schemas, annotations taken from the loopback instead of the server's
/// rule (specification 43 §3.1), or the list being asked again for every request.
#[test]
fn tools_list_answers_the_loopbacks_tools() {
    let bridge = FakeBridge::start(Calls::Never);
    let session = Session::start(&bridge, Limits::PRODUCT);
    let listed = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"});
    let expected = json!([
        listed_tool("hmi_get_context", "Read the project", true, false),
        listed_tool("hmi_render_draft", "Render a draft", true, false),
    ]);
    for _ in 0..2 {
        assert_eq!(
            session.ask(&listed),
            json!({"jsonrpc": "2.0", "id": 2, "result": {"tools": expected}})
        );
    }
    assert_eq!(
        bridge.received_on(TOOLS).len(),
        1,
        "asked once, at the start"
    );
    session.close();
}

/// Catches: standard output carrying anything but one JSON-RPC 2.0 message per line, or the
/// capability.
#[test]
fn standard_output_carries_only_protocol_messages() {
    let bridge = FakeBridge::start(Calls::Answer(json!({"content": []})));
    let session = Session::start(&bridge, Limits::PRODUCT);
    let messages = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "hmi_get_context", "arguments": {"contractVersion": 1, "input": {}}}}),
        json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "unknown", "arguments": {}}}),
        ping(json!(5)),
    ];
    for message in &messages {
        session.send(message);
    }
    session.send_raw(b"not json\n");
    let mut lines = Vec::new();
    for _ in 0..6 {
        lines.push(session.next());
    }
    lines.extend(
        session
            .close()
            .iter()
            .map(|line| serde_json::from_str::<Value>(line).expect("JSON")),
    );
    assert_eq!(lines.len(), 6, "{lines:?}");
    for line in &lines {
        assert_eq!(line["jsonrpc"], "2.0", "{line}");
        assert!(line.get("id").is_some(), "{line}");
        assert!(!line.to_string().contains(CAPABILITY));
    }
}

/// Specification 43 §3.1: the tools that change the draft or the project are not read-only, and
/// the one that writes the project's files is destructive, whatever the loopback says. Catches a
/// client told that Apply only reads, so that it applies a draft without asking.
#[test]
fn the_tools_that_change_things_are_annotated_so() {
    let schema = json!({"type": "object"});
    let bridge = FakeBridge::with_tools(
        Calls::Never,
        json!({"tools": [
            {"name": "hmi_apply_draft", "description": "Apply", "inputSchema": schema,
             "annotations": {"readOnlyHint": true, "destructiveHint": false}},
            {"name": "hmi_compose_page", "description": "Compose", "inputSchema": schema},
        ]}),
    );
    let session = Session::start(&bridge, Limits::PRODUCT);
    let listed = session.ask(&json!({"jsonrpc": "2.0", "id": 3, "method": "tools/list"}));
    assert_eq!(
        listed["result"]["tools"],
        json!([
            listed_tool("hmi_apply_draft", "Apply", false, true),
            listed_tool("hmi_compose_page", "Compose", false, false),
        ])
    );
    session.close();
}
