//! Simulation of a linear model with a pure input delay and held inputs: the exact delay realization
//! at the sample instants, the open-loop time response, the frozen-model prediction from a recording,
//! and the exact scalar delay form. Each function states the recursion it computes. The caller
//! passes the state budget and the output-value limit, which are product limits.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::discretize::{discretize_state_space, DiscretizationMethod};
use crate::linalg::{
    dot, solve_least_squares, DenseMatrix, NumericalRejectionReason, NumericalTolerance,
};
use crate::model::{
    transfer_function_state_space, OperatingPoint, StateSpace, Timing, TransferFunction,
};

/// The tolerance of the initial-state estimate of a frozen-model prediction.
const ESTIMATION_TOLERANCE: NumericalTolerance = NumericalTolerance {
    absolute: 1e-10,
    relative: 1e-9,
    condition_limit: 1e10,
};

/// A plant with a pure input delay: its state-space matrices, its timing and its delay in seconds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DelayedSystem {
    /// The matrices of the plant before its delay.
    pub system: StateSpace,
    /// The timing: a discrete system's matrices are used as they are.
    pub timing: Timing,
    /// The pure input delay `L` in seconds, zero or more.
    pub delay_seconds: f64,
}

/// The exact realization of a delayed system at a sample period. Its states are the plant's states
/// followed by `history` blocks of `m` input slots, the newest first.
#[derive(Debug, Clone, PartialEq)]
pub struct Realization {
    /// The discrete matrices of the realization.
    pub system: StateSpace,
    /// The plant's initial state followed by `history` copies of the initial input.
    pub initial_state: Vec<f64>,
    /// The number of input-history blocks: the whole sample periods of delay, plus one for a
    /// fractional part.
    pub history: usize,
}

/// Why a simulation calculation is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SimulationError {
    /// The initial state or the initial input does not match the states or inputs.
    #[error("initial coordinates do not match")]
    InitialCoordinates,
    /// The delay or the step is not finite, the step is not positive, or the delay is negative.
    #[error("invalid delay or step")]
    InvalidDelayOrStep,
    /// A discrete system's delay is not a whole number of its sample periods.
    #[error("discrete delay is not a whole number of periods")]
    NonIntegerDiscreteDelay,
    /// The realization has no states, or more states than the caller allows.
    #[error("state budget")]
    StateBudget,
    /// The exact conversion at a sample period was refused for this reason.
    #[error("exact conversion refused: {0:?}")]
    Conversion(NumericalRejectionReason),
    /// A realization matrix is not finite.
    #[error("nonfinite realization")]
    NonfiniteRealization,
    /// The sample period of a discrete model differs from the simulation step.
    #[error("sample periods differ")]
    SamplePeriodMismatch,
    /// The sample period of a discrete model differs from the recording's (tolerance 1e-8).
    #[error("recording and model sample periods differ")]
    RecordingPeriodMismatch,
    /// A response value is not finite.
    #[error("nonfinite response")]
    NonfiniteResponse,
    /// The recording has no samples.
    #[error("empty recording")]
    EmptyRecording,
    /// A recorded sample does not have one value per input or output.
    #[error("record shape")]
    RecordShape,
    /// The recording is too short to separate the initial-state estimate from the validation.
    #[error("recording too short")]
    RecordingTooShort,
    /// The initial state cannot be estimated from the recording.
    #[error("initial state is not observable")]
    UnobservableInitialState,
    /// A predicted value is not finite.
    #[error("nonfinite prediction")]
    NonfinitePrediction,
    /// The scalar adapter needs a first-order model with no constant in its numerator.
    #[error("not a first-order scalar model")]
    NotFirstOrderScalar,
    /// The first-order dead-time parameters are outside the adapter's domain.
    #[error("outside the adapter domain")]
    OutsideAdapterDomain,
    /// The step or the duration is not finite and positive, or the duration is negative.
    #[error("time settings are not finite and positive")]
    InvalidSettings,
}

