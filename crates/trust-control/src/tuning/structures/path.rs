//! The paths that the structures design (specification 35, C6 to C9; numerical annex N4.7 and N5.15): a model's settling horizon,
//! its open-loop step response, the two-point reduction of a settled step response, the model at the task period, and the
//! single-loop design of a reduced path (B4.4).
//!
//! A path is a plant of its own, at its own operating point, so the design of a loop on it needs the engine's response predictor
//! and pole placement for that plant. The engine supplies them through [`PathSupport`]; no rule here predicts a response.

#![allow(clippy::module_name_repetitions)] // `PathSupport` and `LoopFacts` name the module's subject.

use trust_control_math::discretize::{discretize_state_space, DiscretizationMethod};
use trust_control_math::identify::gain::ControllerAction;
use trust_control_math::identify::ParameterInterval;
use trust_control_math::linalg::{eigenvalues, NumericalTolerance};
use trust_control_math::model::{StateSpace, Timing};
use trust_control_math::simulate::{open_loop_response, time_settings, DelayedSystem};
use trust_control_math::tuning::two_point::{
    two_point_reduction, ReductionRefusal, TwoPointReduction,
};
use trust_control_math::tuning::FirstOrderPlusDelay;

use super::texts;
use crate::identification::refusal::Refusal;
use crate::model::limits::{MAXIMUM_DELAY_STATES, MAXIMUM_TIME_RESPONSE_VALUES};
use crate::model::prediction::delayed_system_of;
use crate::model::GeneralModel;
use crate::tuning::design_to_requirements::design_to_requirements;
use crate::tuning::placement::PolePlacement;
use crate::tuning::plant::TuningPlant;
use crate::tuning::prediction::RecordShaping;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::shortfall::{DesignContext, GainLimits, OutputLimits, ResponsePredictor};
use crate::tuning::structure::{ParallelGains, Structure};

/// The tolerance of the eigenvalues that set a settling horizon: the tolerance the linear algebra of `trust-control-math` uses.
const POLE_TOLERANCE: NumericalTolerance = NumericalTolerance {
    absolute: 1e-12,
    relative: 1e-10,
    condition_limit: 1e12,
};

/// A decay rate at or below this does not settle within a horizon (TypeScript `settlingHorizon`).
const SLOWEST_RATE_LIMIT: f64 = 1e-9;

/// The engine's part of a path design (Automatic, B4.4 and B4.5): the response predictor and the pole placement of a single loop
/// closed around a reduced path. A path is a plant of its own, so the engine makes both for the plant it is given, for the loop's
/// facts and the design's requirements. The production support is `tuning::path_support::ReducedPathSupport`.
pub trait PathSupport {
    /// The response predictor of the loop on `path` (B4.5): the loop's task period, output and rate limits and record shaping from
    /// `facts`, at the path's operating input and output `operating`.
    fn predictor(
        &self,
        path: &TuningPlant,
        facts: &LoopFacts,
        operating: (f64, f64),
    ) -> Box<dyn ResponsePredictor + '_>;

    /// The pole placement of the loop on `path` for the requirements and the design context (N5.12(c)); the context carries the
    /// predictor, the period and the operating point that the placement reads.
    fn placement(
        &self,
        path: &TuningPlant,
        requirements: &TuningRequirements,
        context: &DesignContext<'_>,
    ) -> Box<dyn PolePlacement + '_>;
}

/// The facts of one loop that a single-loop design reads: the controller action, the task period, the parameter bounds, the
/// manipulated variable's range, the units of the measurement and of the manipulated variable, and what the predictor of the loop
/// reads besides its model: the output rate limits and the record shaping of the candidate it predicts (B4.5, C0).
#[derive(Debug, Clone, PartialEq)]
pub struct LoopFacts {
    /// The controller action (N6).
    pub action: ControllerAction,
    /// The task period of the controller in milliseconds.
    pub task_period_ms: f64,
    /// The bounds of the loop's parameters (N6.4).
    pub gain_limits: GainLimits,
    /// The range of the manipulated variable, which limits the controller's output.
    pub output_limits: OutputLimits,
    /// The output rate limits per second: the loop's declared rates, or `±10⁶` where it declares none.
    pub rate_limits: ParameterInterval,
    /// The setpoint weights and the measurement filter that the predicted record holds (C0).
    pub shaping: RecordShaping,
    /// The unit of the measurement.
    pub process_unit: String,
    /// The unit of the manipulated variable.
    pub manipulated_unit: String,
}

/// The open-loop unit-step response of every output of a model to one of its inputs (TypeScript `openLoopStepResponse`).
#[derive(Debug, Clone, PartialEq)]
pub struct StepResponse {
    /// The sample times in seconds.
    pub times: Vec<f64>,
    /// Each output's values at the sample times: the first index is the output, the second the sample.
    pub outputs: Vec<Vec<f64>>,
    /// Each output's last value, the response's final value.
    pub finals: Vec<f64>,
}

