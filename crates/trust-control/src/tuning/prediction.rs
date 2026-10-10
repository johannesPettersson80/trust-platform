//! The predicted closed-loop response of a candidate, and of the current settings, on the selected model
//! (B4.5 and B9.6 of the Control Studio specification; `engineerPrediction.ts`).
//!
//! The loop is the `Control.PID` law of `control_pid` in feedback with the sampled plant of the identified
//! model, at the controller's task period, from the physical operating point. The library carries the loop
//! (`trust_control_math::closed_loop`); this module holds Control Studio's rules: which plant an identified form
//! is, which conditions a prediction may run under, and how the response is laid out for the pages.
//!
//! Automatic's requirement check (`tuning::shortfall`) reads a candidate's response through
//! [`ResponsePredictor`]; [`SelectedModelPredictor`] is the implementation for the selected model. The check looks
//! at every period from the settling deadline to the end of the horizon, so the predictor returns every period,
//! and the plots decimate only their own copy.

#![allow(clippy::module_name_repetitions)] // `PredictedGains` and `PredictionConditions` name the prediction they belong to; the precedent is `model.rs`.

use schemars::JsonSchema;
use serde::Serialize;
use trust_control_math::closed_loop::{
    simulate, ClosedLoopError, ClosedLoopRun, ClosedLoopSample, RunRefusal, SampledDynamics,
    SampledPlant,
};
use trust_control_math::identify::{LowOrderModelKind, ParameterInterval};

use crate::identification::plant::IdentifiedModel;
use crate::identification::refusal::Refusal;
use crate::limits::ISOLATED_SIMULATION_SCAN_LIMIT;
use crate::tuning::candidate::{AntiWindup, ControlPidParameters};
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::shortfall::{OutputLimits, ResponsePredictor, TimePoint, Trial};
use crate::tuning::structure::{is_exactly_zero, ParallelGains};
use crate::tuning::typed_pid::TypedDesign;

pub mod control_pid;

use control_pid::{ControlPidLaw, ControlPidState};

/// The longest delay a prediction keeps, in periods: one more than the scan limit, as `engineeringPlots.ts` caps it.
const MAXIMUM_DELAY_PERIODS: u64 = ISOLATED_SIMULATION_SCAN_LIMIT + 1;

/// The sentence for a model that is not a stable plant with a usable gain.
const INVALID_MODEL: &str = "Engineering plots require a valid stable plant model.";
/// The sentence for a time constant that is not finite and positive.
const TIME_CONSTANT: &str = "Engineering plots require finite positive time constants.";
/// The sentence for a horizon or a period that cannot be simulated.
const CONDITIONS: &str =
    "Prediction needs finite coordinates and a bounded number of controller periods.";
/// The sentence for a measurement filter time that is not finite or is negative.
const FILTER: &str = "The measurement filter time must be finite and not negative.";
/// The sentence for an input disturbance that is not finite.
const DISTURBANCE: &str = "The input disturbance must be finite.";
/// The sentence for a plant delay that is not finite or is negative.
const DELAY: &str = "The plant's delay must be finite and not negative.";
/// The sentence for a value that became not finite during a run.
const NON_FINITE: &str = "Controller prediction became non-finite.";

/// The settings of one controller in a prediction: its parallel gains, its setpoint weights and its
/// measurement filter time (N0.5 and C0 of the Control Studio specification). Each controller keeps its own
/// filter, so two controllers are simulated separately.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PredictedGains {
    /// The proportional gain `Kp`.
    pub proportional_gain: f64,
    /// The integral gain `Ki = Kp / Ti`; zero selects the positional form.
    pub integral_gain: f64,
    /// The derivative gain `Kd = Kp · Td`.
    pub derivative_gain: f64,
    /// The setpoint weight of the proportional term (`b`, C0 of the Control Studio specification).
    pub proportional_setpoint_weight: f64,
    /// The setpoint weight of the derivative term (`c`).
    pub derivative_setpoint_weight: f64,
    /// The first-order measurement filter time `Tf` in seconds; zero bypasses the filter.
    pub measurement_filter_seconds: f64,
}

