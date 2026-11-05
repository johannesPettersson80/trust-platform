//! The HMI authoring tools of one VS Code window, as the MCP server's tool provider (specification
//! 43, §3.1): the window's loopback lists and runs them; the provider adds their annotations and
//! the checks of a call, and gives every call the connection's workspace.

use std::io::Write;
use std::sync::Arc;

use serde_json::{json, Map, Value};

use super::bridge::{self, Bridge};
use super::connection::Connection;
use super::Limits;
use crate::mcp::{self, ServerInfo, ToolProvider, Unavailable};

/// The argument that names the workspace of a tool call.
const WORKSPACE_ID: &str = "workspaceId";

/// What the server says of itself (specification 43, §3.1).
const INFO: ServerInfo = ServerInfo {
    name: "trust-hmi-authoring",
    version: "1.0.0",
    instructions: "Use the connected VS Code project's HMI tools. Inspect context and the visual library before editing. Render and inspect proposals, validate bindings, and review changes before applying. Authoring is separate from deployment and equipment operation.",
    unavailable: "HMI authoring connection unavailable or request limit exceeded. Reconnect in the selected VS Code window.",
};

/// Why the server cannot start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartError {
    /// The loopback did not answer the tool list.
    Bridge,
    /// The loopback's tool list is not a list of named tools.
    Tools,
    /// No random call-id prefix could be drawn.
    Random,
}

/// The tools of one connection.
struct HmiTools {
    bridge: Bridge,
    workspace_id: String,
    /// The loopback's tools, as it listed them when the server started.
    tools: Vec<Value>,
}

/// Starts the server of a connection: it asks the loopback for its tools first, so that a
/// connection that cannot answer fails before the agent uses it.
pub(crate) fn start(
    connection: Connection,
    limits: Limits,
    output: Box<dyn Write + Send>,
) -> Result<Arc<mcp::Server>, StartError> {
    let bridge = Bridge::new(
        connection.address,
        &connection.capability,
        limits.bridge_time,
        limits.answer_bytes,
    );
    let answer = bridge
        .post(bridge::TOOLS, &json!({}))
        .map_err(|_| StartError::Bridge)?;
    let tools = tool_list(answer).ok_or(StartError::Tools)?;
    let provider = HmiTools {
        bridge,
        workspace_id: connection.workspace_id,
        tools,
    };
    mcp::Server::start(Arc::new(provider), limits.connection(), output)
        .map_err(|_| StartError::Random)
}

impl ToolProvider for HmiTools {
    fn info(&self) -> ServerInfo {
        INFO
    }

    fn tools(&self) -> &[Value] {
        &self.tools
    }

    /// A listed tool, an argument object, and no workspace other than the connection's. The
    /// arguments come back with the connection's workspace.
    fn check(
        &self,
        params: &Map<String, Value>,
    ) -> Result<(String, Map<String, Value>), &'static str> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| self.is_tool(name))
            .ok_or("Unknown authoring tool.")?;
        let Some(Value::Object(arguments)) = params.get("arguments") else {
            return Err("Tool arguments must be an object.");
        };
        match arguments.get(WORKSPACE_ID) {
            None => {}
            Some(Value::String(workspace)) if *workspace == self.workspace_id => {}
            Some(_) => return Err("The tool belongs to another workspace."),
        }
        let mut arguments = arguments.clone();
        arguments.insert(
            WORKSPACE_ID.to_owned(),
            Value::String(self.workspace_id.clone()),
        );
        Ok((name.to_owned(), arguments))
    }

    /// The loopback runs the call; a failed exchange is the failure answer.
    fn call(
        &self,
        id: &str,
        name: &str,
        arguments: Map<String, Value>,
    ) -> Result<Value, Unavailable> {
        let body = json!({ "id": id, "name": name, "arguments": arguments });
        self.bridge
            .post(bridge::CALL, &body)
            .map_err(|_| Unavailable)
    }

    fn cancel(&self, id: &str) {
        let _ = self.bridge.post(bridge::CANCEL, &json!({ "id": id }));
    }
}

impl HmiTools {
    /// Whether the loopback lists a tool with this name.
    fn is_tool(&self, name: &str) -> bool {
        self.tools
            .iter()
            .any(|tool| tool.get("name").and_then(Value::as_str) == Some(name))
    }
}

/// The tools that change the editor's draft or the project (specification 43 §3.1); every other
/// tool only reads.
const CHANGING_TOOLS: [&str; 4] = [
    "hmi_compose_page",
    "hmi_edit_draft",
    "hmi_apply_draft",
    "hmi_preview_states",
];

/// The one tool that writes the project's files (specification 43 §3.1).
const DESTRUCTIVE_TOOL: &str = "hmi_apply_draft";

/// `tool` with the annotations a client reads before it asks the user (specification 43 §3.1):
/// whether it only reads, whether it writes the project, and that it reaches nothing beyond the
/// workspace.
fn annotated(mut tool: Value) -> Value {
    let name = tool
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    tool["annotations"] = json!({
        "readOnlyHint": !CHANGING_TOOLS.contains(&name.as_str()),
        "destructiveHint": name == DESTRUCTIVE_TOOL,
        "openWorldHint": false,
    });
    tool
}

/// The tools of the loopback's answer `{ "tools": [...] }`, each one an object with a name, with
/// their annotations.
fn tool_list(answer: Value) -> Option<Vec<Value>> {
    let Value::Object(mut answer) = answer else {
        return None;
    };
    let Some(Value::Array(tools)) = answer.remove("tools") else {
        return None;
    };
    let named = tools
        .iter()
        .all(|tool| tool.get("name").is_some_and(Value::is_string));
    named.then(|| tools.into_iter().map(annotated).collect())
}
