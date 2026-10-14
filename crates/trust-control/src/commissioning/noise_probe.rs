//! The transfer noise probe (specification 35, A6.4), from `transferNoiseProbe.ts`: the trial's plant, held at rest in Manual
//! for one second without the transfer, shows only its measurement noise. That noise sets the Auto-entry allowance when no
//! identification recording measured it, and the transfer check starts the controller where the plant actually rests.

use super::starting_point::model_with_operating_outputs;
use crate::model::GeneralModel;

/// The resting probe of a transfer trial (`restingNoiseProbeRequest`): one second at rest, in whole steps. The probe carries
/// no events, so its plant stays in Manual and no transfer happens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RestingProbe {
    /// The trial's step in ms, or 20 ms when the trial names none.
    pub step_ms: f64,
    /// The probe's duration in ms: whole steps, at least ten of them.
    pub duration_ms: f64,
    /// The most cycles the run may take: the probe's steps and one more for the end of the run.
    pub max_total_cycles: u64,
}

/// The resting probe for a trial whose step is `step_ms`: one second at rest, and at least ten steps (`restingNoiseProbeRequest`).
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)] // A count of cycles: a whole number well below 2^53.
pub fn resting_probe(step_ms: Option<f64>) -> RestingProbe {
    let step_ms = step_ms.filter(|step| *step > 0.0).unwrap_or(20.0);
    let cycles = (1000.0 / step_ms).round().max(10.0);
    RestingProbe {
        step_ms,
        duration_ms: cycles * step_ms,
        max_total_cycles: cycles as u64 + 1,
    }
}

/// One recorded value of a channel: whether its quality is good, and the value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RecordedValue {
    /// Whether the sample's quality is `good`.
    pub good: bool,
    /// The sample's value.
    pub value: f64,
}

/// The good, finite values of a channel: the `TypeScript`'s filter `quality === "good" && Number.isFinite(value)`.
fn good_values(values: &[RecordedValue]) -> Vec<f64> {
    values
        .iter()
        .filter(|sample| sample.good && sample.value.is_finite())
        .map(|sample| sample.value)
        .collect()
}

/// Half the range of a recorded signal's good samples, which is its noise half-amplitude; `None` without at least four good
/// samples (`recordedHalfRange`, A6.4).
#[must_use]
pub fn recorded_half_range(values: &[RecordedValue]) -> Option<f64> {
    let good = good_values(values);
    if good.len() < 4 {
        return None;
    }
    let (lowest, highest) = good.iter().fold(
        (f64::INFINITY, f64::NEG_INFINITY),
        |(lowest, highest), value| (lowest.min(*value), highest.max(*value)),
    );
    Some((highest - lowest) / 2.0)
}

/// The mean of a channel's good samples, `None` when it has none (the mean of `restingOperatingModel`).
#[must_use]
#[allow(clippy::cast_precision_loss)] // A count of samples, far below 2^53.
pub fn recorded_mean(values: &[RecordedValue]) -> Option<f64> {
    let good = good_values(values);
    if good.is_empty() {
        return None;
    }
    let sum = good.iter().fold(0.0, |sum, value| sum + value);
    Some(sum / good.len() as f64)
}

/// The model as seen from the plant's own rest, measured by the resting probe: the outputs take their resting means as the
/// operating point, and the dynamics stay the fitted ones (`restingOperatingModel`). The model is returned unchanged when it
/// has no operating point or a resting mean is missing.
#[must_use]
pub fn resting_operating_model(
    model: &GeneralModel,
    resting_means: &[Option<f64>],
) -> GeneralModel {
    let means: Option<Vec<f64>> = resting_means.iter().copied().collect();
    match (model.identity().operating_point.is_some(), means) {
        (true, Some(outputs)) => model_with_operating_outputs(model, outputs),
        _ => model.clone(),
    }
}

#[cfg(test)]
#[path = "noise_probe_tests.rs"]
mod tests;
