//! The durations of an experiment: the limits a plan's duration must keep (A7), the duration of a project step test
//! from its applied hold (`projectExperimentHold.ts`, `experimentRecording.ts`), and the runtime window of a Test
//! recording (A7).

use crate::limits::{
    MAXIMUM_EXPERIMENT_DURATION_MS, MINIMUM_EXPERIMENT_DURATION_MS, RECORDING_DURATION_LIMIT_MS,
};
use crate::loop_definition::{Experiment, LoopDefinition};

use super::number_text::js_number;
use super::plan::{non_empty, PlanInput};

/// The duration of a project-owned experiment, in ms: the recording of its hold when the loop declares a hold
/// parameter, else the requested duration rounded to whole milliseconds.
pub(super) fn project_duration_ms(
    input: &PlanInput<'_>,
    experiment: &Experiment,
) -> Result<f64, String> {
    let hold = project_hold(input, experiment)?;
    Ok(hold.unwrap_or_else(|| (input.duration_seconds * 1000.0).round()))
}

/// The hold of a project step test, checked against the parameter's bounds and the declared maximum duration
/// (`projectHoldRecording`). `None` when the loop declares no hold parameter.
fn project_hold(input: &PlanInput<'_>, experiment: &Experiment) -> Result<Option<f64>, String> {
    let Some(symbol) = non_empty(experiment.hold_parameter.as_deref()) else {
        return Ok(None);
    };
    let loop_definition = input.loop_definition;
    let parameter = loop_definition
        .parameter_binding
        .as_ref()
        .and_then(|binding| {
            binding
                .parameters
                .iter()
                .find(|parameter| parameter.symbol == symbol)
        });
    let hold = input.hold_seconds.filter(|hold| hold.is_finite());
    let within = match (parameter, hold) {
        (Some(parameter), Some(hold)) => hold >= parameter.minimum && hold <= parameter.maximum,
        _ => false,
    };
    let (Some(hold), true) = (hold, within) else {
        let range = parameter.map_or_else(
            || "its–declared".to_string(),
            |parameter| {
                format!(
                    "{}–{}",
                    js_number(parameter.minimum),
                    js_number(parameter.maximum)
                )
            },
        );
        return Err(format!(
            "Apply a {symbol} value within {range} s before reviewing the project test."
        ));
    };
    let duration_ms = project_step_recording_ms(loop_definition, hold);
    // A missing declared duration has no maximum to exceed; Identify names it before it records (B3.4 step 1).
    if let Some(declared_seconds) = experiment.duration_seconds {
        let declared_ms = (declared_seconds * 1000.0).round();
        if duration_ms > declared_ms {
            return Err(format!(
                "A {} s hold needs {} s, more than the declared {} s maximum.",
                js_number(hold),
                js_number(duration_ms / 1000.0),
                js_number(declared_seconds)
            ));
        }
    }
    Ok(Some(duration_ms))
}

/// The duration of the recording that holds a project step test, in ms: its baseline, every scan of the applied
/// hold, and one restoration sample (`projectStepRecordingMs`).
#[must_use]
pub fn project_step_recording_ms(loop_definition: &LoopDefinition, hold_seconds: f64) -> f64 {
    let period = loop_definition.observed_sample_timing.nominal_period_ms;
    let baseline_seconds = loop_definition
        .experiment
        .as_ref()
        .and_then(|experiment| experiment.baseline_seconds)
        .unwrap_or(0.0);
    let baseline_ms = (baseline_seconds * 1000.0 / period).ceil() * period;
    let hold_ms = (hold_seconds * 1000.0 / period - 1e-9).ceil() * period;
    baseline_ms + hold_ms + period
}

/// The runtime window of a Test recording, in whole milliseconds: its planned duration plus a restoration margin of at least
/// 10 s or 10 % of the duration, whichever is larger, capped at the recording limit (A7; `experimentRecordingWindowMs`). The
/// margin rounds up to whole milliseconds, because a recording starts with whole milliseconds (E4.2).
#[must_use]
pub fn experiment_recording_window_ms(planned_duration_ms: u64) -> u64 {
    let margin = planned_duration_ms.div_ceil(10).max(10_000);
    RECORDING_DURATION_LIMIT_MS.min(planned_duration_ms.saturating_add(margin))
}

/// Whether a requested duration lies within the limits of an experiment: 1 s to 24 h of process time (A7).
pub(super) fn duration_within_limits(duration_seconds: f64) -> bool {
    let (minimum, maximum) = duration_limits_seconds();
    duration_seconds.is_finite() && duration_seconds >= minimum && duration_seconds <= maximum
}

/// The shortest and longest experiment in s (A7).
#[allow(clippy::cast_precision_loss)] // The limits of 1 s and 24 h are exact in f64.
fn duration_limits_seconds() -> (f64, f64) {
    (
        MINIMUM_EXPERIMENT_DURATION_MS as f64 / 1000.0,
        MAXIMUM_EXPERIMENT_DURATION_MS as f64 / 1000.0,
    )
}
