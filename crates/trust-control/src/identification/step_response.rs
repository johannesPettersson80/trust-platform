//! Step-response identification (`stepResponseIdentification.ts`): the bounded step analysis of a recording with one
//! held input step and its restoration (numerical annex N2.2 to N2.6 and N3.2; specification 35, A4.3, A4.5, A10.3
//! and B3.4). The types are here; the settings and their admission rules are in `step_response/settings.rs`, the
//! preparation of the recording in `step_response/preparation.rs`, the measurement of a held step in
//! `step_response/measurement.rs`, and the fit with its acceptance rules and consistent set in
//! `step_response/analysis.rs`. The arithmetic is the library's (`trust_control_math::identify::step_response`); those
//! modules apply the rules in order and word their refusals. The sensor-time grid and the model-adequacy limits of a
//! sensor-lag comparison (N2.10) are optional settings: the form sends them with the held-step settings, and only that
//! comparison reads them.

// The step-response records keep the names of the `TypeScript` records.
#![allow(clippy::module_name_repetitions)]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trust_control_math::identify::step_response::{ConsistentSet, StepResponseWindow};

use crate::identification::sensor_lag_comparison::ModelAdequacy;

mod analysis;
mod measurement;
mod preparation;
mod settings;

pub use analysis::analyze_step_response;
pub use measurement::{measure_step_response, qualify_step_response};
pub(crate) use preparation::fit_record_span;
pub use settings::{step_response_settings_problem, valid_step_response_settings};

/// The tolerance of the step rules' comparisons of times, inputs and window statistics (`EPSILON`).
const EPSILON: f64 = 1e-9;

/// The normalized error limit of N3.2 when a caller names none: the annex default
/// (`DEFAULT_MAXIMUM_NORMALIZED_ERROR`).
pub(crate) const DEFAULT_MAXIMUM_NORMALIZED_ERROR: f64 = 0.15;

/// The refusal of settings that [`valid_step_response_settings`] does not admit.
pub(crate) const INVALID_SETTINGS: &str = "Invalid step-response identification settings.";

/// The side of the baseline on which the sustained onset is sought (N2.5): `above` when the mean of the hold's last tenth lies above the
/// baseline mean, and when it does not differ from it (such a hold finds no onset), `below` when it lies below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum OnsetSide {
    /// The onset threshold lies above the baseline mean.
    Above,
    /// The onset threshold lies below the baseline mean.
    Below,
}

/// A search grid of one parameter (N2.2, N2.4): the values `minimum + k·step` from the minimum up to the maximum.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchGrid {
    /// The smallest value of the grid.
    pub minimum: f64,
    /// The largest value of the grid.
    pub maximum: f64,
    /// The spacing of the grid's values.
    pub step: f64,
}

/// The settings of a bounded step analysis (N2.4 to N2.6, B3.4): the windows, their steadiness limits, the measurement
/// bound, the onset rule, the fit's grids, the consistent set's lattice steps and the timestamp-shift sensitivity.
/// They are engineering inputs, not plant coefficients (N2.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StepResponseSettings {
    /// The length of the baseline window, which ends at the step, in s.
    pub baseline_duration_seconds: f64,
    /// The length of the final window, which ends at the restoration, in s.
    pub final_duration_seconds: f64,
    /// The largest output range a steady window may have.
    pub maximum_window_range: f64,
    /// The largest output slope a steady window may have, per s.
    pub maximum_window_slope_per_second: f64,
    /// The absolute measurement bound `b_m` of the consistent set (N2.4).
    pub measurement_bound: f64,
    /// The sustained-onset threshold `h`, an offset from the baseline mean (N2.5).
    pub onset_threshold: f64,
    /// The number of consecutive samples beyond the threshold that establish onset (N2.5).
    pub onset_samples: usize,
    /// The delay grid, in s.
    pub delay: SearchGrid,
    /// The time-constant grid, in s.
    pub time_constant: SearchGrid,
    /// The spacing of the consistent set's baseline lattice.
    pub baseline_step: f64,
    /// The spacing of the consistent set's gain lattice.
    pub gain_step: f64,
    /// The timestamp shift of the sensitivity study, at most one sample time (N2.4), in s.
    pub timestamp_shift_seconds: f64,
    /// The sensor-time grid of a sensor-lag comparison (`sensorTimeConstant`; N2.10); the annex's default grid when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensor_time_constant: Option<SearchGrid>,
    /// The residual requirements of a sensor-lag comparison (`modelAdequacy`; N2.10); the annex's defaults when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_adequacy: Option<ModelAdequacy>,
}

/// A problem in the analysis settings: the field it belongs to and what to enter instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SettingsProblem {
    /// The field's path in the settings' JSON words, such as `measurementBound` or `timeConstant.minimum`.
    pub field: String,
    /// The sentence that says what to enter.
    pub message: String,
}

