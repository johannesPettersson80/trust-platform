//! The state-feedback law as a controller adapter (specification 35, C4 and B6): the law's declared coefficients (`K`, `N` and, for LQG,
//! `L`), its declared states, and the start plan of its Auto entry. The first-output allowance of C4 is `entry_allowance`'s, and the
//! design rules are `tuning::state_controller`'s; this module reads the declared law and runs its start plan.

use std::collections::BTreeMap;

use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::LoopDefinition;
use crate::model::GeneralModel;
use crate::tuning::state_controller::{
    decode_state_controller_binding, StateControllerBinding, StateControllerFamily,
};
use crate::validation::engineer_verification_plan::EngineerRequirements;

use super::declared_initialization::declared_initialization_plan;
use super::entry_allowance::auto_entry_allowance;
use super::initial_state::ControllerInitialization;
use super::initialization_plan::StartPlan;

/// The refusal of a state law whose record does not decode against the loop's parameter record (C4).
const DOES_NOT_DECODE: &str =
    "The state-feedback law does not decode against the loop's parameter record (C0, C4).";

/// A state-feedback law of a loop (C4): its decoded coefficients and the declared states that its Auto entry starts.
#[derive(Debug, Clone, PartialEq)]
pub struct StateLaw {
    /// The decoded law: the symbols of `K`, `N` and, for LQG, `L`, with its family.
    pub binding: StateControllerBinding,
    /// The declared states of the controller; a law that declares none has an empty list.
    pub initialization: ControllerInitialization,
}

/// Decodes the state-feedback law that `loop_definition` declares, against its parameter record (C4).
///
/// # Errors
/// The loop declares no state feedback, or its record does not decode against the parameter record.
pub fn decode_state_law(
    loop_definition: &LoopDefinition,
) -> Result<StateControllerBinding, Failure> {
    loop_definition
        .controller
        .state_feedback
        .as_ref()
        .and_then(|value| {
            decode_state_controller_binding(value, loop_definition.parameter_binding.as_ref())
        })
        .ok_or_else(|| {
            Failure::new(
                DOES_NOT_DECODE,
                Cause::ControllerCapability,
                NextAction::OpenLoopSetup,
            )
        })
}

/// The state law of a loop (C4): `None` for a loop that declares no state feedback, and the decoded law otherwise. A law that declares
/// no states starts none at its Auto entry.
///
/// # Errors
/// The loop declares state feedback, and its record does not decode or its declared states are not valid.
pub fn state_law(loop_definition: &LoopDefinition) -> Result<Option<StateLaw>, Failure> {
    if loop_definition.controller.state_feedback.is_none() {
        return Ok(None);
    }
    let binding = decode_state_law(loop_definition)?;
    let initialization = match &binding.initialization {
        Some(declared) => declared.clone(),
        None => ControllerInitialization::new(Vec::new())?,
    };
    Ok(Some(StateLaw {
        binding,
        initialization,
    }))
}

/// The label that a state law shows for its family (C4): the family's own word.
#[must_use]
pub fn family_label(family: StateControllerFamily) -> &'static str {
    match family {
        StateControllerFamily::StateFeedback => "State feedback",
        StateControllerFamily::Lqr => "LQR",
        StateControllerFamily::Lqg => "LQG",
    }
}

/// The parameter symbols that a state law writes (C4): the gains `K`, then the reference gains `N`, then the observer gains `L` of an
/// LQG law, in the order the declaration gives them.
#[must_use]
pub fn coefficient_symbols(binding: &StateControllerBinding) -> Vec<String> {
    binding
        .gain_symbols
        .iter()
        .chain(&binding.reference_gain_symbols)
        .chain(binding.observer_gain_symbols.iter().flatten())
        .flatten()
        .cloned()
        .collect()
}

/// The start plan of a state law's Auto entry (C4, A6.4): the declared plan of its states, with the C4 first-output allowance of the
/// candidate's coefficients as the reviewed tolerance of the run. The declared plan takes its allowance from the task's tolerance, so the
/// computed one is passed as that tolerance; the task itself is not changed.
///
/// # Errors
/// The allowance cannot be built (see `auto_entry_allowance`), or the declared plan does not fit the loop.
pub fn initialization_plan(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    task: &EngineerRequirements,
    values: &BTreeMap<String, f64>,
    process_noise: Option<f64>,
    law: &StateLaw,
) -> Result<StartPlan, Failure> {
    let tolerance =
        auto_entry_allowance(loop_definition, model, task, values, process_noise, None)?;
    let entry_task = EngineerRequirements {
        transfer_tolerance: Some(tolerance),
        ..task.clone()
    };
    declared_initialization_plan(
        loop_definition,
        model,
        &entry_task,
        values,
        &law.initialization,
        process_noise,
    )
}

#[cfg(test)]
#[path = "state_adapter_tests.rs"]
mod tests;
