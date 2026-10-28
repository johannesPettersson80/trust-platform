//! The measured-disturbance feedforward (specification 35, C8). The PI is designed on the controller output's path to the measurement.
//! The feedforward is designed from both paths, each reduced to first order with delay by the two-point method:
//! `K_ff = −K_d/K_u`, `T_lead = τ_u`, and `T_lag = τ_d + max(0, θ_d − θ_u)`, the last two when the program has a lead and a lag.
//! Tune shows the largest deviation of the measurement after a disturbance step, on the model, with the PI alone and with the
//! feedforward. The TypeScript it ports is `feedforwardPidDesign.ts`.

#![allow(clippy::module_name_repetitions)] // `design_feedforward` and `FeedforwardDesign` name the structure.

use trust_control_math::sampled_loop::{
    largest_disturbance_deviation, DisturbanceCase, Feedforward, LeadLag, SamplePi,
};
use trust_control_math::tuning::two_point::TwoPointReduction;

use super::path::{
    delayed_plant, design_pi_on_path, open_loop_step_response, operating_point, operating_value,
    reduced_plant, sampled_model, two_point_path, LoopFacts, PathSupport,
};
use super::texts;
use crate::identification::refusal::Refusal;
use crate::model::GeneralModel;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::structure::ParallelGains;

/// The model's channels of a feedforward (C8): the controller output and the measured disturbance among its inputs, and the
/// measurement among its outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedforwardChannels {
    /// The index of the controller output among the model's inputs.
    pub controller_input: usize,
    /// The index of the measured disturbance among the model's inputs.
    pub disturbance_input: usize,
    /// The index of the measurement among the model's outputs.
    pub measurement: usize,
}

/// The largest deviation of the measurement after the disturbance step, on the model (C8).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LargestDeviation {
    /// With the PI alone.
    pub feedback: f64,
    /// With the PI and the feedforward.
    pub feedforward: f64,
}

/// The design of a feedforward (C8): both paths, the three feedforward values, the disturbance step, the largest deviations, and the PI.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedforwardDesign {
    /// The path from the controller output to the measurement.
    pub output_path: TwoPointReduction,
    /// The path from the measured disturbance to the measurement.
    pub disturbance_path: TwoPointReduction,
    /// The feedforward gain `K_ff`.
    pub gain: f64,
    /// The lead time `T_lead` in seconds, when the program has a lead and a lag.
    pub lead_seconds: Option<f64>,
    /// The lag time `T_lag` in seconds, when the program has a lead and a lag.
    pub lag_seconds: Option<f64>,
    /// The disturbance step that the deviations are computed for: the disturbance's identification step.
    pub disturbance_step: f64,
    /// The largest deviation on the model, with the PI alone and with the feedforward.
    pub largest_deviation: LargestDeviation,
    /// The PI on the controller output's path, with its derivative gain zero.
    pub pi: ParallelGains,
}

/// The two reduced paths of a feedforward (C8): from the controller output to the measurement, and from the measured disturbance to it.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedforwardPaths {
    /// The path from the controller output to the measurement.
    pub output: TwoPointReduction,
    /// The path from the measured disturbance to the measurement.
    pub disturbance: TwoPointReduction,
}

/// Checks that the model has the feedforward's two inputs, the controller output and the measured disturbance, and one output,
/// the measurement (C8). The two inputs are different inputs, and the measurement is an output of the model.
///
/// # Errors
/// Returns a refusal naming the feedforward's channels when the model or the channel indices do not fit.
fn check_channels(model: &GeneralModel, channels: &FeedforwardChannels) -> Result<(), Refusal> {
    let identity = model.identity();
    let input_count = identity.inputs.len();
    let output_count = identity.outputs.len();
    if input_count != 2
        || output_count != 1
        || channels.controller_input >= input_count
        || channels.disturbance_input >= input_count
        || channels.controller_input == channels.disturbance_input
        || channels.measurement >= output_count
    {
        return Err(Refusal::new(texts::FEEDFORWARD_CHANNELS));
    }
    Ok(())
}

