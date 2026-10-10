//! The messages between the Control Studio panel and the language server (specification 35,
//! §0.1a): the arguments and results of the `trust-lsp.controlStudio…` commands and the
//! `trust/controlStudio/update` notification. The extension's `TypeScript` types are generated from
//! these definitions, never written by hand.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use trust_control_math::margins::MarginValue;

use crate::failure::Failure;
use crate::identification::acquisition_settings::AcquisitionSettings;
use crate::identification::connected_record::RunEvidence;
use crate::identification::independent_validation::RequirementStatus;
use crate::loop_definition::LoopDefinition;
use crate::runtime::RuntimeConnection;
use crate::stages::StageAvailability;
use crate::study::Page;
use crate::tuning::candidate::ControlPidCandidate;
use crate::tuning::candidate_margins::CandidateMargins;
use crate::tuning::objective::{TuningMethod, TuningObjective};
use crate::tuning::recommendation::Attempt;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::settings_pins::ControllerSettingsPins;
use crate::tuning::structure::{ParallelGains, Structure};

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
        /// The page last viewed, which reopening shows (A8.1, A8.6); Identify for a new study.
        page: Page,
    },
}

/// The arguments of `controlStudio/page` (specification 35, §0.1a; A8.1): the loop, and the page the engineer views. It
/// changes nothing else and starts nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
    /// operation's identifier, at once; the result follows as the operation's update.
    Identify {
        /// The method (`automatic`, or a named one) and the limits of the experiment.
        settings: AcquisitionSettings,
    },
    /// Fits a recording of the study again, without live excitation (B3.6). The study's Identify result must name it.
    OpenRecording {
        /// The recording's identifier, `rec-…` (annex E4).
        recording_id: String,
    },
    /// Follows the journaled identification campaign to its end (A8.5, A8.6). The answer is the operation's identifier.
    Resume,
    /// Starts a new working identification study; the previous one moves to history (A8.4). Refused while an operation of the
    /// loop runs or is unconfirmed. The answer is at once, as [`ResetResult`].
    Reset,
}

/// What `controlStudio/identify` answers at once for `reset`: the stages of the new working study.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResetResult {
    /// The status of every stage.
    pub stages: Vec<StageAvailability>,
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
    /// The recording the model was fitted from, by its identifier (A8.3).
    pub recording_id: String,
    /// The status of every stage after this result.
    pub stages: Vec<StageAvailability>,
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
}

impl TuneMethod {
    /// The formula of a formula method (N5.2 to N5.5); `None` for the relay rule and for manual settings.
    #[must_use]
    pub fn formula(self) -> Option<TuningMethod> {
        match self {
            Self::Simc => Some(TuningMethod::Simc),
            Self::Amigo => Some(TuningMethod::Amigo),
            Self::Relay | Self::Manual => None,
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
        }
    }
}

/// The arguments of `controlStudio/tune` (specification 35, §0.1a; B4.1 and B4.2): the loop, the method, the declared
/// response, the structure and the pinned settings. Tune never reaches the runtime (B4.1), so the request carries no
/// connection.
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
}

/// The arguments of `controlStudio/preview` (specification 35, §0.1a; B4.5): the loop and the settings the engineer enters.
/// It answers at once with the predicted response of those settings, and stores nothing.
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
}