/// The settings of an open-loop time response: the step, the duration, the initial state and the
/// input held over the response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimeSettings {
    /// The output step in seconds.
    pub step_seconds: f64,
    /// The duration in seconds: a whole number of steps.
    pub duration_seconds: f64,
    /// The plant's initial state, one value per state.
    pub initial_state: Vec<f64>,
    /// The input held over the response, one value per input.
    pub input: Vec<f64>,
}

/// The settings are not valid for the model: the step or the duration is not finite and positive,
/// a vector has the wrong length or a value that is not finite, the duration is not a whole number
/// of steps, the response would have more values than allowed, or a discrete model's sample period
/// differs from the step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("time-response settings are not valid")]
pub struct TimeSettingsError;

/// One output sample of a time response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimeResponsePoint {
    /// The time of the sample: its index times the step.
    pub time_seconds: f64,
    /// The output from the held input alone, from zero initial state, one value per output.
    pub zero_state: Vec<f64>,
    /// The output from the initial state alone, with zero input, one value per output.
    pub initial_condition: Vec<f64>,
    /// The sum of the two parts.
    pub total: Vec<f64>,
}

/// An open-loop time response: its settings and its samples from time zero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimeResponse {
    /// The settings the response was computed with.
    pub settings: TimeSettings,
    /// The samples at `0, T, …, duration`.
    pub points: Vec<TimeResponsePoint>,
}

/// The frozen-model prediction: the index of the first predicted sample and the predicted outputs
/// from it to the end of the recording.
#[derive(Debug, Clone, PartialEq)]
pub struct FrozenPrediction {
    /// The index of the first predicted sample.
    pub start: usize,
    /// The predicted outputs, one row per sample from `start`.
    pub values: Vec<Vec<f64>>,
}

/// The runtime form of an exact scalar delay model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScalarDelayForm {
    /// `G(s) = gain·e^{−Ls}/s`, an integrator with dead time.
    IntegratorDeadTime {
        /// The gain `gain` of the integrator.
        gain: f64,
    },
    /// `G(s) = gain·e^{−Ls}/(τs + 1)`, a first-order lag with dead time.
    FirstOrderDeadTime {
        /// The static gain.
        gain: f64,
        /// The time constant `τ` in seconds.
        time_constant_seconds: f64,
    },
}

/// The scalar delay form with its dead time and the output state that matches the model's
/// coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct ScalarDelayAdapter {
    /// The form of the model.
    pub form: ScalarDelayForm,
    /// The dead time `L` in seconds.
    pub dead_time_seconds: f64,
    /// The output-deviation state `c·x₀`, where `c` is the companion output row and `x₀` the
    /// companion state.
    pub initial_output_state: f64,
}

