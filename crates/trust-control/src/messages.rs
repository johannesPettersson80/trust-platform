//! The messages between the Control Studio panel and the language server (specification 35,
//! §0.1a): the arguments and results of the `trust-lsp.controlStudio…` commands and the
//! `trust/controlStudio/update` notification. The extension's `TypeScript` types are generated from
//! these definitions, never written by hand.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::analysis::parameter_writes::WrittenValue;
use crate::commissioning::application_offers::PrimaryAction;
use crate::commissioning::application_records::{
    ApplicationReview, RestoreOutcome, ReviewConfirmation,
};
use crate::commissioning::parameter_status::ParameterStatus;
use crate::failure::Failure;
use crate::identification::experiment_limits::ExperimentLimits;
use crate::identification::method_offers::IdentifyMethodOffer;
use crate::identification::project_test::ProjectTestForm;
use crate::loop_definition::{ChannelChoices, ChannelSelection, LoopDefinition};
use crate::runtime::RuntimeConnection;
use crate::stages::{StageAvailability, StageEvidence};
use crate::study::inputs::{InputsSection, Invalidation};
use crate::study::Page;
use crate::tuning::search::SearchCandidate;
use crate::tuning::state_controller::preferences::FormField;
use crate::validation::scenario_contract::EngineerScenario;

mod analyze;
mod analyze_declared;
mod answer_kind;
mod held_inputs;
mod identify;
mod live_sides;
mod open_offers;
mod parameter_tool;
mod plot_signals;
mod requirement_outputs;
mod tune;
mod tune_words;
mod validate_answer;
mod validate_initialization;

pub use analyze::{
    AnalysisEnvelope, AnalysisInputs, AnalysisPage, AnalysisResult, AnalysisTool, AnalyzeAnswer,
    AnalyzeRequest, ChannelRef, FrequencyAnswer, FrequencyChannel, FrequencyGrid, FrequencyPoint,
    HeldAnswer, InitialStateAnswer, LeadLagAnswer, ModelRevision, NamedMatrix, PoleDomain,
    PoleZeroAnswer, PoleZeroView, RelativeGainAnswer, RelativeGainStatus, StateStructureAnswer,
    ToolOffer, TransferPolynomials,
};
pub use analyze_declared::{FeedbackAnswer, MeasuredFrequencyAnswer, RootLocusAnswer, RouthAnswer};
pub use answer_kind::{AnswerKind, NamedAnswer};
pub use held_inputs::HeldInputs;
pub use identify::{
    CampaignRestoration, IdentifyAction, IdentifyPlot, IdentifyRequest, IdentifyResult,
    ImportedRecording, ManualAction, ManualIdentificationAnswer, ManualModelChoice, ManualStage,
    ManualStatus, PlotRole, PlotTrace, ProjectTestResult, RecordedChartSample, ResetResult,
};
pub use live_sides::{live_sides, LiveSides, SideSignal};

pub use open_offers::{
    controller_offer, default_family, scenario_row_signal, scenario_template, setpoint_choices,
    ControllerOffer, FamilyOffer, SetpointChoice, SettingRow,
};
pub use parameter_tool::{
    first_tool_offer, parameter_tool_offer, write_outcome, ParameterToolOffer, ParameterToolStep,
    ParameterWriteStatus,
};
pub use plot_signals::{plot_signals, recorded_values, PlotSignal, PlotSignalKind};
pub use requirement_outputs::{requirement_outputs, RequirementOutput};
pub use tune::{
    CandidateOutcome, ComparisonEntry, FeedforwardAnswer, FeedforwardPath, PreviewRequest,
    ScheduleAnswer, ScheduleAnswerPoint, ScheduleAnswerVariable, ScheduleChoiceWord, TuneChoice,
    TuneGainUnits, TuneMethod, TuneRequest, TuneResponse, TuneResult, TuneRole, TuneTrace,
};
pub use tune_words::{GainLabels, SettingLabel, TuneWords};
pub use validate_answer::{
    LimitSide, ModelCheck, PlotSeries, RequirementChannel, RequirementOutcome, RunOutcome,
    ScenarioOutcome, ValidateLabels, ValidateResult, ValidateRow, ValidationPlot, ValueRange,
};
pub use validate_initialization::InitializationOutcome;

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

