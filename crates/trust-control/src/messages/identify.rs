//! The wire types of `controlStudio/identify` (specification 35, §0.1a; B3, B10.2, C12, A8.4 to A8.6): the request and its actions, the
//! Recorded-data fit's state for the page, the answers of `reset` and `importRecording`, and the identification's result with its plot,
//! its recorded samples and the project test's answer. `messages.rs` re-exports them.

// The wire names of this module are the stage's names: `IdentifyRequest`, `IdentifyAction` and `IdentifyResult` repeat the module's name.
#![allow(clippy::module_name_repetitions)]

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{AnswerKind, TuneMethod};
use crate::identification::acquisition_settings::AcquisitionSettings;
use crate::identification::data::QualifiedIdentificationData;
use crate::identification::evidence::ModelFitEvidence;
use crate::identification::form_draft::IdentificationFormDraft;
use crate::identification::plant::{IdentifiedModel, ResultSource};
use crate::identification::presentation::IdentifiedModelPresentation;
use crate::identification::recording_import::ImportCheck;
use crate::identification::review::IdentificationModelSelection;
use crate::identification::sensor_lag_comparison::StructureChoice;
use crate::identification::workflow::FitRole;
use crate::recording::channels::{ChannelQuality, RecordedChannel};
use crate::recording::opened_channels::RecordingSignalCatalog;
use crate::runtime::RuntimeConnection;
use crate::stages::{StageAvailability, StageEvidence};
use crate::tuning::requirements::TuningRequirements;

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
    /// The mode of the window's runtime lifecycle when no runtime is running, as the extension reports it with a live request that
    /// carries no connection: `simulate` for the local simulator, `online` for a runtime attached through its endpoint. A live request
    /// without a connection is refused with the next action that applies: Start simulator for `simulate`, otherwise Connect runtime
    /// (A2.8, E11.1). Absent when a runtime is running, and on the command line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_mode: Option<String>,
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
        /// The method (`automatic`, or a named one) and the limits of the experiment. Absent for the project test of a project-scripted
        /// loop, which runs the loop's declared experiment with the Automatic method (C12); any other loop is refused without them.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        settings: Option<AcquisitionSettings>,
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
    /// Reattaches the loop's journaled operations (A8.5, A8.6): follows a campaign's dispatched step to its end, completes a journal
    /// without one, or ends an identification whose simulator restarted or reloaded its program since. The answer is the operation's
    /// identifier.
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
        /// The method and the experiment limits the campaign would run with (B3.2). Absent for the project test of a project-scripted
        /// loop, which is reviewed with the loop's declared experiment and the Automatic method (C12); any other loop is refused
        /// without them.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        settings: Option<AcquisitionSettings>,
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
        /// The identifier of the recording to check, `rec-…` (annex E4). Absent, the study's last opened recording is checked, as
        /// `OpenRecording` fits it, and a study that keeps none refuses the check (B3.6).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        recording_id: Option<String>,
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
    /// Restores the saved manual identification by replaying its checks and fits (A8.6). An operation whose final result is a
    /// [`ManualIdentificationAnswer`]. The open answer carries the same state (`manualIdentification`), so the extension asks for no
    /// replay when it reopens a loop.
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
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManualIdentificationAnswer {
    /// The answer's kind (§0.1a): the page shows the answer by it, never by the fields it carries.
    pub kind: AnswerKind<ManualIdentificationAnswer>,
    /// `complete`, or `failed` with `reason`.
    pub status: ManualStatus,
    /// The sentence of a failure: a refusal, a qualification that does not hold, or a failed validation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The sentence the page shows under the stage, as the engine words it (the stage's status line).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// The sentence that says how the project test that recorded the stage's independent recording ended (C12), as
    /// `IdentifyResult::test_outcome` gives it; absent for every other answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_outcome: Option<String>,
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
    /// The model choices the page may enable for the form it holds (`modelChoices`): every model of its model select, in the
    /// select's order. While the held-step measurement is on, only `fopdt` and `sopdt` are enabled, and the others are listed with
    /// `enabled` false.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub model_choices: Vec<ManualModelChoice>,
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
    /// The largest absolute residual of that fit's samples, in output units: the engine's number for the evidence row that the page
    /// shows beside the prediction's root mean square error (B3.3). Absent without a fit, and when the fit holds no sample.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_absolute_residual: Option<f64>,
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

/// One model of the page's model select and whether the page may enable it (`modelChoices` of the manual identification answer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManualModelChoice {
    /// The model, which the select's option value names in snake case (`fopdt`, `first_order` and so on).
    pub model: IdentificationModelSelection,
    /// Whether the page may enable the option.
    pub enabled: bool,
    /// The words of the option, as the engine offers them with the choice: the page lists the options from the choices and keeps no
    /// label table of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<&'static str>,
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
    /// The answer's kind (§0.1a): the page shows the answer by it, never by the fields it carries.
    pub kind: AnswerKind<ResetResult>,
    /// The sentence the Identify page's status line shows after a reset (A8.4, B9.9), as the engine words it: `New identification study
    /// started.` The page writes none of its own.
    pub status_sentence: String,
    /// The status of every stage.
    pub stages: Vec<StageAvailability>,
    /// The stage evidence of the new working study, from which `stages` is computed: a new study holds no model, candidate or
    /// validation, so the page takes that from the engine (A8.4, B9.4).
    pub stage_evidence: StageEvidence,
    /// The Recorded-data fit of the new working study (B3.3): it keeps none, so this is the engine's default form and the action that
    /// opens a recording. The page shows it when the Recorded-data fit is the method it shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_identification: Option<ManualIdentificationAnswer>,
}