/// The exact realization of a delayed system at `step_seconds` for held inputs.
///
/// Write the delay as `L/T = k + f` with a whole `k` and `0 ≤ f < 1`; a ratio within `10⁻¹⁰` of a
/// whole number has `f = 0`. The realization keeps `h = k` input-history blocks, plus one more when
/// `f > 0`. A continuous plant is converted by the exact zero-order hold: the input held `k` periods
/// ago enters with `B_recent` (the hold over `(1 − f)T`), and the input held `k + 1` periods ago
/// enters with `B − B_recent` when `f > 0`. A discrete plant is used as it is, with an integer delay.
///
/// # Errors
/// Returns [`SimulationError::InitialCoordinates`], [`SimulationError::InvalidDelayOrStep`],
/// [`SimulationError::NonIntegerDiscreteDelay`], [`SimulationError::StateBudget`] when the realization
/// has no states or more than `maximum_states`, [`SimulationError::Conversion`] when the exact
/// conversion is refused, and [`SimulationError::NonfiniteRealization`].
pub fn delayed_realization(
    plant: &DelayedSystem,
    step_seconds: f64,
    initial_state: &[f64],
    initial_input: &[f64],
    maximum_states: usize,
) -> Result<Realization, SimulationError> {
    let system = &plant.system;
    let (states, inputs) = (system.states(), system.inputs());
    if initial_state.len() != states || initial_input.len() != inputs {
        return Err(SimulationError::InitialCoordinates);
    }
    let ratio = plant.delay_seconds / step_seconds;
    if !ratio.is_finite() || ratio < 0.0 {
        return Err(SimulationError::InvalidDelayOrStep);
    }
    let nearest = ratio.round();
    let on_grid = (ratio - nearest).abs() < 1e-10;
    let whole = if on_grid { nearest } else { ratio.floor() };
    let fraction = if on_grid { 0.0 } else { ratio - whole };
    let discrete = matches!(plant.timing, Timing::Discrete { .. });
    if discrete && fraction != 0.0 {
        return Err(SimulationError::NonIntegerDiscreteDelay);
    }
    let lag = lag_count(whole);
    let history = lag + usize::from(fraction > 0.0);
    let total = states.saturating_add(history.saturating_mul(inputs));
    if total == 0 || total > maximum_states {
        return Err(SimulationError::StateBudget);
    }
    let (full_a, full_b) = held_matrices(system, step_seconds, discrete)?;
    let recent_b = if fraction == 0.0 {
        full_b.clone()
    } else {
        held_matrices(system, (1.0 - fraction) * step_seconds, discrete)?.1
    };
    let (a, b, c, d) =
        augmented_matrices(system, (&full_a, &full_b), &recent_b, lag, fraction > 0.0);
    if ![&a, &b, &c, &d]
        .iter()
        .all(|matrix| matrix.iter().flatten().all(|value| value.is_finite()))
    {
        return Err(SimulationError::NonfiniteRealization);
    }
    let mut realization_state = initial_state.to_vec();
    for _ in 0..history {
        realization_state.extend_from_slice(initial_input);
    }
    Ok(Realization {
        system: StateSpace { a, b, c, d },
        initial_state: realization_state,
        history,
    })
}

/// The discrete `(A, B)` of a plant at `period`: its own matrices when discrete, the exact
/// zero-order hold when continuous, and empty matrices for a plant without states.
fn held_matrices(
    system: &StateSpace,
    period: f64,
    discrete: bool,
) -> Result<(DenseMatrix, DenseMatrix), SimulationError> {
    if discrete {
        return Ok((system.a.clone(), system.b.clone()));
    }
    if system.states() == 0 {
        return Ok((Vec::new(), Vec::new()));
    }
    let held = discretize_state_space(system, period, DiscretizationMethod::ExactZoh)
        .map_err(|refusal| SimulationError::Conversion(refusal.reason))?;
    Ok((held.system.a, held.system.b))
}

/// The matrices `(A, B, C, D)` of the realization. The plant's `n` states come first, then `h`
/// blocks of `m` input slots: block `j` holds the input delayed `j + 1` periods.
fn augmented_matrices(
    system: &StateSpace,
    full: (&DenseMatrix, &DenseMatrix),
    recent_b: &DenseMatrix,
    lag: usize,
    has_fraction: bool,
) -> (DenseMatrix, DenseMatrix, DenseMatrix, DenseMatrix) {
    let (states, inputs, outputs) = (system.states(), system.inputs(), system.outputs());
    let history = lag + usize::from(has_fraction);
    let total = states + history * inputs;
    let (full_a, full_b) = full;
    let mut a = vec![vec![0.0; total]; total];
    let mut b = vec![vec![0.0; inputs]; total];
    let mut c = vec![vec![0.0; total]; outputs];
    let mut d = vec![vec![0.0; inputs]; outputs];
    for row in 0..states {
        a[row][..states].copy_from_slice(&full_a[row]);
        for input in 0..inputs {
            place_input(
                &mut a,
                &mut b,
                (states, inputs),
                (row, lag, input),
                recent_b[row][input],
            );
            if has_fraction {
                place_input(
                    &mut a,
                    &mut b,
                    (states, inputs),
                    (row, lag + 1, input),
                    full_b[row][input] - recent_b[row][input],
                );
            }
        }
    }
    for slot in 0..history {
        for input in 0..inputs {
            let row = states + slot * inputs + input;
            if slot == 0 {
                b[row][input] = 1.0;
            } else {
                a[row][states + (slot - 1) * inputs + input] = 1.0;
            }
        }
    }
    for output in 0..outputs {
        c[output][..states].copy_from_slice(&system.c[output]);
        for input in 0..inputs {
            if history == 0 {
                d[output][input] = system.d[output][input];
            } else {
                c[output][states + (history - 1) * inputs + input] = system.d[output][input];
            }
        }
    }
    (a, b, c, d)
}