/// The result of `controlStudio/open`.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
#[allow(clippy::large_enum_variant)] // One answer per call, built once and serialized at once: boxing would buy nothing.
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
        /// The status of every stage. While an Apply's checkpoint waits for its Restore, the stages that write or change the study
        /// cannot start and name the block's reason (B6.4, B6.7), so a reopened loop holds the block.
        stages: Vec<StageAvailability>,
        /// The journaled operations of the loop that reopening reattaches to (A8.5, A8.6): the recordings, experiments, campaigns and
        /// isolated jobs that the journal holds unfinished. A checkpoint is not an operation, so it is not listed (B6.4).
        reattached: Vec<String>,
        /// The operations this engine runs for the loop now, which the page follows: their updates reach it (A8.6).
        running: Vec<String>,
        /// The last Validate result while it is current for the study's model, candidate, requirements and scenarios (A8.4, A8.6): its
        /// table and whether Apply is available (B5.4). Absent otherwise. The study keeps no plot and no plant reason, so the result has none.
        #[serde(skip_serializing_if = "Option::is_none")]
        validation: Option<ValidateResult>,
        /// The stage evidence of the opened study, which the page gates its actions on (B9.4, A8.6): whether a model, a candidate
        /// and a passed validation exist now. It is the evidence `stages` is computed from, as `controlStudio/run` answers it.
        stage_evidence: StageEvidence,
        /// The study's current identification, as Identify answers it (A8.6): its readable result, plot, model revision and
        /// requirements. Absent without a current model, and after Reset.
        #[serde(skip_serializing_if = "Option::is_none")]
        identification: Option<IdentifyResult>,
        /// The Recorded-data fit the study keeps, as its answers state it (B3.3, A8.6): the saved stage replayed from the kept
        /// recordings, or the engine's default form for a study that keeps none. A saved stage that cannot be replayed is answered with
        /// the sentence that says why. The page shows it when the Recorded-data fit is the method it shows; it decides no replay.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        manual_identification: Option<ManualIdentificationAnswer>,
        /// The study's current candidate, as Tune answers it (B4.5, A8.6), rebuilt from the study's Tune section: its settings,
        /// method, reason, attempts and requirements. The study keeps no prediction, so `response` is absent, the outcome is
        /// unpredicted and no margins or comparison are given; a new prediction is not run on open. Absent without a current
        /// candidate (A8.4).
        #[serde(skip_serializing_if = "Option::is_none")]
        tune: Option<TuneResult>,
        /// The study's current finite-search candidate (C5, A8.6), rebuilt from the study's Tune section: its values, its trials and the
        /// unit of J. Given under the rule of `tune` (a current candidate, A8.4), and for a search law only; absent otherwise.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        search_candidate: Option<SearchCandidate>,
        /// The tuning methods the study offers the Tune page, in its order (C4, N5.R): the loop's named methods, and the relay rule beside
        /// Manual when the study holds a qualified Feedback relay record. Absent when the loop has no method list.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        methods: Option<Vec<TuneMethod>>,
        /// The title of each method of `methods`, keyed by the method's word (`SIMC`, `AMIGO`, `relay`, `manual`, `state-space`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        method_titles: Option<BTreeMap<String, String>>,
        /// The page last viewed, which reopening shows (A8.1, A8.6); Identify for a new study.
        page: Page,
        /// The analysis tool the Tune page showed last, with its inputs, which reopening shows again (B9.9, A8.6); absent when none
        /// was shown.
        #[serde(skip_serializing_if = "Option::is_none")]
        page_analysis: Option<AnalysisPage>,
        /// The page's own view choices the study keeps, which reopening shows again (A8.1, A8.6); absent when none were sent.
        #[serde(skip_serializing_if = "Option::is_none")]
        page_view: Option<PageView>,
        /// The analysis records the study keeps that are current for its loop and model (B10.2, A8.4).
        analysis: crate::study::analysis_records::AnalysisRecords,
        /// The inputs the study records, which the editors show again (A8.1, A8.6); empty when none was recorded.
        inputs: InputsSection,
        /// Whether Apply is offered (B5.4, B7, A8.6), by the one rule of every answer that states it
        /// (`application_offers::apply_offered`): the study's validation passed for the current candidate, no Apply's checkpoint is
        /// unresolved, and the newest Apply applied another validation, whether its settings stand, were restored or were retired.
        /// The page shows the statement.
        apply_available: bool,
        /// Whether Restore previous settings is offered from the journaled checkpoint of the loop's Apply (A8.6, B6.4): a reopened
        /// loop offers it, and the runtime confirms the review when the engineer asks for it.
        restore_available: bool,
        /// Whether Keep in project is offered (A8.6, B6.5): an Apply's settings stand, and the study's Apply receipts allow it, the same
        /// receipts `controlStudio/keep` reads.
        keep_available: bool,
        /// Whether a journaled Apply observation is left to follow to its end, with `controlStudio/apply` and `resume` (A8.5, A8.6): the
        /// journal holds the observation while the runtime may still record it, and no operation of this engine follows it now.
        apply_observation_resumable: bool,
        /// The action that holds the Validate page's position (B7): Apply while it is offered, then Keep in project once it is offered,
        /// else Restore previous settings; absent when none is offered (`application_offers::primary_action`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        primary_action: Option<PrimaryAction>,
        /// The design-form fields of a state law, with their labels, under the solver policy the stored draft chooses (D-B12): the
        /// page shows them before the first Tune. Absent for a loop without a state law.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        state_fields: Option<Vec<FormField>>,
        /// The rows of the Channels editor (A8.1, the Tune page's **Channels…**): each model channel's plant signal as the study
        /// selects it, and the signals it may take. Absent for a loop with one input and one output.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        channel_choices: Option<ChannelChoices>,
        /// Whether the measurement noise band is required (B4.3): the loop declares a low-pass or a notch filter, which the
        /// requirements check against the noise. The requirements page shows the band's field when this is true.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        measurement_noise_band_required: bool,
        /// Whether the loop declares a project-scripted experiment, so that the page offers the project test (C12): the project owns
        /// the excitation. The engine's rule for the project test (`identify_manual::is_project_test`) reads the same declaration.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        project_test: bool,
        /// The identification methods the Identify page's Method list offers, in its order, each with its label, whether the loop can run
        /// it and the sentence that says why not (B3.2, B3.3). The page builds the list from them.
        identify_methods: Vec<IdentifyMethodOffer>,
        /// The limits of the identification experiment as the Limits editor shows them (B3.2, A8.6): every plant input and measured
        /// output with its range, and the maximum duration. They are the loop's declared defaults with the study's own record over
        /// them, so the page merges nothing; the amplitude, the relay direction and the review's duration are the declared ones.
        experiment_limits: ExperimentLimits,
        /// What the study's recorded inputs mean for the page's editors (A8.6): the Method list's entry that holds the study's method,
        /// and the controller family and pinned settings that apply to the tuning method the study holds. The page decodes no record.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        held_inputs: Option<HeldInputs>,
        /// The form of the project test of a project-scripted loop (B6.1, C12): the signals it offers, its duration and baseline, and the
        /// parameter tool the page shows. Absent for a loop Control Studio runs.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project_test_form: Option<ProjectTestForm>,
        /// The controller the Tune page offers the loop, with the rows of its manual editor (C2, C4, B4.2): its families and their labels,
        /// the family the page shows by default and the settings each family takes. Absent when the engine adapts no controller of the loop.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        controller_offer: Option<ControllerOffer>,
        /// The setpoints that a loop without a declared setpoint may take (B3.6); absent when the loop declares its setpoint.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        setpoint_choices: Option<Vec<SetpointChoice>>,
        /// The scenario that the page adds to its list (B4.3): the engine's template, which has no name. The engine names it when the
        /// list changes, with the first identity that no other scenario of the list holds.
        scenario_template: EngineerScenario,
        /// The signal that a new initial value or timed change of a scenario starts at (B4.3): the loop's setpoint. Absent when the loop
        /// declares none, which leaves the new row's signal blank for the engine to refuse.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scenario_row_signal: Option<String>,
        /// The text the design form shows for the loop's declared design when the study holds no draft (C4): each declared matrix or pole
        /// list as JSON. Empty for a loop without a state law.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        state_declared_text: BTreeMap<String, String>,
        /// The live sides of the loop (B3.6): the plant signals each live selector offers, the loop's channels in channel order, or its
        /// manipulated and process variables when it declares no channels. The page offers each side from this answer.
        live_sides: LiveSides,
        /// The controlled outputs of the Requirements editor (B4.3, C7), in channel order: the outputs that declare a setpoint, or every
        /// output when none does, each with the channel its rows are named by and the requirements they start from. Empty for a loop
        /// without channels, whose requirements are the record's own. The page lists the outputs from this answer.
        requirement_outputs: Vec<RequirementOutput>,
    },
}

