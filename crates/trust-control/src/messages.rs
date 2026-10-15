//! The messages between the Control Studio panel and the language server (specification 35,
//! §0.1a): the arguments and results of the `trust-lsp.controlStudio…` commands and the
//! `trust/controlStudio/update` notification. The extension's `TypeScript` types are generated from
//! these definitions, never written by hand.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use trust_control_math::margins::MarginValue;

use crate::analysis::parameter_writes::WrittenValue;
use crate::commissioning::application_records::ApplicationReview;
use crate::commissioning::parameter_status::ParameterStatus;
use crate::failure::Failure;
use crate::identification::acquisition_settings::AcquisitionSettings;
use crate::identification::data::QualifiedIdentificationData;
use crate::identification::evidence::ModelFitEvidence;
use crate::identification::form_draft::IdentificationFormDraft;
use crate::identification::plant::{IdentifiedModel, ResultSource};
use crate::identification::presentation::IdentifiedModelPresentation;
use crate::identification::recording_import::ImportCheck;
use crate::identification::sensor_lag_comparison::StructureChoice;
use crate::identification::workflow::FitRole;
use crate::loop_definition::LoopDefinition;
use crate::recording::channels::{ChannelQuality, RecordedChannel};
use crate::recording::opened_channels::RecordingSignalCatalog;
use crate::runtime::RuntimeConnection;
use crate::stages::{StageAvailability, StageEvidence};
use crate::study::inputs::{InputsSection, Invalidation};
use crate::study::Page;
use crate::tuning::candidate::ControlPidCandidate;
use crate::tuning::candidate_margins::CandidateMargins;
use crate::tuning::group_candidate::GroupCandidate;
use crate::tuning::objective::{TuningMethod, TuningObjective};
use crate::tuning::project_candidate::ProjectCandidate;
use crate::tuning::recommendation::Attempt;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::search::SearchCandidate;
use crate::tuning::settings_pins::ControllerSettingsPins;
use crate::tuning::state_controller::answer::StateAnswer;
use crate::tuning::state_controller::preferences::FormField;
use crate::tuning::structure::{ParallelGains, Structure};

mod analyze;
mod analyze_declared;
mod validate_answer;
mod validate_initialization;

pub use analyze::{
    AnalysisEnvelope, AnalysisInputs, AnalysisPage, AnalysisResult, AnalysisTool, AnalyzeAnswer,
    AnalyzeRequest, ChannelRef, FrequencyAnswer, FrequencyChannel, FrequencyGrid, FrequencyPoint,
    InitialStateAnswer, LeadLagAnswer, ModelRevision, NamedMatrix, PoleDomain, PoleZeroAnswer,
    PoleZeroView, RelativeGainAnswer, RelativeGainStatus, StateStructureAnswer, ToolOffer,
    TransferPolynomials,
};
pub use analyze_declared::{FeedbackAnswer, MeasuredFrequencyAnswer, RootLocusAnswer, RouthAnswer};

pub use validate_answer::{
    LimitSide, ModelCheck, PlotSeries, RequirementChannel, RequirementOutcome, RunOutcome,
    ScenarioOutcome, ValidateLabels, ValidateResult, ValidationPlot, ValueRange,
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
        /// The status of every stage.
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
        /// Whether Restore previous settings is offered from the journaled checkpoint of the loop's Apply (A8.6, B6.4): a reopened
        /// loop offers it, and the runtime confirms the review when the engineer asks for it.
        restore_available: bool,
        /// Whether Keep in project is offered from the study's Apply receipts (A8.6, B6.5), the same rule `controlStudio/keep` applies.
        keep_available: bool,
        /// Whether a journaled Apply observation is left to follow to its end, with `controlStudio/apply` and `resume` (A8.5, A8.6): the
        /// journal holds the observation while the runtime may still record it.
        apply_observation_resumable: bool,
        /// The design-form fields of a state law, with their labels, under the solver policy the stored draft chooses (D-B12): the
        /// page shows them before the first Tune. Absent for a loop without a state law.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        state_fields: Option<Vec<FormField>>,
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
}

/// The answer of a command that starts a long operation: its identifier, at once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OperationStarted {
    /// The operation.
    pub operation_id: String,
}

/// The arguments of `controlStudio/identify` (specification 35, §0.1a; B3, A8.4, A8.5): the loop, the runtime and the action.
/// The connection's token is a credential: it is never journaled (E11.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IdentifyRequest {
    /// The project folder.
    pub project_root: String,
    /// The runtime selected in VS Code or on the command line.
    pub runtime_id: String,
    /// The loop of the study.
    pub loop_id: String,
    /// The folder of the operation journals (A8.3).
    pub storage_folder: String,
    /// Where the runtime listens, and its control token (E11.1). `identify` and `resume` reach the runtime and need it, and
    /// are refused without it (Connect runtime). `openRecording` and `reset` need no runtime, so they carry none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection: Option<RuntimeConnection>,
    /// What to do.
    pub action: IdentifyAction,
}