impl PredictedGains {
    /// The gains of a parallel law with the defaults of `engineerPrediction`: a setpoint weight of one, a
    /// derivative setpoint weight of zero, and no measurement filter.
    #[must_use]
    pub const fn parallel(
        proportional_gain: f64,
        integral_gain: f64,
        derivative_gain: f64,
    ) -> Self {
        Self {
            proportional_gain,
            integral_gain,
            derivative_gain,
            proportional_setpoint_weight: 1.0,
            derivative_setpoint_weight: 0.0,
            measurement_filter_seconds: 0.0,
        }
    }

    /// The gains of a design's parallel record, with the defaults of [`PredictedGains::parallel`]. A calculated
    /// candidate writes a setpoint weight of one and no filter (C0 and C1).
    #[must_use]
    pub const fn from_parallel(gains: ParallelGains) -> Self {
        Self::parallel(
            gains.proportional_gain,
            gains.integral_gain,
            gains.derivative_gain,
        )
    }
}

/// The settings of a candidate's record besides its gains that a prediction and its margins read (C0): the setpoint weights
/// and the measurement filter time. A calculated candidate writes the defaults; a pinned or entered record writes its values.
/// The filter is `None` when the loop's PID law declares no filter, and then the prediction runs without one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RecordShaping {
    /// The proportional setpoint weight `β`.
    pub proportional_setpoint_weight: f64,
    /// The derivative setpoint weight `γ`.
    pub derivative_setpoint_weight: f64,
    /// The measurement filter time `Tf` in seconds, when the loop's PID law declares one.
    pub measurement_filter_seconds: Option<f64>,
}

impl Default for RecordShaping {
    /// The values of a calculated candidate: a setpoint weight of one, a derivative weight of zero, and no filter declared.
    fn default() -> Self {
        Self {
            proportional_setpoint_weight: 1.0,
            derivative_setpoint_weight: 0.0,
            measurement_filter_seconds: None,
        }
    }
}

/// The conditions a prediction runs under (B9.6, N8.1): the task period and the horizon, the setpoint change at
/// time zero, the physical operating point, a constant load, and the output and rate limits.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PredictionConditions {
    /// The actual task period `T` of the controller in seconds.
    pub period_seconds: f64,
    /// The horizon in seconds; the response runs for `ceil(duration / period)` periods.
    pub duration_seconds: f64,
    /// The setpoint change applied at time zero; zero for a load trial.
    pub reference_change: f64,
    /// The physical operating input `u₀`.
    pub operating_input: f64,
    /// The physical operating output `y₀`, from which the response starts.
    pub operating_output: f64,
    /// A constant load added to the plant input from the start, before the delay; zero when there is none.
    pub input_disturbance: f64,
    /// The lowest output of the controller.
    pub output_minimum: f64,
    /// The highest output of the controller.
    pub output_maximum: f64,
    /// The most negative output rate of the controller, per second.
    pub rate_minimum: f64,
    /// The most positive output rate of the controller, per second.
    pub rate_maximum: f64,
}

/// One sample of the predicted response (B9.6): the setpoint, the candidate's process value and manipulated
/// variable, and those of the current settings when they are predicted too. The first sample is at time zero,
/// at the operating point.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResponsePoint {
    /// The time of the sample in seconds: its period index times the period.
    pub time_seconds: f64,
    /// The setpoint, which steps at time zero.
    pub setpoint: f64,
    /// The candidate's process value (the plant output) at the sample.
    pub candidate_process_value: f64,
    /// The candidate's manipulated variable (the command) of the period that ends at the sample.
    pub candidate_manipulated: f64,
    /// The current settings' process value, when they are predicted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_process_value: Option<f64>,
    /// The current settings' manipulated variable, when they are predicted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_manipulated: Option<f64>,
}

/// The predicted response of the candidate and, when it is predicted, of the current settings, on one set of
/// conditions (B4.5, B9.6).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PredictedResponse {
    /// The conditions that the response was computed with.
    pub conditions: PredictionConditions,
    /// The sample at time zero, then one sample per period, the last one at the horizon.
    pub points: Vec<ResponsePoint>,
}

