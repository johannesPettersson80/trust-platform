//! A program's own law as the commissioning adapters read it (specification 35, C1, C2 and C13): the seed that names its gain
//! variables, the gain variables a loop's law names, the route a law takes, and the application states that its entry writes. The
//! route follows the variables the seed names, not the structure Tune chose: a seed with an integral or a derivative variable is a
//! declared-state PID law, and a seed with neither is a connected P (C13). The families a block offers are
//! `ControllerProject::offered_families`.

use std::collections::BTreeMap;

use super::adapters::ControllerGains;
use super::initial_state::transfer_member;
use super::texts;
use super::transfer::{rounded_transfer_value, TransferBinding, TransferMember, TransferValue};
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{
    ControllerLaw, ControllerProject, InitialValue, InitializedState, LoopDefinition,
};
use crate::tuning::project_candidate::{served_feedforward, served_trajectory, ProjectCandidate};
use crate::tuning::project_seed::ParallelPidSeed;

/// A program's own law that this engine serves (C2): its controller block and the seed that names its gain variables.
#[derive(Debug, Clone, Copy)]
pub struct ServedLaw<'a> {
    /// The controller's `project` block.
    pub project: &'a ControllerProject,
    /// The `parallel-pid` seed of the block.
    pub seed: &'a ParallelPidSeed,
}

/// The program's own law of a loop, when its block is served: a `parallel-pid` or a trajectory-feedforward seed, whose PID part it reads,
/// and no feature the gate parks (a tuning axis, a component or a model parameter, `ControllerProject::parked_feature`). Any other block is not served here: the parked-feature gate
/// refuses it by name before an adapter is chosen.
#[must_use]
pub fn served_law(loop_definition: &LoopDefinition) -> Option<ServedLaw<'_>> {
    let project = loop_definition.controller.project.as_ref()?;
    if project.parked_feature().is_some() {
        return None;
    }
    let seed = project.seed.as_ref()?.pid_seed()?;
    Some(ServedLaw { project, seed })
}

/// The gains of a program's own law's candidate for the adapter (C2, C8, C11): the three PID gains, for a trajectory-following law its
/// rate and acceleration feedforward gains, and for a measured-disturbance feedforward its gain and its lead and lag times, each read
/// from the variable its seed names. Apply and Validate write these values (C0).
#[must_use]
pub fn project_gains(
    loop_definition: &LoopDefinition,
    candidate: &ProjectCandidate,
) -> ControllerGains {
    let feedforward = served_trajectory(loop_definition);
    let measured = served_feedforward(loop_definition);
    let lead_lag = measured.and_then(|seed| seed.lead_lag.as_ref());
    ControllerGains {
        proportional_gain: Some(candidate.gains.proportional_gain),
        integral_gain: Some(candidate.gains.integral_gain),
        derivative_gain: Some(candidate.gains.derivative_gain),
        rate_gain: feedforward.and_then(|seed| candidate.values.get(&seed.rate_symbol).copied()),
        acceleration_gain: feedforward
            .and_then(|seed| candidate.values.get(&seed.acceleration_symbol).copied()),
        feedforward_gain: measured
            .and_then(|seed| candidate.values.get(&seed.gain_symbol).copied()),
        lead_seconds: lead_lag.and_then(|names| candidate.values.get(&names.lead_symbol).copied()),
        lag_seconds: lead_lag.and_then(|names| candidate.values.get(&names.lag_symbol).copied()),
        ..ControllerGains::default()
    }
}

/// Whether a seed names an integral or a derivative variable. Such a law starts declared states (C2); one with neither does not.
#[must_use]
pub fn has_pid_terms(seed: &ParallelPidSeed) -> bool {
    seed.integral_symbol.is_some() || seed.derivative_symbol.is_some()
}

/// The gain variables of a loop's law, by term. The proportional variable is always named; the integral and derivative ones
/// only where the law has those terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GainSymbols<'a> {
    /// The variable of the proportional gain `Kp`.
    pub proportional: &'a str,
    /// The variable of the integral gain `Ki`, when the law has an integral term.
    pub integral: Option<&'a str>,
    /// The variable of the derivative gain `Kd`, when the law has a derivative term.
    pub derivative: Option<&'a str>,
}