/// What `controlStudio/identify` does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "action",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum IdentifyAction {
    /// Runs a live campaign with the method and the experiment limits of `settings` (B3.2, B3.3, B3.4). The answer is the
    /// operation's identifier, at once; the result follows as the operation's update. A start that names a review must name
    /// the review's current identity, and on a physical target it must also carry the engineer's approval (§0.1a, A2.5).
    Identify {
        /// The method (`automatic`, or a named one) and the limits of the experiment.
        settings: AcquisitionSettings,
        /// The identity of the review the engineer saw (`identity` of the review, §0.1a). The start is refused when the campaign
        /// now differs from the reviewed one (E11.8). Absent on a simulated target, where pressing the action is the confirmation.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        review: Option<String>,
        /// Whether the engineer approved the reviewed campaign on a physical target (A2.5). Accepted with `review` only.
        #[serde(default)]
        physical_approved: bool,
    },
    /// Fits a recording the study keeps again, without live excitation (B3.6). An opened recording follows the pairing of
    /// `importRecording` and is answered at once in its shape; a lone opened connected recording is fitted by an operation (§0.1a).
    OpenRecording {
        /// The recording's identifier, `rec-…` (annex E4). Absent, the study's last opened recording is fitted, and a study that keeps
        /// none refuses (§0.1a).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        recording_id: Option<String>,
        /// The plant inputs of the fit, by the recording's channel names; absent for the loop's plant input.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        inputs: Option<Vec<String>>,
        /// The measured outputs of the fit, by the recording's channel names; absent for the loop's measured output.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        outputs: Option<Vec<String>>,
    },
    /// Follows the journaled identification campaign to its end (A8.5, A8.6). The answer is the operation's identifier.
    Resume,
    /// Starts a new working identification study; the previous one moves to history (A8.4). Refused while an operation of the
    /// loop runs or is unconfirmed. The answer is at once, as [`ResetResult`].
    Reset,
    /// Checks a saved Control Studio result or recording file, given as its text (B10.4, B3.6, B3.9), and keeps it as a recording
    /// of the study with its original source and the opened marker (B10.2, A9). A file that is refused keeps nothing. The answer
    /// is at once, as [`ImportedRecording`]; `openRecording` then fits the recording.
    ImportRecording {
        /// The text of the saved result or recording file (at most 32 MiB of text).
        json: String,
    },
    /// Checks a supplied plant model, given as its JSON text of at most 256,000 bytes (B3.7, N4.1), and keeps it as the study's
    /// model, provisional unless it carries evidence of independent validation. A model that is refused leaves the study as it
    /// was. The answer is at once, as [`IdentifyResult`] of the study's current model; it starts no campaign and writes nothing to a runtime.
    ImportModel {
        /// The text of the model file.
        json: String,
    },
    /// Answers the campaign that `identify` would run with these settings: its method, excitation, limits and duration, and for
    /// a project-scripted experiment its activation, abort and restoration (C12). It starts nothing and writes nothing; the answer
    /// is the review of the identification campaign (`identification::campaign_review`), at once.
    Review {
        /// The method and the experiment limits the campaign would run with (B3.2).
        settings: AcquisitionSettings,
    },
    /// Stores the page's form of the manual identification, the Recorded-data fit (`identify.edit`; B3.3, A8.4). The check, the
    /// fit and any accepted model are cleared, and the answer is the page's state at once, as [`ManualIdentificationAnswer`].
    ManualEdit {
        /// The form the page holds: the requested model, whether the held-step analysis is on, and the text of each field.
        form: IdentificationFormDraft,
    },
    /// Checks a recording the study keeps with the page's form (`identify.run`; B3.3, B3.6): the form's settings and excluded
    /// interval are read, and the recording is the estimation, or after a preview the validation recording. The answer is at once,
    /// as [`ManualIdentificationAnswer`].
    ManualQualify {
        /// The identifier of the recording to check, `rec-…` (annex E4).
        recording_id: String,
        /// The form the page holds when it checks the recording.
        form: IdentificationFormDraft,
    },
    /// Fits the checked recording (`identify.fit`; B3.3): the first fit gives the provisional model, and the second scores it on the
    /// validation recording. An operation that can be cancelled: a cancelled fit leaves the stored state as it was. Its final result
    /// is a [`ManualIdentificationAnswer`].
    ManualFit,
    /// Keeps the sensor-lag structure the comparison recommends (`identify.choose-structure`; N3.3). The answer is at once, as
    /// [`ManualIdentificationAnswer`].
    ManualChooseStructure {
        /// The structure to keep: the recommended one.
        choice: StructureChoice,
    },
    /// Accepts the validated model as the study's current model (`identify.accept`; B3.7), so that Tune designs on it. The answer is
    /// at once, as [`ManualIdentificationAnswer`].
    ManualAccept,
    /// Restores the saved manual identification by replaying its checks and fits (`identify` on reopen; A8.6). An operation whose
    /// final result is a [`ManualIdentificationAnswer`].
    ManualRestore,
    /// Keeps a recording file the engineer opens for the manual identification as a recording of the study, and selects it for the
    /// role the stage expects next: the estimation, or the validation recording after a preview (`identify.import`; B3.3, B3.6). The
    /// answer is at once, as [`ManualIdentificationAnswer`] at the recording-selected stage. A file the check refuses keeps nothing and
    /// answers the check's sentence as its failure.
    ManualImport {
        /// The text of the recording file (at most 32 MiB of text).
        json: String,
    },
}

