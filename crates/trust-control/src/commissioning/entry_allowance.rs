//! The Auto-entry allowance (specification 35, A6.4): how far the first controller output may move at an Auto entry, from the
//! exact check, the reviewed tolerance, the measured noise and the error the controller enters with. Ported from
//! `proportionalTransfer.ts` (`transferTolerance`, `noiseAwareTransferTolerance`) and `commissioningStartingPoint.ts`
//! (`autoEntryTolerance`, `autoEntryError`, `lawEntryTolerance`).

use std::collections::BTreeMap;

use trust_control_math::linalg::DenseMatrix;

use super::project_law::gain_symbols;
use super::state_adapter::decode_state_law;
use super::texts;
use super::transfer::{output_limits, TransferValue};
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{ControllerProject, LoopDefinition};
use crate::model::GeneralModel;
use crate::tuning::state_controller::sampling::state_space;
use crate::tuning::structure::is_exactly_zero;
use crate::validation::engineer_verification_plan::EngineerRequirements;
use crate::validation::model_route::bound_channels;

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

/// The refusal of a search law's Auto entry without a reviewed tolerance (C5, A6.4): its finite search has no first-output formula.
pub const REVIEWED_TOLERANCE_REQUIRED: &str = "Auto entry needs a reviewed Auto-entry tolerance, because a search law has no first-output formula (C5, A6.4). Declare the tolerance in Requirements.";
/// The refusal of a state law's allowance asked for without its model, which says which states are measured outputs (C4).
const STATE_NEEDS_MODEL: &str = "The Auto-entry allowance of a state law needs its model (C4).";

/// The allowance of the first Auto output for a law that needs no model, with its gains in `values` (`autoEntryTolerance`, A6.4). A
/// reviewed tolerance of the task replaces every other term. Otherwise a PID or P law gets its allowance of
/// [`noise_aware_transfer_tolerance`], and a loop without a law the exact check. A search law has no formula, so it has no allowance
/// without a reviewed tolerance (C5). A state law's allowance needs its model: [`auto_entry_allowance`] gives it for any law.
///
/// # Errors
/// The reviewed tolerance is out of range, an output limit is not set, the loop declares state feedback (which needs its model), or
/// the loop is a search law with no reviewed tolerance.
pub fn auto_entry_tolerance(
    loop_definition: &LoopDefinition,
    task: &EngineerRequirements,
    values: &BTreeMap<String, f64>,
    process_noise: Option<f64>,
    entry_error: Option<f64>,
) -> Result<f64, Failure> {
    // A binary output has no band (C3): its first output is the applied level, so its allowance is zero, not the A6.4 band.
    if loop_definition.controller.on_off.is_some() {
        return Ok(0.0);
    }
    let exact = transfer_tolerance(loop_definition, task.transfer_tolerance)?;
    if task.transfer_tolerance.is_some() {
        return Ok(exact);
    }
    if loop_definition.controller.state_feedback.is_some() {
        return Err(Failure::new(
            STATE_NEEDS_MODEL,
            Cause::Implementation,
            NextAction::None,
        ));
    }
    if search_law(loop_definition) {
        return Err(Failure::new(
            REVIEWED_TOLERANCE_REQUIRED,
            Cause::Settings,
            NextAction::OpenRequirements,
        ));
    }
    law_entry_tolerance(loop_definition, values, process_noise, entry_error)
}

/// The allowance of the first Auto output for any law (A6.4, C4): a state law's C4 allowance from its model, and every other law's as
/// [`auto_entry_tolerance`] gives it. A reviewed tolerance replaces the state law's allowance as it replaces every other.
///
/// # Errors
/// As [`auto_entry_tolerance`], and as [`state_entry_tolerance`] for a state law without a reviewed tolerance.
pub fn auto_entry_allowance(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    task: &EngineerRequirements,
    values: &BTreeMap<String, f64>,
    process_noise: Option<f64>,
    entry_error: Option<f64>,
) -> Result<f64, Failure> {
    if loop_definition.controller.state_feedback.is_some() && task.transfer_tolerance.is_none() {
        return state_entry_tolerance(loop_definition, model, task, values);
    }
    auto_entry_tolerance(loop_definition, task, values, process_noise, entry_error)
}

