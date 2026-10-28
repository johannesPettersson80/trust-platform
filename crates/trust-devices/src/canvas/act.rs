//! `trust-lsp.devicesCanvasAct` (specification 25 §5.15): one action of the canvas, performed here.
//! The canvas sends the action's name, its arguments, the open project's folder, and the runtime
//! that serves its live data while that runtime answers (its control endpoint and token); it shows
//! the reply's notice, opens the generated documents it names, reads its data again when the reply
//! asks, and hands the reply's result to its webview. Every decision is the action's own.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{ads, ads_tags, comm, hosts, identify, plans, views, CANVAS_SCHEMA_VERSION};
use crate::engine::ReplyError;

/// One action of the canvas.
#[derive(Debug, Clone, Deserialize)]
pub struct CanvasActRequest {
    /// The action: `comm_apply`, `comm_test`, `comm_save`, `ads_add_device`, `ads_finds`,
    /// `ads_port`, `ads_tags_add`, `ads_tags_import`, `ads_tag_remove`, `expose`,
    /// `ethercat_channels`, `opcua_add`, `add_host`, `identify`, `browse_error`,
    /// `discover_label` or `discover_failure`.
    pub op: String,
    /// The open project's folder, into whose files an offline action writes.
    #[serde(default)]
    pub project: Option<PathBuf>,
    /// The runtime that serves the canvas's live data, while it answers.
    #[serde(default)]
    pub runtime: Option<LiveRuntime>,
    /// The action's own arguments.
    #[serde(default)]
    pub args: Value,
}

impl CanvasActRequest {
    /// The open project's folder, when the canvas has one.
    pub(super) fn project(&self) -> Option<&Path> {
        self.project
            .as_deref()
            .filter(|project| !project.as_os_str().is_empty())
    }

    /// The runtime that serves the live data, when it answers at a control endpoint.
    pub(super) fn live(&self) -> Option<&LiveRuntime> {
        self.runtime
            .as_ref()
            .filter(|runtime| !runtime.endpoint.trim().is_empty())
    }
}

/// The runtime that serves the canvas's live data.
#[derive(Debug, Clone, Deserialize)]
pub struct LiveRuntime {
    /// Its control endpoint.
    pub endpoint: String,
    /// The token stored for that endpoint, when one is.
    #[serde(default)]
    pub token: Option<String>,
}

/// The reply of one action.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CanvasReply {
    /// [`CANVAS_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Whether the action changed anything.
    pub applied: bool,
    /// What the canvas tells the user, when anything.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notice: Option<CanvasNotice>,
    /// Whether the canvas reads its data again.
    pub refresh: bool,
    /// The generated documents the canvas opens, so that their diagnostics refresh
    /// (specification 25 §5.4).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub open_documents: Vec<String>,
    /// The action's own answer for the canvas's webview.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
}

/// What the canvas tells the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CanvasNotice {
    /// `info` or `warning`.
    pub tone: &'static str,
    /// The sentence.
    pub text: String,
    /// The detail under it, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Whether the notice waits for the user's answer.
    pub modal: bool,
    /// The actions the notice offers.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<CanvasNoticeAction>,
}

/// An action that a notice offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CanvasNoticeAction {
    /// What the canvas does when it is chosen: `start_runtime` starts the project's runtime.
    pub id: &'static str,
    /// Its label.
    pub label: &'static str,
}

/// Performs one action of the canvas.
///
/// # Errors
///
/// The action is not one of the canvas, or its arguments do not decode.
pub fn act(request: &CanvasActRequest) -> Result<CanvasReply, ReplyError> {
    Ok(match request.op.as_str() {
        "comm_apply" => comm::apply(request, &args(request)?),
        "comm_test" => comm::test(request, &args(request)?),
        "comm_save" => comm::save(request, &args(request)?),
        "ads_add_device" => ads::add_device(request, &args(request)?),
        "ads_finds" => ads::finds(&args(request)?),
        "ads_port" => ads::port(&args(request)?),
        "ads_tags_add" => ads_tags::add_batch(request, &args(request)?),
        "ads_tags_import" => ads_tags::import(request, &args(request)?),
        "ads_tag_remove" => ads_tags::remove(request, &args(request)?),
        "expose" => plans::expose(request, &args(request)?),
        "ethercat_channels" => plans::ethercat_channels(request, &args(request)?),
        "opcua_add" => plans::opcua_add(request, &args(request)?),
        "add_host" => hosts::add_host(&args(request)?),
        "identify" => identify::identify(request, &args(request)?),
        "browse_error" => answered(browse_error(&args(request)?)),
        "discover_label" => answered(discover_label(&args(request)?)),
        "discover_failure" => answered(discover_failure(&args(request)?)),
        other => {
            return Err(ReplyError::new(
                "unknown_canvas_action",
                format!("{other} is not an action of the Devices & Connections canvas."),
            ))
        }
    })
}