/// The predictor of the selected model (B4.5) for Automatic's requirement check: it implements
/// [`ResponsePredictor`] by simulating the candidate's `Control.PID` law on the identified plant, from the operating
/// point of the loop, at its task period and under its output and rate limits.
#[derive(Debug, Clone, Copy)]
pub struct SelectedModelPredictor<'a> {
    /// The selected model, whose plant is simulated.
    pub model: &'a IdentifiedModel,
    /// The task period of the loop in milliseconds.
    pub task_period_ms: f64,
    /// The physical operating input `u₀`.
    pub operating_input: f64,
    /// The physical operating output `y₀`.
    pub operating_output: f64,
    /// The output limits of the loop.
    pub output_limits: OutputLimits,
    /// The output rate limits per second of the loop: its declared rates, or `±10⁶` when it declares none.
    pub rate_limits: ParameterInterval,
    /// The setpoint weights and the measurement filter of the candidate record that is predicted (C0).
    pub shaping: RecordShaping,
}

impl SelectedModelPredictor<'_> {
    /// The predicted gains of a candidate on this loop: its parallel gains with the setpoint weights and the measurement
    /// filter of the record (C0). A loop without a declared filter predicts without one.
    #[must_use]
    pub fn predicted_gains(&self, gains: ParallelGains) -> PredictedGains {
        PredictedGains {
            proportional_gain: gains.proportional_gain,
            integral_gain: gains.integral_gain,
            derivative_gain: gains.derivative_gain,
            proportional_setpoint_weight: self.shaping.proportional_setpoint_weight,
            derivative_setpoint_weight: self.shaping.derivative_setpoint_weight,
            measurement_filter_seconds: self.shaping.measurement_filter_seconds.unwrap_or(0.0),
        }
    }
}

impl ResponsePredictor for SelectedModelPredictor<'_> {
    /// The process values of the candidate from the start of the trial to the end of the horizon. The setpoint trial
    /// steps the setpoint by the requirement's change; the load trial keeps the setpoint and applies the declared input
    /// disturbance. Both return `None` when the prediction cannot be made, and the load trial also when no load is
    /// declared.
    fn predict(
        &self,
        design: &TypedDesign,
        trial: Trial,
        requirements: &TuningRequirements,
    ) -> Option<Vec<TimePoint>> {
        let (reference_change, input_disturbance) = match trial {
            Trial::SetpointChange => (requirements.reference_change, 0.0),
            Trial::InputDisturbance => (0.0, requirements.constant_input_disturbance?),
        };
        let conditions = PredictionConditions {
            period_seconds: self.task_period_ms / 1000.0,
            duration_seconds: requirements.duration_seconds,
            reference_change,
            operating_input: self.operating_input,
            operating_output: self.operating_output,
            input_disturbance,
            output_minimum: self.output_limits.minimum,
            output_maximum: self.output_limits.maximum,
            rate_minimum: self.rate_limits.minimum,
            rate_maximum: self.rate_limits.maximum,
        };
        let gains = self.predicted_gains(design.gains);
        let response = predict_response(self.model, &gains, None, &conditions).ok()?;
        Some(
            response
                .points
                .iter()
                .map(|point| TimePoint {
                    time_seconds: point.time_seconds,
                    process_value: point.candidate_process_value,
                })
                .collect(),
        )
    }
}

/// Checks that a model can be read as a stable plant with a usable gain and usable time constants: the rules of
/// `validateModel` in `engineeringPlots.ts`.
///
/// # Errors
/// Returns the sentence that names the first rule the model breaks: it is not stable, or its gain is not finite
/// or is zero, or a time constant is not finite and positive.
pub fn validate_plant(model: &IdentifiedModel) -> Result<(), Refusal> {
    if !model.stable || !model.gain.is_finite() || is_exactly_zero(model.gain) {
        return Err(Refusal::new(INVALID_MODEL));
    }
    let usable = [
        model.time_constant_seconds,
        model.second_time_constant_seconds,
    ]
    .into_iter()
    .flatten()
    .all(|time| time.is_finite() && time > 0.0);
    if usable {
        Ok(())
    } else {
        Err(Refusal::new(TIME_CONSTANT))
    }
}

/// The sampled plant of an identified model under the conditions: its form, its gain and its delay, and the
/// operating point of the conditions (B9.6).
///
/// # Errors
/// Returns the sentence of [`validate_plant`], or the time-constant sentence when the form lacks a time constant
/// that it needs.
pub fn sampled_plant(
    model: &IdentifiedModel,
    conditions: &PredictionConditions,
) -> Result<SampledPlant, Refusal> {
    validate_plant(model)?;
    let dynamics = dynamics_of(model).ok_or_else(|| Refusal::new(TIME_CONSTANT))?;
    Ok(SampledPlant {
        dynamics,
        gain: model.gain,
        delay_seconds: model.dead_time_seconds.unwrap_or(0.0),
        operating_input: conditions.operating_input,
        operating_output: conditions.operating_output,
    })
}