/// What `controlStudio/identify` answers for the manual identification (the Recorded-data fit, B3.3): the stage the page shows, its
/// status, the checks and the model, and the primary action the page offers with its label. The engine decides all of it; the page
/// renders it and sends the request of `primary_action`, and keeps no copy of the rules (AGENTS.md).
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManualIdentificationAnswer {
    /// `complete`, or `failed` with `reason`.
    pub status: ManualStatus,
    /// The sentence of a failure: a refusal, a qualification that does not hold, or a failed validation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The sentence the page shows under the stage, as the engine words it (the stage's status line).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// The stage the page shows; absent while nothing is checked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<ManualStage>,
    /// Whether the next fit estimates the model or validates it.
    pub next_fit_role: FitRole,
    /// Whether the study keeps a recording to check.
    pub has_recording: bool,
    /// The recording the next check reads, `rec-…`: the recording the stage selected or checked. Absent while a preview awaits the
    /// independent recording, and while no recording is kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_id: Option<String>,
    /// The form the study keeps, which the page shows and sends back with its next request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form: Option<IdentificationFormDraft>,
    /// The check of the recording, in the stages after a check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qualification: Option<QualifiedIdentificationData>,
    /// The check of the estimation recording, once a model is fitted from it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimation_qualification: Option<QualifiedIdentificationData>,
    /// The check of the validation recording, at the validated review.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_qualification: Option<QualifiedIdentificationData>,
    /// Where the model's values come from (A9).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<ResultSource>,
    /// The model the stage shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<IdentifiedModel>,
    /// The fit evidence of that model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit: Option<ModelFitEvidence>,
    /// What the page shows of the model, with the sensor-lag comparison when one was made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<IdentifiedModelPresentation>,
    /// The action the page offers, decided by the engine.
    pub primary_action: ManualAction,
    /// The label the page shows on that action.
    pub primary_action_label: String,
    /// Whether the page may accept the review: the validation qualifies, and a compared structure is kept.
    pub acceptance_ready: bool,
    /// The sensor-lag structure the primary action keeps, when it keeps one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure_choice: Option<StructureChoice>,
    /// The evidence the study's stages are computed from, as the study is saved after the action (A8.4). An accepted model is the
    /// study's current model (B3.7), so Tune is available once it is kept. The engine fills it from the saved study.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage_evidence: Option<StageEvidence>,
    /// The status of every stage, computed from `stage_evidence` as the open answer computes its stages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stages: Option<Vec<StageAvailability>>,
}

/// The status of a manual identification answer (`status` of the page's state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ManualStatus {
    /// The stage stands.
    Complete,
    /// The fit, the check or the validation failed; the answer names the reason.
    Failed,
}

/// The stage a manual identification answer shows, named as the page names its stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
pub enum ManualStage {
    /// The form is being edited.
    #[serde(rename = "editing")]
    Editing,
    /// A recording is selected and waits for its check.
    #[serde(rename = "recording-selected")]
    RecordingSelected,
    /// A recording qualifies.
    #[serde(rename = "qualified")]
    Qualified,
    /// The preview of the estimation, waiting for a validation recording or a kept structure.
    #[serde(rename = "validation_required")]
    ValidationRequired,
    /// The validated model waits for acceptance.
    #[serde(rename = "acceptance_required")]
    AcceptanceRequired,
    /// The model is accepted.
    #[serde(rename = "accepted")]
    Accepted,
}

/// The primary action the page offers for the manual identification. Its request is the action's: a check, a fit, a comparison,
/// a structure, an acceptance, or an opened recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ManualAction {
    /// Opens a recording file to import it (`importRecording`).
    OpenRecording,
    /// Checks the study's recording with the form (`manualQualify`).
    CheckRecording,
    /// Fits the plant model (`manualFit`).
    FitPlant,
    /// Scores the model on the validation recording (`manualFit`).
    CompareWithModel,
    /// Keeps the structure the comparison recommends (`manualChooseStructure`).
    ChooseStructure,
    /// Shows the settings to review; no request.
    ReviewSettings,
    /// Opens the validation recording file to import it (`importRecording`).
    OpenValidationRecording,
    /// Accepts the validated model (`manualAccept`).
    Accept,
    /// The model is accepted; nothing is requested.
    ModelReady,
}

/// What `controlStudio/identify` answers at once for `reset`: the stages of the new working study.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResetResult {
    /// The status of every stage.
    pub stages: Vec<StageAvailability>,
    /// The stage evidence of the new working study, from which `stages` is computed: a new study holds no model, candidate or
    /// validation, so the page takes that from the engine (A8.4, B9.4).
    pub stage_evidence: StageEvidence,
}

/// What `controlStudio/identify` answers at once for `importRecording` (B10.2, B3.6, B5.2): the checked identity of the saved
/// result, the recording the study keeps and the channels it offers. A file of two independent archived runs is fitted at once.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedRecording {
    /// The check of the file: its status (`checked`, or `recording-needed`), the saved result's identity, source and sample count.
    #[serde(flatten)]
    pub check: ImportCheck,
    /// The recording the study keeps, `rec-…` (A8.3); for a fitted pair, the estimation recording.
    pub recording_id: String,
    /// The channels the recording offers with their units, and the selection the study keeps (B3.6).
    pub recording_catalog: RecordingSignalCatalog,
    /// The fit of a pair of independent recordings (B3.5, B5.2), when it was fitted at once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identification: Option<IdentifyResult>,
    /// What the file still needs, when it is kept and not yet fitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// That the model was checked against an independent recording, when it was fitted from a pair.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<String>,
}