/// Reduces both paths of a feedforward to first order with delay by the two-point method (C7, C8), from the open-loop step responses
/// of the model: the controller output's path and the disturbance's path, each to the measurement.
///
/// # Errors
/// Returns a refusal when the model does not have the feedforward's channels, or when a path cannot be reduced.
pub fn reduce_paths(
    model: &GeneralModel,
    channels: &FeedforwardChannels,
) -> Result<FeedforwardPaths, Refusal> {
    check_channels(model, channels)?;
    let plant = delayed_plant(model)?;
    let output_response =
        open_loop_step_response(&plant, channels.controller_input, "feedforward")?;
    let output = two_point_path(
        &output_response.times,
        &output_response.outputs[channels.measurement],
        "controller output",
    )?;
    let disturbance_response =
        open_loop_step_response(&plant, channels.disturbance_input, "feedforward")?;
    let disturbance = two_point_path(
        &disturbance_response.times,
        &disturbance_response.outputs[channels.measurement],
        "disturbance",
    )?;
    Ok(FeedforwardPaths {
        output,
        disturbance,
    })
}

/// Designs the PI on the controller output's path and the feedforward from both paths (C8).
///
/// # Errors
/// Returns a refusal when the model does not have the two inputs and one output of the feedforward, when the disturbance has no
/// identification step, when a path cannot be reduced, or when the PI cannot meet its requirements.
pub fn design_feedforward(
    model: &GeneralModel,
    facts: &LoopFacts,
    channels: &FeedforwardChannels,
    disturbance_step: Option<f64>,
    with_lead_lag: bool,
    task: &TuningRequirements,
    support: &dyn PathSupport,
) -> Result<FeedforwardDesign, Refusal> {
    check_channels(model, channels)?;
    let Some(step) = disturbance_step.filter(|value| *value > 0.0) else {
        return Err(Refusal::new(texts::FEEDFORWARD_STEP));
    };

    let FeedforwardPaths {
        output: output_path,
        disturbance: disturbance_path,
    } = reduce_paths(model, channels)?;
    let plant = delayed_plant(model)?;
    let (operating_inputs, operating_outputs) = operating_point(model);
    let pi = design_pi_on_path(
        &reduced_plant(&output_path, output_path.delay_seconds),
        facts,
        (
            operating_value(&operating_inputs, channels.controller_input),
            operating_value(&operating_outputs, channels.measurement),
        ),
        task,
        support,
    )?;

    // K_ff cancels the disturbance in steady state. The lead cancels the output path's lag, and the lag reproduces the
    // disturbance's, with the delay the disturbance takes longer than the output as further lag.
    let gain = -disturbance_path.gain / output_path.gain;
    let lead_lag = with_lead_lag.then(|| LeadLag {
        lead_seconds: output_path.time_constant_seconds,
        lag_seconds: disturbance_path.time_constant_seconds
            + (disturbance_path.delay_seconds - output_path.delay_seconds).max(0.0),
    });

    let period_seconds = facts.task_period_ms / 1000.0;
    let sampled = sampled_model(model, period_seconds, "feedforward")?;
    let case = DisturbanceCase {
        system: &sampled.system,
        sample_time_seconds: sampled.sample_time_seconds,
        delay_seconds: plant.delay_seconds,
        controller_input: channels.controller_input,
        disturbance_input: channels.disturbance_input,
        measurement: channels.measurement,
        law: SamplePi {
            proportional_gain: pi.proportional_gain,
            integral_gain: pi.integral_gain,
        },
        period_seconds,
        disturbance_step: step,
        duration_seconds: task.duration_seconds,
    };
    let feedback = largest_disturbance_deviation(&case, None);
    let with_feedforward =
        largest_disturbance_deviation(&case, Some(Feedforward { gain, lead_lag }));
    Ok(FeedforwardDesign {
        output_path,
        disturbance_path,
        gain,
        lead_seconds: lead_lag.map(|lead| lead.lead_seconds),
        lag_seconds: lead_lag.map(|lead| lead.lag_seconds),
        disturbance_step: step,
        largest_deviation: LargestDeviation {
            feedback,
            feedforward: with_feedforward,
        },
        pi,
    })
}

#[cfg(test)]
#[path = "feedforward_tests.rs"]
mod tests;
