//! The sampled closed loops of the structures (specification 35, C6 and C8; numerical annex N4.7(a)): the unit-step response of
//! a sampled plant whose input is delayed by whole samples, open loop or with a PI around one measurement; and the largest
//! deviation of a measurement after a measured-disturbance step, with a PI and a feedforward.
//!
//! Both start from a zero state and advance one sample at a time. Within a sample the measurement is read first, then the
//! command is formed, the input that reaches the plant is the one delayed by the delay's whole samples, and the integral term
//! is summed after the sample's output. The delay is rounded to whole samples, and the samples stop at 200 000 (C6, C8).
//!
//! With a delay of zero, the PI's error reads the measurement without the feedthrough of the sample's own command, because no
//! delayed input exists yet; the recorded output includes it. The two agree when the plant has no feedthrough, which every
//! structure's plant has (C6, C8), and the rule is kept as the design rules of Control Studio state it.

#![allow(clippy::module_name_repetitions)] // `SampledResponse` and `SamplePi` name the module's subject.

use std::collections::VecDeque;

use crate::model::StateSpace;

/// The most samples a response is simulated for (C6 and C8).
const MAXIMUM_SAMPLES: usize = 200_000;

/// A sampled response: the time of each sample from the start of the test, and the value of the output at it.
#[derive(Debug, Clone, PartialEq)]
pub struct SampledResponse {
    /// The sample times in seconds: the sample index times the sample period.
    pub times: Vec<f64>,
    /// The output at each sample time.
    pub values: Vec<f64>,
}

/// A proportional-integral law on one measurement: `u = Kp·e + ∫Ki·e dt`, the integral summed once per sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SamplePi {
    /// The proportional gain `Kp`.
    pub proportional_gain: f64,
    /// The integral gain `Ki`.
    pub integral_gain: f64,
}

/// A lead and lag filter of a feedforward (C8): `T_lead·s + 1` over `T_lag·s + 1`, with unit steady-state gain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeadLag {
    /// The lead time `T_lead` in seconds.
    pub lead_seconds: f64,
    /// The lag time `T_lag` in seconds.
    pub lag_seconds: f64,
}

/// A feedforward from a measured disturbance (C8): `K_ff·d`, or with a lead and a lag `K_ff·(z + (T_lead/T_lag)·(d − z))`, where
/// `z` is `d` through the lag.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Feedforward {
    /// The gain `K_ff`.
    pub gain: f64,
    /// The lead and lag, when the program has them.
    pub lead_lag: Option<LeadLag>,
}

/// A measured-disturbance case (C8): the sampled plant and its delay, the plant's inputs for the controller output and the
/// measured disturbance, the measurement it reads, the PI on that measurement with the task period of its integral step, and
/// the disturbance step and the duration of the test.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisturbanceCase<'a> {
    /// The plant, sampled at `sample_time_seconds`.
    pub system: &'a StateSpace,
    /// The sample period of the plant in seconds.
    pub sample_time_seconds: f64,
    /// The plant's input delay in seconds.
    pub delay_seconds: f64,
    /// The index of the controller output among the plant's inputs.
    pub controller_input: usize,
    /// The index of the measured disturbance among the plant's inputs.
    pub disturbance_input: usize,
    /// The index of the measurement among the plant's outputs.
    pub measurement: usize,
    /// The PI on the measurement.
    pub law: SamplePi,
    /// The integral step in seconds: the task period.
    pub period_seconds: f64,
    /// The size of the disturbance step, in the disturbance's unit.
    pub disturbance_step: f64,
    /// The duration of the test in seconds.
    pub duration_seconds: f64,
}

/// The unit-step response of `output` for `horizon_seconds`, with the plant's single input at the unit step from the start.
///
/// Open loop (`feedback` is `None`) the input is the unit step itself. With `feedback` set to `(measurement, law)`, the input is
/// the command of the PI on that measurement, which tracks a setpoint of one, and the response is that of `output` through the
/// closed loop (C6). The plant's input is delayed by `delay_seconds`, rounded to whole samples. The plant must have one input.
#[must_use]
pub fn unit_step_response(
    system: &StateSpace,
    sample_time_seconds: f64,
    delay_seconds: f64,
    horizon_seconds: f64,
    output: usize,
    feedback: Option<(usize, SamplePi)>,
) -> SampledResponse {
    let delay_steps = whole_samples(delay_seconds / sample_time_seconds);
    let steps = sample_count(horizon_seconds / sample_time_seconds);
    let mut delay = Delay::new(delay_steps, 1);
    let mut state = vec![0.0; system.states()];
    let mut integral = 0.0;
    let mut times = Vec::with_capacity(steps + 1);
    let mut values = Vec::with_capacity(steps + 1);
    for step in 0..=steps {
        let (error, command) = match feedback {
            Some((measurement, law)) => {
                let delayed = delay.oldest();
                let error = 1.0 - output_value(system, &state, measurement, &delayed);
                (error, law.proportional_gain * error + integral)
            }
            None => (0.0, 1.0),
        };
        let applied = delay.push(vec![command]);
        times.push(seconds(step, sample_time_seconds));
        values.push(output_value(system, &state, output, &applied));
        if let Some((_, law)) = feedback {
            integral += law.integral_gain * error * sample_time_seconds;
        }
        state = advance(system, &state, &applied);
    }
    SampledResponse { times, values }
}

