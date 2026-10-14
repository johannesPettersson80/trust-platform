//! The sensor-lag fit of a held step (B3.5, N2.2, N2.4, N3.2): the rule that prepares the search from the step-response
//! settings and the reference analysis, runs the grid search of `trust_control_math::sensor_lag`, and attaches the warnings
//! and the boundary flags the specification requires (`fitSensorLagStep` in `sensorLagStepIdentification.ts`).
//!
//! The grids of the sensor lag are the declared `sensor_time_constant` or, when none is declared, the default grid of the
//! annex. The other grids and the measurement assumptions are those of the reference analysis.

use trust_control_math::identify::step_response::StepResponseWindow;
use trust_control_math::sensor_lag::{
    fit_sensor_lag_step, HeldSample, SearchRange, SensorLagCandidate, SensorLagConsistency,
    SensorLagError, SensorLagSearch, StepReference,
};

use crate::identification::data::IdentificationSample;
use crate::identification::evidence::ModelFitPoint;
use crate::identification::refusal::Refusal;
use crate::identification::sensor_lag_texts::{
    GRID_BOUNDARY_WARNING, GRID_TOO_LARGE, HYPOTHESIS_WARNING, NO_CANDIDATE,
    NO_CONSISTENT_POINT_WARNING, SEARCH_BOUNDARY_WARNING, SEARCH_TOO_LARGE,
    TIMESTAMP_SHIFT_WARNING,
};
use crate::identification::step_response::{SearchGrid, StepResponseAnalysis};

/// The sensor-time grid of a sensor-lag fit when the settings declare none: 0.1 s to 10 s in steps of 0.1 s.
pub const DEFAULT_SENSOR_TIME_CONSTANT: SearchGrid = SearchGrid {
    minimum: 0.1,
    maximum: 10.0,
    step: 0.1,
};

/// The sensor-lag analysis of one held step (`SensorLagStepAnalysis`): the reference windows and step, the best candidate,
/// the measurement-consistent set, the boundary and zero-delay findings, the warnings and the fit's points.
#[derive(Debug, Clone)]
pub struct SensorLagStepAnalysis {
    /// The steady baseline window of the reference analysis.
    pub baseline: StepResponseWindow,
    /// The final window of the reference analysis.
    pub final_window: StepResponseWindow,
    /// The fitted input change `Δu`.
    pub input_change: f64,
    /// The time of the step in seconds.
    pub step_time_seconds: f64,
    /// The end of the held step in seconds.
    pub hold_end_seconds: f64,
    /// The sensor-time grid the fit searched, in seconds.
    pub sensor_time_constant: SearchRange,
    /// The least-squares candidate over the whole grid.
    pub best: SensorLagCandidate,
    /// The measurement-consistent set of the search.
    pub consistency: SensorLagConsistency,
    /// Whether any best parameter lies on the edge of its grid.
    pub boundary_limited: bool,
    /// Whether the best dead time is zero and the search allowed zero, a one-sided finding rather than an unresolved bound.
    pub zero_delay_lower_bound: bool,
    /// The warnings of the fit, in the order the specification gives them.
    pub warnings: Vec<String>,
    /// The measured output and the fitted prediction at each held sample.
    pub points: Vec<ModelFitPoint>,
}

