//! The controller structures a loop declares, checked when the loop opens as the page's loop validator gates them (`loopDefinition.ts`,
//! the gates of a state-feedback law, an ON/OFF controller and a program's own law; specification 35, C2, C3 and C4). A state-feedback
//! law and a program's own law need the project design and no controller law. A state-feedback law also decodes against the loop's
//! parameter record (`decodeStateControllerBinding`, the engine's decoder in `tuning::state_controller`). A program's own law takes
//! neither a state-feedback law nor an ON/OFF controller. The ON/OFF controller's gate is checked with its own decode (`on_off.rs`).
//! The declared blocks of states of these structures and of a Control.PID hold at most 32 states each, and a decoupler's matrix is
//! checked as the page checks it (`controllerInitialState.ts`, C7).

use crate::tuning::state_controller::decode_state_controller_binding;

use super::{
    Design, InitialSource, InitialValue, InitializedState, LoopDefinition, PidInitialization,
};

/// The most states one declared block holds (`decodeControllerInitialization`, `value.states.length > 32`).
const MAXIMUM_BLOCK_STATES: usize = 32;

/// The most rows a decoupler's matrix has: the runtime's decoupled-output formula takes at most eight (specification 35, C7).
const MAXIMUM_DECOUPLED_ROWS: usize = 8;

/// Checks the controller structures the loop declares against the gates the page applies when the loop opens.
///
/// # Errors
/// A state-feedback law or a program's own law is not in the project's design with the structures it excludes, or a state-feedback law
/// does not decode against the loop's parameter record, or a declared block of states is too large or holds a malformed decoupler.
pub(super) fn check(loop_definition: &LoopDefinition) -> Result<(), String> {
    check_structures(loop_definition)?;
    check_blocks(loop_definition)
}

/// The gates of the controller structures (`loopDefinition.ts`, lines 213 to 222).
fn check_structures(loop_definition: &LoopDefinition) -> Result<(), String> {
    let controller = &loop_definition.controller;
    let project_designed = controller.design == Some(Design::Project);
    if let Some(state_feedback) = &controller.state_feedback {
        // A second controller structure beside it is the gate's refusal, with the gate's sentence (C2,
        // `LoopDefinition::require_one_controller_structure`).
        if !project_designed {
            return Err("a state-feedback law needs the project design".into());
        }
        if decode_state_controller_binding(
            state_feedback,
            loop_definition.parameter_binding.as_ref(),
        )
        .is_none()
        {
            return Err(
                "the state-feedback law does not decode against the loop's parameter record".into(),
            );
        }
    }
    if controller.project.is_some() && !project_designed {
        return Err("a program's own law needs the project design".into());
    }
    Ok(())
}

/// The declared blocks of states, each of at most 32 states as the page's decoder reads them: a switching controller's, a program's
/// (its own states and its application states together), and a Control.PID's (its seven states and its application states together,
/// `libraryPidInitialization`). A decoupler's matrix in one of them is square, of one to eight rows, and its index names a row.
///
/// # Errors
/// A block holds more than 32 states, or a decoupled output's matrix or index does not fit.
fn check_blocks(loop_definition: &LoopDefinition) -> Result<(), String> {
    let controller = &loop_definition.controller;
    let mut blocks: Vec<&[InitializedState]> = Vec::new();
    if let Some(on_off) = &controller.on_off {
        blocks.push(on_off.initialization.states());
    }
    if let Some(project) = &controller.project {
        blocks.push(project.initialization.states());
    }
    let control_pid: Vec<InitializedState>;
    if let Some(PidInitialization::ControlPid {
        states,
        application_states,
        ..
    }) = &controller.pid_initialization
    {
        control_pid = states
            .iter()
            .chain(application_states.iter().flatten())
            .cloned()
            .collect();
        blocks.push(&control_pid);
    }
    for states in blocks {
        if states.len() > MAXIMUM_BLOCK_STATES {
            return Err(format!(
                "a declared block holds {} states, and at most {MAXIMUM_BLOCK_STATES} are allowed",
                states.len()
            ));
        }
        for state in states {
            if let InitialValue::From(InitialSource::DecoupledOutput {
                index,
                matrix_symbols,
            }) = &state.initial
            {
                check_decoupled(*index, matrix_symbols)?;
            }
        }
    }
    Ok(())
}

/// A decoupler's matrix is square, of one to eight rows, with a symbol in each entry, and its index names one of its rows.
fn check_decoupled(index: usize, matrix: &[Vec<String>]) -> Result<(), String> {
    let rows = matrix.len();
    if rows == 0
        || rows > MAXIMUM_DECOUPLED_ROWS
        || index >= rows
        || matrix
            .iter()
            .any(|row| row.len() != rows || row.iter().any(String::is_empty))
    {
        return Err(
            "a decoupled output's matrix is square, of one to eight rows, with a symbol in each entry, and its index names a row"
                .into(),
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "structure_rules_tests.rs"]
mod tests;
