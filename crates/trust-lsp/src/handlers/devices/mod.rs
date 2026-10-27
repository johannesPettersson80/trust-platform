//! The Devices & Connections commands. The redesigned panel reaches the panel engine through five
//! of them (design §1.4.1): `trust-lsp.devicesView`, `trust-lsp.devicesAct`,
//! `trust-lsp.devicesBrowse`, `trust-lsp.devicesSearch` and `trust-lsp.devicesClose`. Today's
//! canvas reaches the same rules through two more (specification 25 §5.15):
//! `trust-lsp.devicesTopology` merges what the runtimes answered into the fleet view it draws, and
//! `trust-lsp.devicesCanvasAct` performs one of its actions.
//!
//! This module holds the command names, the decoding of each request, the blocking thread the work
//! runs on and the JSON reply. Every rule, label and decision is in `trust-devices` (design §1.6):
//! the engine answers, this module hands the answer on.

use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tower_lsp::lsp_types::ExecuteCommandParams;

use trust_devices::canvas::{self, CanvasActRequest, CanvasTopologyRequest};
use trust_devices::engine::{DevicesEngine, ReplyError, SearchCall, SCHEMA_VERSION};
use trust_devices::request::{ActRequest, BrowseRequest, ViewRequest};

use crate::state::ServerState;

/// One view of the panel (design §1.4.1).
pub const DEVICES_VIEW_COMMAND: &str = "trust-lsp.devicesView";
/// One action that a view offered (design §1.4.1).
pub const DEVICES_ACT_COMMAND: &str = "trust-lsp.devicesAct";
/// The browse pane of a connection or a found device (design §1.4.1).
pub const DEVICES_BROWSE_COMMAND: &str = "trust-lsp.devicesBrowse";
/// The one search, with the verb `start`, `status` or `cancel` (design §1.4.1).
pub const DEVICES_SEARCH_COMMAND: &str = "trust-lsp.devicesSearch";
/// The end of a panel's session (design §1.4.1).
pub const DEVICES_CLOSE_COMMAND: &str = "trust-lsp.devicesClose";
/// The canvas's merged topology and fleet view (specification 25 §5.15).
pub const DEVICES_TOPOLOGY_COMMAND: &str = "trust-lsp.devicesTopology";
/// One action of the canvas (specification 25 §5.15).
pub const DEVICES_CANVAS_ACT_COMMAND: &str = "trust-lsp.devicesCanvasAct";

/// The devices commands, in the order the server advertises them. The command names follow the
/// lesson of round 5b: one camel-case segment after `trust-lsp.`, so they never collide with an
/// extension command such as `trust-lsp.networkCanvas.open` (design §1.4.1).
pub const DEVICES_COMMANDS: [&str; 7] = [
    DEVICES_VIEW_COMMAND,
    DEVICES_ACT_COMMAND,
    DEVICES_BROWSE_COMMAND,
    DEVICES_SEARCH_COMMAND,
    DEVICES_CLOSE_COMMAND,
    DEVICES_TOPOLOGY_COMMAND,
    DEVICES_CANVAS_ACT_COMMAND,
];

/// Runs one of the devices commands, or answers nothing when `params` names another command.
///
/// The work runs on a blocking thread, so the language server's async workers stay free while the
/// engine answers (design §1.5). A canvas action answers when it is done, as the canvas waited for
/// its command line before (specification 25 §5.15).
pub async fn execute(state: &ServerState, params: &ExecuteCommandParams) -> Option<Value> {
    if !DEVICES_COMMANDS.contains(&params.command.as_str()) {
        return None;
    }
    let devices = Arc::clone(&state.devices);
    let command = params.command.clone();
    let arguments = params.arguments.clone();
    let reply = tokio::task::spawn_blocking(move || run(&devices, &command, &arguments))
        .await
        .unwrap_or_else(|_| {
            error_json(&ReplyError::new(
                "devices_failed",
                "The panel's work did not finish.",
            ))
        });
    Some(reply)
}

