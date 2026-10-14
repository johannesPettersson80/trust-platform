//! The predicted response of a Tune candidate on the selected model (specification 35, B4.5, B4.6 and B9.6): the traces of
//! the setpoint, the measurement and the command, the predicted settling time and overshoot (B4.3; numerical annex N8.3),
//! the margins (N7.7), and the reason a candidate has no predicted outcome. The closed loop is `tuning::prediction`'s, run
//! under the conditions of the same `SelectedModelPredictor` that Automatic's requirement check reads, so the plot, the
//! numbers and the outcome describe one response.
//!
//! The settling time, the overshoot and the outcome read every controller period of the test. The response the answer carries
//! for the plot keeps each group's extremes of every trace when the prediction is long (B4.5).

use crate::identification::plant::IdentifiedModel;
use crate::loop_definition::LoopDefinition;
use crate::messages::{CandidateOutcome, TuneResponse, TuneRole, TuneTrace};
use crate::tuning::candidate_margins::{pid_margins, CandidateMargins};
use crate::tuning::prediction::{
    predict_response, PredictedGains, PredictionConditions, ResponsePoint, SelectedModelPredictor,
};
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::shortfall::{has_load, Outcome};
use crate::tuning::structure::{is_exactly_zero, ParallelGains};
use trust_control_math::sampling::plot_extrema::extrema_indices;

/// A prediction longer than this many points is reduced for the plot (B4.5).
const PLOT_POINT_LIMIT: usize = 6_000;
/// The groups a long prediction is split into for the plot; each keeps its extremes of every trace (B4.5).
const PLOT_GROUPS: usize = 1_000;

/// The reason of a candidate whose predictions run but leave no sample from the settling deadline on.
const NO_SAMPLE_AFTER_DEADLINE: &str =
    "The predicted response has no sample from the settling deadline to the end of the test, so Validate decides.";

/// The symbols and units that the traces of the predicted response carry (B4.5, B9.6): the loop's setpoint, or its
/// measurement when it declares none, its measurement and its command, each with the unit the loop declares.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TraceNames<'a> {
    setpoint: &'a str,
    measurement: &'a str,
    command: &'a str,
    process_unit: &'a str,
    manipulated_unit: &'a str,
}

impl<'a> TraceNames<'a> {
    /// The names of the traces of `loop_definition`.
    pub(crate) fn of(loop_definition: &'a LoopDefinition) -> Self {
        let signals = &loop_definition.signals;
        let measurement = &signals.process_variable;
        Self {
            setpoint: signals
                .setpoint
                .as_ref()
                .map_or(measurement.symbol.as_str(), |setpoint| {
                    setpoint.symbol.as_str()
                }),
            measurement: measurement.symbol.as_str(),
            command: signals.manipulated_variable.symbol.as_str(),
            process_unit: measurement.unit.as_str(),
            manipulated_unit: signals.manipulated_variable.unit.as_str(),
        }
    }
}

/// The predicted setpoint response of a candidate as the result shows it (B4.5): its traces, or the reason there are none,
/// with its settling time and its overshoot.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CandidateResponse {
    /// The traces of the response, when it is predicted.
    pub(crate) response: Option<TuneResponse>,
    /// Why there is no response, when there is none.
    pub(crate) unavailable: Option<String>,
    /// The predicted settling time in s (N8.3), when the response settles within the test; whether it settles by the
    /// deadline is the outcome's to say.
    pub(crate) settling_time_seconds: Option<f64>,
    /// The predicted overshoot in percent of the setpoint change (N8.3), when the response is predicted.
    pub(crate) overshoot_percent: Option<f64>,
}

/// The predicted setpoint response of a candidate's gains on the predictor's loop (B4.5, B9.6): the traces with the loop's
/// symbols and units, the settling time and the overshoot, or the prediction's refusal when there is no response. The test
/// is the requirements' setpoint change from the operating point, over their duration, with no load.
pub(crate) fn candidate_prediction(
    predictor: &SelectedModelPredictor<'_>,
    gains: ParallelGains,
    requirements: &TuningRequirements,
    names: TraceNames<'_>,
) -> CandidateResponse {
    let conditions = trial_conditions(predictor, requirements, requirements.reference_change, 0.0);
    match predict_response(
        predictor.model,
        &predictor.predicted_gains(gains),
        None,
        &conditions,
    ) {
        Ok(response) => {
            let target = conditions.operating_output + conditions.reference_change;
            CandidateResponse {
                settling_time_seconds: settling_time_seconds(
                    &response.points,
                    target,
                    requirements.settling_band,
                ),
                overshoot_percent: overshoot_percent(
                    &response.points,
                    target,
                    conditions.reference_change,
                ),
                response: Some(traces(&response.points, names)),
                unavailable: None,
            }
        }
        Err(refusal) => CandidateResponse {
            response: None,
            unavailable: Some(refusal.0),
            settling_time_seconds: None,
            overshoot_percent: None,
        },
    }
}

/// The margins of a candidate's record on the selected model (B4.6, N7.7, C0), as the result shows them: the margins, with the
/// measurement filter lag of the record, or the sentence that says why there are none.
pub(crate) fn candidate_margins(
    model: &IdentifiedModel,
    predicted: &PredictedGains,
) -> (Option<CandidateMargins>, Option<String>) {
    match pid_margins(model, predicted) {
        Ok(margins) => (Some(margins), None),
        Err(refusal) => (None, Some(refusal.0)),
    }
}

