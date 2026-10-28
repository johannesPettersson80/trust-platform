//! The measurement of a library PID (specification 35, C1 "The measurement" and §17.1; specification 46 §13.2): the PID's own
//! `<instance>.StateValue.ProcessValue`, which its last Update received, and the feedback target that a simulated plant
//! writes it to. Ported from `pidMeasurement.ts`. A PID group's members (C7) are parked and not ported.

use std::collections::BTreeSet;

use super::initial_state::ControllerInitialization;
use crate::loop_definition::{AuxiliaryRole, InitializedState, LoopDefinition};

/// The library PID's own measurement: `<instance>.StateValue.ProcessValue`, what its last Update received, whatever the
/// program computed it from (`pidOwnMeasurement`).
#[must_use]
pub fn pid_own_measurement(instance: &str) -> String {
    format!("{instance}.StateValue.ProcessValue")
}

/// The library PID whose own measurement `symbol` is, for a loop that controls a `Control.PID` (`measuredPid`). The symbols
/// are compared without regard to case.
#[must_use]
pub fn measured_pid(loop_definition: &LoopDefinition, symbol: &str) -> Option<String> {
    if loop_definition.controller.type_name != "Control.PID" {
        return None;
    }
    let instance = &loop_definition.controller.instance;
    (pid_own_measurement(instance).to_uppercase() == symbol.to_uppercase())
        .then(|| instance.clone())
}

/// Where a simulated plant writes a measurement (`plantFeedbackTarget`, specification 46 §13.2): its address when the signal
/// has one; `pid:<instance>` for a library PID's own measurement, which the model gives to that PID as its Update receives;
/// otherwise the program variable itself.
#[must_use]
pub fn plant_feedback_target(
    loop_definition: &LoopDefinition,
    symbol: &str,
    address: Option<&str>,
) -> String {
    if let Some(address) = address {
        return address.to_owned();
    }
    measured_pid(loop_definition, symbol)
        .map_or_else(|| symbol.to_owned(), |instance| format!("pid:{instance}"))
}

/// The declared states that a transfer initializes (`initializedStates`). The loop's measurement, and any further
/// measurement with the PV role, is read at the task boundary like every measurement and is never written, so it is left
/// out. A loop that measures another signal initializes all its states.
#[must_use]
pub fn initialized_states<'a>(
    loop_definition: &LoopDefinition,
    definition: &'a ControllerInitialization,
) -> Vec<&'a InitializedState> {
    let measured: BTreeSet<String> = std::iter::once(
        loop_definition
            .signals
            .process_variable
            .symbol
            .to_uppercase(),
    )
    .chain(
        loop_definition
            .signals
            .additional
            .iter()
            .flatten()
            .filter(|signal| signal.role == AuxiliaryRole::ProcessVariable)
            .map(|signal| signal.symbol.to_uppercase()),
    )
    .collect();
    definition
        .states()
        .iter()
        .filter(|state| !measured.contains(&state.symbol.to_uppercase()))
        .collect()
}

#[cfg(test)]
#[path = "pid_measurement_tests.rs"]
mod tests;