/// The JSON reply of one devices command (design §1.4.1).
fn run(devices: &DevicesEngine, command: &str, arguments: &[Value]) -> Value {
    match decode(command, arguments) {
        Ok(call) => call.answer(devices),
        Err(error) => error_json(&error),
    }
}

/// One decoded devices command.
enum Command {
    /// `trust-lsp.devicesView`.
    View(Box<ViewRequest>),
    /// `trust-lsp.devicesAct`.
    Act(Box<ActRequest>),
    /// `trust-lsp.devicesBrowse`.
    Browse(Box<BrowseRequest>),
    /// `trust-lsp.devicesSearch`.
    Search(Box<SearchCall>),
    /// `trust-lsp.devicesClose`, whose only argument is the session.
    Close(String),
    /// `trust-lsp.devicesTopology`.
    Topology(Box<CanvasTopologyRequest>),
    /// `trust-lsp.devicesCanvasAct`.
    CanvasAct(Box<CanvasActRequest>),
}

impl Command {
    /// Performs the command on the engine and answers its JSON (design §1.4.1).
    fn answer(self, devices: &DevicesEngine) -> Value {
        match self {
            Self::View(request) => reply_json(devices.view(*request)),
            Self::Act(request) => reply_json(devices.act(&request)),
            Self::Browse(request) => reply_json(devices.browse(&request)),
            Self::Search(call) => reply_json(devices.search(&call)),
            Self::Close(session) => reply_json(devices.close(&session)),
            Self::Topology(request) => reply_json(canvas::topology(&request)),
            Self::CanvasAct(request) => match canvas::act(&request) {
                Ok(reply) => reply_json(reply),
                Err(error) => error_json(&error),
            },
        }
    }
}

/// The one argument of `trust-lsp.devicesClose` (design §1.4.1).
#[derive(Debug, Deserialize)]
struct CloseCall {
    /// The panel's session.
    session: String,
}

/// The one argument object of `command`, decoded into its request.
fn decode(command: &str, arguments: &[Value]) -> Result<Command, ReplyError> {
    let [argument] = arguments else {
        return Err(ReplyError::new(
            "invalid_arguments",
            format!("{command} expects one argument object."),
        ));
    };
    match command {
        DEVICES_VIEW_COMMAND => from_json(argument).map(|request| Command::View(Box::new(request))),
        DEVICES_ACT_COMMAND => from_json(argument).map(|request| Command::Act(Box::new(request))),
        DEVICES_BROWSE_COMMAND => {
            from_json(argument).map(|request| Command::Browse(Box::new(request)))
        }
        DEVICES_SEARCH_COMMAND => from_json(argument).map(|call| Command::Search(Box::new(call))),
        DEVICES_CLOSE_COMMAND => {
            from_json::<CloseCall>(argument).map(|call| Command::Close(call.session))
        }
        DEVICES_TOPOLOGY_COMMAND => {
            from_json(argument).map(|request| Command::Topology(Box::new(request)))
        }
        DEVICES_CANVAS_ACT_COMMAND => {
            from_json(argument).map(|request| Command::CanvasAct(Box::new(request)))
        }
        other => Err(ReplyError::new(
            "unknown_command",
            format!("{other} is not a Devices & Connections command."),
        )),
    }
}

/// A request decoded from one argument object.
fn from_json<T: DeserializeOwned>(value: &Value) -> Result<T, ReplyError> {
    serde_json::from_value(value.clone())
        .map_err(|error| ReplyError::new("invalid_arguments", error.to_string()))
}

/// The JSON of a reply. These plain data types always serialize; the fallback keeps a failure from
/// becoming a missing answer.
fn reply_json<T: Serialize>(reply: T) -> Value {
    serde_json::to_value(reply).unwrap_or_else(|error| {
        error_json(&ReplyError::new("devices_reply_failed", error.to_string()))
    })
}

/// The error reply of design §1.4.1, which the panel keeps its last view for and shows in the
/// banner place.
fn error_json(error: &ReplyError) -> Value {
    json!({ "schema_version": SCHEMA_VERSION, "error": error })
}

#[cfg(test)]
mod tests;
