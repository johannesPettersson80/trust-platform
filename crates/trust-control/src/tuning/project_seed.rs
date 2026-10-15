//! The program's own laws (specification 35, C2, C11 and §17.16; numerical annex N6.7): a `parallel-pid` seed names the variables
//! that hold a program's gains, so that the law can be designed on the model as a `Control.PID` is. A `trajectory-feedforward-pid`
//! seed names the same three gains and, beside them, the variables of the rate and acceleration feedforward gains of a law that
//! follows a planned trajectory (C11). The designed gains are written to the variables the seed names; nothing here replaces the
//! program's equations.
//!
//! The seeds of a measured-disturbance feedforward (C8) and of a cascade (C6) are here too. The other seed kinds (decentralized,
//! Smith predictor and decoupled groups) are not part of this module; the decoder refuses them.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trust_control_math::model::state_space_transfer_polynomials;

use crate::identification::refusal::Refusal;
use crate::model::limits::MAXIMUM_CONVERSION_ORDER;
use crate::model::GeneralModel;
use crate::tuning::structure::{is_exactly_zero, ParallelGains, Structure};
use crate::tuning::texts;

/// A program law whose parallel gains are named by variables (C2): the family, the proportional variable, and the integral and derivative
/// variables when the law has them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParallelPidSeed {
    /// The family the law has: P, PI, PD or PID.
    pub family: Structure,
    /// The variable that holds the proportional gain.
    pub proportional_symbol: String,
    /// The variable that holds the integral gain, when the law has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integral_symbol: Option<String>,
    /// The variable that holds the derivative gain, when the law has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derivative_symbol: Option<String>,
}

/// The seed of a trajectory-following law (C11): the PID part, whose three gain variables are all named, and the variables of the rate
/// and acceleration feedforward gains `K_v` and `K_a`. The five variables are distinct (`validTrajectorySeed`).
#[derive(Debug, Clone, PartialEq)]
pub struct TrajectoryFeedforwardSeed {
    /// The PID part: the family and the proportional, integral and derivative variables.
    pub pid: ParallelPidSeed,
    /// The variable that holds the rate feedforward gain `K_v`.
    pub rate_symbol: String,
    /// The variable that holds the acceleration feedforward gain `K_a`.
    pub acceleration_symbol: String,
}

impl TrajectoryFeedforwardSeed {
    /// The five variables the seed names: the PID's proportional, integral and derivative variables, then the rate and the acceleration
    /// gains.
    #[must_use]
    pub fn variables(&self) -> Vec<&str> {
        let mut symbols = vec![self.pid.proportional_symbol.as_str()];
        symbols.extend(self.pid.integral_symbol.as_deref());
        symbols.extend(self.pid.derivative_symbol.as_deref());
        symbols.push(self.rate_symbol.as_str());
        symbols.push(self.acceleration_symbol.as_str());
        symbols
    }
}

/// The variables of the lead and the lag time of a feedforward (C8): both are named, or neither is.
#[derive(Debug, Clone, PartialEq)]
pub struct LeadLagSymbols {
    /// The variable that holds the lead time `T_lead` in seconds.
    pub lead_symbol: String,
    /// The variable that holds the lag time `T_lag` in seconds.
    pub lag_symbol: String,
}

/// The seed of a measured-disturbance feedforward (C8): the PI's three gain variables, the variable of the feedforward gain `K_ff`, and
/// the variables of the lead and the lag times when the program has both (`validFeedforwardSeed`). The feedback law is a PI, so its
/// derivative variable is named and written as zero.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedforwardPidSeed {
    /// The PI part: the family PI, with its proportional, integral and derivative variables.
    pub pid: ParallelPidSeed,
    /// The variable that holds the feedforward gain `K_ff`.
    pub gain_symbol: String,
    /// The lead and lag variables, when the program has a lead and a lag.
    pub lead_lag: Option<LeadLagSymbols>,
}

