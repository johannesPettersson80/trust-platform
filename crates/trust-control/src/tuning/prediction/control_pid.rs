//! The `Control.PID` law that Control Studio predicts with: Specification 45 §3 in the discrete form of its
//! update, as `controllerCandidate.ts` computes it (C0 and C1 of the Control Studio specification).
//!
//! The positional form (`P` or `PD`) runs when the integral gain is exactly zero, the incremental form
//! (`PI` or `PID`) otherwise. The limits of §3.5 bound the output, the anti-windup equation of §3.13 or
//! §3.14 pulls the retained nominal output back, and the setpoint weights of C0 shape the proportional and
//! derivative terms. The parameter record and its validation are the candidate's (`tuning::candidate`); this
//! module is the prediction's copy of the controller's equations. The runtime's block is what Validate runs.

#![allow(clippy::module_name_repetitions)] // `ControlPidState` and `ControlPidLaw` name the law whose records they are; the precedent is `model.rs`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trust_control_math::closed_loop::DiscreteLaw;

use crate::identification::refusal::Refusal;
use crate::tuning::candidate::{validate_parameters, AntiWindup, ControlPidParameters};
use crate::tuning::structure::is_exactly_zero;

/// The sentence for a cycle time that is not finite and positive.
pub const CYCLE_TIME_NOT_POSITIVE: &str = "Control.PID cycle time must be finite and positive.";

/// The operating modes of the law (spec 45 §2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlPidMode {
    /// The controller equations calculate the output.
    Auto,
    /// The manual output is the nominal output before limiting (spec 45 §3.8).
    Manual,
    /// The tracking output replaces the retained nominal output before the controller equations (§3.7).
    Track,
}

/// The retained state of the law (spec 45 §2.5): the seven values that an initialization sets.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlPidState {
    /// The latest setpoint `xr`.
    pub setpoint: f64,
    /// The latest setpoint derivative `xdr`.
    pub setpoint_rate: f64,
    /// The latest process value `xy`.
    pub process_value: f64,
    /// The latest process-value derivative `xdy`.
    pub process_value_rate: f64,
    /// The latest feedforward contribution `xuff`.
    pub feed_forward: f64,
    /// The latest unsaturated output `xu`, after the anti-windup correction.
    pub nominal_output: f64,
    /// The latest output after the absolute and rate limits `xus`.
    pub saturated_output: f64,
}

impl ControlPidState {
    /// The state at an operating point: the setpoint and the process value at the operating output, zero
    /// rates and feedforward, and the operating input as both outputs. This is where a prediction starts
    /// (B4.5).
    #[must_use]
    pub const fn at_operating_point(operating_input: f64, operating_output: f64) -> Self {
        Self {
            setpoint: operating_output,
            setpoint_rate: 0.0,
            process_value: operating_output,
            process_value_rate: 0.0,
            feed_forward: 0.0,
            nominal_output: operating_input,
            saturated_output: operating_input,
        }
    }
}

/// One sample of the law (spec 45 §2.6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlPidInput {
    /// The current setpoint `r`.
    pub setpoint: f64,
    /// The current process value `y`, already filtered where the program filters it.
    pub process_value: f64,
    /// The current feedforward contribution `uff`.
    pub feed_forward: f64,
    /// The nominal output requested in Manual mode `uman`.
    pub manual_output: f64,
    /// The output applied externally, used as the calculation base in Track mode `utrack`.
    pub tracking_output: f64,
    /// The operating mode.
    pub mode: ControlPidMode,
    /// The actual interval since the previous update, in seconds (`Dt`).
    pub cycle_time_seconds: f64,
}

/// The output of one update and the state it leaves (spec 45 §3.15).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlPidStep {
    /// The limited controller output.
    pub output: f64,
    /// The retained state after the update.
    pub state: ControlPidState,
}