/// The gain variables of a loop's law: a `Control.PID` or parallel PID record names all three, a proportional law names its
/// gain, and a program's own law names the variables of its seed. A loop without a law names none.
#[must_use]
pub fn gain_symbols(loop_definition: &LoopDefinition) -> Option<GainSymbols<'_>> {
    match loop_definition.controller.law.as_ref() {
        Some(ControllerLaw::Pid(law)) => Some(GainSymbols {
            proportional: &law.proportional_gain_symbol,
            integral: Some(&law.integral_gain_symbol),
            derivative: Some(&law.derivative_gain_symbol),
        }),
        Some(ControllerLaw::Proportional { gain_symbol }) => Some(GainSymbols {
            proportional: gain_symbol,
            integral: None,
            derivative: None,
        }),
        None => served_law(loop_definition).map(|law| GainSymbols {
            proportional: &law.seed.proportional_symbol,
            integral: law.seed.integral_symbol.as_deref(),
            derivative: law.seed.derivative_symbol.as_deref(),
        }),
    }
}

/// The gain variable of a connected P law (C13): a proportional law's gain, or a program's law whose seed names neither an
/// integral nor a derivative variable. A law with either term is not a connected P.
#[must_use]
pub fn connected_p_gain(loop_definition: &LoopDefinition) -> Option<&str> {
    match gain_symbols(loop_definition)? {
        GainSymbols {
            proportional,
            integral: None,
            derivative: None,
        } => Some(proportional),
        GainSymbols { .. } => None,
    }
}

/// The application states of a program's own law, in declaration order (C1): the states that accompany the law's own states and that
/// an Auto entry sets, such as a startup-done flag. A loop without a served law has none.
#[must_use]
pub fn application_states(loop_definition: &LoopDefinition) -> Vec<&InitializedState> {
    let Some(law) = served_law(loop_definition) else {
        return Vec::new();
    };
    law.project
        .initialization
        .states()
        .iter()
        .filter(|state| law.project.is_application_state(&state.symbol))
        .collect()
}

/// The application states of a program's own law as members of a transfer binding (`transfer_member`), in declaration order.
#[must_use]
pub fn application_members(loop_definition: &LoopDefinition) -> Vec<TransferMember> {
    application_states(loop_definition)
        .into_iter()
        .map(transfer_member)
        .collect()
}

/// The member of a binding that a state of the law names.
fn member_of<'a>(
    binding: &'a TransferBinding,
    symbol: &str,
) -> Result<&'a TransferMember, Failure> {
    binding
        .states
        .iter()
        .find(|member| member.symbol == symbol)
        .ok_or_else(|| {
            Failure::new(
                &texts::transfer_configure_binding(),
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            )
        })
}

/// The values that an Auto entry sets for the application states of a program's own P law (C1, C13 Apply step 4): each takes its
/// declared constant, which is TRUE for the startup-done flag, rounded as its member is typed.
///
/// # Errors
/// An application state's initial value is not a constant, or the binding has no member for a state.
pub fn entry_application_values(
    loop_definition: &LoopDefinition,
    binding: &TransferBinding,
) -> Result<BTreeMap<String, f64>, Failure> {
    application_states(loop_definition)
        .into_iter()
        .map(|state| {
            let value = match &state.initial {
                InitialValue::Constant(value) => *value,
                InitialValue::From(_) => {
                    return Err(Failure::new(
                        &texts::project_p_application_formula(&state.symbol),
                        Cause::ControllerCapability,
                        NextAction::OpenLoopSetup,
                    ))
                }
            };
            let member = member_of(binding, &state.symbol)?;
            Ok((state.symbol.clone(), rounded_transfer_value(member, value)?))
        })
        .collect()
}

/// The values that a Manual or reviewed-target transfer keeps for the application states of a program's own P law (C1, C13): the
/// live values the runtime reports, rounded as their members are typed. The declared-state transfer keeps live values the same way
/// when it does not align its output, and the Restore's state reads keep them (`restore_states`).
///
/// # Errors
/// An application state is not reported, or a value does not fit its member.
pub fn live_application_values(
    loop_definition: &LoopDefinition,
    binding: &TransferBinding,
    observed: &BTreeMap<String, TransferValue>,
) -> Result<BTreeMap<String, f64>, Failure> {
    application_states(loop_definition)
        .into_iter()
        .map(|state| {
            let number = match observed.get(&state.symbol) {
                Some(TransferValue::Number(value)) => *value,
                Some(TransferValue::Boolean(flag)) => f64::from(u8::from(*flag)),
                None => {
                    return Err(Failure::new(
                        &texts::declared_state_unavailable(&state.symbol),
                        Cause::Runtime,
                        NextAction::None,
                    ))
                }
            };
            let member = member_of(binding, &state.symbol)?;
            Ok((
                state.symbol.clone(),
                rounded_transfer_value(member, number)?,
            ))
        })
        .collect()
}

#[cfg(test)]
#[path = "project_law_tests.rs"]
mod tests;