impl FeedforwardPidSeed {
    /// The variables the seed names: the PI's three gains, the feedforward gain, and the lead and lag times when the seed names them.
    #[must_use]
    pub fn variables(&self) -> Vec<&str> {
        let mut symbols = vec![self.pid.proportional_symbol.as_str()];
        symbols.extend(self.pid.integral_symbol.as_deref());
        symbols.extend(self.pid.derivative_symbol.as_deref());
        symbols.push(self.gain_symbol.as_str());
        if let Some(lead_lag) = &self.lead_lag {
            symbols.push(lead_lag.lead_symbol.as_str());
            symbols.push(lead_lag.lag_symbol.as_str());
        }
        symbols
    }
}

/// The variables of a program that uses the library block `Control.SmithPredictorFOPDT` (C9): the variable of its `Update` result, the
/// variable of its `MaximumCorrection`, and the variables of its `OperatingInput` and `OperatingOutput`. The result and the maximum are
/// read by Validate and never written; the operating point is written from the model (D-18). The four are named together or not at all.
#[derive(Debug, Clone, PartialEq)]
pub struct SmithLibraryBlock {
    /// The variable that holds the block's `SmithPredictorOutput`, which Validate reads for the predictor's flags (C9).
    pub output_symbol: String,
    /// The variable that holds the program's `MaximumCorrection`: the application's protection, read by Validate and not tuned (C9).
    pub maximum_correction_symbol: String,
    /// The variable that holds the block's `OperatingInput`: the model's operating input.
    pub operating_input_symbol: String,
    /// The variable that holds the block's `OperatingOutput`: the model's operating output.
    pub operating_output_symbol: String,
}

/// The seed of a Smith predictor (C9): the PI's three gain variables, the variables of the model gain `K`, the time constant `T` and the
/// delay `θ`, and, when the program uses `Control.SmithPredictorFOPDT`, the block's variables (`SmithLibraryBlock`).
#[derive(Debug, Clone, PartialEq)]
pub struct SmithPidSeed {
    /// The PI part: the family PI, with its proportional, integral and derivative variables.
    pub pid: ParallelPidSeed,
    /// The variable that holds the model gain `K`.
    pub gain_symbol: String,
    /// The variable that holds the model time constant `T` in seconds.
    pub time_constant_symbol: String,
    /// The variable that holds the model delay `θ` in seconds.
    pub delay_symbol: String,
    /// The library block's variables, when the program uses `Control.SmithPredictorFOPDT`.
    pub library_block: Option<SmithLibraryBlock>,
}

impl SmithPidSeed {
    /// The variables that Apply writes: the PI's three gains, the model's three values, and the block's operating point when the
    /// program uses the block. The block's result and its maximum are read, not written, so they are not listed.
    #[must_use]
    pub fn variables(&self) -> Vec<&str> {
        let mut symbols = vec![self.pid.proportional_symbol.as_str()];
        symbols.extend(self.pid.integral_symbol.as_deref());
        symbols.extend(self.pid.derivative_symbol.as_deref());
        symbols.push(self.gain_symbol.as_str());
        symbols.push(self.time_constant_symbol.as_str());
        symbols.push(self.delay_symbol.as_str());
        if let Some(block) = &self.library_block {
            symbols.push(block.operating_input_symbol.as_str());
            symbols.push(block.operating_output_symbol.as_str());
        }
        symbols
    }
}

/// The values of a Smith predictor at its design (C9): the model's gain, time constant and delay, and the operating point the model was
/// identified at, which the library block takes as its `OperatingInput` and `OperatingOutput` (D-18).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmithTerms {
    /// The model gain `K`.
    pub gain: f64,
    /// The model time constant `T` in seconds.
    pub time_constant_seconds: f64,
    /// The model delay `θ` in seconds.
    pub delay_seconds: f64,
    /// The model's operating input and operating output (zero where the model declares none).
    pub operating: (f64, f64),
}

