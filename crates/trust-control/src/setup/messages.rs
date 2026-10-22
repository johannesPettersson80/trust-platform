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
use super::mapping_match::{MappingAnswer, MappingRequest};
use super::prompts::ChoiceItem;
use crate::failure::Failure;
use crate::loop_definition::LoopDefinition;

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
    /// The plant signals that the binding selectors name, with the loop the panel shows (B1.2, B3.6): the engine answers the mapping as
    /// `mapping`. Absent for a listing and for a selection.
    #[serde(default)]
    pub mapping: Option<MappingRequest>,
    /// The loop the panel shows (B1.2, B3.6). A request that names it and no `mapping` is answered with the choices of that loop's own
    /// mapping as `mapping`: the other loops that bind the same plant signals. Absent for a listing without a panel.
    #[serde(default)]
    pub loop_id: Option<String>,
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
    /// The open request of the loop the selection names (B1.3, §0.1a), when the engine accepts the selection; absent otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_request: Option<ConfiguredLoopRequest>,
    /// The engine's answer to the request's mapping (B1.2, B3.6); absent when the request names none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mapping: Option<MappingAnswer>,
}

/// The project and runtime of a configured loop, as its open request names them (B1.3).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoopRequestScope {
    /// The project's identifier: `project:` and the SHA-256 digest of the project's folder address.
    pub project_id: String,
    /// The runtime the loop is bound to.
    pub runtime_id: String,
}

/// A configured loop as the page's open request names it (B1.3, §0.1a). The engine checked the loop and builds the request, so the
/// page opens the loop from the answer and never reads the loop file.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfiguredLoopRequest {
    /// The request's version, 1.
    pub schema_version: u32,
    /// Where the loop comes from: `ConfiguredLoop`, a loop file of the project.
    pub source: String,
    /// The project and runtime of the loop.
    pub scope: LoopRequestScope,
    /// The loop, as the engine checked it.
    pub loop_definition: LoopDefinition,
}

impl ConfiguredLoopRequest {
    /// The open request of a loop the engine checked.
    #[must_use]
    pub fn of(loop_definition: LoopDefinition) -> Self {
        Self {
            schema_version: 1,
            source: "ConfiguredLoop".to_owned(),
            scope: LoopRequestScope {
                project_id: loop_definition.project_id.clone(),
                runtime_id: loop_definition.runtime_id.clone(),
            },
            loop_definition,
        }
    }
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
    /// The open request of the first loop written: the page opens Control Studio on it after a save (B1.3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_request: Option<ConfiguredLoopRequest>,
}

/// A request for the loop files of the workspace folders (`controlStudio/projectLoops`): which loops the folders configure for the
/// runtime, which folder declares a loop, and which loops belong to an equipment (A8.3, B1.3, §0.1a).
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectLoopsRequest {
    /// The workspace folders, in the order the editor lists them.
    pub project_roots: Vec<String>,
    /// The runtime the loops must belong to.
    pub runtime_id: String,
    /// An equipment's instance: the answer names the accepted loops whose controller belongs to it (`equipment`).
    pub equipment: Option<String>,
    /// The loop to locate: the answer names the folder that declares it (`declaringRoot`).
    pub loop_identity: Option<LoopIdentity>,
}

/// The identity a loop file declares: its id, its project and its binding revision (A8.2).
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LoopIdentity {
    /// The loop's id.
    pub id: String,
    /// The project the loop belongs to.
    pub project_id: String,
    /// The loop definition's binding revision.
    pub revision: String,
}

/// The answer to a loop listing (`controlStudio/projectLoops`).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectLoopsResult {
    /// Each folder's loop files, in the order the request names the folders.
    pub folders: Vec<FolderLoops>,
    /// The folder that declares the requested loop: the first whose accepted loop declares its revision, else the first whose loop
    /// declares its id and project. Absent when the request names no loop or no folder declares it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declaring_root: Option<String>,
    /// The decision on the file that declares the requested loop: the accepted file with its open request, or the refused file, or the
    /// missing loop, each with its failure and next action (A8.3, §0.1a). Absent when the request names no loop.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<LoopResolution>,
    /// The loop the workspace opens at once, when the request names neither a loop nor an equipment (B1.2): the one configured loop, or the
    /// first folder's first decision when none is configured, or the sentence that asks for a project. Absent when several are configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open: Option<LoopChoice>,
    /// The configured loops the engineer chooses among, in folder order, when the request names neither a loop nor an equipment and several
    /// loops are configured (B1.2). Empty otherwise.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<LoopChoice>,
    /// The loops of the requested equipment, when the request names one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub equipment: Option<EquipmentLoops>,
}

/// One loop file of a folder, with the folder that holds it (B1.2): a choice the engineer makes or the loop the workspace opens.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoopChoice {
    /// The folder that holds the loop file; empty when no project folder is open.
    pub project_root: String,
    /// The file's decision.
    pub resolution: LoopResolution,
}

