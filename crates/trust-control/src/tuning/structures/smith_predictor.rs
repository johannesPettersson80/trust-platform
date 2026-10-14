//! The Smith predictor (specification 35, C9). The model's step response from the controller output to the measurement is reduced to
//! first order with delay by the two-point method, which gives the model's gain `K`, time constant `T` and delay `θ`. The PI is designed
//! with Automatic on the model without its delay, for the requirement's settling time less the delay. A settling time that is no
//! longer than the delay is refused. The TypeScript it ports is `smithPredictorDesign.ts`.

#![allow(clippy::module_name_repetitions)] // `design_smith_predictor` names the structure.

use super::path::{
    delayed_plant, design_pi_on_path, open_loop_step_response, operating_point, operating_value,
    reduced_plant, two_point_path, LoopFacts, PathSupport,
};
use super::texts;
use crate::identification::refusal::Refusal;
use crate::model::GeneralModel;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::structure::ParallelGains;

/// The design of a Smith predictor (C9): the PI for the model without its delay, and the model's values that the program keeps.
#[derive(Debug, Clone, PartialEq)]
pub struct SmithDesign {
    /// The PI, with its derivative gain zero.
    pub pi: ParallelGains,
    /// The model's gain `K`, from the two-point reduction.
    pub model_gain: f64,
    /// The model's time constant `T` in seconds.
    pub model_time_constant_seconds: f64,
    /// The model's delay `θ` in seconds.
    pub model_delay_seconds: f64,
}

/// Designs the Smith predictor's PI and the model values that it keeps (C9).
///
/// # Errors
/// Returns a refusal when the model is not one input and one output, when the settling time is no longer than the model's delay,
/// when the response cannot be reduced, or when the PI cannot meet its requirements.
pub fn design_smith_predictor(
    model: &GeneralModel,
    facts: &LoopFacts,
    task: &TuningRequirements,
    support: &dyn PathSupport,
) -> Result<SmithDesign, Refusal> {
    let identity = model.identity();
    if identity.inputs.len() != 1 || identity.outputs.len() != 1 {
        return Err(Refusal::new(texts::SMITH_MODEL));
    }
    let plant = delayed_plant(model)?;
    let response = open_loop_step_response(&plant, 0, "Smith predictor")?;
    let path = two_point_path(&response.times, &response.outputs[0], "Smith predictor")?;

    // With a correct model the loop responds as the delay-free loop, delayed by θ: that loop must settle θ sooner.
    let settling_time_seconds = task.settling_time_seconds - path.delay_seconds;
    if settling_time_seconds.is_nan() || settling_time_seconds <= 0.0 {
        return Err(Refusal::new(texts::smith_delay(path.delay_seconds)));
    }
    let requirements = TuningRequirements {
        settling_time_seconds,
        duration_seconds: (task.duration_seconds - path.delay_seconds)
            .max(2.0 * settling_time_seconds),
        ..task.clone()
    };
    let (operating_inputs, operating_outputs) = operating_point(model);
    let pi = design_pi_on_path(
        &reduced_plant(&path, 0.0),
        facts,
        (
            operating_value(&operating_inputs, 0),
            operating_value(&operating_outputs, 0),
        ),
        &requirements,
        support,
    )?;
    Ok(SmithDesign {
        pi,
        model_gain: path.gain,
        model_time_constant_seconds: path.time_constant_seconds,
        model_delay_seconds: path.delay_seconds,
    })
}

#[cfg(test)]
#[path = "smith_predictor_tests.rs"]
mod tests;