/// A model at the task period (TypeScript `sampledStateSpace`): its discrete matrices and their sample period in seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct SampledModel {
    /// The discrete matrices.
    pub system: StateSpace,
    /// The sample period in seconds.
    pub sample_time_seconds: f64,
}

/// The settling horizon of a plant (TypeScript `settlingHorizon`): ten time constants of its slowest pole, plus its delay. A
/// continuous pole `p` has the decay rate `|Re p|`, and a discrete pole `z` the rate `|ln|z|| / T`.
///
/// # Errors
/// Returns a refusal when the poles cannot be computed, or when a pole does not decay at a rate above `10⁻⁹` (the plant does
/// not settle).
pub fn settling_horizon(plant: &DelayedSystem, subject: &str) -> Result<f64, Refusal> {
    let solution = eigenvalues(&plant.system.a, &POLE_TOLERANCE)
        .map_err(|_| Refusal::new(texts::poles_unavailable(subject)))?;
    let rates: Vec<f64> = solution
        .values
        .iter()
        .map(|pole| match &plant.timing {
            Timing::Continuous => pole.real.abs(),
            Timing::Discrete {
                sample_time_seconds,
            } => pole.real.hypot(pole.imaginary).ln().abs() / sample_time_seconds,
        })
        .collect();
    if rates.is_empty()
        || rates
            .iter()
            .any(|rate| rate.is_nan() || *rate <= SLOWEST_RATE_LIMIT)
    {
        return Err(Refusal::new(texts::does_not_settle(subject)));
    }
    let slowest = rates.iter().copied().fold(f64::INFINITY, f64::min);
    Ok(10.0 / slowest + plant.delay_seconds)
}

/// The open-loop unit-step response of every output to one input of a plant, from its zero state (TypeScript
/// `openLoopStepResponse`). The samples run to the settling horizon: a continuous plant takes `⌊4096 / outputs⌋ − 1` equal
/// steps to it, and a discrete plant takes its own sample period, at most the same number of steps.
///
/// # Errors
/// Returns a refusal when the horizon cannot be set, when the response cannot be calculated, or when the response has not
/// reached its steady state at the horizon.
pub fn open_loop_step_response(
    plant: &DelayedSystem,
    input: usize,
    subject: &str,
) -> Result<StepResponse, Refusal> {
    let horizon = settling_horizon(plant, subject)?;
    let output_count = plant.system.outputs();
    let count = (MAXIMUM_TIME_RESPONSE_VALUES / output_count.max(1)).saturating_sub(1);
    let (step_seconds, steps) = match &plant.timing {
        Timing::Discrete {
            sample_time_seconds,
        } => (
            *sample_time_seconds,
            count.min(ceiling_count(horizon / sample_time_seconds)),
        ),
        Timing::Continuous => (horizon / as_seconds(count), count),
    };
    let mut input_vector = vec![0.0; plant.system.inputs()];
    if let Some(value) = input_vector.get_mut(input) {
        *value = 1.0;
    }
    let initial_state = vec![0.0; plant.system.states()];
    let duration_seconds = as_seconds(steps) * step_seconds;
    let settings = time_settings(
        &plant.system,
        plant.timing,
        step_seconds,
        duration_seconds,
        &initial_state,
        &input_vector,
        MAXIMUM_TIME_RESPONSE_VALUES,
    )
    .map_err(|_| Refusal::new(texts::responses_unavailable(subject)))?;
    let response = open_loop_response(plant, &settings, MAXIMUM_DELAY_STATES)
        .map_err(|_| Refusal::new(texts::responses_unavailable(subject)))?;
    let times: Vec<f64> = response
        .points
        .iter()
        .map(|point| point.time_seconds)
        .collect();
    if times.last().is_none_or(|last| *last < horizon - 1e-9) {
        return Err(Refusal::new(texts::too_slow(subject)));
    }
    let outputs: Vec<Vec<f64>> = (0..output_count)
        .map(|output| {
            response
                .points
                .iter()
                .map(|point| point.zero_state[output])
                .collect()
        })
        .collect();
    let finals: Vec<f64> = outputs
        .iter()
        .map(|values| values.last().copied().unwrap_or(0.0))
        .collect();
    Ok(StepResponse {
        times,
        outputs,
        finals,
    })
}

/// The two-point reduction of a settled step response to `K·exp(−L·s)/(τ·s + 1)` (N5.15, C7): `τ = 1.5·(t₆₃ − t₂₈)` and
/// `L = t₆₃ − τ`, or `L = 0` and `τ = t₆₃` when `L` would be negative. The gain is the response's final change.
///
/// # Errors
/// Returns a refusal naming `subject` when the response has no steady-state change, or does not reach the fractions of it.
pub fn two_point_path(
    times: &[f64],
    values: &[f64],
    subject: &str,
) -> Result<TwoPointReduction, Refusal> {
    two_point_reduction(times, values).map_err(|refusal| match refusal {
        ReductionRefusal::NoSteadyChange => Refusal::new(texts::no_steady_gain(subject)),
        _ => Refusal::new(texts::no_step_response(subject)),
    })
}