/// The arguments of `controlStudio/page` (specification 35, §0.1a; A8.1): the loop, and the page the engineer views. It
/// changes nothing else and starts nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PageRequest {
    /// The project folder.
    pub project_root: String,
    /// The runtime selected in VS Code; a loop of another runtime is refused, as `open` refuses it.
    pub runtime_id: String,
    /// The loop of the study.
    pub loop_id: String,
    /// The page the engineer views now.
    pub page: Page,
    /// On Tune, the analysis tool shown and its inputs (B9.9); absent when none is shown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis: Option<AnalysisPage>,
    /// The page's own view choices (A8.1, A8.6); absent when the page sends none, which keeps the recorded ones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<PageView>,
}

/// The page's own view choices, which the engine keeps with the page and answers on `open`, and never reads (§0.1a,
/// `controlStudio/page`; A8.1, A8.6). They are view state: what decides a result is a study input (`controlStudio/inputs`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PageView {
    /// The entry the Identify page's Method shows, as the page names it: a live method, or the opened-file or supplied-model entry
    /// (B3.6, B3.7), which start no campaign. At most 64 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identify_method: Option<String>,
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
    /// The stage a run is in (specification 35, B8): Identify, Tune or Validate. Absent for an operation that runs no stages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<Page>,
    /// What it is doing, in words.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<String>,
    /// Its result once finished, in the form its stage defines.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Its failure once failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<Failure>,
    /// The sentence the page shows for the update, as the engine words it: every cancelled update carries one, `Identification cancelled.`
    /// for an identify operation, `Analysis cancelled.` for an analysis and `The operation was cancelled.` for the others
    /// (specification 35, B3, B9.4); the page writes none of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// The status of every stage that a failed Apply or Restore leaves (specification 35, B6.4, B6.7, B9.4): one that stopped after its
    /// first write leaves its checkpoint waiting for its Restore, which holds the stages, and the page shows that block without a
    /// reopen. Absent for every other update.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stages: Option<Vec<StageAvailability>>,
}

