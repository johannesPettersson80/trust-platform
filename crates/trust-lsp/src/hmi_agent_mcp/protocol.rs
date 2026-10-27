//! The JSON-RPC 2.0 messages of the MCP connection (specification 43, §3.1): how a message is read,
//! the answers the server gives itself and its refusals.

use serde_json::{json, Map, Value};

/// The message is not JSON.
pub(crate) const PARSE_ERROR: i64 = -32700;
/// The message is not a JSON-RPC 2.0 request, its id is not a string or a number, or the request
/// cannot run now.
pub(crate) const INVALID_REQUEST: i64 = -32600;
/// The server has no such method.
pub(crate) const METHOD_NOT_FOUND: i64 = -32601;
/// The tool call names an unknown tool, has no argument object or names another workspace.
pub(crate) const INVALID_PARAMS: i64 = -32602;
/// The loopback could not answer.
pub(crate) const UNAVAILABLE: i64 = -32000;

/// The notification that cancels a request.
pub(crate) const CANCELLED: &str = "notifications/cancelled";

/// The protocol versions the server speaks, newest first.
pub(crate) const VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// The server's name and version in the answer to `initialize`.
const SERVER_NAME: &str = "trust-hmi-authoring";
const SERVER_VERSION: &str = "1.0.0";

/// The instructions the server gives the agent in the answer to `initialize`.
const INSTRUCTIONS: &str = "Use the connected VS Code project's HMI tools. Inspect context and the visual library before editing. Render and inspect proposals, validate bindings, and review changes before applying. Authoring is separate from deployment and equipment operation.";

/// The failure answer of a request that the loopback could not answer.
const UNAVAILABLE_MESSAGE: &str = "HMI authoring connection unavailable or request limit exceeded. Reconnect in the selected VS Code window.";

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
/// the newest one the server speaks.
pub(crate) fn initialize(params: &Map<String, Value>) -> Value {
    let requested = params.get("protocolVersion").and_then(Value::as_str);
    let version = requested
        .filter(|version| VERSIONS.contains(version))
        .unwrap_or(VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION },
        "instructions": INSTRUCTIONS,
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

/// The answer to request `id` that the loopback could not answer.
pub(crate) fn unavailable(id: Value) -> Value {
    refusal(id, UNAVAILABLE, UNAVAILABLE_MESSAGE)
}
