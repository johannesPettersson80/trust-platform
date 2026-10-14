//! The Validate answer (specification 35, B5.1 to B5.5 and B9.6): the result of `controlStudio/validate`, its independent model
//! check, its scenarios and runs, each requirement's row with its measured and declared ranges, the words the table names its rows
//! with, and the Validate plot. `messages.rs` re-exports them, so their names and wire shapes are those of the answer.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::failure::Failure;
use crate::identification::connected_record::RunEvidence;
use crate::identification::independent_validation::RequirementStatus;
use crate::stages::{StageAvailability, StageEvidence};
use crate::tuning::requirements::TuningRequirements;

use super::validate_initialization::InitializationOutcome;

/// A range of values, the lowest and the highest (B5.4). An operating-range or state-range row states its measured range and its
/// declared bounds with it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValueRange {
    /// The lowest value.
    pub minimum: f64,
    /// The highest value.
    pub maximum: f64,
}

/// The words the engine names for the rows of the Validate table (B9.9): the page composes no label of its own.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ValidateLabels {
    /// The run of the current settings in a scenario: "Current settings".
    pub current_settings: String,
    /// The run of the candidate in a scenario: "Candidate".
    pub candidate: String,
    /// The candidate's overall row: "Candidate overall".
    pub candidate_overall: String,
    /// The mark of a limit that Automatic generated: "Automatic".
    pub automatic: String,
    /// The words of the row that counts the passing guards (B9.9): "Ranges, states and transfer".
    pub guards: String,
    /// The limit column of that row (B9.9): "Within declared limits".
    pub guards_limit: String,
}

/// The Validate plot (B5.4, B9.6; the Validate page specification, "Results and plot"): the measured output of the target-response
/// case, as the candidate's run and the current settings' run recorded it. A served loop has one measured output, because the
/// engine's support rule refuses the channels of several outputs, so the plot has one.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ValidationPlot {
    /// The plot's title: "Measured response".
    pub title: String,
    /// The measured output's role in words (B9.9): "Measurement".
    pub role: String,
    /// The output's unit.
    pub unit: String,
    /// The candidate's run, named "Tuned".
    pub tuned: PlotSeries,
    /// The current settings' run, named "Current"; absent when it has no sample.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<PlotSeries>,
}

/// One run's trace on the Validate plot: its name, its times, and its values, where a sample that is not good is absent (B9.6).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlotSeries {
    /// The run's name in words: "Tuned" or "Current".
    pub name: String,
    /// The times in s from the start of the run, reduced as B4.5 reduces the Tune plot.
    pub time_seconds: Vec<f64>,
    /// The output at each time; absent where the sample is not good.
    pub values: Vec<Option<f64>>,
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
    /// The requirements this validation judged against: the candidate's, frozen when it was tuned (B5.4).
    pub requirements: TuningRequirements,
    /// The overall result of the candidate (N8.5): Fail when a requirement fails, Inconclusive when one lacks evidence,
    /// and Pass otherwise.
    pub status: RequirementStatus,
    /// Whether Apply is available: the model is qualified and the candidate passed every requirement (B5.4).
    pub apply_available: bool,
    /// Why Apply is not available, with the next action that fixes it (B5.5); absent when Apply is available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<Failure>,
    /// Why the model replaced the configured plant for every case (B5.3): the target-response run of the current settings started
    /// outside the settling band around the model's operating point. Absent when the configured plant ran the cases, and when the
    /// model was the plant from the start.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plant_reason: Option<String>,
    /// The stage evidence after this result, from which `stages` is computed: the model, the candidate and this validation's
    /// result, which passed or did not (B5.4). The page gates Apply on it (B9.4).
    pub stage_evidence: StageEvidence,
    /// The status of every stage after this result.
    pub stages: Vec<StageAvailability>,
    /// The independent model check as the table's first row (B5.2, N3.4): its normalized error and its limit in percent of the
    /// measured output's span. Absent when the model was already qualified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_agreement: Option<RequirementOutcome>,
    /// The Manual-to-Auto entry case of an Auto commissioning (B5.3, B5.4); absent when this validation ran no such case.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initialization: Option<InitializationOutcome>,
    /// The words the engine names for the rows of the table (B9.9).
    pub labels: ValidateLabels,
    /// The measured response of the target-response case for the plot (B5.4, B9.6); absent when the candidate's run has no sample.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plot: Option<ValidationPlot>,
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
    /// The scenario's name in words (B9.9), for example "Target response".
    pub label: String,
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
    /// What the requirement checks, in words (B9.9), for example "Maximum settling time".
    pub label: String,
    /// The channel it measures, named by its role in the loop and its unit (B9.9).
    pub channel: RequirementChannel,
    /// Which way the limit bounds the value (B5.4): at most, or at least.
    pub limit_side: LimitSide,
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
    /// Whether Automatic generated the limit (B4.3): a settling time or a maximum error that the criteria record names.
    #[serde(default)]
    pub automatic: bool,
    /// Whether the engine generated the check from the loop's declaration, not from a requirement the engineer sets: an operating-range,
    /// state-range or signal-bound check, or a check of the Manual-to-Auto entry (B9.9). The page counts the passing guards in one row.
    #[serde(default)]
    pub guard: bool,
    /// The lowest and the highest good value of an operating-range or state-range channel in its window (B5.4); absent for the
    /// other checks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measured_range: Option<ValueRange>,
    /// The declared bounds of an operating-range or state-range requirement, which are its limit (B5.4); absent for the other
    /// checks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_range: Option<ValueRange>,
}

/// The channel a requirement measures, as the page names it (B9.9): its role in the loop, such as Measurement, Output or
/// Setpoint, or an auxiliary channel's id, and its unit. A parameter or a controller state is named by its own name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequirementChannel {
    /// The role in words, never the symbol.
    pub role: String,
    /// The channel's unit; empty means dimensionless.
    pub unit: String,
    /// The channel's symbol, which the page shows on hover (B9.9); empty when the requirement names no symbol.
    pub symbol: String,
}

/// Which way a requirement's limit bounds its value (B5.4): the value must stay at or below the limit, or at or above it (a
/// dwell's shortest stay).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum LimitSide {
    /// The value must be at or below the limit.
    AtMost,
    /// The value must be at or above the limit.
    AtLeast,
}