/// The answer of a command that starts a long operation: its identifier, at once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OperationStarted {
    /// The answer's kind (§0.1a): the page shows the answer by it, never by the fields it carries.
    pub kind: AnswerKind<OperationStarted>,
    /// The operation.
    pub operation_id: String,
}

/// The arguments of `controlStudio/inputs` (specification 35, §0.1a; A8.1, A8.4): the inputs the study records, as the editors
/// change them. The study is written and nothing runs; a loop with an operation that runs refuses the change (B8), and so does a loop
/// whose Apply's checkpoint waits for its Restore (B6.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputsRequest {
    /// The project folder.
    pub project_root: String,
    /// The runtime selected in VS Code or on the command line; a loop of another runtime is refused, as `open` refuses it.
    pub runtime_id: String,
    /// The loop of the study.
    pub loop_id: String,
    /// The inputs the study records from now on, as a whole: an absent input is not yet given. The request names either these
    /// inputs or one `change`, not both.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs: Option<InputsSection>,
    /// One change of the page, with the page's keys: `acquisitionSettings`, `family`, `tuningPreferences`, `pinnedValues`, and the
    /// requirements record that `referenceChange` marks (with its `scenarioDefinitions`), or that the rows typed for each controlled
    /// output (`outputRequirements`) mark: the engine assembles the record from them (B4.3). The engine merges it into the study's
    /// inputs (A8.1, A8.4) and answers the inputs the study then holds. The channel mapping and the binding changes name no input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change: Option<serde_json::Map<String, Value>>,
    /// The channel selection of a multichannel loop (the Tune page's **Channels…**, A8.1): the plant signal of every declared channel.
    /// Absent leaves the study's selection as it is; a present one is checked against the loop's declared channels and replaces it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_selection: Option<ChannelSelection>,
    /// The folder of the operation journals (A8.3), where the engine reads whether an Apply's checkpoint waits for its Restore, which
    /// refuses the change (B6.4). Absent or empty, the engine reads its own journal folder, the one the command line uses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_folder: Option<String>,
}

