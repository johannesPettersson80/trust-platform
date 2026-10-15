//! The Auto-entry allowance (specification 35, A6.4): how far the first controller output may move at an Auto entry, from the
//! exact check, the reviewed tolerance, the measured noise and the error the controller enters with. Ported from
//! `proportionalTransfer.ts` (`transferTolerance`, `noiseAwareTransferTolerance`) and `commissioningStartingPoint.ts`
//! (`autoEntryTolerance`, `autoEntryError`, `lawEntryTolerance`).

use std::collections::BTreeMap;

use super::project_law::gain_symbols;
use super::texts;
use super::transfer::{output_limits, TransferValue};
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::LoopDefinition;
use crate::validation::engineer_verification_plan::EngineerRequirements;

/// The terms of the A6.4 allowance besides the measured noise: the integral and derivative gains, the task period in s and
/// the error the controller enters Auto with. An absent term contributes nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AllowanceTerms {
    /// The integral gain `Ki`.
    pub integral_gain: Option<f64>,
    /// The derivative gain `Kd`.
    pub derivative_gain: Option<f64>,
    /// The period of the loop's task, in s.
    pub cycle_seconds: Option<f64>,
    /// The error `e` at entry: the setpoint less the measurement.
    pub entry_error: Option<f64>,
}

/// The tolerance of an Auto entry's output: the reviewed value when there is one, otherwise the exact check of
/// `1e-6` times the larger of 1 and the output span (`transferTolerance`, A6.4).
///
/// # Errors
/// The reviewed tolerance is not finite or lies outside `0..=span`, or an output limit is not set.
pub fn transfer_tolerance(
    loop_definition: &LoopDefinition,
    manual: Option<f64>,
) -> Result<f64, Failure> {
    let (minimum, maximum) = output_limits(loop_definition)?;
    let span = maximum - minimum;
    match manual {
        Some(value) if !value.is_finite() || !(0.0..=span).contains(&value) => Err(Failure::new(
            &texts::entry_tolerance_outside_span(),
            Cause::Settings,
            NextAction::OpenRequirements,
        )),
        Some(value) => Ok(value),
        None => Ok(1e-6 * span.max(1.0)),
    }
}

/// The allowance of an Auto entry for a proportional or PID law, from the measured output noise `n` (`processNoise`), the
/// gains and the entry error (`noiseAwareTransferTolerance`, A6.4). The first output moves by at most
/// `2·|Kp|·n` from P on the measured change, `Ki·Δt·(|e| + n)` from I, and `4·|Kd|·n/Δt` from D on the change of the sampled
/// rate; the larger of that and the exact check is the allowance. A reviewed tolerance replaces it, and so does a missing or
/// zero noise or a non-finite proportional gain: the check is then the exact one.
///
/// # Errors
/// The reviewed tolerance is out of range, or an output limit is not set.
pub fn noise_aware_transfer_tolerance(
    loop_definition: &LoopDefinition,
    reviewed: Option<f64>,
    proportional_gain: Option<f64>,
    process_noise: Option<f64>,
    terms: AllowanceTerms,
) -> Result<f64, Failure> {
    // A zero (or NaN) noise is falsy in the `TypeScript`, which then keeps the exact check.
    let noise = process_noise.filter(|noise| noise.abs() > 0.0);
    let gain = proportional_gain.filter(|gain| gain.is_finite());
    let (Some(noise), Some(gain), None) = (noise, gain, reviewed) else {
        return transfer_tolerance(loop_definition, reviewed);
    };
    let cycle = terms.cycle_seconds.filter(|cycle| *cycle > 0.0);
    let error = terms
        .entry_error
        .filter(|error| error.is_finite())
        .map_or(0.0, f64::abs);
    let integral = match (cycle, terms.integral_gain.filter(|gain| gain.is_finite())) {
        (Some(cycle), Some(integral_gain)) => integral_gain.abs() * cycle * (error + noise),
        _ => 0.0,
    };
    let derivative = match (cycle, terms.derivative_gain.filter(|gain| gain.is_finite())) {
        (Some(cycle), Some(derivative_gain)) => 4.0 * derivative_gain.abs() * noise / cycle,
        _ => 0.0,
    };
    let exact = transfer_tolerance(loop_definition, None)?;
    Ok(exact.max(2.0 * gain.abs() * noise + integral + derivative))
}

/// The allowance of the first Auto output for a controller, with its gains in `values` (`autoEntryTolerance`, A6.4). A
/// reviewed tolerance of the task replaces every other term. Otherwise a PID or P law gets its allowance of
/// [`noise_aware_transfer_tolerance`], and a loop without a law the exact check.
///
/// The state-feedback allowance of the `TypeScript` is a parked feature: a loop that declares state feedback is refused.
///
/// # Errors
/// The reviewed tolerance is out of range, an output limit is not set, or the loop declares state feedback, which the Rust
/// engine does not run yet.
pub fn auto_entry_tolerance(
    loop_definition: &LoopDefinition,
    task: &EngineerRequirements,
    values: &BTreeMap<String, f64>,
    process_noise: Option<f64>,
    entry_error: Option<f64>,
) -> Result<f64, Failure> {
    let exact = transfer_tolerance(loop_definition, task.transfer_tolerance)?;
    if task.transfer_tolerance.is_some() {
        return Ok(exact);
    }
    if loop_definition.controller.state_feedback.is_some() {
        return Err(Failure::new(
            &texts::entry_state_feedback_parked(),
            Cause::ControllerCapability,
            NextAction::None,
        ));
    }
    law_entry_tolerance(loop_definition, values, process_noise, entry_error)
}

/// The allowance of a PID or P law with the gains `values` holds (`lawEntryTolerance`): the gains are the law's own, or a program's
/// seed variables (C2). A loop without a law has the exact check.
fn law_entry_tolerance(
    loop_definition: &LoopDefinition,
    values: &BTreeMap<String, f64>,
    process_noise: Option<f64>,
    entry_error: Option<f64>,
) -> Result<f64, Failure> {
    let cycle_seconds = Some(loop_definition.controller.task_period_ms / 1000.0);
    let gain = |symbol: &str| values.get(symbol).copied();
    match gain_symbols(loop_definition) {
        Some(symbols) => noise_aware_transfer_tolerance(
            loop_definition,
            None,
            gain(symbols.proportional),
            process_noise,
            AllowanceTerms {
                integral_gain: symbols.integral.and_then(gain),
                derivative_gain: symbols.derivative.and_then(gain),
                cycle_seconds,
                entry_error,
            },
        ),
        None => transfer_tolerance(loop_definition, None),
    }
}

/// The error a controller enters Auto with: its setpoint less its measurement, when the loop has a setpoint and both are
/// numbers (`autoEntryError`, A6.4).
#[must_use]
pub fn auto_entry_error(
    loop_definition: &LoopDefinition,
    values: &BTreeMap<String, TransferValue>,
) -> Option<f64> {
    let setpoint = loop_definition
        .signals
        .setpoint
        .as_ref()
        .and_then(|signal| values.get(&signal.symbol).copied())
        .and_then(TransferValue::as_number)?;
    let measured = values
        .get(&loop_definition.signals.process_variable.symbol)
        .copied()
        .and_then(TransferValue::as_number)?;
    Some(setpoint - measured)
}

#[cfg(test)]
#[path = "entry_allowance_tests.rs"]
mod tests;