/// One held sample with the fitted model's value (N2.2, N3.1): its time, the measured output `y_k`, the fitted output
/// `ŷ_k = y0 + K·s_k` and the residual `y_k − ŷ_k`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StepResponsePoint {
    /// The sample's time, in s.
    pub time_seconds: f64,
    /// The measured output.
    pub measured: f64,
    /// The fitted output.
    pub predicted: f64,
    /// The measured less the fitted output.
    pub residual: f64,
}

/// The best grid fit of a held step (N2.2): the measured baseline mean `y0`, the least-squares gain `K` of the best
/// delay and time constant, and the squared error it leaves.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StepBestFit {
    /// The baseline mean `y0`; measured, not fitted.
    pub baseline: f64,
    /// The gain `K`.
    pub gain: f64,
    /// The grid delay `L`, in s.
    pub delay_seconds: f64,
    /// The grid time constant `T`, in s.
    pub time_constant_seconds: f64,
    /// The squared error `E = Σ(y_k − y0 − K·s_k)²` over the held samples.
    pub squared_error: f64,
}

/// The analysis of a held step (N2.2 to N2.6): the measured windows, the step and its hold, the onset and the 63 %
/// level, the best first-order-with-delay fit, the measurement-consistent set, the warnings, the fitted held samples,
/// the fraction of the fitted final change reached (A10.3) and the final-window check (N2.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StepResponseAnalysis {
    /// The settings the analysis used.
    pub settings: StepResponseSettings,
    /// The baseline window, which ends at the step.
    pub baseline: StepResponseWindow,
    /// The final window, which ends at the restoration.
    #[serde(rename = "final")]
    pub final_window: StepResponseWindow,
    /// The step amplitude `Δu`: the mean held input less the mean input over the baseline (N2.2).
    pub input_change: f64,
    /// The time of the step, in s.
    pub step_time_seconds: f64,
    /// The time of the restoration, which ends the hold, in s.
    pub hold_end_seconds: f64,
    /// The sample time of the fitted samples, in s.
    pub sample_time_seconds: f64,
    /// The detectable onset after the step, in s; absent when no sustained run crosses the threshold (N2.5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detectable_onset_seconds: Option<f64>,
    /// The onset threshold `y0 ± h` (N2.5).
    pub onset_threshold: f64,
    /// The side of the baseline the onset threshold lies on (N2.5), so that the page words the criterion without deriving it from the
    /// windows. Absent in a record kept before the engine stated it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onset_side: Option<OnsetSide>,
    /// The 63 % level `y0 + 0.632·K·Δu`.
    pub level63: f64,
    /// The best grid fit.
    pub best: StepBestFit,
    /// The grid combinations consistent with the measurement bound (N2.4).
    pub consistency: ConsistentSet,
    /// Whether the best fit lies on a search boundary.
    pub boundary_limited: bool,
    /// The warnings the engineer reads: a search boundary, an empty consistent set, a timestamp shift.
    pub warnings: Vec<String>,
    /// The held samples with the fitted output.
    pub points: Vec<StepResponsePoint>,
    /// The fraction of the fitted final change the held response reached (A10.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_change_fraction: Option<f64>,
    /// The final-window mean less the fitted steady value `y0 + K·Δu`: a reported check, never an input (N2.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_window_difference: Option<f64>,
}

/// The measurement of a held step without a fit (`measureStepResponse`; N2.5, N2.6): the windows, the step and its
/// hold, the onset and the 63 % level of the measured change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StepResponseMeasurement {
    /// The settings the measurement used.
    pub settings: StepResponseSettings,
    /// The baseline window, which ends at the step.
    pub baseline: StepResponseWindow,
    /// The final window, which ends at the restoration.
    #[serde(rename = "final")]
    pub final_window: StepResponseWindow,
    /// The step amplitude `Δu`: the mean held input less the mean input over the baseline (N2.2).
    pub input_change: f64,
    /// The time of the step, in s.
    pub step_time_seconds: f64,
    /// The time of the restoration, which ends the hold, in s.
    pub hold_end_seconds: f64,
    /// The sample time of the prepared samples, in s.
    pub sample_time_seconds: f64,
    /// The detectable onset after the step, in s; absent when no sustained run crosses the threshold (N2.5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detectable_onset_seconds: Option<f64>,
    /// The onset threshold `y0 ± h` (N2.5).
    pub onset_threshold: f64,
    /// The side of the baseline the onset threshold lies on (N2.5), so that the page words the criterion without deriving it from the
    /// windows. Absent in a record kept before the engine stated it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onset_side: Option<OnsetSide>,
    /// The 63 % level `y0 + 0.632·(ȳ_final − y0)` of the measured change.
    pub level63: f64,
    /// The warnings the engineer reads: a timestamp shift.
    pub warnings: Vec<String>,
}

#[cfg(test)]
#[path = "step_response_tests.rs"]
mod tests;
