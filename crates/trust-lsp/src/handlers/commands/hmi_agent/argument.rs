//! The argument an authoring command receives (specification 43, §3 and §4): the tool call's envelope,
//! the project folder and its draft storage, the open editor documents, and the facts the extension
//! reads from its own state (builder panel selections, the remote runtime's variables, simulator
//! sessions). This module is transport only; the rules live in the operations.
//!
//! The argument is one JSON object: `workspaceId` (the connection's workspace), `root_uri` (a `file`
//! URI), `storage` (an absolute folder), `open` (open documents by project-relative path, each with
//! `text`, `version` and `dirty`), `request` (the envelope `{contractVersion, workspaceId, input}`),
//! and the optional facts `remote`, `selections` and `simulatorSessions`. A command may read further
//! fields of the argument itself (`phase`, `action`, `draftId`, `key`, `status`), which its module names.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;
use tower_lsp::lsp_types::Url;

use super::documents::OpenDocument;
use super::error::{AgentError, AgentResult, UNSUPPORTED_CONTEXT};

/// A builder panel's selection, as the extension reports it (`Selection` in `queryMetadata.ts`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct PanelSelection {
    /// The page the panel shows, project-relative.
    pub(super) file: String,
    /// The objects selected in it, in the order the panel holds them.
    pub(super) ids: Vec<String>,
    /// The editor document's version when the selection was taken.
    pub(super) document_version: i64,
    /// Whether the panel is the active one.
    pub(super) active: bool,
}

/// The decoded argument of one authoring command.
#[derive(Debug)]
pub(super) struct Argument {
    /// The connected workspace this request belongs to.
    pub(super) workspace_id: String,
    /// The tool call's envelope, to be checked by the boundary.
    pub(super) request: Value,
    /// The project folder's URI, as the HMI commands take it.
    pub(super) root_uri: String,
    /// The project folder.
    pub(super) root: PathBuf,
    /// The folder that holds the draft records.
    pub(super) storage: PathBuf,
    /// The open editor documents under the project, saved or unsaved, by project-relative path.
    pub(super) open: BTreeMap<String, OpenDocument>,
    /// The remote runtime's variables by path, as an object (`{}` when no runtime is connected).
    pub(super) remote: Value,
    /// The builder panels and their selections.
    pub(super) selections: Vec<PanelSelection>,
    /// The open simulator sessions, as the extension lists them.
    pub(super) simulator_sessions: Vec<Value>,
    /// The whole argument, for the command's own fields.
    pub(super) raw: Value,
}

impl Argument {
    /// Reads the one argument object of a command.
    ///
    /// # Errors
    ///
    /// `INVALID_ARGUMENT` for a missing or malformed field; `UNSUPPORTED_CONTEXT` when the project is not
    /// a local `file` folder, since a remote or virtual workspace must not be resolved as a local path.
    pub(super) fn read(arguments: Vec<Value>) -> AgentResult<Self> {
        let [raw] = <[Value; 1]>::try_from(arguments).map_err(|_| {
            AgentError::invalid("the authoring command expects one argument object.")
        })?;
        let workspace_id = text_field(&raw, "workspaceId")?;
        let root_uri = text_field(&raw, "root_uri")?;
        let root = project_folder(&root_uri)?;
        let storage = PathBuf::from(text_field(&raw, "storage")?);
        if !storage.is_absolute() {
            return Err(AgentError::invalid(
                "storage must be an absolute folder path.",
            ));
        }
        let open = optional::<BTreeMap<String, OpenDocument>>(&raw, "open")?.unwrap_or_default();
        let remote = match raw.get("remote") {
            None | Some(Value::Null) => Value::Object(serde_json::Map::new()),
            Some(value) if value.is_object() => value.clone(),
            Some(_) => {
                return Err(AgentError::invalid(
                    "remote must be an object of variables.",
                ))
            }
        };
        let selections = optional::<Vec<PanelSelection>>(&raw, "selections")?.unwrap_or_default();
        let simulator_sessions =
            optional::<Vec<Value>>(&raw, "simulatorSessions")?.unwrap_or_default();
        Ok(Self {
            workspace_id,
            request: raw.get("request").cloned().unwrap_or(Value::Null),
            root_uri,
            root,
            storage,
            open,
            remote,
            selections,
            simulator_sessions,
            raw,
        })
    }
}

/// A required, non-blank text field of the argument.
fn text_field(raw: &Value, key: &str) -> AgentResult<String> {
    raw.get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| AgentError::invalid(format!("{key} is required.")))
}

/// An optional field of the argument, decoded; absent and null are `None`.
fn optional<T: DeserializeOwned>(raw: &Value, key: &str) -> AgentResult<Option<T>> {
    match raw.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => serde_json::from_value(value.clone())
            .map(Some)
            .map_err(|error| AgentError::invalid(format!("{key} is malformed: {error}."))),
    }
}

/// The project folder of a `file` URI. Any other scheme, such as a remote or virtual workspace, is an
/// unsupported context: it is bound to its own extension host and is never resolved as a local path.
fn project_folder(uri: &str) -> AgentResult<PathBuf> {
    let url = Url::parse(uri).map_err(|_| AgentError::invalid("root_uri must be a URI."))?;
    if url.scheme() != "file" {
        return Err(AgentError::new(
            UNSUPPORTED_CONTEXT,
            "the project is not a local folder; connect the workspace on the extension host that owns it.",
        ));
    }
    url.to_file_path().map_err(|()| {
        AgentError::new(
            UNSUPPORTED_CONTEXT,
            "the project URI does not name a local folder.",
        )
    })
}

#[cfg(test)]
#[path = "argument_tests.rs"]
mod tests;
