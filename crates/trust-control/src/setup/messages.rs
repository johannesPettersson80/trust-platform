//! The engine's request and answer records for loop setup (specification 35, §0.1a and B1.2 to B1.3): what the setup page sends and
//! receives, in `camelCase` as the extension shows them. A request refuses unknown fields. A draft travels as a JSON value, and
//! `save::parse_setup_draft` reads it, so that no tagged enum is decoded through `serde_json` directly.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::discovery::{
    BindingSelection, ControllerCompatibility, DiscoveredController, EstimatorModelFile, Rejection,
    SelectableSignal,
};
use super::form::SetupFormModel;

/// A request to discover the loops of a project (B1.2), or to select one of them (B1.3).
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetupDiscoverRequest {
    /// The project's folder.
    pub project_root: String,
    /// The runtime the loops must belong to.
    pub runtime_id: String,
    /// The language tool's executable, when it is not beside the engine or on the PATH (§0.1a).
    pub language_tool: Option<String>,
    /// The binding the engineer chose on the three-page panel: the loop, its controller and its channels. The engine checks it and
    /// answers the loop's file; absent for a listing (B1.3).
    pub selection: Option<BindingSelection>,
    /// The controller family the engineer chose with the binding, which the selected loop's adapter must realize (C0).
    pub family: Option<String>,
}

/// The answer to a discovery: the form's model, the discovered loops and the refusals (B1.2, B1.3).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetupDiscoverResult {
    /// What the compiled program shows for a new loop.
    pub form_model: SetupFormModel,
    /// The loops a selector can offer.
    pub loops: Vec<LoopSummary>,
    /// The loop files that are refused, and why.
    pub rejected: Vec<Rejection>,
    /// The file of the loop the request's selection names, when the engine accepts the selection (B1.3); absent otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_file: Option<String>,
    /// The model files an LQG estimator can implement (C4 Setup): the project folder's and `models/`, each with its state count.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub estimator_models: Vec<EstimatorModelFile>,
}

/// One discovered loop, as the setup page lists it (B1.2).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoopSummary {
    /// The loop's id.
    pub loop_id: String,
    /// The loop's name.
    pub name: String,
    /// The file the loop was read from.
    pub configuration_path: String,
    /// The controller the loop runs in the compiled program.
    pub controller: DiscoveredController,
    /// The offered inputs, in declared order.
    pub inputs: Vec<SelectableSignal>,
    /// The offered outputs, in declared order.
    pub outputs: Vec<SelectableSignal>,
    /// Whether the loop's adapter can realize it.
    pub controller_compatibility: ControllerCompatibility,
}

/// A request to save a confirmed setup draft (B1.3, §0.1b).
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetupSaveRequest {
    /// The project's folder.
    pub project_root: String,
    /// The runtime the new loops are bound to.
    pub runtime_id: String,
    /// The language tool's executable, when it is not beside the engine or on the PATH (§0.1a).
    pub language_tool: Option<String>,
    /// The confirmed draft, as the setup form writes it; `save::parse_setup_draft` reads it.
    pub draft: Value,
}

/// One loop the save wrote (B1.3).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SavedLoop {
    /// The loop's id.
    pub id: String,
    /// The loop's name.
    pub name: String,
    /// The file it was written to, relative to the project root, with forward slashes.
    pub file: String,
}

/// The answer to a save: the loops written, in the order the setup writes them (B1.3, §0.1b).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetupSaveResult {
    /// The loops written.
    pub loops: Vec<SavedLoop>,
}

#[cfg(test)]
#[path = "messages_tests.rs"]
mod tests;
