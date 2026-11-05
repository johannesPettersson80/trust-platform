//! The JSON-RPC 2.0 messages of the MCP connection (specification 43, §3.1): how a message is read,
//! the answers the server gives itself and its refusals.

use serde_json::{json, Map, Value};

use super::ServerInfo;

/// The message is not JSON.
pub(crate) const PARSE_ERROR: i64 = -32700;
/// The message is not a JSON-RPC 2.0 request, its id is not a string or a number, or the request
/// cannot run now.
pub(crate) const INVALID_REQUEST: i64 = -32600;
/// The server has no such method.
pub(crate) const METHOD_NOT_FOUND: i64 = -32601;
/// The tool call's parameters are refused by the tool provider: for the HMI authoring tools, an
/// unknown tool, no argument object or another workspace.
pub(crate) const INVALID_PARAMS: i64 = -32602;
/// The tool provider could not answer the call, the server could not run it, or the line was over
/// the line limit.
pub(crate) const UNAVAILABLE: i64 = -32000;

/// The notification that cancels a request.
pub(crate) const CANCELLED: &str = "notifications/cancelled";

/// The protocol versions the server speaks, newest first.
pub(crate) const VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// A message read from a line.
#[derive(Debug, PartialEq)]
pub(crate) enum Message {
    /// A request: it is answered with its id.
    Request {
        id: Value,
        method: String,
        params: Map<String, Value>,
    },
    /// A notification: it has no id and is never answered.
    Notification {
        method: String,
        params: Map<String, Value>,
    },
    /// Not a request the server can run; it is refused with this id and reason.
    Invalid { id: Value, reason: &'static str },
}

/// Whether `id` can identify a request: a string or a number.
pub(crate) fn is_request_id(id: &Value) -> bool {
    id.is_string() || id.is_number()
}

/// Reads a JSON value as a message. A message must be a JSON-RPC 2.0 object with a string method;
/// one that is not is refused with its id when it has a valid one. Parameters that are not an
/// object are read as none.
pub(crate) fn classify(message: Value) -> Message {
    let Value::Object(mut message) = message else {
        return Message::Invalid {
            id: Value::Null,
            reason: "Invalid request.",
        };
    };
    let id = message.remove("id");
    let versioned = message.get("jsonrpc").and_then(Value::as_str) == Some("2.0");
    let method = match message.remove("method") {
        Some(Value::String(method)) if versioned => method,
        _ => {
            return Message::Invalid {
                id: id.filter(is_request_id).unwrap_or(Value::Null),
                reason: "Invalid request.",
            };
        }
    };
    let params = match message.remove("params") {
        Some(Value::Object(params)) => params,
        _ => Map::new(),
    };
    match id {
        None => Message::Notification { method, params },
        Some(id) if is_request_id(&id) => Message::Request { id, method, params },
        Some(_) => Message::Invalid {
            id: Value::Null,
            reason: "Invalid request id.",
        },
    }
}

/// The result of `initialize`: the client's protocol version when the server speaks it, otherwise
/// the newest one the server speaks, and what the server says of itself.
pub(crate) fn initialize(params: &Map<String, Value>, info: &ServerInfo) -> Value {
    let requested = params.get("protocolVersion").and_then(Value::as_str);
    let version = requested
        .filter(|version| VERSIONS.contains(version))
        .unwrap_or(VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": info.name, "version": info.version },
        "instructions": info.instructions,
    })
}

/// The answer to request `id`.
pub(crate) fn answer(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// The refusal of request `id` with an error code and message.
pub(crate) fn refusal(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// The failure answer of request `id`, with the server's `message`.
pub(crate) fn unavailable(id: Value, message: &str) -> Value {
    refusal(id, UNAVAILABLE, message)
}