/// The model at the task period (TypeScript `sampledStateSpace`): a discrete model keeps its own matrices and sample period, and
/// a continuous one is converted by the exact zero-order hold at `period_seconds` (N4.7(a)).
///
/// # Errors
/// Returns a refusal for a transfer function, which the structures do not design on, and for a continuous model that cannot be
/// sampled at the period.
pub fn sampled_model(
    model: &GeneralModel,
    period_seconds: f64,
    subject: &str,
) -> Result<SampledModel, Refusal> {
    match model {
        GeneralModel::TransferFunction { .. } => {
            Err(Refusal::new(texts::state_space_only(subject)))
        }
        GeneralModel::DiscreteStateSpace {
            system,
            sample_time_seconds,
            ..
        } => Ok(SampledModel {
            system: system.clone(),
            sample_time_seconds: *sample_time_seconds,
        }),
        GeneralModel::ContinuousStateSpace { system, .. } => {
            let discrete =
                discretize_state_space(system, period_seconds, DiscretizationMethod::ExactZoh)
                    .map_err(|_| Refusal::new(texts::cannot_sample(subject)))?;
            Ok(SampledModel {
                system: discrete.system,
                sample_time_seconds: period_seconds,
            })
        }
    }
}

/// The model's plant with its delay, from the model's own matrices and timing (`delayed_system_of`).
///
/// # Errors
/// Returns the sentence of the model's refusal when a transfer function cannot be realized.
pub fn delayed_plant(model: &GeneralModel) -> Result<DelayedSystem, Refusal> {
    delayed_system_of(model).map_err(|refusal| Refusal::new(refusal.to_string()))
}

/// The first-order-plus-delay plant of a reduced path (TypeScript `firstOrderPathModel`): its gain and time constant, and the
/// delay given. A path designed without its delay, as the Smith predictor's is, passes zero.
#[must_use]
pub fn reduced_plant(path: &TwoPointReduction, delay_seconds: f64) -> TuningPlant {
    TuningPlant::FirstOrder(FirstOrderPlusDelay {
        gain: path.gain,
        time_constant_seconds: path.time_constant_seconds,
        delay_seconds,
    })
}

/// The model's operating point as its inputs and its outputs, each empty when the model declares none.
#[must_use]
pub fn operating_point(model: &GeneralModel) -> (Vec<f64>, Vec<f64>) {
    model.identity().operating_point.as_ref().map_or_else(
        || (Vec::new(), Vec::new()),
        |point| (point.inputs.clone(), point.outputs.clone()),
    )
}

/// The value of an operating point at an index, or zero where the point has none (TypeScript `?? 0`).
#[must_use]
pub fn operating_value(values: &[f64], index: usize) -> f64 {
    values.get(index).copied().unwrap_or(0.0)
}

/// The single-loop PI on a reduced path (`designPidToRequirements` on the path model, B4.4): the PI that [`design_to_requirements`]
/// designs for the path, with the path's operating point, the loop's limits and units, and the engine's predictor and placement
/// for the path.
///
/// # Errors
/// Returns the refusal of [`design_to_requirements`], such as a requirement no response or placement meets.
pub fn design_pi_on_path(
    path: &TuningPlant,
    facts: &LoopFacts,
    operating: (f64, f64),
    requirements: &TuningRequirements,
    support: &dyn PathSupport,
) -> Result<ParallelGains, Refusal> {
    let (operating_input, operating_output) = operating;
    let predictor = support.predictor(path, facts, operating);
    let integrating = if path.is_integrating() { "/s" } else { "" };
    let unit = format!(
        "{}/{}{integrating}",
        facts.process_unit, facts.manipulated_unit
    );
    let context = DesignContext {
        action: facts.action,
        task_period_ms: facts.task_period_ms,
        operating_input,
        operating_output,
        output_limits: facts.output_limits,
        gain_limits: facts.gain_limits.clone(),
        process_unit: facts.process_unit.as_str(),
        manipulated_unit: facts.manipulated_unit.as_str(),
        predictor: predictor.as_ref(),
    };
    let placement = support.placement(path, requirements, &context);
    let design = design_to_requirements(
        path,
        Structure::PI,
        requirements,
        &context,
        placement.as_ref(),
        &unit,
    )?;
    Ok(ParallelGains {
        derivative_gain: 0.0,
        ..design.gains
    })
}

/// A count of samples as a number of seconds or steps: exact for every count the response grid can hold.
#[allow(clippy::cast_precision_loss)] // A sample count is at most the response's 4096 values.
fn as_seconds(count: usize) -> f64 {
    count as f64
}

/// A ratio rounded up to a count of samples, and zero for a ratio that is not a number.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // A non-negative ratio of seconds over a period.
fn ceiling_count(ratio: f64) -> usize {
    ratio.ceil() as usize
}

#[cfg(test)]
#[path = "path_tests.rs"]
mod tests;