/// The dynamics of a form: a static gain has none, an integrating form integrates (with its lag when it has
/// one), a first-order form has its time constant, and a second-order form has both.
fn dynamics_of(model: &IdentifiedModel) -> Option<SampledDynamics> {
    let time = model.time_constant_seconds;
    match (model.kind, time, model.second_time_constant_seconds) {
        (LowOrderModelKind::StaticGain, _, _) => Some(SampledDynamics::Static),
        (LowOrderModelKind::Integrating, None, _) => Some(SampledDynamics::Integrating),
        (LowOrderModelKind::Integrating, Some(lag), _) => {
            Some(SampledDynamics::IntegratingWithLag {
                time_constant_seconds: lag,
            })
        }
        (LowOrderModelKind::FirstOrder | LowOrderModelKind::FirstOrderDeadTime, Some(lag), _) => {
            Some(SampledDynamics::FirstOrder {
                time_constant_seconds: lag,
            })
        }
        (
            LowOrderModelKind::SecondOrder | LowOrderModelKind::SecondOrderDeadTime,
            Some(first),
            Some(second),
        ) => Some(SampledDynamics::SecondOrder {
            first_time_constant_seconds: first,
            second_time_constant_seconds: second,
        }),
        _ => None,
    }
}

/// The number of periods that cover the horizon: `ceil(duration / period)`.
///
/// # Errors
/// Returns the conditions sentence when the period or the horizon is not finite and positive, or when the count
/// is above the scan limit (A7).
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)] // The scan limit is far below 2^53, and the count is checked to lie in [1, limit] before it is converted.
pub fn period_count(conditions: &PredictionConditions) -> Result<usize, Refusal> {
    let period = conditions.period_seconds;
    let duration = conditions.duration_seconds;
    let ratio = (duration / period).ceil();
    let limit = ISOLATED_SIMULATION_SCAN_LIMIT as f64;
    let usable = period.is_finite()
        && period > 0.0
        && duration.is_finite()
        && duration > 0.0
        && ratio.is_finite()
        && (1.0..=limit).contains(&ratio);
    if usable {
        Ok(ratio as usize)
    } else {
        Err(Refusal::new(CONDITIONS))
    }
}

/// The closed-loop samples of one set of gains on a plant, from the operating point, over the horizon of the
/// conditions. Each sample holds the plant output after its period and the command of that period.
///
/// # Errors
/// Returns the sentence of the disturbance, the filter time, the horizon, the controller's settings, or a value
/// that is not finite during the run.
pub fn simulate_loop(
    plant: &SampledPlant,
    gains: &PredictedGains,
    conditions: &PredictionConditions,
) -> Result<Vec<ClosedLoopSample>, Refusal> {
    if !conditions.input_disturbance.is_finite() {
        return Err(Refusal::new(DISTURBANCE));
    }
    if !(gains.measurement_filter_seconds.is_finite() && gains.measurement_filter_seconds >= 0.0) {
        return Err(Refusal::new(FILTER));
    }
    let periods = period_count(conditions)?;
    let initial = ControlPidState::at_operating_point(
        conditions.operating_input,
        conditions.operating_output,
    );
    let mut law = ControlPidLaw::new(control_parameters(gains, conditions), initial)?;
    let run = ClosedLoopRun {
        period_seconds: conditions.period_seconds,
        periods,
        setpoint: conditions.operating_output + conditions.reference_change,
        measurement_filter_seconds: gains.measurement_filter_seconds,
        load: conditions.input_disturbance,
        command_minimum: conditions.output_minimum,
        command_maximum: conditions.output_maximum,
        maximum_delay_periods: usize::try_from(MAXIMUM_DELAY_PERIODS).unwrap_or(usize::MAX),
    };
    simulate(plant, &mut law, &run).map_err(refusal_of_loop)
}