/// The arguments of `browse_error`: the browse's protocol and its protocol error.
#[derive(Debug, Deserialize)]
struct BrowseErrorArgs {
    #[serde(default)]
    protocol: String,
    #[serde(default)]
    error: BrowsedError,
}

/// A protocol error of a browse, as the runtime answers it.
#[derive(Debug, Default, Deserialize)]
struct BrowsedError {
    code: Option<String>,
    message: Option<String>,
}

/// The arguments of `discover_label` and `discover_failure`: the protocol searched, the known
/// address or subnet, and the failure's own text.
#[derive(Debug, Deserialize)]
struct DiscoveryArgs {
    #[serde(default)]
    protocol: String,
    host: Option<String>,
    cidr: Option<String>,
    #[serde(default)]
    message: String,
}

/// `browse_error`: the recovery of a failed browse (specification 25 §5.4 and §11.2).
fn browse_error(args: &BrowseErrorArgs) -> Value {
    serde_json::to_value(views::browse_error(
        &args.protocol,
        args.error.code.as_deref(),
        args.error.message.as_deref(),
    ))
    .unwrap_or_default()
}

/// `discover_label`: the label of a discovery's progress row.
fn discover_label(args: &DiscoveryArgs) -> Value {
    serde_json::json!({
        "label": views::discovery_label(&args.protocol, args.host.as_deref(), args.cidr.as_deref()),
    })
}

/// `discover_failure`: the text of a discovery that failed at the selected runtime.
fn discover_failure(args: &DiscoveryArgs) -> Value {
    serde_json::json!({ "text": views::discovery_failure(&args.protocol, &args.message) })
}

/// A reply that only answers `result`.
fn answered(result: Value) -> CanvasReply {
    CanvasReply {
        result: Some(result),
        ..CanvasReply::nothing()
    }
}

/// The action's arguments, decoded.
fn args<T: DeserializeOwned>(request: &CanvasActRequest) -> Result<T, ReplyError> {
    let args = if request.args.is_null() {
        Value::Object(serde_json::Map::new())
    } else {
        request.args.clone()
    };
    serde_json::from_value(args).map_err(|error| {
        ReplyError::new(
            "invalid_arguments",
            format!("{} arguments: {error}", request.op),
        )
    })
}

impl CanvasReply {
    /// A reply that changed nothing and tells nothing.
    pub(super) fn nothing() -> Self {
        Self {
            schema_version: CANVAS_SCHEMA_VERSION,
            applied: false,
            notice: None,
            refresh: false,
            open_documents: Vec::new(),
            result: None,
        }
    }

    /// A reply that changed nothing and warns with `text`.
    pub(super) fn warning(text: impl Into<String>) -> Self {
        Self {
            notice: Some(CanvasNotice::warning(text)),
            ..Self::nothing()
        }
    }

    /// A reply that changed the project, tells `text` and reads the data again.
    pub(super) fn done(text: impl Into<String>) -> Self {
        Self {
            applied: true,
            notice: Some(CanvasNotice::info(text)),
            refresh: true,
            ..Self::nothing()
        }
    }
}

impl CanvasNotice {
    /// An information notice.
    pub(super) fn info(text: impl Into<String>) -> Self {
        Self {
            tone: "info",
            text: text.into(),
            detail: None,
            modal: false,
            actions: Vec::new(),
        }
    }

    /// A warning notice.
    pub(super) fn warning(text: impl Into<String>) -> Self {
        Self {
            tone: "warning",
            ..Self::info(text)
        }
    }
}