/// Adds `value` to the coefficient of `input`, delayed `lag` periods, in row `row`: it enters `B`
/// when the lag is zero, and the input block `lag − 1` of `A` otherwise.
fn place_input(
    a: &mut DenseMatrix,
    b: &mut DenseMatrix,
    (states, inputs): (usize, usize),
    (row, lag, input): (usize, usize, usize),
    value: f64,
) {
    if lag == 0 {
        b[row][input] += value;
    } else {
        a[row][states + (lag - 1) * inputs + input] += value;
    }
}

/// The whole number of periods of a non-negative finite delay ratio. The float-to-integer
/// conversion saturates, so a huge ratio becomes a count that the state budget then refuses.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn lag_count(periods: f64) -> usize {
    periods as usize
}

/// A count as a float. The counts here are at most the state or sample limits, far below 2^53.
#[allow(clippy::cast_precision_loss)]
fn as_f64(count: usize) -> f64 {
    count as f64
}

/// Checks the settings against the model and returns them as [`TimeSettings`].
///
/// The step and the duration are finite and positive, the input has one value per input and the
/// initial state one per state, all finite. The duration is a whole number of steps (within
/// `10⁻⁸`) of at least one step, the response has at most `maximum_output_values` values (outputs
/// times samples, with `⌊duration/step⌋ + 1` samples), and a discrete model's sample period equals
/// the step (within `10⁻¹⁰`).
///
/// # Errors
/// Returns [`TimeSettingsError`] when any condition above fails.
pub fn time_settings(
    system: &StateSpace,
    timing: Timing,
    step_seconds: f64,
    duration_seconds: f64,
    initial_state: &[f64],
    input: &[f64],
    maximum_output_values: usize,
) -> Result<TimeSettings, TimeSettingsError> {
    let positive = |value: f64| value.is_finite() && value > 0.0;
    let all_finite = |values: &[f64]| values.iter().all(|value| value.is_finite());
    let count = duration_seconds / step_seconds;
    let sample_period_matches = match timing {
        Timing::Continuous => true,
        Timing::Discrete {
            sample_time_seconds,
        } => (sample_time_seconds - step_seconds).abs() <= 1e-10,
    };
    let valid = positive(step_seconds)
        && positive(duration_seconds)
        && input.len() == system.inputs()
        && all_finite(input)
        && initial_state.len() == system.states()
        && all_finite(initial_state)
        && count.is_finite()
        && count >= 1.0
        && (count - count.round()).abs() <= 1e-8
        && (count.floor() + 1.0) * as_f64(system.outputs()) <= as_f64(maximum_output_values)
        && sample_period_matches;
    if !valid {
        return Err(TimeSettingsError);
    }
    Ok(TimeSettings {
        step_seconds,
        duration_seconds,
        initial_state: initial_state.to_vec(),
        input: input.to_vec(),
    })
}

