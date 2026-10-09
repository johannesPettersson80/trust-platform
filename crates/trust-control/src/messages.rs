//! The messages between the Control Studio panel and the language server (specification 35,
//! §0.1a): the arguments and results of the `trust-lsp.controlStudio…` commands and the
//! `trust/controlStudio/update` notification. The extension's TypeScript types are generated from
//! these definitions, never written by hand.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::failure::Failure;
use crate::loop_definition::LoopDefinition;
use crate::stages::StageAvailability;

/// The arguments of `trust-lsp.controlStudioOpen`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenRequest {
    /// The project folder.
    pub project_root: String,
    /// The runtime selected in VS Code; a loop of another runtime is refused.
    pub runtime_id: String,
    /// The loop to open; absent lists the project's loops.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_id: Option<String>,
    /// The extension's storage folder for this workspace, which holds the operation journal
    /// (A8.3, A8.5).
    pub storage_folder: String,
}

/// The result of `trust-lsp.controlStudioOpen`.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum OpenResult {
    /// The project's loop files, for the engineer to choose one.
    Loops {
        /// One entry per loop file.
        loops: Vec<LoopEntry>,
    },
    /// The loop is open.
    #[serde(rename_all = "camelCase")]
    Opened {
        /// The checked loop definition.
        #[serde(rename = "loop")]
        loop_definition: Box<LoopDefinition>,
        /// The status of every stage.
        stages: Vec<StageAvailability>,
        /// The operations that were journaled for this loop and are reattached (A8.5, A8.6).
        reattached: Vec<String>,
        /// Present when the loop uses a feature the Rust engine does not run yet.
        #[serde(skip_serializing_if = "Option::is_none")]
        engine_refusal: Option<Failure>,
    },
}

/// One loop file of a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoopEntry {
    /// The file.
    pub file: String,
    /// The loop's identifier, when the file is a loop definition.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_id: Option<String>,
    /// The loop's name, when the file is a loop definition.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Why the loop cannot be opened; absent when it can.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
}

/// The arguments of `trust-lsp.controlStudioStatus` and `trust-lsp.controlStudioCancel`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationRequest {
    /// The operation.
    pub operation_id: String,
}

/// The state of a long operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum OperationState {
    /// It is running.
    Running,
    /// It ended with its result.
    Finished,
    /// It ended with a failure.
    Failed,
    /// It was cancelled.
    Cancelled,
}

/// What a long operation reports: the `trust/controlStudio/update` notification, and the answer of
/// `trust-lsp.controlStudioStatus`.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OperationUpdate {
    /// The operation.
    pub operation_id: String,
    /// The command that started it, for example `identify`.
    pub command: String,
    /// Its state.
    pub state: OperationState,
    /// What it is doing, in words.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<String>,
    /// Its result once finished, in the form its stage defines.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Its failure once failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<Failure>,
}

/// The answer of a command that starts a long operation: its identifier, at once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OperationStarted {
    /// The operation.
    pub operation_id: String,
}