/// One update of the law (spec 45 §3.4 to §3.15), for the mode of the input.
///
/// # Errors
/// Returns the refusal of the parameter record (`tuning::candidate::validate_parameters`), or the sentence
/// [`CYCLE_TIME_NOT_POSITIVE`] when the cycle time is not finite and positive.
pub fn update_control_pid(
    parameters: &ControlPidParameters,
    state: &ControlPidState,
    input: &ControlPidInput,
) -> Result<ControlPidStep, Refusal> {
    validate_parameters(parameters)?;
    let period = input.cycle_time_seconds;
    if !(period.is_finite() && period > 0.0) {
        return Err(Refusal::new(CYCLE_TIME_NOT_POSITIVE));
    }
    // §3.5: the limits follow the previous saturated output, not the nominal one.
    let lower_limit = parameters
        .output_minimum
        .max(state.saturated_output + period * parameters.rate_minimum);
    let upper_limit = parameters
        .output_maximum
        .min(state.saturated_output + period * parameters.rate_maximum);
    // §3.6: the derivatives use the actual cycle time.
    let setpoint_rate = (input.setpoint - state.setpoint) / period;
    let process_value_rate = (input.process_value - state.process_value) / period;
    // §3.7: Track replaces the retained nominal output before the integral switch.
    let retained_nominal = if input.mode == ControlPidMode::Track {
        input.tracking_output
    } else {
        state.nominal_output
    };
    let nominal_output = match input.mode {
        ControlPidMode::Manual => input.manual_output,
        _ if is_exactly_zero(parameters.integral_gain) => {
            // §3.10: the positional form; the proportional setpoint weight is ignored there.
            parameters.bias
                + parameters.proportional_gain * (input.setpoint - input.process_value)
                + parameters.derivative_gain
                    * (parameters.derivative_setpoint_weight * setpoint_rate - process_value_rate)
                + input.feed_forward
        }
        _ => {
            // §3.11: the incremental form, with the increments of the signals.
            let proportional_increment = parameters.proportional_gain
                * (parameters.proportional_setpoint_weight * (input.setpoint - state.setpoint)
                    - (input.process_value - state.process_value));
            let integral_increment =
                parameters.integral_gain * period * (input.setpoint - input.process_value);
            let derivative_increment = parameters.derivative_gain
                * (parameters.derivative_setpoint_weight * (setpoint_rate - state.setpoint_rate)
                    - (process_value_rate - state.process_value_rate));
            let feed_forward_increment = input.feed_forward - state.feed_forward;
            let candidate = retained_nominal
                + proportional_increment
                + integral_increment
                + derivative_increment
                + feed_forward_increment;
            anti_windup(
                parameters,
                AntiWindupInputs {
                    candidate,
                    integral_increment,
                    period,
                    lower_limit,
                    upper_limit,
                },
            )
        }
    };
    let output = clamp_between(nominal_output, lower_limit, upper_limit);
    Ok(ControlPidStep {
        output,
        state: ControlPidState {
            setpoint: input.setpoint,
            setpoint_rate,
            process_value: input.process_value,
            process_value_rate,
            feed_forward: input.feed_forward,
            nominal_output,
            saturated_output: output,
        },
    })
}

/// The values that the anti-windup correction reads from one update.
#[derive(Debug, Clone, Copy)]
struct AntiWindupInputs {
    candidate: f64,
    integral_increment: f64,
    period: f64,
    lower_limit: f64,
    upper_limit: f64,
}

/// The nominal output after the anti-windup equation of §3.13 or §3.14. `candidate` is the nominal output before
/// the correction.
fn anti_windup(parameters: &ControlPidParameters, inputs: AntiWindupInputs) -> f64 {
    let saturated_candidate =
        clamp_between(inputs.candidate, inputs.lower_limit, inputs.upper_limit);
    match parameters.anti_windup {
        AntiWindup::BackCalculation => {
            inputs.candidate
                - inputs.period / parameters.tracking_time
                    * (inputs.candidate - saturated_candidate)
        }
        AntiWindup::Combined => {
            // §3.14: integration that points farther into saturation is limited first.
            let saturation_error = inputs.candidate - saturated_candidate;
            let mut nominal = inputs.candidate;
            if inputs.integral_increment * saturation_error > 0.0 {
                let correction = inputs.integral_increment.abs().min(saturation_error.abs());
                nominal += if saturation_error > 0.0 {
                    -correction
                } else {
                    correction
                };
            }
            nominal
                - (inputs.period / parameters.tracking_time).min(1.0)
                    * (nominal - saturated_candidate)
        }
    }
}

/// `clamp(value, lower, upper)` as the library block computes it: the lower bound wins when the bounds cross.
fn clamp_between(value: f64, lower: f64, upper: f64) -> f64 {
    lower.max(upper.min(value))
}

/// The law as a discrete controller of the closed-loop simulation: it keeps its parameter record and its
/// retained state, and gives the Auto output for each period.
#[derive(Debug, Clone, PartialEq)]
pub struct ControlPidLaw {
    parameters: ControlPidParameters,
    state: ControlPidState,
}

impl ControlPidLaw {
    /// Makes the law from a parameter record and the state it starts from.
    ///
    /// # Errors
    /// Returns the refusal of the parameter record.
    pub fn new(parameters: ControlPidParameters, state: ControlPidState) -> Result<Self, Refusal> {
        validate_parameters(&parameters)?;
        Ok(Self { parameters, state })
    }

    /// The retained state after the last update.
    #[must_use]
    pub fn state(&self) -> &ControlPidState {
        &self.state
    }
}

impl DiscreteLaw for ControlPidLaw {
    type Refusal = Refusal;

    fn command(
        &mut self,
        setpoint: f64,
        measurement: f64,
        period_seconds: f64,
    ) -> Result<f64, Refusal> {
        let input = ControlPidInput {
            setpoint,
            process_value: measurement,
            feed_forward: 0.0,
            manual_output: 0.0,
            tracking_output: 0.0,
            mode: ControlPidMode::Auto,
            cycle_time_seconds: period_seconds,
        };
        let step = update_control_pid(&self.parameters, &self.state, &input)?;
        self.state = step.state;
        Ok(step.output)
    }
}

#[cfg(test)]
#[path = "control_pid_tests.rs"]
mod tests;