/// The open-loop time response of a delayed system to the held input, from its initial state.
///
/// The zero-state part is the output of the held input with zero initial state, and the
/// initial-condition part is the output of the initial state with zero input. Each sample at
/// `t = iT` is `y = Cx + Du` with `x(k+1) = Ax(k) + Bu(k)`, using the exact realization of the
/// delayed system. A discrete system's sample period must equal the step. A system with no states
/// and no delay is its static gain.
///
/// # Errors
/// Returns [`SimulationError::SamplePeriodMismatch`] for a discrete system with another period, the
/// realization errors of [`delayed_realization`], and [`SimulationError::NonfiniteResponse`] when a
/// value is not finite.
pub fn open_loop_response(
    plant: &DelayedSystem,
    settings: &TimeSettings,
    maximum_states: usize,
) -> Result<TimeResponse, SimulationError> {
    let ratio = settings.duration_seconds / settings.step_seconds;
    if !(settings.step_seconds.is_finite() && settings.step_seconds > 0.0 && ratio.is_finite())
        || ratio < 0.0
    {
        return Err(SimulationError::InvalidSettings);
    }
    let realization = sampled_realization(plant, settings, maximum_states)?;
    let system = &realization.system;
    let steps = lag_count((settings.duration_seconds / settings.step_seconds).round());
    let zero_input = vec![0.0; settings.input.len()];
    let mut forced_state = vec![0.0; system.states()];
    let mut initial_state = realization.initial_state.clone();
    let mut points = Vec::with_capacity(steps + 1);
    for index in 0..=steps {
        let (forced_output, next_forced) = advance(system, &forced_state, &settings.input);
        let (initial_output, next_initial) = advance(system, &initial_state, &zero_input);
        let total: Vec<f64> = forced_output
            .iter()
            .zip(&initial_output)
            .map(|(forced, initial)| forced + initial)
            .collect();
        let finite = [
            &forced_output,
            &initial_output,
            &total,
            &next_forced,
            &next_initial,
        ]
        .iter()
        .all(|values| values.iter().all(|value| value.is_finite()));
        if !finite {
            return Err(SimulationError::NonfiniteResponse);
        }
        points.push(TimeResponsePoint {
            time_seconds: as_f64(index) * settings.step_seconds,
            zero_state: forced_output,
            initial_condition: initial_output,
            total,
        });
        forced_state = next_forced;
        initial_state = next_initial;
    }
    Ok(TimeResponse {
        settings: settings.clone(),
        points,
    })
}

/// Whether a stored response has the shape of a response of this model with these settings: the
/// settings are valid for the model, there are `⌊duration/step⌋ + 1` samples, each sample time is
/// its index times the step, and every vector has one value per output and is finite.
#[must_use]
#[allow(clippy::float_cmp)] // The sample times are compared exactly: they are recomputed the same way.
pub fn is_time_response(
    response: &TimeResponse,
    system: &StateSpace,
    timing: Timing,
    maximum_output_values: usize,
) -> bool {
    let Ok(settings) = time_settings(
        system,
        timing,
        response.settings.step_seconds,
        response.settings.duration_seconds,
        &response.settings.initial_state,
        &response.settings.input,
        maximum_output_values,
    ) else {
        return false;
    };
    let steps = lag_count((settings.duration_seconds / settings.step_seconds).round());
    let outputs = system.outputs();
    response.points.len() == steps + 1
        && response.points.iter().enumerate().all(|(index, point)| {
            point.time_seconds == as_f64(index) * settings.step_seconds
                && vector_has_length(&point.zero_state, outputs)
                && vector_has_length(&point.initial_condition, outputs)
                && vector_has_length(&point.total, outputs)
        })
}

fn vector_has_length(values: &[f64], length: usize) -> bool {
    values.len() == length && values.iter().all(|value| value.is_finite())
}