impl PreviewRequest {
    /// The Tune request that a preview evaluates: the manual method with these settings. The storage folder is empty,
    /// because a preview keeps no journal.
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

/// The final result of `controlStudio/tune` (specification 35, B4.3 to B4.6; the Tune page specification, "Result and
/// plot"): the operation's result, and the `result` of the command line's `--json` object. Nothing in it is applied
/// (B4.1).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TuneResult {
    /// The candidate, unapplied: its structure, its source and the complete `Control.PID` record it would write.
    pub candidate: ControlPidCandidate,
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
    /// The predicted response of the candidate (B4.5); absent when none is available.
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
    /// The status of every stage after this result.
    pub stages: Vec<StageAvailability>,
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

/// The final result of `controlStudio/validate` (specification 35, B5.1 to B5.5), and the `result` of the command line's
/// `validate --json` object. It holds the independent model check when Validate ran one, each scenario's current and
/// candidate runs, the overall result of the candidate and whether Apply is available. Nothing in it is written to the
/// controller (N8.9).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ValidateResult {
    /// The independent model check (B5.2), when this validation ran it; absent when the model was already qualified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_check: Option<ModelCheck>,
    /// Each scenario of the campaign in its order, the target-response case first: the current settings' run and the
    /// candidate's (B5.3).
    pub scenarios: Vec<ScenarioOutcome>,
    /// The overall result of the candidate (N8.5): Fail when a requirement fails, Inconclusive when one lacks evidence,
    /// and Pass otherwise.
    pub status: RequirementStatus,
    /// Whether Apply is available: the model is qualified and the candidate passed every requirement (B5.4).
    pub apply_available: bool,
    /// The status of every stage after this result.
    pub stages: Vec<StageAvailability>,
}

/// The independent model check of a provisional model (B5.2; numerical annex N3.4): the frozen model's free-running prediction
/// against the recorded output of a distinct live campaign. The study keeps the recording the check used.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelCheck {
    /// Pass when the error is within the identification error limit over the whole response and over its final 10 %; Fail
    /// when it is not; Inconclusive when the check could not be made (B5.4).
    pub status: RequirementStatus,
    /// The revision of the frozen model that was checked.
    pub model_revision: String,
    /// The identifier of the check's recording, `rec-…` (A8.3, B10.3).
    pub recording_id: String,
    /// The error limit, normalized by the measured output's span (N3.4).
    pub limit: f64,
    /// The largest normalized error over the checked channels, when one was measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_error: Option<f64>,
    /// The reason in words: the error and the limit, and what to change when the check failed (B5.5).
    pub reason: String,
}

/// One scenario of the campaign (B5.3): the current settings' run and the candidate's, on the same plant. The runs are the
/// isolated simulations of the project's actual Structured Text (A9); their samples stay in the runtime's answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScenarioOutcome {
    /// The scenario's identity: `target-response`, an envelope case's identity, or a declared scenario's id.
    pub id: String,
    /// The plant the runs ran against, named as the evidence names it (A9): the configured plant by its id, or the model.
    pub plant: String,
    /// The run of the current settings: the program's own compiled record, without parameter overrides.
    pub current: RunOutcome,
    /// The run of the candidate settings, its record written as parameter overrides.
    pub candidate: RunOutcome,
}

/// One isolated run of a scenario: its identity, its evidence, its status and what each requirement found (B5.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunOutcome {
    /// The run's identifier: the job's request identifier, which the operation journals (E8.4, E11.2).
    pub run_id: String,
    /// The project revision the run ran on (E8.2).
    pub project_revision: String,
    /// Where its values come from: `ST-executed`, the project's Structured Text in an isolated simulation (A9).
    pub evidence: RunEvidence,
    /// The run's status: the overall result of its requirements (N8.5). A run that did not complete is Inconclusive.
    pub status: RequirementStatus,
    /// Each requirement's result, in the order of the scenario's requirements.
    pub requirements: Vec<RequirementOutcome>,
}

/// One requirement's result (B5.4): its measured value, its limit, its unit, its source and its reason.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequirementOutcome {
    /// The requirement's identity.
    pub id: String,
    /// Pass, Fail or Inconclusive (N8.4, N8.5).
    pub status: RequirementStatus,
    /// The measured value, when one was measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// The limit the value was judged against, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<f64>,
    /// The unit of the value and of the limit.
    pub unit: String,
    /// The source of the result: the plant the isolated run ran against (A9).
    pub source: String,
    /// The reason in words (A11.1).
    pub reason: String,
}
