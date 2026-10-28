//! The cascade designed together (specification 35, C6). The inner PI is designed first, on the path from the plant input to the inner
//! measurement, for the inner requirements. The outer PI is then designed on the outer measurement's response to a step of the inner
//! setpoint through the closed inner loop, for the outer requirements. Both paths are reduced to first order with delay by the
//! two-point method (C7). Without inner requirements the inner loop settles in a fifth of the outer settling time, with its band and
//! step scaled to the inner range. The TypeScript it ports is `cascadePidDesign.ts`.

#![allow(clippy::module_name_repetitions)] // `CascadeDesign` and its records name the module's subject.

use trust_control_math::identify::gain::ControllerAction;
use trust_control_math::identify::ParameterInterval;
use trust_control_math::sampled_loop::{unit_step_response, SamplePi};

use super::channel::ChannelRequirements;
use super::path::{
    delayed_plant, design_pi_on_path, operating_point, operating_value, reduced_plant,
    sampled_model, settling_horizon, two_point_path, LoopFacts, PathSupport,
};
use super::texts;
use crate::identification::refusal::Refusal;
use crate::model::GeneralModel;
use crate::tuning::prediction::RecordShaping;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::shortfall::{GainLimits, OutputLimits};
use crate::tuning::structure::ParallelGains;

/// The divisor of the outer settling time that gives the inner settling time, when the inner measurement declares no requirements of
/// its own: one fifth (numerical annex N10, C6).
const INNER_SETTLING_DIVISOR: f64 = 5.0;
/// The number of fifths of the outer settling time that give the inner duration, when the inner measurement declares no requirements of
/// its own: two fifths (numerical annex N10, C6).
const INNER_DURATION_FIFTHS: f64 = 2.0;

/// One measurement of a cascade: its output in the model, its unit, and its range. The inner range scales the outer band (C6).
#[derive(Debug, Clone, PartialEq)]
pub struct CascadeMeasurement {
    /// The measurement's index among the model's outputs.
    pub model_output: usize,
    /// The measurement's unit.
    pub unit: String,
    /// The measurement's range, as its loop signal declares it.
    pub range: OutputLimits,
}

/// The loop facts that a cascade's design reads: the controller action, the task period, the bounds of each controller's gains, the
/// output rate limits, and the plant input's unit and range, which the inner controller drives.
#[derive(Debug, Clone, PartialEq)]
pub struct CascadeLoop {
    /// The controller action (N6).
    pub action: ControllerAction,
    /// The task period of the controllers in milliseconds.
    pub task_period_ms: f64,
    /// The bounds of the inner controller's gains (N6.4).
    pub inner_gain_limits: GainLimits,
    /// The bounds of the outer controller's gains (N6.4).
    pub outer_gain_limits: GainLimits,
    /// The output rate limits per second of the loop: its declared rates, or `±10⁶` where it declares none.
    pub rate_limits: ParameterInterval,
    /// The plant input's unit: the inner controller's manipulated unit.
    pub plant_input_unit: String,
    /// The plant input's range: the inner controller's output range.
    pub plant_input_limits: OutputLimits,
}

/// The requirements of a cascade: the loop's own, and each measurement's own when it declares one.
#[derive(Debug, Clone, PartialEq)]
pub struct CascadeRequirements {
    /// The loop's own requirements.
    pub task: TuningRequirements,
    /// The inner measurement's own requirements, when it declares them.
    pub inner: Option<ChannelRequirements>,
    /// The outer measurement's own requirements, when it declares them.
    pub outer: Option<ChannelRequirements>,
}

/// The two PIs of a cascade, each with its derivative gain zero (C6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CascadeDesign {
    /// The inner controller's PI.
    pub inner: ParallelGains,
    /// The outer controller's PI.
    pub outer: ParallelGains,
}