/// The realization used by the open-loop response: a static gain needs no realization, and any
/// other system uses its exact delayed realization at the step.
fn sampled_realization(
    plant: &DelayedSystem,
    settings: &TimeSettings,
    maximum_states: usize,
) -> Result<Realization, SimulationError> {
    if let Timing::Discrete {
        sample_time_seconds,
    } = plant.timing
        && (sample_time_seconds - settings.step_seconds).abs() > 1e-10
    {
        return Err(SimulationError::SamplePeriodMismatch);
    }
    if plant.system.states() == 0 && plant.delay_seconds == 0.0 {
        return Ok(Realization {
            system: plant.system.clone(),
            initial_state: Vec::new(),
            history: 0,
        });
    }
    let zero_input = vec![0.0; plant.system.inputs()];
    delayed_realization(
        plant,
        settings.step_seconds,
        &settings.initial_state,
        &zero_input,
        maximum_states,
    )
}

/// One step of the held-input recursion: the output `Cx + Du` and the next state `Ax + Bu`.
fn advance(system: &StateSpace, state: &[f64], input: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let output = system
        .c
        .iter()
        .zip(&system.d)
        .map(|(c_row, d_row)| dot(c_row, state) + dot(d_row, input))
        .collect();
    let next = system
        .a
        .iter()
        .zip(&system.b)
        .map(|(a_row, b_row)| dot(a_row, state) + dot(b_row, input))
        .collect();
    (output, next)
}

/// The predicted output `Cx + Du + y₀` at an operating output `y₀`.
fn observe(system: &StateSpace, state: &[f64], input: &[f64], output_offset: &[f64]) -> Vec<f64> {
    system
        .c
        .iter()
        .zip(&system.d)
        .zip(output_offset)
        .map(|((c_row, d_row), offset)| dot(c_row, state) + dot(d_row, input) + offset)
        .collect()
}