/// The predicted response of a candidate, and of the current settings when `current` is given, on the
/// conditions (B4.5). The current settings are a second run on the same plant and conditions. When that run
/// cannot be simulated, its series is left out and the candidate's response is still returned.
///
/// # Errors
/// Returns the sentence of the plant, of the conditions, or of the candidate's settings.
pub fn predict_response(
    model: &IdentifiedModel,
    candidate: &PredictedGains,
    current: Option<&PredictedGains>,
    conditions: &PredictionConditions,
) -> Result<PredictedResponse, Refusal> {
    let plant = sampled_plant(model, conditions)?;
    let candidate_samples = simulate_loop(&plant, candidate, conditions)?;
    let current_samples = current.and_then(|gains| simulate_loop(&plant, gains, conditions).ok());
    Ok(PredictedResponse {
        conditions: *conditions,
        points: response_points(&candidate_samples, current_samples.as_deref(), conditions),
    })
}

/// The response points of the candidate and, when given, of the current settings: the sample at time zero at
/// the operating point, then one sample per period.
#[must_use]
pub fn response_points(
    candidate: &[ClosedLoopSample],
    current: Option<&[ClosedLoopSample]>,
    conditions: &PredictionConditions,
) -> Vec<ResponsePoint> {
    let setpoint = conditions.operating_output + conditions.reference_change;
    let mut points = Vec::with_capacity(candidate.len() + 1);
    points.push(ResponsePoint {
        time_seconds: 0.0,
        setpoint,
        candidate_process_value: conditions.operating_output,
        candidate_manipulated: conditions.operating_input,
        current_process_value: current.map(|_| conditions.operating_output),
        current_manipulated: current.map(|_| conditions.operating_input),
    });
    for (index, sample) in candidate.iter().enumerate() {
        let current_sample = current.and_then(|samples| samples.get(index));
        points.push(ResponsePoint {
            time_seconds: period_time(index + 1, conditions.period_seconds),
            setpoint,
            candidate_process_value: sample.process_value,
            candidate_manipulated: sample.command,
            current_process_value: current_sample.map(|sample| sample.process_value),
            current_manipulated: current_sample.map(|sample| sample.command),
        });
    }
    points
}

/// The time of a period: its index times the period.
#[must_use]
fn period_time(index: usize, period_seconds: f64) -> f64 {
    f64::from(u32::try_from(index).unwrap_or(u32::MAX)) * period_seconds
}

/// The Control.PID parameter record of a prediction: the candidate's gains and weights, the conditions' limits, a
/// tracking time of one second and back-calculation anti-windup, with a bias of zero (the record that
/// `engineerPrediction` builds).
fn control_parameters(
    gains: &PredictedGains,
    conditions: &PredictionConditions,
) -> ControlPidParameters {
    ControlPidParameters {
        proportional_gain: gains.proportional_gain,
        integral_gain: gains.integral_gain,
        derivative_gain: gains.derivative_gain,
        proportional_setpoint_weight: gains.proportional_setpoint_weight,
        derivative_setpoint_weight: gains.derivative_setpoint_weight,
        bias: 0.0,
        output_minimum: conditions.output_minimum,
        output_maximum: conditions.output_maximum,
        rate_minimum: conditions.rate_minimum,
        rate_maximum: conditions.rate_maximum,
        tracking_time: 1.0,
        anti_windup: AntiWindup::BackCalculation,
    }
}

/// Maps a run's refusal to the prediction's sentence.
fn refusal_of_run(refusal: RunRefusal) -> Refusal {
    match refusal {
        RunRefusal::Period
        | RunRefusal::NoPeriods
        | RunRefusal::Nonfinite
        | RunRefusal::Saturation => Refusal::new(CONDITIONS),
        RunRefusal::Filter => Refusal::new(FILTER),
        RunRefusal::Delay => Refusal::new(DELAY),
        RunRefusal::TimeConstant => Refusal::new(TIME_CONSTANT),
    }
}

/// Maps the closed loop's failure to the prediction's sentence.
fn refusal_of_loop(error: ClosedLoopError<Refusal>) -> Refusal {
    match error {
        ClosedLoopError::Run(refusal) => refusal_of_run(refusal),
        ClosedLoopError::Law(refusal) => refusal,
        ClosedLoopError::NonFinite => Refusal::new(NON_FINITE),
    }
}

#[cfg(test)]
#[path = "prediction_tests.rs"]
mod tests;