/// Whether the loop's controller is the search-law form of a program's law (C5): a `controller.project` with tuning axes and no seed.
fn search_law(loop_definition: &LoopDefinition) -> bool {
    loop_definition
        .controller
        .project
        .as_ref()
        .is_some_and(ControllerProject::is_search_law)
}

/// The C4 first-output allowance of a state law without a reviewed tolerance (`autoEntryTolerance`, A6.4, C4): for each plant input `i`,
/// `Σⱼ|Kᵢⱼ|·bⱼ + Σₖ|Nᵢₖ|·bₖ`, where `b` is the settling band of the output the state or the controlled output is: the output's channel
/// requirement when it declares one, else the task's band. A state is a measured output when its column of `C` selects exactly that
/// output and that output's row selects only that state. A state that is not a measured output has no band, so the exact check
/// stands; otherwise the allowance is the largest input's sum, and the exact check stands where it is larger.
///
/// # Errors
/// The record does not decode as a state law; a coefficient has no value in `values`; or the loop's channels do not bind the model.
pub fn state_entry_tolerance(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    task: &EngineerRequirements,
    values: &BTreeMap<String, f64>,
) -> Result<f64, Failure> {
    let law = decode_state_law(loop_definition)?;
    let exact = transfer_tolerance(loop_definition, None)?;
    let Some(system) = state_space(model) else {
        return Ok(exact);
    };
    let bound = bound_channels(loop_definition, model)?;
    let bands: Vec<f64> = bound
        .outputs
        .iter()
        .map(|output| {
            output
                .requirements
                .as_ref()
                .map_or(task.settling_band, |requirements| {
                    requirements.settling_band
                })
        })
        .collect();
    let controlled: Vec<f64> = bound
        .outputs
        .iter()
        .zip(&bands)
        .filter(|(output, _)| output.reference.is_some())
        .map(|(_, band)| *band)
        .collect();
    let state_bands: Option<Vec<f64>> = (0..system.a.len())
        .map(|state| {
            measured_output_of(&system.c, state).and_then(|output| bands.get(output).copied())
        })
        .collect();
    let Some(state_bands) = state_bands else {
        return Ok(exact);
    };
    let mut largest = f64::NEG_INFINITY;
    for (gains, references) in law.gain_symbols.iter().zip(&law.reference_gain_symbols) {
        let mut sum = 0.0;
        for (column, symbol) in gains.iter().enumerate() {
            let Some(band) = state_bands.get(column) else {
                return Ok(exact);
            };
            sum += coefficient_of(values, symbol)?.abs() * band;
        }
        for (column, symbol) in references.iter().enumerate() {
            let Some(band) = controlled.get(column) else {
                return Ok(exact);
            };
            sum += coefficient_of(values, symbol)?.abs() * band;
        }
        if !sum.is_finite() {
            return Ok(exact);
        }
        largest = largest.max(sum);
    }
    Ok(if largest.is_finite() {
        exact.max(largest)
    } else {
        exact
    })
}

/// The output that a state of the model measures (C4): the one output whose row of `C` selects only this state, with a one in the
/// state's column, and no other output selects the state. `None` for any other state.
fn measured_output_of(c: &DenseMatrix, state: usize) -> Option<usize> {
    let selecting: Vec<usize> = c
        .iter()
        .enumerate()
        .filter(|(_, row)| row.get(state).is_some_and(|value| !is_exactly_zero(*value)))
        .map(|(output, _)| output)
        .collect();
    let [output] = selecting.as_slice() else {
        return None;
    };
    let row = &c[*output];
    let selects_only_this_state = row
        .iter()
        .enumerate()
        .all(|(column, value)| column == state || is_exactly_zero(*value));
    (row[state].to_bits() == 1.0_f64.to_bits() && selects_only_this_state).then_some(*output)
}

/// The value of one coefficient of the candidate record (C4). A candidate that holds no value for it is an implementation fault.
fn coefficient_of(values: &BTreeMap<String, f64>, symbol: &str) -> Result<f64, Failure> {
    values.get(symbol).copied().ok_or_else(|| {
        Failure::new(
            &format!("The candidate holds no value for the state coefficient {symbol}."),
            Cause::Implementation,
            NextAction::None,
        )
    })
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