/// The frozen-model prediction of a recording. The model's coefficients stay fixed; only the
/// initial state is estimated, from the first samples, and then the model predicts the outputs.
///
/// The inputs and outputs are deviations from the operating point (zero when there is none).
/// A system with no states and no delay predicts `y = D·u + y₀`. Otherwise, with `history_states`
/// (the recording's states are its past outputs and inputs, and the delay is zero), the first
/// `order − 1` samples supply the history directly, where `order = (n + m)/(p + m)`. Otherwise the
/// initial state `x₀` solves, by least squares, `C·A^k·x₀ = y_k − (zero-state output)` over the
/// first `count = max(3, 2n)` samples, and the model predicts from `count`.
///
/// # Errors
/// Returns [`SimulationError::RecordShape`] when the recording and the operating point disagree in
/// length, [`SimulationError::RecordingPeriodMismatch`] for a discrete model with another period,
/// [`SimulationError::EmptyRecording`], [`SimulationError::RecordingTooShort`] when `3·count` is not
/// below the number of samples, [`SimulationError::UnobservableInitialState`] when the estimate is
/// refused, and [`SimulationError::NonfinitePrediction`].
pub fn frozen_model_prediction(
    plant: &DelayedSystem,
    operating_point: Option<&OperatingPoint>,
    inputs: &[Vec<f64>],
    outputs: &[Vec<f64>],
    step_seconds: f64,
    history_states: bool,
    maximum_states: usize,
) -> Result<FrozenPrediction, SimulationError> {
    let system = &plant.system;
    let (states, input_count, output_count) = (system.states(), system.inputs(), system.outputs());
    let zero_input = vec![0.0; input_count];
    let zero_output = vec![0.0; output_count];
    let input_offset =
        operating_point.map_or(zero_input.as_slice(), |point| point.inputs.as_slice());
    let output_offset =
        operating_point.map_or(zero_output.as_slice(), |point| point.outputs.as_slice());
    if input_offset.len() != input_count
        || output_offset.len() != output_count
        || inputs.len() != outputs.len()
        || inputs.iter().any(|row| row.len() != input_count)
        || outputs.iter().any(|row| row.len() != output_count)
    {
        return Err(SimulationError::RecordShape);
    }
    let deviations_u: Vec<Vec<f64>> = inputs
        .iter()
        .map(|row| {
            row.iter()
                .zip(input_offset)
                .map(|(value, offset)| value - offset)
                .collect()
        })
        .collect();
    let deviations_y: Vec<Vec<f64>> = outputs
        .iter()
        .map(|row| {
            row.iter()
                .zip(output_offset)
                .map(|(value, offset)| value - offset)
                .collect()
        })
        .collect();
    if states == 0 && plant.delay_seconds == 0.0 {
        let values = deviations_u
            .iter()
            .map(|input| observe(system, &[], input, output_offset))
            .collect();
        return Ok(FrozenPrediction { start: 0, values });
    }
    if let Timing::Discrete {
        sample_time_seconds,
    } = plant.timing
        && (sample_time_seconds - step_seconds).abs() > 1e-8
    {
        return Err(SimulationError::RecordingPeriodMismatch);
    }
    let first_input = deviations_u
        .first()
        .ok_or(SimulationError::EmptyRecording)?;
    let zero_state = vec![0.0; states];
    let realization = delayed_realization(
        plant,
        step_seconds,
        &zero_state,
        first_input,
        maximum_states,
    )?;
    let sys = &realization.system;
    let mut state = realization.initial_state.clone();
    let mut start = 0;
    let numerator_order = states + input_count;
    let denominator_order = output_count + input_count;
    let discrete = matches!(plant.timing, Timing::Discrete { .. });
    if discrete
        && plant.delay_seconds == 0.0
        && history_states
        && denominator_order > 0
        && numerator_order.is_multiple_of(denominator_order)
    {
        let order = numerator_order / denominator_order;
        if deviations_u.len() < order {
            return Err(SimulationError::RecordingTooShort);
        }
        start = order - 1;
        state = (0..order)
            .flat_map(|lag| deviations_y[start - lag].iter().copied())
            .chain((0..order - 1).flat_map(|lag| deviations_u[start - lag - 1].iter().copied()))
            .collect();
    } else if states > 0 {
        let (estimated, count) =
            estimate_initial_state(sys, &state, states, &deviations_u, outputs, output_offset)?;
        state = estimated;
        start = count;
    }
    let mut values = Vec::with_capacity(deviations_u.len().saturating_sub(start));
    for input in deviations_u.iter().skip(start) {
        let value = observe(sys, &state, input, output_offset);
        if !value.iter().all(|entry| entry.is_finite()) {
            return Err(SimulationError::NonfinitePrediction);
        }
        values.push(value);
        state = advance(sys, &state, input).1;
    }
    Ok(FrozenPrediction { start, values })
}

/// The least-squares estimate of the plant's initial state from the first `count = max(3, 2n)`
/// samples, with `n` the plant's states. The row for output `p` at sample `k` is `C_p·A^k` applied
/// to the initial-state basis, and its target is the recorded output less the zero-state output.
/// Returns the state advanced over those samples and `count`. The recording must hold more than
/// `3·count` samples.
///
/// # Errors
/// Returns [`SimulationError::RecordingTooShort`] when `3·count` is not below the number of samples,
/// and [`SimulationError::UnobservableInitialState`] when the estimate is refused.
fn estimate_initial_state(
    sys: &StateSpace,
    state: &[f64],
    plant_states: usize,
    deviations_u: &[Vec<f64>],
    outputs: &[Vec<f64>],
    output_offset: &[f64],
) -> Result<(Vec<f64>, usize), SimulationError> {
    let count = (2 * plant_states).max(3);
    if count * 3 >= outputs.len() {
        return Err(SimulationError::RecordingTooShort);
    }
    let total_states = sys.states();
    let mut basis: Vec<Vec<f64>> = (0..plant_states)
        .map(|index| {
            (0..total_states)
                .map(|column| f64::from(u8::from(column == index)))
                .collect()
        })
        .collect();
    let mut forced = state.to_vec();
    let mut matrix: DenseMatrix = Vec::new();
    let mut target: Vec<f64> = Vec::new();
    for (sample, input) in deviations_u.iter().take(count).enumerate() {
        let zero_state_output = observe(sys, &forced, input, output_offset);
        for (output, zero_state) in zero_state_output.iter().enumerate() {
            matrix.push(
                basis
                    .iter()
                    .map(|column| dot(&sys.c[output], column))
                    .collect(),
            );
            target.push(outputs[sample][output] - zero_state);
        }
        forced = advance(sys, &forced, input).1;
        for column in &mut basis {
            let next: Vec<f64> = sys
                .a
                .iter()
                .map(|row| dot(row, column.as_slice()))
                .collect();
            *column = next;
        }
    }
    let estimate = solve_least_squares(&matrix, &target, &ESTIMATION_TOLERANCE)
        .map_err(|_| SimulationError::UnobservableInitialState)?;
    let mut advanced = state.to_vec();
    advanced[..plant_states].copy_from_slice(&estimate.solution);
    for input in deviations_u.iter().take(count) {
        advanced = advance(sys, &advanced, input).1;
    }
    Ok((advanced, count))
}