/// Fits the sensor-lag model to the held step of a reference analysis (`fitSensorLagStep`).
///
/// The held samples are those from the step time to the end of the hold; the baseline bounds come from the samples of the
/// baseline window before the step.
///
/// # Errors
/// Returns a [`Refusal`] naming the limit when the search is too large, when a grid is too large, or when no baseline and
/// gain lie within the qualified windows.
pub fn fit_sensor_lag(
    samples: &[IdentificationSample],
    reference: &StepResponseAnalysis,
    sensor_time_constant: Option<SearchGrid>,
) -> Result<SensorLagStepAnalysis, Refusal> {
    let settings = &reference.settings;
    let sensor = sensor_time_constant.unwrap_or(DEFAULT_SENSOR_TIME_CONSTANT);
    let hold: Vec<HeldSample> = samples
        .iter()
        .filter(|sample| {
            sample.time_seconds >= reference.step_time_seconds
                && sample.time_seconds < reference.hold_end_seconds
        })
        .map(|sample| HeldSample {
            time_seconds: sample.time_seconds,
            output: sample.output,
        })
        .collect();
    let pre_step: Vec<f64> = samples
        .iter()
        .filter(|sample| {
            sample.time_seconds >= reference.baseline.start_seconds
                && sample.time_seconds < reference.step_time_seconds
        })
        .map(|sample| sample.output)
        .collect();
    let search = SensorLagSearch {
        delay: range(&settings.delay),
        time_constant: range(&settings.time_constant),
        sensor_time_constant: range(&sensor),
        measurement_bound: settings.measurement_bound,
        baseline_step: settings.baseline_step,
        gain_step: settings.gain_step,
        timestamp_shift_seconds: settings.timestamp_shift_seconds,
    };
    let step = StepReference {
        baseline: &reference.baseline,
        final_window: &reference.final_window,
        input_change: reference.input_change,
        step_time_seconds: reference.step_time_seconds,
    };
    let fit = fit_sensor_lag_step(&hold, &pre_step, &step, &search)
        .map_err(|error| Refusal::new(refusal_sentence(error)))?;
    let points = hold
        .iter()
        .zip(&fit.predicted)
        .map(|(sample, &predicted)| ModelFitPoint {
            time_seconds: sample.time_seconds,
            measured: sample.output,
            predicted,
            residual: sample.output - predicted,
        })
        .collect();
    let boundary_limited = fit.amplitude_on_boundary || fit.shape_on_boundary;
    Ok(SensorLagStepAnalysis {
        baseline: reference.baseline.clone(),
        final_window: reference.final_window.clone(),
        input_change: reference.input_change,
        step_time_seconds: reference.step_time_seconds,
        hold_end_seconds: reference.hold_end_seconds,
        sensor_time_constant: range(&sensor),
        zero_delay_lower_bound: fit.best.delay_seconds == 0.0 && settings.delay.minimum == 0.0,
        warnings: warnings(
            boundary_limited,
            fit.amplitude_on_boundary,
            fit.consistency.count,
            settings.timestamp_shift_seconds,
        ),
        best: fit.best,
        consistency: fit.consistency,
        boundary_limited,
        points,
    })
}

/// The search range of a declared grid.
fn range(grid: &SearchGrid) -> SearchRange {
    SearchRange {
        minimum: grid.minimum,
        maximum: grid.maximum,
        step: grid.step,
    }
}

/// The sentence of a refused fit.
fn refusal_sentence(error: SensorLagError) -> &'static str {
    match error {
        SensorLagError::SearchTooLarge => SEARCH_TOO_LARGE,
        SensorLagError::GridTooLarge => GRID_TOO_LARGE,
        SensorLagError::NoCandidate => NO_CANDIDATE,
    }
}

/// The warnings of a fit, in the order the specification gives them: the hypothesis that every fit states, then the boundary
/// warnings, the measurement-consistency warning when no grid point is consistent, and the timestamp-shift warning.
fn warnings(
    boundary_limited: bool,
    amplitude_on_boundary: bool,
    consistent_count: usize,
    timestamp_shift_seconds: f64,
) -> Vec<String> {
    let mut warnings = vec![HYPOTHESIS_WARNING.to_string()];
    if boundary_limited {
        warnings.push(SEARCH_BOUNDARY_WARNING.to_string());
    }
    if amplitude_on_boundary {
        warnings.push(GRID_BOUNDARY_WARNING.to_string());
    }
    if consistent_count == 0 {
        warnings.push(NO_CONSISTENT_POINT_WARNING.to_string());
    }
    if timestamp_shift_seconds != 0.0 {
        warnings.push(TIMESTAMP_SHIFT_WARNING.to_string());
    }
    warnings
}

#[cfg(test)]
#[path = "sensor_lag_tests.rs"]
pub(crate) mod tests;