/// What `controlStudio/identify` answers at once for `importRecording` (B10.2, B3.6, B5.2): the checked identity of the saved
/// result, the recording the study keeps and the channels it offers. A file of two independent archived runs is fitted at once.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedRecording {
    /// The answer's kind (§0.1a): the page shows the answer by it, never by the fields it carries.
    pub kind: AnswerKind<ImportedRecording>,
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
    /// The operation that fits a checked recording of one connected record at once (B3.6): the page follows its updates, and its
    /// final result is the identification. Absent when the answer carries the fit, or when the file needs another recording.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
}

/// The result of an identification: the operation's final update of `controlStudio/identify` (B3.8; Identify page
/// specification, "Identified model and plot"). It holds the readable parts the page shows.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IdentifyResult {
    /// The answer's kind (§0.1a): the page shows the answer by it, never by the fields it carries.
    pub kind: AnswerKind<IdentifyResult>,
    /// The model type in plain words, for example "First-order model with delay".
    pub model_type: String,
    /// The plant-model equation with the identified coefficients.
    pub equation: String,
    /// One plain line of the key numbers and their units.
    pub key_numbers: String,
    /// Whether the model is provisional (B3.5): it is labelled "Provisional" and cannot authorize Apply.
    pub provisional: bool,
    /// The sentence the Identify page's status line shows with this identification (B3.8, B9.9), as the engine words it: `Model
    /// identified.`, or `Provisional model identified.` for a provisional model. The page writes none of its own.
    pub status_sentence: String,
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
    /// The sentence that says the finished project test releases its experiment settings (C12): present exactly when `restoration`
    /// is, because the settings' ownership ends only after the terminal restoration. The page shows it with the parameter tool.
    /// Absent where the run kept no restoration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings_released: Option<String>,
    /// The sentence that says how the project test ended (C12): `Project test completed.`, followed by `Outputs restored.` when the
    /// runtime confirmed the restoration. The page shows it as the test's status. Absent for an identification that is no project test.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_outcome: Option<String>,
    /// The samples of the kept recording, which the page draws its recorded signals from, one per recorded sample and bounded
    /// by the recording limit (A7). Absent where the run kept none, as for `restoration`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded_samples: Option<Vec<RecordedChartSample>>,
    /// The tuning methods the study offers the Tune page after this identification (C4, N5.R): the loop's named methods, and the relay
    /// rule beside Manual when the kept record is a qualified Feedback relay record. Absent when the loop has no method list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub methods: Option<Vec<TuneMethod>>,
    /// The title of each method of `methods`, keyed by the method's word (as the open answer gives them).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method_titles: Option<BTreeMap<String, String>>,
}

/// The answer of a project test whose recording its fit cannot support (C12, B3.5): the test completed, so its recording, its samples
/// and its restoration are answered, with the fit's whole refusal as the note. The study keeps no model, so the answer names none.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTestResult {
    /// The answer's kind (§0.1a): the page shows the answer by it, never by the fields it carries.
    pub kind: AnswerKind<ProjectTestResult>,
    /// The recording the test kept, by its file identifier, as `IdentifyResult::recording_id` names it.
    pub recording_id: String,
    /// The restoration the test's experiment confirmed (C12); absent where the test kept none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restoration: Option<CampaignRestoration>,
    /// The sentence that says the finished test releases its experiment settings (C12), as `IdentifyResult::settings_released` gives it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings_released: Option<String>,
    /// The sentence that says how the test ended (C12), as `IdentifyResult::test_outcome` gives it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_outcome: Option<String>,
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
/// the additional recorded signals by identifier (A4.2, A8.3), and the value each plotted signal takes by its identifier (`plotted`,
/// B9.6). The page draws the signals it selects from `plotted` and decides nothing.
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
    /// The value each plotted signal takes at the sample, by the signal's identifier (`PlotSignal::id` of `liveSides.plotSignals`): the
    /// value the plot draws, or null where it draws a gap. A missing, bad or forced value is a gap in its own trace and removes no good
    /// value of another signal (B9.6).
    pub plotted: BTreeMap<String, Option<f64>>,
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