/// What `controlStudio/inputs` answers at once (A8.4): the status of every stage after the change, and what the change invalidated.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InputsResult {
    /// The status of every stage after the change.
    pub stages: Vec<StageAvailability>,
    /// The stage evidence after the change, from which `stages` is computed. Its `validationHasResult` says whether a Validate result
    /// is still current, so the page hides the table of a result the change left not current (A8.4).
    pub stage_evidence: StageEvidence,
    /// Whether Apply is offered after the change (B5.4, B7, A8.4), by the one rule of every answer that states it
    /// (`application_offers::apply_offered`): a change that leaves the validation not current withdraws it.
    #[serde(default)]
    pub apply_available: bool,
    /// The action that holds the Validate page's position after the change (B7; `application_offers::primary_action`); absent when
    /// none is offered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_action: Option<PrimaryAction>,
    /// What the change invalidated (A8.4); the results stay in the study as history.
    pub invalidated: Invalidation,
    /// The inputs the study holds after the answer's change (A8.1): the merged inputs of a change, or the inputs the request gave.
    pub inputs: InputsSection,
    /// Whether the change altered the inputs the study held before it (A8.1). The study is written only when it did.
    pub changed: bool,
    /// The design-form fields of a state law, with their labels, under the solver policy of the inputs it now records (D-B12): the
    /// page shows them before the first Tune. Absent for a loop without a state law.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_fields: Option<Vec<FormField>>,
    /// The rows of the Channels editor after the change (A8.1): the plant signal each channel now has. Absent for a loop with one
    /// input and one output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_choices: Option<ChannelChoices>,
    /// The tools whose recorded analysis is no longer current for the study's model after the change (B9.9, A8.4): the engine records
    /// the analysis it answers with the revision of the model it used, and names it here when that model is not the study's any more.
    /// The page drops exactly the answers listed and asks the engine nothing. Absent when none is stale.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stale: Vec<AnalysisTool>,
}

/// The arguments of `controlStudio/monitor` (specification 35, §0.1a; B2 and A3.4): the loop the page shows, the runtime it is
/// observed on, and the debug session the runtime is attached to. The engine reads the runtime and decides the sample. The
/// connection's token is a credential: it is never journaled (E11.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MonitorRequest {
    /// The project folder.
    pub project_root: String,
    /// The runtime the loop is bound to; a connection to another runtime is refused before the loop is read.
    pub runtime_id: String,
    /// The loop observed.
    pub loop_id: String,
    /// The editor session of the panel; one panel observes one loop, and a new panel starts a new observation.
    pub editor_session_id: String,
    /// The connection of the running runtime; absent when no runtime is running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection: Option<RuntimeConnection>,
    /// The debug session the runtime is attached to; a new session starts a new observation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_session_id: Option<String>,
}

/// What `controlStudio/monitor` answers (B2, B9.2): the loop's sample, and the target's label when it changed since this panel's
/// last answer.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MonitorResult {
    /// The loop's sample: available from one coherent scan, or unavailable with the reason.
    pub sample: crate::loop_monitor::LoopMonitorSample,
    /// The selected target's label ("Simulation ×10", "Physical") when it changed; absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_label: Option<String>,
}

/// The stage a run goes through to, at most (specification 35, B8): Identify, Tune or Validate. A run never applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum RunThrough {
    /// Identify the plant, and stop on Identify.
    Identify,
    /// Identify where needed, then tune the controller, and stop on Tune.
    Tune,
    /// Identify, tune and validate where needed, and stop at Apply.
    Validate,
}

/// The arguments of `controlStudio/run` (specification 35, §0.1a; B8): the loop, the runtime and the stage the run goes through to.
/// The connection's token is a credential: it is never journaled (E11.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunRequest {
    /// The project folder.
    pub project_root: String,
    /// The runtime selected in VS Code or on the command line.
    pub runtime_id: String,
    /// The loop of the study.
    pub loop_id: String,
    /// The folder of the operation journals (A8.3).
    pub storage_folder: String,
    /// Where the runtime listens, and its control token (E11.1); a run that identifies or validates needs it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection: Option<RuntimeConnection>,
    /// The stage the run goes through to (B8).
    pub through: RunThrough,
    /// Whether Identify runs its campaign even when the study holds a current model: Identify plant does (specification 35,
    /// §0.1a; the complete workflow specification). Absent or false, a current model is reused.
    #[serde(default)]
    pub identify_again: bool,
}

/// An input the engineer must give before a run can go on (B8): the page opens the editor that owns it, and Done runs the
/// request again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum RunNeeds {
    /// The settings of the manual method (B4.2, B4.5).
    ManualSettings,
    /// A requirement that Automatic cannot fill, or that the engineer must correct (B4.3).
    Requirements,
    /// A limit that Identify needs (B3.2, B3.4 step 1).
    Limits,
}