/// Designs the two PIs of a cascade (C6): the inner on the plant input's path to the inner measurement, then the outer on the outer
/// measurement's response to an inner setpoint step through the closed inner loop.
///
/// # Errors
/// Returns a refusal when the model is a transfer function, when its channels do not match the cascade, when a path cannot be
/// reduced, or when a PI cannot meet its requirements.
pub fn design_cascade(
    model: &GeneralModel,
    cascade: &CascadeLoop,
    inner: &CascadeMeasurement,
    outer: &CascadeMeasurement,
    requirements: &CascadeRequirements,
    support: &dyn PathSupport,
) -> Result<CascadeDesign, Refusal> {
    if matches!(model, GeneralModel::TransferFunction { .. }) {
        return Err(Refusal::new(texts::CASCADE_MODEL));
    }
    let identity = model.identity();
    let output_count = identity.outputs.len();
    if identity.inputs.len() != 1
        || inner.model_output >= output_count
        || outer.model_output >= output_count
    {
        return Err(Refusal::new(texts::CASCADE_CHANNELS));
    }
    let period_seconds = cascade.task_period_ms / 1000.0;
    let sampled = sampled_model(model, period_seconds, "cascade")?;
    let plant = delayed_plant(model)?;
    let horizon = settling_horizon(&plant, "cascade")?;
    let delay = plant.delay_seconds;
    let (operating_inputs, operating_outputs) = operating_point(model);
    let ratio =
        (inner.range.maximum - inner.range.minimum) / (outer.range.maximum - outer.range.minimum);

    // The inner controller first, on the path from the plant input to the inner measurement.
    let open = unit_step_response(
        &sampled.system,
        sampled.sample_time_seconds,
        delay,
        horizon,
        inner.model_output,
        None,
    );
    let inner_path = two_point_path(&open.times, &open.values, "cascade inner")?;
    let outer_task = match &requirements.outer {
        Some(own) => own.apply(&requirements.task),
        None => requirements.task.clone(),
    };
    let inner_task = match &requirements.inner {
        Some(own) => own.apply(&requirements.task),
        None => scaled_inner(&outer_task, ratio),
    };
    let inner_pi = design_pi_on_path(
        &reduced_plant(&inner_path, inner_path.delay_seconds),
        &inner_facts(cascade, inner),
        (
            operating_value(&operating_inputs, 0),
            operating_value(&operating_outputs, inner.model_output),
        ),
        &inner_task,
        support,
    )?;

    // Then the outer controller, on the outer measurement's response to an inner setpoint step through the closed inner loop.
    let law = SamplePi {
        proportional_gain: inner_pi.proportional_gain,
        integral_gain: inner_pi.integral_gain,
    };
    let closed = unit_step_response(
        &sampled.system,
        sampled.sample_time_seconds,
        delay,
        horizon,
        outer.model_output,
        Some((inner.model_output, law)),
    );
    let outer_path = two_point_path(&closed.times, &closed.values, "cascade outer")?;
    let outer_pi = design_pi_on_path(
        &reduced_plant(&outer_path, outer_path.delay_seconds),
        &outer_facts(cascade, inner, outer),
        (
            operating_value(&operating_outputs, inner.model_output),
            operating_value(&operating_outputs, outer.model_output),
        ),
        &outer_task,
        support,
    )?;
    Ok(CascadeDesign {
        inner: inner_pi,
        outer: outer_pi,
    })
}

/// The facts of the inner loop's design (C6): the plant input is its manipulated variable and the inner measurement its process variable.
fn inner_facts(cascade: &CascadeLoop, inner: &CascadeMeasurement) -> LoopFacts {
    LoopFacts {
        action: cascade.action,
        task_period_ms: cascade.task_period_ms,
        gain_limits: cascade.inner_gain_limits.clone(),
        output_limits: cascade.plant_input_limits,
        rate_limits: cascade.rate_limits,
        shaping: RecordShaping::default(),
        process_unit: inner.unit.clone(),
        manipulated_unit: cascade.plant_input_unit.clone(),
    }
}

/// The facts of the outer loop's design (C6): the inner setpoint is its manipulated variable, in the inner measurement's unit and range.
fn outer_facts(
    cascade: &CascadeLoop,
    inner: &CascadeMeasurement,
    outer: &CascadeMeasurement,
) -> LoopFacts {
    LoopFacts {
        action: cascade.action,
        task_period_ms: cascade.task_period_ms,
        gain_limits: cascade.outer_gain_limits.clone(),
        output_limits: inner.range,
        rate_limits: cascade.rate_limits,
        shaping: RecordShaping::default(),
        process_unit: outer.unit.clone(),
        manipulated_unit: inner.unit.clone(),
    }
}

/// The inner requirements of a cascade whose inner measurement declares none (C6): the outer band, error and step scaled to the
/// inner range by `ratio`, and a fifth of the outer settling time as the inner settling time, with two fifths of it as its duration.
fn scaled_inner(outer: &TuningRequirements, ratio: f64) -> TuningRequirements {
    TuningRequirements {
        reference_change: outer.reference_change * ratio,
        maximum_error: outer.maximum_error * ratio,
        settling_band: outer.settling_band * ratio,
        settling_time_seconds: outer.settling_time_seconds / INNER_SETTLING_DIVISOR,
        duration_seconds: INNER_DURATION_FIFTHS * outer.settling_time_seconds
            / INNER_SETTLING_DIVISOR,
        ..outer.clone()
    }
}

#[cfg(test)]
#[path = "cascade_tests.rs"]
mod tests;