/// The loop files of one project folder, each decided (B1.2): the root file, then `control-loops/` in name order.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FolderLoops {
    /// The folder's path.
    pub project_root: String,
    /// The folder's loop files. A missing file is left out when another file is configured.
    pub resolutions: Vec<LoopResolution>,
}

/// One loop file's decision: the loop it opens, or why it does not open.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoopResolution {
    /// The loop file's path.
    pub configuration_path: String,
    /// Whether the file opens a loop, is refused, or does not exist.
    pub state: ResolutionState,
    /// Why the file is refused or missing; absent for an accepted loop.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The same refusal as a failure, with its cause and the next action the page offers (A11.2); absent for an accepted loop.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<Failure>,
    /// The open request of an accepted loop.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<ConfiguredLoopRequest>,
}

/// A loop file's state (B1.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ResolutionState {
    /// The file holds a loop the engine accepts for the runtime.
    Accepted,
    /// The file is refused: unreadable, not a loop, another runtime's, or a duplicate of another file's identifier.
    Invalid,
    /// The project has no such file.
    Missing,
}

/// The loops of one equipment (B1.3, the HMI Builder's handoff): the accepted loops whose controller belongs to it, and the reason
/// when none does.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EquipmentLoops {
    /// The open requests of the loops that belong to the equipment, in the order of the folders.
    pub requests: Vec<ConfiguredLoopRequest>,
    /// Why no loop belongs to the equipment: the first refused loop file's reason, else the sentence that asks for a configured loop.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// A request to answer the step-by-step loop setup (`controlStudio/setupStep`, B1.3, C12): the session the last answer returned, and
/// what the engineer did at the question it shows. The engine reads the compiled project as `setupDiscover` does; the page asks no question.
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetupStepRequest {
    /// The project's folder.
    pub project_root: String,
    /// The runtime the new loop is bound to.
    pub runtime_id: String,
    /// The language tool's executable, when it is not beside the engine or on the PATH (§0.1a).
    pub language_tool: Option<String>,
    /// The session the last answer returned, which the page keeps as it is; absent at the start of the setup.
    pub session: Option<Value>,
    /// The engineer's answer to the question the session shows; absent when the page asks no answer (the first question, or Back with no
    /// text to keep). The answer is wrapped, so that a null answer (no setpoint) is an answer.
    pub answer: Option<AnswerValue>,
    /// Whether the engineer pressed Back at the question the session shows.
    #[serde(default)]
    pub back: bool,
}

/// One answer as the page sends it: the text the engineer typed, or the `value` of the item the engineer chose (`ChoiceItem`).
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnswerValue {
    /// The answer as the question takes it: text for a text, number, rate or file question; the item's value for a choice.
    #[schemars(schema_with = "crate::json::any_value_schema")]
    pub value: Value,
}

/// The engine's answer to a step-by-step request (`controlStudio/setupStep`, B1.3): the question to ask next, or the finished draft and
/// its confirmation, with the session the page sends back with its next request.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetupStepResult {
    /// The session the page sends back with its next request. The page keeps it as it is and reads nothing in it.
    pub session: Value,
    /// The question to ask now; absent when every answer is in, or when the setup cannot start.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<SetupQuestion>,
    /// The answer the engine refused, in its words: the same question is asked again. Absent when the answer was accepted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    /// Whether Back is offered at the question.
    pub can_go_back: bool,
    /// The answer the engineer gave this question before, which the page shows again.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<Value>,
    /// The draft of the finished setup, which `controlStudio/setupSave` writes; absent while a question is open.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft: Option<Value>,
    /// The sentence the engineer confirms before the save, which says what the loop does; absent while a question is open.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// One question of the step-by-step setup (B1.3): its draft key, its phase and title, the kind of answer it takes, and its choices.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetupQuestion {
    /// The draft key the answer is stored under, such as `processVariable` or `controllerInstance`.
    pub key: String,
    /// The phase the question belongs to, 1 to 4.
    pub phase: u8,
    /// The name of the phase.
    pub phase_name: String,
    /// The question's title.
    pub title: String,
    /// The kind of answer the question takes.
    pub kind: QuestionKind,
    /// The sentence under the answer, when the question has one (the rate limits say a blank answer is no limit).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// The choices of a choice question, in the order they are offered; empty for the other kinds.
    pub items: Vec<ChoiceItem>,
}

/// The kind of answer a question of the step-by-step setup takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum QuestionKind {
    /// Any text that is not blank once trimmed.
    Text,
    /// A finite number that passes the question's rule.
    Number,
    /// A rate limit: a nonnegative number, or blank for no limit.
    Rate,
    /// One of the items the question offers.
    Choice,
    /// A file the engine reads (the controller adapter mapping).
    File,
}

#[cfg(test)]
#[path = "messages_tests.rs"]
mod tests;