/// The arguments of `controlStudio/inputs` (specification 35, §0.1a; A8.1, A8.4): the inputs the study records, as the editors
/// change them. The study is written and nothing runs; a loop with an operation that runs refuses the change (B8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputsRequest {
    /// The project folder.
    pub project_root: String,
    /// The runtime selected in VS Code or on the command line; a loop of another runtime is refused, as `open` refuses it.
    pub runtime_id: String,
    /// The loop of the study.
    pub loop_id: String,
    /// The inputs the study records from now on, as a whole: an absent input is not yet given.
    pub inputs: InputsSection,
}

/// What `controlStudio/inputs` answers at once (A8.4): the status of every stage after the change, and what the change invalidated.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InputsResult {
    /// The status of every stage after the change.
    pub stages: Vec<StageAvailability>,
    /// What the change invalidated (A8.4); the results stay in the study as history.
    pub invalidated: Invalidation,
    /// The design-form fields of a state law, with their labels, under the solver policy of the inputs it now records (D-B12): the
    /// page shows them before the first Tune. Absent for a loop without a state law.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_fields: Option<Vec<FormField>>,
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
}

/// The result of the command line's `cancel` (specification 35, §0.1b): the journaled operations it cancelled, by their
/// runtime identifiers. It is an empty list when nothing was journaled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CancelResult {
    /// The identifiers of the operations that were cancelled.
    pub cancelled: Vec<String>,
}

/// The result of an identification: the operation's final update of `controlStudio/identify` (B3.8; Identify page
/// specification, "Identified model and plot"). It holds the readable parts the page shows.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IdentifyResult {
    /// The model type in plain words, for example "First-order model with delay".
    pub model_type: String,
    /// The plant-model equation with the identified coefficients.
    pub equation: String,
    /// One plain line of the key numbers and their units.
    pub key_numbers: String,
    /// Whether the model is provisional (B3.5): it is labelled "Provisional" and cannot authorize Apply.
    pub provisional: bool,
    /// The readable text of the result: the model type, "Provisional" when it is, the equation and the key numbers.
    pub text: String,
    /// The response plot: the applied plant input, the measurement and the model's prediction, on one time base.
    pub plot: IdentifyPlot,
    /// The recording the model was fitted from, by its identifier (A8.3); absent for a supplied model, which no recording made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_id: Option<String>,
    /// The revision of the identified model: the one Validate's model check names when it checks this model (B5.2).
    pub model_revision: String,
    /// The requirements Tune designs to for this model now (B4.3): the values the study records as given, and Automatic's
    /// values for the rest, listed in `automaticCriteria`. Absent where Automatic cannot form them (B4.7): Tune then names the
    /// missing prerequisite when it runs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirements: Option<TuningRequirements>,
    /// The stage evidence after this result, from which `stages` is computed. An identification leaves no candidate and no
    /// validation current for the new model (A8.4), so the answer says so, and the page does not deduce it.
    pub stage_evidence: StageEvidence,
    /// The status of every stage after this result.
    pub stages: Vec<StageAvailability>,
    /// The restoration the campaign's experiments confirmed (C12). Absent where the run kept none: a supplied model, a refit
    /// and a resumed campaign.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restoration: Option<CampaignRestoration>,
    /// The samples of the kept recording, which the page draws its recorded signals from, one per recorded sample and bounded
    /// by the recording limit (A7). Absent where the run kept none, as for `restoration`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded_samples: Option<Vec<RecordedChartSample>>,
}

/// The answer of a project test whose recording its fit cannot support (C12, B3.5): the test completed, so its recording, its samples
/// and its restoration are answered, with the fit's whole refusal as the note. The study keeps no model, so the answer names none.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTestResult {
    /// The recording the test kept, by its file identifier, as `IdentifyResult::recording_id` names it.
    pub recording_id: String,
    /// The restoration the test's experiment confirmed (C12); absent where the test kept none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restoration: Option<CampaignRestoration>,
    /// The samples of the kept recording, one per recorded sample and bounded by the recording limit (A7).
    pub recorded_samples: Vec<RecordedChartSample>,
    /// The fit's refusal in full: why the recording cannot yet support a model (B3.5). The test is finished and the study keeps no model.
    pub fit_refusal: String,
    /// The stage evidence after the test, from which `stages` is computed: the study's current model, if it has one, is unchanged.
    pub stage_evidence: StageEvidence,
    /// The status of every stage after the test.
    pub stages: Vec<StageAvailability>,
}

/// The restoration a campaign's experiments confirmed (C12, E5.6): `restored` once the runtime confirmed the outputs written
/// back to their original values, or `notRequired` when the experiment wrote none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum CampaignRestoration {
    /// The runtime confirmed the restoring writes.
    Restored,
    /// The experiment wrote no output, so there was nothing to restore.
    NotRequired,
}