/// How a candidate stands against the mandatory requirements on its prediction (B4.4, B4.5): the outcome that
/// `tuning::shortfall::predicted_outcome` gave, with the reason when no outcome is predicted.
pub(crate) fn candidate_outcome(
    outcome: Outcome,
    predictor: &SelectedModelPredictor<'_>,
    gains: ParallelGains,
    requirements: &TuningRequirements,
) -> CandidateOutcome {
    match outcome {
        Outcome::Meets => CandidateOutcome::Meets,
        Outcome::Misses(shortfall) => CandidateOutcome::Misses { shortfall },
        Outcome::Unpredicted => CandidateOutcome::Unpredicted {
            reason: unpredicted_reason(predictor, gains, requirements),
        },
    }
}

/// Why a candidate has no predicted outcome (N5.12: "Without a model prediction, ... Validate decides"): the refusal of its
/// setpoint trial, else the refusal of its load trial when the requirements declare a load, else that no sample lies from
/// the settling deadline on.
fn unpredicted_reason(
    predictor: &SelectedModelPredictor<'_>,
    gains: ParallelGains,
    requirements: &TuningRequirements,
) -> String {
    let gains = predictor.predicted_gains(gains);
    let setpoint = trial_conditions(predictor, requirements, requirements.reference_change, 0.0);
    let load = requirements
        .constant_input_disturbance
        .filter(|_| has_load(requirements))
        .map(|load| trial_conditions(predictor, requirements, 0.0, load));
    [Some(setpoint), load]
        .into_iter()
        .flatten()
        .find_map(|conditions| predict_response(predictor.model, &gains, None, &conditions).err())
        .map_or_else(|| NO_SAMPLE_AFTER_DEADLINE.to_string(), |refusal| refusal.0)
}

/// The conditions of one trial on the predictor's loop (B9.6, N8.1), as `SelectedModelPredictor` runs Automatic's
/// requirement check: the task period, the test's duration, the trial's setpoint change and constant load, the operating
/// point, and the output and rate limits.
fn trial_conditions(
    predictor: &SelectedModelPredictor<'_>,
    requirements: &TuningRequirements,
    reference_change: f64,
    input_disturbance: f64,
) -> PredictionConditions {
    PredictionConditions {
        period_seconds: predictor.task_period_ms / 1000.0,
        duration_seconds: requirements.duration_seconds,
        reference_change,
        operating_input: predictor.operating_input,
        operating_output: predictor.operating_output,
        input_disturbance,
        output_minimum: predictor.output_limits.minimum,
        output_maximum: predictor.output_limits.maximum,
        rate_minimum: predictor.rate_limits.minimum,
        rate_maximum: predictor.rate_limits.maximum,
    }
}

/// The traces of a response (B4.5, B9.6): the setpoint, and the candidate's measurement and command, on the times of the
/// response.
fn traces(points: &[ResponsePoint], names: TraceNames<'_>) -> TuneResponse {
    let setpoint = series(points, |point| point.setpoint);
    let measurement = series(points, |point| point.candidate_process_value);
    let command = series(points, |point| point.candidate_manipulated);
    // The plot keeps each group's extremes of every trace (B4.5); the assessments above read the full prediction.
    let kept = extrema_indices(
        points.len(),
        &[&setpoint, &measurement, &command],
        PLOT_GROUPS,
        PLOT_POINT_LIMIT,
    );
    let pick = |values: &[f64]| {
        kept.iter()
            .map(|index| values[*index])
            .collect::<Vec<f64>>()
    };
    TuneResponse {
        time_seconds: pick(&series(points, |point| point.time_seconds)),
        traces: vec![
            trace(
                TuneRole::Setpoint,
                names.setpoint,
                names.process_unit,
                pick(&setpoint),
            ),
            trace(
                TuneRole::Measurement,
                names.measurement,
                names.process_unit,
                pick(&measurement),
            ),
            trace(
                TuneRole::Command,
                names.command,
                names.manipulated_unit,
                pick(&command),
            ),
        ],
    }
}

/// One trace of the response: its role, its symbol, its unit and its values.
fn trace(role: TuneRole, symbol: &str, unit: &str, values: Vec<f64>) -> TuneTrace {
    TuneTrace {
        role,
        symbol: symbol.to_string(),
        unit: unit.to_string(),
        values,
    }
}

/// One value of each point of a response.
fn series(points: &[ResponsePoint], value: impl Fn(&ResponsePoint) -> f64) -> Vec<f64> {
    points.iter().map(value).collect()
}

/// The predicted settling time (B4.5; N8.3, "settling into a target band by a deadline"): the arrival time, the time of the
/// first sample after which every sample stays within the settling band around the target. A response that is outside
/// the band at the end of the test has none. A late arrival is still reported: the engineer sees how late it is, and the
/// outcome says that it misses the deadline (B9.5).
fn settling_time_seconds(points: &[ResponsePoint], target: f64, band: f64) -> Option<f64> {
    let last_outside = points
        .iter()
        .rposition(|point| (point.candidate_process_value - target).abs() > band);
    let arrival = match last_outside {
        Some(index) => points.get(index + 1)?,
        None => points.first()?,
    };
    Some(arrival.time_seconds)
}

/// The predicted overshoot in percent of the setpoint change (B4.3; N8.3, "overshoot beyond a declared step target"): the
/// largest excursion beyond the target in the direction of the step, `max(0, maxₖ sign(ΔR)·(yₖ − target))`, over `|ΔR|`,
/// times 100. `None` without samples, or for a change that is zero or not finite.
fn overshoot_percent(points: &[ResponsePoint], target: f64, reference_change: f64) -> Option<f64> {
    if points.is_empty() || !reference_change.is_finite() || is_exactly_zero(reference_change) {
        return None;
    }
    let direction = reference_change.signum();
    let beyond = points
        .iter()
        .map(|point| direction * (point.candidate_process_value - target))
        .fold(0.0_f64, f64::max);
    Some(beyond / reference_change.abs() * 100.0)
}

#[cfg(test)]
#[path = "prediction_tests.rs"]
mod tests;
