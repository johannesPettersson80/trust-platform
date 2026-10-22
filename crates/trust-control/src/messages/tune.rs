//! The wire types of `controlStudio/tune` and `controlStudio/preview` (specification 35, §0.1a; B4.1 to B4.6, C0, C2, C4, C7, C8, C10, N5):
//! the methods and the declared response, the two requests, the candidate's outcome and predicted response, the feedforward and gain
//! schedule answers, and the Tune result. `messages.rs` re-exports them.

// The wire names of this module are the stage's names: `TuneRequest`, `TuneMethod` and `TuneResult` repeat the module's name.
#![allow(clippy::module_name_repetitions)]

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use trust_control_math::margins::MarginValue;

use super::{AnswerKind, TuneWords};
use crate::runtime::RuntimeConnection;
use crate::stages::{StageAvailability, StageEvidence};
use crate::tuning::candidate::ControlPidCandidate;
use crate::tuning::candidate_margins::CandidateMargins;
use crate::tuning::group_candidate::GroupCandidate;
use crate::tuning::objective::{TuningMethod, TuningObjective};
use crate::tuning::project_candidate::ProjectCandidate;
use crate::tuning::recommendation::Attempt;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::settings_pins::PinnedSettings;
use crate::tuning::state_controller::answer::StateAnswer;
use crate::tuning::structure::{ParallelGains, Structure};

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

    /// The method's title as the Tune page's method list shows it (C4): the labels of the page's method options.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Simc => "Simple internal model control (SIMC)",
            Self::Amigo => "Approximate M-constrained integral gain optimization (AMIGO)",
            Self::Relay => "Relay rule (Ziegler–Nichols PI)",
            Self::Manual => "Manual",
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
    /// The structure the engineer chose; absent, the engine designs for the structure the study's inputs record (A8.1) and, for the
    /// manual method with none recorded, for the family the Controller selector shows (`structure_of_request`, B4.2). Every other
    /// method then leaves the structure to its own design (B4.4: an explicitly chosen structure is never replaced silently). The page
    /// sends the family the engineer chose in the selector, as a word, and none otherwise: `P`, `PI`, `PD` and `PID` fix that
    /// structure, and any other word (the family of a state law or of a program) fixes none.
    #[serde(
        default,
        deserialize_with = "crate::tuning::structure::structure_of_word",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(schema_with = "crate::tuning::structure::structure_word_schema")]
    pub structure: Option<Structure>,
    /// The settings the engineer pinned (B4.2): the loop revision they belong to and their values. The family is the study's: the
    /// structure this request names, else the family of the study's candidate, else the family of the study's pins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pins: Option<PinnedSettings>,
    /// The settings the engineer enters with the manual method (B4.2): the record symbol of each setting and its value. They
    /// are refused with any other method.
    #[serde(
        default,
        deserialize_with = "crate::json::number_or_text_map",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(schema_with = "crate::json::optional_number_or_text_map_schema")]
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
    /// The settings the engineer enters: the record symbol of each setting and its value. The study's structure and pins are read by
    /// the engine (B4.2, B4.5); a missing value, or one that is not a number, is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<Value>,
    /// Where the runtime listens, and its control token (E11.1); absent when no runtime is connected. A preview reads the
    /// running controller's settings through it for the comparison (B4.5) and writes nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection: Option<RuntimeConnection>,
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
    /// The sentence of the outcome as the page shows it, the same words as the outcome's sentence (B4.4, B4.5).
    pub outcome_text: String,
}

/// One reduced path of a measured-disturbance feedforward, as Tune shows it (C7, C8): the gain, the time constant and the dead time of its
/// first-order model.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FeedforwardPath {
    /// The steady-state gain of the path.
    pub gain: f64,
    /// The unit of the path's gain: the measurement's unit per the unit of the path's input, or `1` when they are the same. The page shows
    /// the gain with it and composes no unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gain_unit: Option<String>,
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
    /// The unit of `K_ff`: the manipulated variable's unit per the disturbance's, or `1` when they are the same.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gain_unit: Option<String>,
    /// The unit of the measured disturbance, which its step is in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disturbance_unit: Option<String>,
    /// The unit of the measurement, which the two deviations are in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measurement_unit: Option<String>,
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
    /// The answer's kind (§0.1a): the page shows the answer by it, never by the fields it carries.
    pub kind: AnswerKind<TuneResult>,
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
    /// The words of the answer that the page shows as text: the margins in words, the outcome sentence and the labels of the setting rows
    /// (the Tune page specification, "Result and plot"). Set for every answer that the engine gives.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub words: Option<TuneWords>,
}