/// One `Control.PID` of a cascade (C6): its instance path, and the variables of its proportional, integral and derivative gains.
#[derive(Debug, Clone, PartialEq)]
pub struct CascadeGains {
    /// The controller's instance path, which names it in the loop (`validCascadeSeed`).
    pub instance: String,
    /// The variable that holds the proportional gain.
    pub proportional_symbol: String,
    /// The variable that holds the integral gain.
    pub integral_symbol: String,
    /// The variable that holds the derivative gain, which the cascade's PI writes as zero (C0).
    pub derivative_symbol: String,
}

/// The seed of a cascade (C6): the inner controller, which drives the plant input, and the outer one, whose output is the inner setpoint.
/// Both are PI, so the seed names the three variables of each; the six variables are distinct (`validCascadeSeed`).
#[derive(Debug, Clone, PartialEq)]
pub struct CascadePidSeed {
    /// The inner controller.
    pub inner: CascadeGains,
    /// The outer controller.
    pub outer: CascadeGains,
}

impl CascadePidSeed {
    /// The six variables the seed names: the inner controller's proportional, integral and derivative variables, then the outer's.
    #[must_use]
    pub fn variables(&self) -> Vec<&str> {
        [&self.inner, &self.outer]
            .into_iter()
            .flat_map(|gains| {
                [
                    gains.proportional_symbol.as_str(),
                    gains.integral_symbol.as_str(),
                    gains.derivative_symbol.as_str(),
                ]
            })
            .collect()
    }
}

/// The values of a feedforward (C8) at its design: the gain `K_ff`, and the lead and the lag times when the program has them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeedforwardTerms {
    /// The feedforward gain `K_ff`.
    pub gain: f64,
    /// The lead time `T_lead` in seconds, when the program has a lead and a lag.
    pub lead_seconds: Option<f64>,
    /// The lag time `T_lag` in seconds, when the program has a lead and a lag.
    pub lag_seconds: Option<f64>,
}

/// The gains that a parallel seed's variables hold, from their values (`parallelSeedGains`): the proportional variable must be
/// finite, and an absent integral or derivative variable is zero.
#[must_use]
pub fn parallel_seed_gains(
    seed: &ParallelPidSeed,
    parameters: &BTreeMap<String, f64>,
) -> Option<ParallelGains> {
    let proportional = parameters
        .get(&seed.proportional_symbol)
        .copied()
        .filter(|value| value.is_finite())?;
    Some(ParallelGains {
        proportional_gain: proportional,
        integral_gain: seed
            .integral_symbol
            .as_ref()
            .and_then(|symbol| parameters.get(symbol).copied())
            .unwrap_or(0.0),
        derivative_gain: seed
            .derivative_symbol
            .as_ref()
            .and_then(|symbol| parameters.get(symbol).copied())
            .unwrap_or(0.0),
    })
}

/// Checks that a seed's law has the terms a family needs (`familyGains` of the TypeScript): an integral variable for PI and
/// PID, a derivative variable for PD and PID.
///
/// # Errors
/// Returns a [`Refusal`] naming the family and the missing term.
pub fn check_family_terms(seed: &ParallelPidSeed, family: Structure) -> Result<(), Refusal> {
    let word = crate::tuning::design_to_requirements::structure_word(family);
    if family.has_integral() && seed.integral_symbol.is_none() {
        return Err(Refusal::new(texts::seed_needs_integral(word)));
    }
    if family.has_derivative() && seed.derivative_symbol.is_none() {
        return Err(Refusal::new(texts::seed_needs_derivative(word)));
    }
    Ok(())
}