/// One sample of a kept recording as the Identify chart draws it (`control-studio.recording.evidence`): its process time from the
/// recording's first scan (A3.2), its scan, its quality, the setpoint, the measurement, the requested and the applied output, and
/// the additional recorded signals by identifier (A4.2, A8.3). The page draws the signals it selects and decides nothing.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecordedChartSample {
    /// The sample's process time in s from the recording's first scan.
    pub elapsed_seconds: f64,
    /// The sample's last scan.
    pub scan: u64,
    /// The worst quality of the sample's signals.
    pub quality: ChannelQuality,
    /// The setpoint; null where the recording holds none.
    pub sp: Option<f64>,
    /// The measurement; null where the recording holds none.
    pub pv: Option<f64>,
    /// The output the controller requested, when the loop declares an actuator; null otherwise.
    pub requested_mv: Option<f64>,
    /// The output the process received: the actuator's position when declared, else the manipulated variable.
    pub applied_mv: Option<f64>,
    /// The additional recorded signals, each with its identifier, value and quality.
    pub additional: Vec<RecordedChannel>,
}

/// The data of the response plot: one time base and the traces that share it.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IdentifyPlot {
    /// The sample times in s, from the origin of the recording (A3.2).
    pub time_seconds: Vec<f64>,
    /// The traces, each with one value per time.
    pub traces: Vec<PlotTrace>,
}

/// One trace of the response plot.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlotTrace {
    /// What the trace is.
    pub role: PlotRole,
    /// The signal's symbol, or the model's channel identifier for the prediction.
    pub symbol: String,
    /// The trace's unit.
    pub unit: String,
    /// One value per time; null where the recording holds no value (B10.3: missing values are not interpolated).
    pub values: Vec<Option<f64>>,
}

/// The role of a plotted trace: the page names its traces by role (Identify page specification).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PlotRole {
    /// The plant input the experiment applied.
    PlantInput,
    /// The measured output.
    Measurement,
    /// The model's prediction of the measured output.
    Prediction,
}

/// The response a formula method is asked for (N5.4), in the words of the command line. The faster the response, the
/// smaller the closed-loop time constant `λ` of the formula.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TuneChoice {
    /// The Faster response: the smallest `λ`.
    Faster,
    /// The Balanced response: twice the Faster `λ`.
    Balanced,
    /// The More robust response: four times the Faster `λ`.
    MoreRobust,
}

impl TuneChoice {
    /// The declared response of the requirement record that this choice stands for (N5.4): Faster is `Fast`,
    /// Balanced is `Balanced` and More robust is `Robust`.
    #[must_use]
    pub fn objective(self) -> TuningObjective {
        match self {
            Self::Faster => TuningObjective::Fast,
            Self::Balanced => TuningObjective::Balanced,
            Self::MoreRobust => TuningObjective::Robust,
        }
    }

    /// The choice that a requirement record's declared response stands for: the inverse of [`TuneChoice::objective`].
    #[must_use]
    pub fn of_objective(objective: TuningObjective) -> Self {
        match objective {
            TuningObjective::Fast => Self::Faster,
            TuningObjective::Balanced => Self::Balanced,
            TuningObjective::Robust => Self::MoreRobust,
        }
    }
}

/// The method of a Tune run (specification 35, B4.2; numerical annexes N5 and N5.R): a formula method, the relay rule, or the
/// settings the engineer enters. Absent in a request, the method is Automatic (B4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum TuneMethod {
    /// SIMC, the formula of N5.2 and N5.3.
    #[serde(rename = "SIMC")]
    Simc,
    /// AMIGO, the formula of N5.5.
    #[serde(rename = "AMIGO")]
    Amigo,
    /// The relay rule of N5.R, from a qualified Feedback relay record of the study.
    #[serde(rename = "relay")]
    Relay,
    /// The settings the engineer enters (B4.2, B4.5).
    #[serde(rename = "manual")]
    Manual,
    /// The state-space design of a state-feedback law (C4): its gains, poles and finite-horizon stages on the study's model.
    #[serde(rename = "state-space")]
    StateSpace,
}

impl TuneMethod {
    /// The formula of a formula method (N5.2 to N5.5); `None` for the relay rule and for manual settings.
    #[must_use]
    pub fn formula(self) -> Option<TuningMethod> {
        match self {
            Self::Simc => Some(TuningMethod::Simc),
            Self::Amigo => Some(TuningMethod::Amigo),
            Self::Relay | Self::Manual | Self::StateSpace => None,
        }
    }

    /// The method's title as the page shows it: the formula's title, or the relay rule's or the manual settings' title.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Simc => TuningMethod::Simc.title(),
            Self::Amigo => TuningMethod::Amigo.title(),
            Self::Relay => "Relay rule",
            Self::Manual => "Manual settings",
            Self::StateSpace => "State-space design",
        }
    }
}

