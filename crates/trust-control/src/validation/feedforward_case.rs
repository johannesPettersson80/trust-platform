//! The measured-disturbance case of a feedforward's Validate (specification 35, C8). It runs on the model only: from the operating point
//! with the setpoint unchanged, the measured disturbance steps by its identification step at the start, and the candidate's PI and
//! feedforward act on the model. The measurement MUST stay within the error allowance from the settling time to the end of the test
//! (C8, the error allowance of the requirements, as C1 states it for the target).
//!
//! The simulation is `trust_control_math::sampled_loop::disturbance_trace`, the same closed loop Tune's deviations are read from, so the
//! case judges what Tune predicted. The judgement is a pure function of the trace, so its rule is tested without a simulation.

#![allow(clippy::module_name_repetitions)] // `measured_disturbance_case` and `DisturbanceOutcome` name the case they belong to.

use trust_control_math::sampled_loop::{
    disturbance_trace, DisturbanceCase, Feedforward, LeadLag, SamplePi,
};

use crate::identification::refusal::Refusal;
use crate::model::GeneralModel;
use crate::tuning::project_seed::FeedforwardTerms;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::structure::ParallelGains;
use crate::tuning::structures::feedforward::FeedforwardChannels;
use crate::tuning::structures::path::{delayed_plant, sampled_model};

/// The sentence of a measured-disturbance case that judges no sample: its test ends before its settling time (C8).
pub const NO_JUDGED_SAMPLE: &str =
    "The measured-disturbance case has no sample after the settling time to judge.";

/// The outcome of the measured-disturbance case (C8): the largest deviation of the measurement from its operating value from the settling
/// time to the end of the test, the error allowance it is held to, and whether it stays within the allowance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisturbanceOutcome {
    /// The largest magnitude of the measurement's deviation from the settling time to the end of the test.
    pub largest_after_settling: f64,
    /// The error allowance of the requirements.
    pub allowance: f64,
    /// Whether every sample from the settling time to the end of the test is within the allowance.
    pub passed: bool,
}

/// Runs the measured-disturbance case of a feedforward on the model (C8): the PI on the controller output with the candidate's gains,
/// and the feedforward with the candidate's gain and its lead and lag when it has them, against a disturbance step at the start.
///
/// # Errors
/// The model cannot be sampled at the task period, or its plant has no realization (`delayed_plant`, `sampled_model`); or the test has no
/// sample from the settling time to its end.
pub fn measured_disturbance_case(
    model: &GeneralModel,
    channels: &FeedforwardChannels,
    gains: ParallelGains,
    terms: FeedforwardTerms,
    disturbance_step: f64,
    period_seconds: f64,
    requirements: &TuningRequirements,
) -> Result<DisturbanceOutcome, Refusal> {
    let plant = delayed_plant(model)?;
    let sampled = sampled_model(model, period_seconds, "feedforward")?;
    let case = DisturbanceCase {
        system: &sampled.system,
        sample_time_seconds: sampled.sample_time_seconds,
        delay_seconds: plant.delay_seconds,
        controller_input: channels.controller_input,
        disturbance_input: channels.disturbance_input,
        measurement: channels.measurement,
        law: SamplePi {
            proportional_gain: gains.proportional_gain,
            integral_gain: gains.integral_gain,
        },
        period_seconds,
        disturbance_step,
        duration_seconds: requirements.duration_seconds,
    };
    let feedforward = Feedforward {
        gain: terms.gain,
        lead_lag: lead_lag_of(terms),
    };
    let trace = disturbance_trace(&case, Some(feedforward));
    judge(&trace, sampled.sample_time_seconds, requirements)
}

/// The lead and lag of a feedforward's terms, when it has both (C8).
fn lead_lag_of(terms: FeedforwardTerms) -> Option<LeadLag> {
    match (terms.lead_seconds, terms.lag_seconds) {
        (Some(lead_seconds), Some(lag_seconds)) => Some(LeadLag {
            lead_seconds,
            lag_seconds,
        }),
        _ => None,
    }
}

/// Judges a measurement trace (C8): every sample at or after the settling time, sample `k` at `k` periods, must lie within the error
/// allowance of the requirements. The judged samples run to the end of the trace, which is the test's duration.
///
/// # Errors
/// No sample of the trace lies at or after the settling time.
pub fn judge(
    trace: &[f64],
    sample_time_seconds: f64,
    requirements: &TuningRequirements,
) -> Result<DisturbanceOutcome, Refusal> {
    let allowance = requirements.maximum_error;
    let judged: Vec<f64> = trace
        .iter()
        .enumerate()
        .filter(|(index, _)| {
            time_of(*index, sample_time_seconds) >= requirements.settling_time_seconds
        })
        .map(|(_, value)| value.abs())
        .collect();
    if judged.is_empty() {
        return Err(Refusal::new(NO_JUDGED_SAMPLE));
    }
    let largest_after_settling = judged.iter().copied().fold(0.0_f64, f64::max);
    Ok(DisturbanceOutcome {
        largest_after_settling,
        allowance,
        passed: largest_after_settling <= allowance,
    })
}

/// The time of a sample: its index times the sample period, in seconds.
#[allow(clippy::cast_precision_loss)] // A sample index is far below 2⁵³.
fn time_of(index: usize, sample_time_seconds: f64) -> f64 {
    index as f64 * sample_time_seconds
}

#[cfg(test)]
#[path = "feedforward_case_tests.rs"]
mod tests;