/// The final result of `controlStudio/run` (specification 35, B8): the status of every stage after the run, and the input the
/// run waits for, when a stage needs one. A run that stopped on a failure answers the failure instead, with every result kept.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunResult {
    /// The status of every stage after the run.
    pub stages: Vec<StageAvailability>,
    /// The stage evidence after the run: whether a model, a candidate and a passed validation exist now. The page gates its
    /// actions on it (B9.4).
    pub stage_evidence: StageEvidence,
    /// The identification the run made, as Identify answers it (B3.8); absent when the run did not identify, because a current
    /// model was reused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identification: Option<Value>,
    /// The input the engineer must give (B8); absent when the run went through to its stage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub needs: Option<RunNeeds>,
    /// The sentence that asks for the input in `needs`: the engine's own wording of what is missing (B8, B3.2). The page shows it and
    /// writes none of its own. Absent where `needs` is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs_message: Option<String>,
    /// The form field that `needsMessage` concerns, when it concerns one: the Limits editor shows the sentence under that field
    /// (`durationSeconds` for a missing maximum duration, B3.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs_field: Option<String>,
}

/// The result of the command line's `cancel` (specification 35, §0.1b): the journaled operations it cancelled, by their
/// runtime identifiers. It is an empty list when nothing was journaled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CancelResult {
    /// The identifiers of the operations that were cancelled.
    pub cancelled: Vec<String>,
}

/// The arguments of `controlStudio/validate` (specification 35, §0.1a; B5): the loop, the runtime and its connection. Validate
/// reaches the runtime for its isolated simulations (E8.1), so it is refused without a connection (Connect runtime), as
/// `identify` is. The connection's token is a credential: it is never journaled (E11.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValidateRequest {
    /// The project folder.
    pub project_root: String,
    /// The runtime selected in VS Code or on the command line; a loop of another runtime is refused, as `identify` refuses it.
    pub runtime_id: String,
    /// The loop of the study.
    pub loop_id: String,
    /// The folder of the operation journals (A8.3).
    pub storage_folder: String,
    /// Where the runtime listens, and its control token (E11.1); absent when no runtime is connected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection: Option<RuntimeConnection>,
}

/// The arguments of `controlStudio/apply` and `controlStudio/restore` (specification 35, §0.1b, B6.1 to B6.4). Without `review`
/// the method answers the review of the change and writes nothing; with the identity of that review it runs the reviewed change
/// as an operation, which refuses to run when the runtime no longer matches the review (§0.1b). A physical target is applied
/// only when `physical_approved` is set by the reviewed approval (A2.5). The connection's token is a credential, never journaled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplicationRequest {
    /// The project folder.
    pub project_root: String,
    /// The runtime selected in VS Code or on the command line.
    pub runtime_id: String,
    /// The loop of the study.
    pub loop_id: String,
    /// The folder of the operation journals (A8.3).
    pub storage_folder: String,
    /// Where the runtime listens, and its control token (E11.1); the review and the change both reach the runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection: Option<RuntimeConnection>,
    /// The identity of the review that the confirmation approves (§0.1b); absent to ask for the review.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<String>,
    /// Whether the reviewed approval of a physical target was given (A2.5).
    #[serde(default)]
    pub physical_approved: bool,
    /// Follows the journaled Apply observation to its end (A8.5, A8.6) instead of reviewing or confirming: `apply` only.
    #[serde(default)]
    pub resume: bool,
}

/// The arguments of `controlStudio/keep` (specification 35, §0.1b, B6.5): the arguments of Apply, and the files that have unsaved
/// changes in an editor, which a Keep must not change, and the language tool that answers the project's command. Keep writes the
/// project and never the controller, so `physical_approved` is accepted for the shape the three methods share and has no effect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeepRequest {
    /// The project folder.
    pub project_root: String,
    /// The runtime selected in VS Code or on the command line.
    pub runtime_id: String,
    /// The loop whose parameter record is kept.
    pub loop_id: String,
    /// The folder of the operation journals (A8.3).
    pub storage_folder: String,
    /// Where the runtime listens, and its control token (E11.1): the running values are read from it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection: Option<RuntimeConnection>,
    /// The identity of the review that the confirmation approves (§0.1b); absent to ask for the review.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<String>,
    /// Accepted for the shape the three methods share; Keep writes no controller (see the type's description).
    #[serde(default)]
    pub physical_approved: bool,
    /// The project files that have unsaved changes in an editor, project-relative or absolute (B6.5).
    #[serde(default)]
    pub unsaved_files: Vec<String>,
    /// The `trust-lsp` executable that answers the project's command; absent, the one beside the engine, then the one on the PATH.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language_tool: Option<String>,
}