/// The arguments of `controlStudio/tune` (specification 35, §0.1a; B4.1, B4.2 and B4.5): the loop, the method, the declared
/// response, the structure and the pinned settings. Tune never writes to the runtime (B4.1); with a connection it reads the
/// running controller's settings for the comparison of the predicted responses (B4.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TuneRequest {
    /// The project folder.
    pub project_root: String,
    /// The runtime selected in VS Code or on the command line; a loop of another runtime is refused, as `open` does.
    pub runtime_id: String,
    /// The loop of the study.
    pub loop_id: String,
    /// The folder of the operation journals (A8.3).
    pub storage_folder: String,
    /// The method: a formula method of N5, the relay rule or manual settings; absent for Automatic (B4.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<TuneMethod>,
    /// The declared response (N5.4). Automatic always designs for More robust (the Tune page specification, "Control
    /// requirements").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice: Option<TuneChoice>,
    /// The structure the engineer fixed; absent leaves the structure to Automatic's choice (B4.4: an explicitly chosen
    /// structure is never replaced silently).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure: Option<Structure>,
    /// The settings the engineer pinned (B4.2): the loop revision they belong to, their family and their values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pins: Option<ControllerSettingsPins>,
    /// The settings the engineer enters with the manual method (B4.2): the record symbol of each setting and its value. They
    /// are refused with any other method.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<BTreeMap<String, f64>>,
    /// Where the runtime listens, and its control token (E11.1); absent when no runtime is connected. Tune reads the running
    /// controller's settings through it for the comparison (B4.5) and writes nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection: Option<RuntimeConnection>,
}

/// The arguments of `controlStudio/preview` (specification 35, §0.1a; B4.5): the loop and the settings the engineer enters.
/// It answers at once with the predicted response of those settings, and stores nothing. With a connection it also reads the
/// running controller's settings for the comparison (B4.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewRequest {
    /// The project folder.
    pub project_root: String,
    /// The runtime selected in VS Code or on the command line; a loop of another runtime is refused, as Tune refuses it.
    pub runtime_id: String,
    /// The loop of the study.
    pub loop_id: String,
    /// The structure the engineer fixed; absent takes the structure the entered gains give (C0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure: Option<Structure>,
    /// The settings the engineer enters: the record symbol of each setting and its value.
    pub settings: BTreeMap<String, f64>,
    /// The settings the engineer pinned (B4.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pins: Option<ControllerSettingsPins>,
    /// Where the runtime listens, and its control token (E11.1); absent when no runtime is connected. A preview reads the
    /// running controller's settings through it for the comparison (B4.5) and writes nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection: Option<RuntimeConnection>,
}

impl PreviewRequest {
    /// The Tune request that a preview evaluates: the manual method with these settings, and the connection the preview names.
    /// The storage folder is empty, because a preview keeps no journal.
    #[must_use]
    pub fn tune_request(&self) -> TuneRequest {
        TuneRequest {
            project_root: self.project_root.clone(),
            runtime_id: self.runtime_id.clone(),
            loop_id: self.loop_id.clone(),
            storage_folder: String::new(),
            method: Some(TuneMethod::Manual),
            choice: None,
            structure: self.structure,
            pins: self.pins.clone(),
            settings: Some(self.settings.clone()),
            connection: self.connection.clone(),
        }
    }
}

/// The units of a candidate's gains in the loop's units, as the sentences about the candidate print them (B4.5).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TuneGainUnits {
    /// The unit of the proportional gain: plant input per measured output.
    pub proportional: String,
    /// The unit of the integral gain: plant input per measured output per second.
    pub integral: String,
    /// The unit of the derivative gain: plant input seconds per measured output.
    pub derivative: String,
}

/// How a candidate stands against the mandatory requirements on its predicted response (B4.4, B4.5).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CandidateOutcome {
    /// The prediction meets every requirement it covers.
    Meets,
    /// The prediction misses a requirement.
    Misses {
        /// The sentence that says what is missed and by how much.
        shortfall: String,
    },
    /// No prediction is available, so Validate decides.
    Unpredicted {
        /// The sentence that says why there is no prediction.
        reason: String,
    },
}

/// What a trace of the predicted response is: the page names its traces by role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TuneRole {
    /// The setpoint of the test.
    Setpoint,
    /// The predicted measurement.
    Measurement,
    /// The predicted controller output.
    Command,
    /// The predicted measurement of the current settings, on the candidate's time base and symbols (B4.5).
    CurrentMeasurement,
    /// The predicted controller output of the current settings, on the candidate's time base and symbols (B4.5).
    CurrentCommand,
}

/// One trace of the predicted response, with the symbol and the unit the loop declares for the signal.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TuneTrace {
    /// What the trace is.
    pub role: TuneRole,
    /// The signal's symbol.
    pub symbol: String,
    /// The trace's unit.
    pub unit: String,
    /// One value per time.
    pub values: Vec<f64>,
}

/// The predicted response of a candidate on the selected model (B4.5, B9.6): one time base and the traces on it.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TuneResponse {
    /// The times in s from the start of the test.
    pub time_seconds: Vec<f64>,
    /// The traces, each with one value per time.
    pub traces: Vec<TuneTrace>,
}

/// One candidate of the constrained comparison around the SIMC candidate (N5.10).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ComparisonEntry {
    /// What the candidate is: `SIMC` for the formula's own, `Constrained comparison` for the others.
    pub label: String,
    /// The candidate's structure.
    pub structure: Structure,
    /// The candidate's signed gains.
    pub gains: ParallelGains,
    /// How the candidate stands against the mandatory requirements on its prediction.
    pub outcome: CandidateOutcome,
}

/// One reduced path of a measured-disturbance feedforward, as Tune shows it (C7, C8): the gain, the time constant and the dead time of its
/// first-order model.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FeedforwardPath {
    /// The steady-state gain of the path.
    pub gain: f64,
    /// The time constant of the path in seconds.
    pub time_constant_seconds: f64,
    /// The dead time of the path in seconds.
    pub delay_seconds: f64,
}