/// The measurement after a measured-disturbance step at the start of the test (C8), with the PI on the measurement and, when given,
/// the feedforward: one value per sample, sample `k` at `k` sample periods. The controller output and the disturbance are the two
/// inputs of the plant, and each value is read with the inputs that reach the plant at that sample. The values are the measurement's
/// deviation from its operating value, since the model's states and inputs start from zero deviation.
#[must_use]
pub fn disturbance_trace(case: &DisturbanceCase<'_>, feedforward: Option<Feedforward>) -> Vec<f64> {
    let system = case.system;
    let sample_time = case.sample_time_seconds;
    let delay_steps = whole_samples(case.delay_seconds / sample_time);
    let steps = sample_count(case.duration_seconds / sample_time);
    let input_count = system.inputs();
    let mut delay = Delay::new(delay_steps, input_count);
    let mut state = vec![0.0; system.states()];
    let mut integral = 0.0;
    let mut filtered = 0.0;
    let mut values = Vec::with_capacity(steps + 1);
    for _ in 0..=steps {
        let delayed = delay.oldest();
        let error = -output_value(system, &state, case.measurement, &delayed);
        let mut command = case.law.proportional_gain * error + integral;
        if let Some(term) = feedforward {
            command += feedforward_term(&term, sample_time, &mut filtered, case.disturbance_step);
        }
        let mut inputs = vec![0.0; input_count];
        inputs[case.controller_input] = command;
        inputs[case.disturbance_input] = case.disturbance_step;
        let applied = delay.push(inputs);
        values.push(output_value(system, &state, case.measurement, &applied));
        integral += case.law.integral_gain * error * case.period_seconds;
        state = advance(system, &state, &applied);
    }
    values
}

/// The largest deviation of the measurement from its operating value after a measured-disturbance step at the start of the test
/// (C8), with the PI on the measurement and, when given, the feedforward: the largest magnitude of [`disturbance_trace`].
#[must_use]
pub fn largest_disturbance_deviation(
    case: &DisturbanceCase<'_>,
    feedforward: Option<Feedforward>,
) -> f64 {
    disturbance_trace(case, feedforward)
        .iter()
        .fold(0.0_f64, |largest, value| largest.max(value.abs()))
}

/// The feedforward term of one sample (C8): `K_ff·d` without a lead and a lag. With them, `z` follows `d` through the lag as
/// `z ← z + h/(T_lag + h)·(d − z)` and the term is `K_ff·(z + (T_lead/T_lag)·(d − z))`.
fn feedforward_term(
    term: &Feedforward,
    sample_time: f64,
    filtered: &mut f64,
    disturbance: f64,
) -> f64 {
    match term.lead_lag {
        Some(lead_lag) => {
            *filtered +=
                sample_time / (lead_lag.lag_seconds + sample_time) * (disturbance - *filtered);
            term.gain
                * (*filtered
                    + lead_lag.lead_seconds / lead_lag.lag_seconds * (disturbance - *filtered))
        }
        None => term.gain * disturbance,
    }
}

/// The input of the plant delayed by whole samples: the value applied at a sample is the one put in that many samples before.
/// Before the first samples the queue holds zeros.
struct Delay {
    queue: VecDeque<Vec<f64>>,
    width: usize,
}

impl Delay {
    /// A delay of `steps` samples for inputs of `width` values.
    fn new(steps: usize, width: usize) -> Self {
        Self {
            queue: (0..steps).map(|_| vec![0.0; width]).collect(),
            width,
        }
    }

    /// The input that the plant receives at this sample: the oldest queued input, or zeros when the queue is empty.
    fn oldest(&self) -> Vec<f64> {
        self.queue
            .front()
            .cloned()
            .unwrap_or_else(|| vec![0.0; self.width])
    }

    /// Queues the input of this sample and returns the input the plant receives now. With no delay that is the input itself.
    fn push(&mut self, input: Vec<f64>) -> Vec<f64> {
        self.queue.push_back(input);
        self.queue.pop_front().unwrap_or_default()
    }
}

/// The dot product of two vectors, summed in order from the first element.
fn dot(weights: &[f64], values: &[f64]) -> f64 {
    weights
        .iter()
        .zip(values)
        .fold(0.0, |sum, (weight, value)| sum + weight * value)
}

/// The output of a row of the plant at a state and an applied input: `C_row·x + D_row·u`.
fn output_value(system: &StateSpace, state: &[f64], row: usize, applied: &[f64]) -> f64 {
    dot(&system.c[row], state) + dot(&system.d[row], applied)
}

/// The next state of the plant: `x(k+1) = A·x(k) + B·u(k)`.
fn advance(system: &StateSpace, state: &[f64], applied: &[f64]) -> Vec<f64> {
    system
        .a
        .iter()
        .zip(&system.b)
        .map(|(row, input_row)| dot(row, state) + dot(input_row, applied))
        .collect()
}

/// A delay in samples: the ratio rounded to the nearest whole number, as Control Studio's `Math.round` does for a ratio of
/// seconds over the sample period.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // A non-negative ratio of seconds, rounded to whole samples.
fn whole_samples(ratio: f64) -> usize {
    ratio.round() as usize
}

/// The number of samples after the start for a duration: the ratio rounded up, and at most [`MAXIMUM_SAMPLES`].
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // A non-negative ratio, saturated at the sample limit.
fn sample_count(ratio: f64) -> usize {
    (ratio.ceil() as usize).min(MAXIMUM_SAMPLES)
}

/// The time of a sample: its index times the sample period.
#[allow(clippy::cast_precision_loss)] // A sample index is far below 2⁵³.
fn seconds(step: usize, sample_time: f64) -> f64 {
    step as f64 * sample_time
}

#[cfg(test)]
#[path = "sampled_loop_tests.rs"]
mod tests;