/// One file a Keep changes, as its review shows it (B6.5): its path, the text it holds now (`None` when the Keep creates it) and
/// the text the Keep gives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KeepFileChange {
    /// The file's path.
    pub path: String,
    /// The text the file holds now, or `None` when it does not exist yet.
    pub before: Option<String>,
    /// The text the file holds once the Keep is written.
    pub after: String,
}

/// What `controlStudio/keep` answers without a review identity (§0.1b, B6.5): the record, the values to keep, the fields it writes,
/// the fields the equipment has no setting for, the files it changes, and the identity that a confirmation gives back.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KeepReviewAnswer {
    /// The answer's kind (§0.1a): the page shows the answer by it, never by the fields it carries.
    pub kind: AnswerKind<KeepReviewAnswer>,
    /// The record, for example `Main.TIC101_Parameters`.
    pub record: String,
    /// The values to keep, by field.
    pub values: BTreeMap<String, f64>,
    /// The fields the Keep writes.
    pub kept: Vec<String>,
    /// The fields that the equipment has no setting for (a record Create PLC code declared).
    pub unknown: Vec<String>,
    /// The files the Keep changes.
    pub files: Vec<KeepFileChange>,
    /// The sentence that describes what the Keep writes (B6.5, A11.1), as the Apply and Restore reviews describe theirs: the page shows
    /// it as the review's heading.
    pub description: String,
    /// The identity of this review.
    pub identity: String,
    /// How the review is confirmed (B6.5, B9.3): always `explicit`, with its own button on any target, because the Keep edits the
    /// project's files and never writes the controller, so it asks for no approval and is not confirmed by a press.
    pub confirmation: ReviewConfirmation,
}

/// The final result of a confirmed Keep (B6.5): the record, the fields kept, the fields without a setting, and the sentence that
/// says what was kept, or that the project already had the values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KeepResult {
    /// The answer's kind (§0.1a): the page shows the answer by it, never by the fields it carries.
    pub kind: AnswerKind<KeepResult>,
    /// Whether Restore previous settings is still offered once the Keep's receipt is kept (B7: the settings still stand).
    pub restore_available: bool,
    /// Whether Keep in project is still offered once the Keep's receipt is kept: not for the values it kept (B6.5).
    pub keep_available: bool,
    /// The action that holds the Validate page's position once the Keep's receipt is kept (B7; `application_offers::primary_action`),
    /// with Apply as the engine offers it now; absent when none is offered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_action: Option<PrimaryAction>,
    /// The record.
    pub record: String,
    /// The fields the Keep wrote.
    pub kept: Vec<String>,
    /// The fields that the equipment has no setting for.
    pub unknown: Vec<String>,
    /// What was kept, in words.
    pub reason: String,
}

/// The arguments of `controlStudio/parameters` (specification 35, C12 and §0.1a; annex E7): the writes of the parameters that the
/// loop's tool lets a write set: those a declared analysis maps, or the settings of the experiment. `values` holds the symbols with
/// the values a review or a confirmed Apply sets; every other symbol of the record keeps its value. `review` is the identity that
/// `confirm` names (A2.5, E11.8). The connection's token is a credential, never journaled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParameterWorkRequest {
    /// The project folder.
    pub project_root: String,
    /// The runtime selected in VS Code or on the command line.
    pub runtime_id: String,
    /// The loop whose tool writes the parameters: its declared analysis maps them, or it declares the experiment's settings.
    pub loop_id: String,
    /// The folder of the operation journals (A8.3).
    pub storage_folder: String,
    /// What the request does.
    pub action: ParameterWorkAction,
    /// The values to write, by symbol (`review`), and the values an Apply's review read, which `confirm` sends back with its identity.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, f64>,
    /// The values of a `review` as the engineer typed them: a JSON object of numbers by symbol, at most 64 kB. The engine reads
    /// the text and refuses one that is none, with its own sentence, and `values` is then not sent. The review answers the values it
    /// read, which `confirm` sends back (C12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json: Option<String>,
    /// The identity of the review that `confirm` names (A2.5); absent to ask for the review.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<String>,
    /// Where the runtime listens, and its control token (E11.1); the record is read and written through it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection: Option<RuntimeConnection>,
    /// Whether the reviewed approval of a physical target was given (A2.5).
    #[serde(default)]
    pub physical_approved: bool,
}