/// The answer of a measured-disturbance feedforward's Tune (C8): both reduced paths, the three feedforward values, the disturbance step
/// and, on the model, the largest deviation of the measurement after that step with the PI alone and with the feedforward.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FeedforwardAnswer {
    /// The path from the controller output to the measurement.
    pub output_path: FeedforwardPath,
    /// The path from the measured disturbance to the measurement.
    pub disturbance_path: FeedforwardPath,
    /// The feedforward gain `K_ff`, in the manipulated variable's unit per the disturbance's unit.
    pub gain: f64,
    /// The lead time `T_lead` in seconds, when the program has a lead and a lag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lead_seconds: Option<f64>,
    /// The lag time `T_lag` in seconds, when the program has a lead and a lag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lag_seconds: Option<f64>,
    /// The disturbance step of the test: the disturbance's identification step.
    pub disturbance_step: f64,
    /// The largest deviation of the measurement from its operating value after the step, with the PI alone, on the model.
    pub feedback_deviation: f64,
    /// The largest deviation of the measurement from its operating value after the step, with the PI and the feedforward, on the model.
    pub feedforward_deviation: f64,
}

/// The choice of a gain schedule's answer (C10): `fixed` when every point has the same proportional and integral gain, `scheduled` when the
/// points' gains differ. The word comes from the table the answer shows, not from the decision that gave the reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ScheduleChoiceWord {
    /// One PI for every operating point.
    Fixed,
    /// Each operating point has its own PI.
    Scheduled,
}

/// The gain schedule of a Tune answer (C10): the choice, the reason, the scheduling variable and one row per operating point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduleAnswer {
    /// Whether the points share one PI (`fixed`) or each has its own (`scheduled`).
    pub choice: ScheduleChoiceWord,
    /// The reason of the candidate, as the study's section stores it.
    pub reason: String,
    /// The scheduling variable: its path and its unit.
    pub scheduling: ScheduleAnswerVariable,
    /// One row per operating point, in the order of the schedule's table.
    pub points: Vec<ScheduleAnswerPoint>,
}

/// The scheduling variable of a gain schedule's answer (C10 Setup).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduleAnswerVariable {
    /// The variable's path.
    pub symbol: String,
    /// The variable's unit.
    pub unit: String,
}

/// One operating point of a gain schedule's answer (C10): its name and value of the scheduling variable, its two gain variables, and the
/// gains that the table gives it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduleAnswerPoint {
    /// The point's name.
    pub name: String,
    /// The point's value of the scheduling variable.
    pub value: f64,
    /// The program's proportional gain variable for the point.
    pub proportional_symbol: String,
    /// The program's integral gain variable for the point.
    pub integral_symbol: String,
    /// The proportional gain that the table gives the point.
    pub proportional_gain: f64,
    /// The integral gain that the table gives the point.
    pub integral_gain: f64,
}

