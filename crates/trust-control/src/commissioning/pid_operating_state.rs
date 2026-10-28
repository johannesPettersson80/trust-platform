//! The operating point of a parallel PID's retained history (specification 35, C1 "The `parallel-pid-v1` initialization"): the
//! integral and derivative history that start the PID at the operating point, so that its first output equals the held one.
//! Ported from `pidOperatingState.ts`.

use std::collections::BTreeMap;

use super::initial_state::{operating_input, operating_output};
use super::texts;
use super::transfer::{rounded_transfer_value, TransferMember};
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{ControllerLaw, LoopDefinition, PidInitialization, StateReference};
use crate::model::GeneralModel;

/// The retained history of a `parallel-pid-v1` PID at the operating point, keyed by state symbol (`pidOperatingState`). With
/// the stationary reference `r₀ = y₀` and the operating command `u₀`, the integral is `u₀ − Kp·(β − 1)·y₀` and the derivative
/// history is `(γ − 1)·y₀`, so the first derivative contribution is zero (C1). A loop without the `parallel-pid-v1`
/// initialization and a PID law has no history, and gets an empty map.
///
/// # Errors
/// The proportional gain in `values` is not finite, or a history value does not fit its state.
pub fn pid_operating_state(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    values: &BTreeMap<String, f64>,
) -> Result<BTreeMap<String, f64>, Failure> {
    let (
        Some(PidInitialization::ParallelPid {
            integral_state,
            derivative_state,
            proportional_setpoint_weight,
            derivative_setpoint_weight,
            ..
        }),
        Some(ControllerLaw::Pid(law)),
    ) = (
        loop_definition.controller.pid_initialization.as_ref(),
        loop_definition.controller.law.as_ref(),
    )
    else {
        return Ok(BTreeMap::new());
    };
    let gain = values
        .get(&law.proportional_gain_symbol)
        .copied()
        .filter(|gain| gain.is_finite())
        .ok_or_else(|| {
            Failure::new(
                &texts::pid_state_gain_required(),
                Cause::Settings,
                NextAction::Tune,
            )
        })?;
    let process = operating_output(model, 0);
    let command = operating_input(model, 0);
    let integral = rounded_state(
        integral_state,
        command - gain * (proportional_setpoint_weight - 1.0) * process,
    )?;
    let derivative = rounded_state(
        derivative_state,
        (derivative_setpoint_weight - 1.0) * process,
    )?;
    Ok(BTreeMap::from([
        (integral_state.symbol.clone(), integral),
        (derivative_state.symbol.clone(), derivative),
    ]))
}

/// A declared state's value, rounded as the state is typed (`roundedTransferValue`, E6.2).
fn rounded_state(state: &StateReference, value: f64) -> Result<f64, Failure> {
    let member = TransferMember {
        symbol: state.symbol.clone(),
        iec_type: state.iec_type.into(),
        unit: state.unit.clone(),
        minimum: state.minimum,
        maximum: state.maximum,
    };
    rounded_transfer_value(&member, value)
}

#[cfg(test)]
#[path = "pid_operating_state_tests.rs"]
mod tests;