/// Whether a transfer function is a first-order scalar with an exact delay adapter, as the runtime
/// adapters need: a denominator of two coefficients with a positive leading coefficient and a
/// non-negative trailing one, and a numerator of one coefficient.
#[must_use]
pub fn is_exact_scalar_delay(transfer: &TransferFunction) -> bool {
    transfer.denominator.len() == 2
        && transfer.numerator.len() == 1
        && transfer.denominator[0] > 0.0
        && transfer.denominator[1] >= 0.0
}

/// The exact scalar delay adapter of a first-order transfer function `N/D` with dead time `L`.
/// Its controllable companion has `n₀ = 0`, so `N/D = n₁/(d₀s + d₁)` after normalisation. A zero
/// `d₁` is an integrator, `gain = n₁`. Otherwise the form is `gain = n₁/d₁` with time constant
/// `τ = 1/d₁`. The initial output state is `c·x₀` with `c = n₁` of the companion output row.
///
/// # Errors
/// Returns [`SimulationError::NotFirstOrderScalar`] unless the companion is first order with a zero
/// numerator constant, and [`SimulationError::OutsideAdapterDomain`] when `τ` or the gain is not
/// finite or `τ` is not positive.
pub fn scalar_delay_adapter(
    transfer: &TransferFunction,
    dead_time_seconds: f64,
    initial_state: f64,
) -> Result<ScalarDelayAdapter, SimulationError> {
    let companion = transfer_function_state_space(&transfer.numerator, &transfer.denominator)
        .map_err(|_| SimulationError::NotFirstOrderScalar)?;
    if companion.order != 1 || companion.numerator[0] != 0.0 {
        return Err(SimulationError::NotFirstOrderScalar);
    }
    let initial_output_state = companion.system.c[0][0] * initial_state;
    let numerator_gain = companion.numerator[1];
    let denominator_gain = companion.denominator[1];
    if denominator_gain == 0.0 {
        return Ok(ScalarDelayAdapter {
            form: ScalarDelayForm::IntegratorDeadTime {
                gain: numerator_gain,
            },
            dead_time_seconds,
            initial_output_state,
        });
    }
    let time_constant_seconds = 1.0 / denominator_gain;
    let gain = numerator_gain / denominator_gain;
    if !time_constant_seconds.is_finite() || time_constant_seconds <= 0.0 || !gain.is_finite() {
        return Err(SimulationError::OutsideAdapterDomain);
    }
    Ok(ScalarDelayAdapter {
        form: ScalarDelayForm::FirstOrderDeadTime {
            gain,
            time_constant_seconds,
        },
        dead_time_seconds,
        initial_output_state,
    })
}

#[cfg(test)]
mod tests;