/// The final result of `controlStudio/tune` (specification 35, B4.3 to B4.6; the Tune page specification, "Result and
/// plot"): the operation's result, and the `result` of the command line's `--json` object. Nothing in it is applied
/// (B4.1).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TuneResult {
    /// The answer of a measured-disturbance feedforward (C8): both paths, the three values and the deviations. Absent for any other
    /// structure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedforward: Option<FeedforwardAnswer>,
    /// The answer of a state-feedback law's Tune (C4): its family, closed-loop poles, finite-horizon stages and design-form fields
    /// (D-B12). Absent for every other law.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<StateAnswer>,
    /// The candidate of a `Control.PID`, unapplied: its structure, its source and the complete record it would write. Absent for a
    /// program's own law, which answers `projectCandidate` (C2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate: Option<ControlPidCandidate>,
    /// The candidate of a program's own law, unapplied (C2): its structure, its source, its gains and the value of each seed
    /// variable that Apply writes. Absent for a `Control.PID`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_candidate: Option<ProjectCandidate>,
    /// The candidate of a PID group, unapplied (C7): each member's design, the static decoupler when the group has one, and the value
    /// of each variable that Apply writes. Absent for every other law.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<GroupCandidate>,
    /// The value the candidate's record gives each bound setting of the loop's controller, under the setting's symbol (B4.2,
    /// C0): the settings the page shows under the loop's own names. A setting the record does not hold is absent.
    pub settings: BTreeMap<String, f64>,
    /// The method the engineer chose; absent for Automatic.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<TuneMethod>,
    /// The method's title as the page shows it: `Automatic`, or the title of the formula (N5.2, N5.5).
    pub method_title: String,
    /// The method in words (B4.4).
    pub method_description: String,
    /// Why the candidate was chosen: the recommendation's reason for Automatic, the method's own otherwise (B4.4).
    pub reason: String,
    /// The response the design used (N5.4); Automatic always uses More robust.
    pub objective: TuningObjective,
    /// The closed-loop time constant `λ` in seconds that the formula used (N5.4: shown with the candidate); absent for
    /// AMIGO and for a design without `λ`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed_loop_time_seconds: Option<f64>,
    /// Every structure Automatic attempted, with its status and reason (B4.4; Part C, "Candidates and results"); empty
    /// for a named method.
    pub attempts: Vec<Attempt>,
    /// The units of the candidate's gains.
    pub gain_units: TuneGainUnits,
    /// The requirements the candidate was designed to (B4.3). The values that Tune generated are listed in their
    /// `automaticCriteria`, so the page can mark them Automatic.
    pub requirements: TuningRequirements,
    /// How the candidate stands against the mandatory requirements on its prediction.
    pub outcome: CandidateOutcome,
    /// The predicted response of the candidate (B4.5); absent when none is available. It holds the two traces of the current
    /// settings as well when the running controller's settings were read and can be predicted (B4.5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<TuneResponse>,
    /// Why there is no predicted response, when there is none (B4.5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prediction_unavailable: Option<String>,
    /// The predicted settling time in s into the settling band (B4.5); absent when the response does not settle or is
    /// not predicted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settling_time_seconds: Option<f64>,
    /// The predicted overshoot in percent of the setpoint change (B4.3); absent when no response is predicted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overshoot_percent: Option<f64>,
    /// The margins of the candidate on the model (B4.6, N7.7); absent when they cannot be given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margins: Option<CandidateMargins>,
    /// The delay margin in s of the candidate, the phase margin in radians over the gain crossover (B4.6, N7.7), from
    /// [`crate::tuning::robustness::delay_margin`]; absent when the margins are.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay_margin_seconds: Option<MarginValue>,
    /// Why there are no margins, when there are none (B4.6).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margins_unavailable: Option<String>,
    /// The constrained comparison around the SIMC candidate (N5.10), which a request for SIMC gives; empty
    /// otherwise. It creates no candidate.
    pub comparison: Vec<ComparisonEntry>,
    /// The stage evidence after this result, from which `stages` is computed: whether the model and the candidate this result
    /// is the answer of exist (B9.4). A preview keeps nothing, so it carries the study's own evidence (B4.5).
    pub stage_evidence: StageEvidence,
    /// The status of every stage after this result.
    pub stages: Vec<StageAvailability>,
    /// The gain schedule of a schedule loop's answer (C10): the choice, the reason and one row per operating point, with the gains that the
    /// table gives each point. Absent for a loop that is not a schedule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<ScheduleAnswer>,
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
    /// The identity of this review.
    pub identity: String,
}

/// The final result of a confirmed Keep (B6.5): the record, the fields kept, the fields without a setting, and the sentence that
/// says what was kept, or that the project already had the values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KeepResult {
    /// The record.
    pub record: String,
    /// The fields the Keep wrote.
    pub kept: Vec<String>,
    /// The fields that the equipment has no setting for.
    pub unknown: Vec<String>,
    /// What was kept, in words.
    pub reason: String,
}

/// The arguments of `controlStudio/parameters` (specification 35, C12 and §0.1a; annex E7): the writes of the parameters that a
/// declared analysis maps. `values` holds the mapped symbols with the values a review or an apply sets; every other symbol of the
/// record keeps its value. `review` is the identity that `apply` and `restore` confirm (A2.5, E11.8). The connection's token is a
/// credential, never journaled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParameterWorkRequest {
    /// The project folder.
    pub project_root: String,
    /// The runtime selected in VS Code or on the command line.
    pub runtime_id: String,
    /// The loop whose declared analysis maps the parameters.
    pub loop_id: String,
    /// The folder of the operation journals (A8.3).
    pub storage_folder: String,
    /// What the request does.
    pub action: ParameterWorkAction,
    /// The values to write, by mapped symbol (`review`, `apply`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, f64>,
    /// The identity of the review that `apply` or `restore` confirms (A2.5); absent to ask for the review.
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
/// values the runtime holds; `review` and `reviewRestore` answer the review of an Apply or of a Restore, which writes nothing; `apply`
/// and `restore` start the confirmed write as an operation.
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
    /// The confirmed Apply of the reviewed values, started as an operation.
    Apply,
    /// The confirmed Restore of the retained values, started as an operation.
    Restore,
}

/// What `controlStudio/parameters` answers (C12). Each action gives the fields it needs: `draft` gives `values`; `inspect` gives
/// `values` and `observed`; `review` and `reviewRestore` give `identity`, `changes` and `physical`. `apply` and `restore` answer an
/// `operationId`, and the operation's result is `observed`, the record that the write was confirmed on.
#[derive(Debug, Clone, Default, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ParameterWorkAnswer {
    /// The mapped values, by symbol (`draft`, `inspect`).
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
    /// The parameters the page shows (`draft`, `inspect`): only those the analysis may write, with the bounds the analysis allows (C12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<crate::loop_definition::ParameterBinding>,
}

/// What `controlStudio/apply`, `controlStudio/restore` and `controlStudio/keep` answer at once (§0.1b): the review of the change, which
/// writes nothing, or the started operation, whose result follows through `controlStudio/update`. It is written untagged, so a review
/// is its own object and a started operation is `{"operationId": …}`; it is never decoded.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum ApplicationAnswer {
    /// The review of the change, answered when the request carried no review identity.
    Review(Box<ApplicationReview>),
    /// The operation that runs the reviewed change.
    Started {
        /// The operation's identifier.
        #[serde(rename = "operationId")]
        operation_id: String,
    },
}