/// What `controlStudio/parameters` does (C12). `draft` answers the values the declared analysis selects; `inspect` answers the mapped
/// values the runtime holds; `review` and `reviewRestore` answer the review of an Apply or of a Restore, which writes nothing; `confirm`
/// starts the write of the review it names as an operation; `released` answers whether a project test left the settings released.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ParameterWorkAction {
    /// The values that the declared analysis selects.
    Draft,
    /// The mapped values that the runtime holds now.
    Inspect,
    /// The review of an Apply of the values.
    Review,
    /// The review of a Restore of the values that the runtime retains from before the last write.
    ReviewRestore,
    /// The write of the review that `review` names, started as an operation: the engine applies the `values` that an Apply's review
    /// read, or restores the values that the runtime retains, whichever review the identity names now; when it names neither, the
    /// record or the loop changed since the review, and the write is refused (E11.8).
    Confirm,
    /// The page edited the values after a review: the review is spent, nothing is written, and the answer is the sentence the page
    /// shows (C12).
    Invalidate,
    /// Whether a project test left the loop's settings released (C12): the answer carries `settingsReleased` once the test's terminal
    /// restoration is confirmed, and otherwise the reason in `message`. It reads no runtime and writes nothing.
    Released,
}

/// What `controlStudio/parameters` answers (C12). Each action gives the fields it needs: `draft` gives `values`; `inspect` gives
/// `values` and `observed`; `review` and `reviewRestore` give `identity`, `changes` and `physical`; `released` gives
/// `settingsReleased` or the reason in `message`. `confirm` answers an `operationId`, and the operation's result is the write's
/// outcome: `status` with its sentence in `message`, and `values` and `observed`, the record the runtime confirmed, when it applied.
/// `inspect`, `review`, `reviewRestore`, `invalidate`, `released` and the write's outcome give the tool's `offer`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ParameterWorkAnswer {
    /// The mapped values, by symbol (`draft`, `inspect`), and the values a `review` read from its `json` or its `values`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<BTreeMap<String, f64>>,
    /// The record that the runtime reported (`inspect`, and the result of a confirmed write).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed: Option<ParameterStatus>,
    /// The identity of the review, which the start names (`review`, `reviewRestore`; A2.5, E11.8).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<String>,
    /// The values that the review changes, with the value in force and the value written.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<WrittenValue>,
    /// Whether the target is physical, so that the write needs the engineer's approval (A2.5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physical: Option<bool>,
    /// The parameters the page shows (`draft`, `inspect`): those the loop's tool may write, with the bounds it allows (C12). A declared
    /// analysis's are only the mapped ones, with the bounds the analysis allows; the experiment's settings are the loop's whole binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<crate::loop_definition::ParameterBinding>,
    /// The sentence the page shows with the answer: the copy note of a `draft`, what an `inspect` or a review answered and what the
    /// engineer does next, the words of an `invalidate`, the sentence of the write's outcome, and the reason a `released` question is
    /// answered without a release (C12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// How the write ended (the result of `confirm`, C12, E7.4): `applied` once the runtime reported the written values after the task
    /// executed, else `failed`; `message` says which, in the engine's words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<ParameterWriteStatus>,
    /// What the tool offers after this answer (C12): the step its own button runs next and the actions the engine takes now, from the
    /// loop's claim and journal and the review the answer gives, also after a project test's end (`released`). The page renders it and
    /// keeps no phase of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offer: Option<ParameterToolOffer>,
    /// The sentence that says a project test left the loop's settings released (`released`, C12): present exactly when no operation of
    /// the engine works on the loop and its journal holds no unfinished operation, that is, once the test's terminal restoration is
    /// confirmed or none was needed. The page shows it with the parameter tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings_released: Option<String>,
}

/// What `controlStudio/apply`, `controlStudio/restore` and `controlStudio/keep` answer at once (§0.1b): the review of the change, which
/// writes nothing, the started operation, whose result follows through `controlStudio/update`, or the end of a Restore on a runtime
/// that was replaced since Apply (B6.4). It is written untagged, so a review is its own object, a started operation is the answer
/// that names an operation (`OperationStarted`) and a Restore's end is its outcome, each with its own `kind` (§0.1a); it is never decoded.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum ApplicationAnswer {
    /// The review of the change, answered when the request carried no review identity.
    Review(Box<ApplicationReview>),
    /// The operation that runs the reviewed change.
    Started(OperationStarted),
    /// The end of a Restore on a runtime that was restarted or whose program was reloaded since Apply (B6.4, B6.7), answered at once
    /// with or without a review identity: nothing of the checkpoint can be restored, so the checkpoint is kept as history, nothing is
    /// written to the runtime, and no operation starts. The outcome says so (`restored` is false), with the stages that follow.
    Retired(Box<RestoreOutcome>),
}