/// The values of the variables a seed names for the designed gains (the return of `familyGains`): the proportional variable
/// always, and the integral and derivative variables only where the seed names them.
#[must_use]
pub fn seed_values(seed: &ParallelPidSeed, gains: ParallelGains) -> BTreeMap<String, f64> {
    let mut values = BTreeMap::from([(seed.proportional_symbol.clone(), gains.proportional_gain)]);
    if let Some(symbol) = &seed.integral_symbol {
        values.insert(symbol.clone(), gains.integral_gain);
    }
    if let Some(symbol) = &seed.derivative_symbol {
        values.insert(symbol.clone(), gains.derivative_gain);
    }
    values
}

/// The rate and acceleration feedforward of `n/(a·s² + b·s)` (`trajectoryFeedforward`): `K_v = b/n` and `K_a = a/n`, the
/// output with which the model follows the trajectory exactly (C11). A zero numerator, a zero `b`, or a value that is not
/// finite has no such feedforward.
///
/// # Errors
/// Returns a [`Refusal`] with the trajectory sentence for a model without that form.
pub fn trajectory_feedforward(numerator: f64, a: f64, b: f64) -> Result<(f64, f64), Refusal> {
    let usable = [numerator, a, b].iter().all(|value| value.is_finite())
        && !is_exactly_zero(numerator)
        && !is_exactly_zero(b);
    if usable {
        Ok((b / numerator, a / numerator))
    } else {
        Err(Refusal::new(texts::TRAJECTORY_REFUSED))
    }
}

/// The feedforward gains of a transfer function given by its coefficients from the highest power down (`trajectoryFeedforward`, C11):
/// the numerator is one nonzero constant, and the denominator is `a·s² + b·s` or, with `a = 0`, `b·s`, with no constant term. A
/// leading zero of either polynomial is no coefficient.
///
/// # Errors
/// Returns a [`Refusal`] with the trajectory sentence when the transfer function has another form.
pub fn trajectory_feedforward_of(
    numerator: &[f64],
    denominator: &[f64],
) -> Result<(f64, f64), Refusal> {
    let [gain] = without_leading_zeros(numerator) else {
        return Err(Refusal::new(texts::TRAJECTORY_REFUSED));
    };
    let (quadratic, linear) = match without_leading_zeros(denominator) {
        [quadratic, linear, constant] if is_exactly_zero(*constant) => (*quadratic, *linear),
        [linear, constant] if is_exactly_zero(*constant) => (0.0, *linear),
        _ => return Err(Refusal::new(texts::TRAJECTORY_REFUSED)),
    };
    trajectory_feedforward(*gain, quadratic, linear)
}

/// The feedforward gains of a model (C11): its transfer function, or the exact transfer polynomials of a continuous state-space
/// model (`state_space_transfer_polynomials`). A discrete model has z-plane poles, so it has no trajectory feedforward rule.
///
/// # Errors
/// Returns a [`Refusal`] with the trajectory sentence when the model has no trajectory feedforward rule.
pub fn trajectory_feedforward_of_model(model: &GeneralModel) -> Result<(f64, f64), Refusal> {
    match model {
        GeneralModel::TransferFunction { transfer, .. } => {
            trajectory_feedforward_of(&transfer.numerator, &transfer.denominator)
        }
        GeneralModel::ContinuousStateSpace { system, .. } => {
            let transfer = state_space_transfer_polynomials(system, MAXIMUM_CONVERSION_ORDER)
                .map_err(|_| Refusal::new(texts::TRAJECTORY_REFUSED))?;
            trajectory_feedforward_of(&transfer.numerator, &transfer.denominator)
        }
        GeneralModel::DiscreteStateSpace { .. } => Err(Refusal::new(texts::TRAJECTORY_REFUSED)),
    }
}

/// The coefficients of a polynomial without its leading zeros, from the highest power that is not zero.
fn without_leading_zeros(coefficients: &[f64]) -> &[f64] {
    let first = coefficients
        .iter()
        .position(|value| !is_exactly_zero(*value))
        .unwrap_or(coefficients.len());
    &coefficients[first..]
}

#[cfg(test)]
#[path = "project_seed_tests.rs"]
mod tests;
