//! The open path's reasons for a loop file that is missing or refused (specification 35, B1.2; `controlStudio/projectLoop.ts`). The
//! engine reads the loop files in their order (`trust-control-studio/src/loops.rs`); the sentences are decided here, in the
//! `TypeScript`'s wording. The id check of a save is [`super::save::check_setup_targets`].

use super::save::validation_reason_code;
use super::texts::{discovery, project};
use crate::loop_definition::LoopRejection;

/// Why a loop file of the open path is refused (B1.2, `controlStudioRequestForFile` and `controlStudioRequestsForFolder`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InvalidLoop<'a> {
    /// The file is not JSON, or it cannot be read.
    NotValidJson,
    /// The file is JSON that is not a complete loop definition of this format.
    NotDefinition,
    /// The file belongs to another runtime than the selected one.
    RuntimeMismatch {
        /// The runtime the loop declares.
        loop_runtime: &'a str,
        /// The runtime that is selected.
        selected: &'a str,
    },
    /// The file fails a rule of the loop's own signals.
    Validation(&'a LoopRejection),
    /// Another file of the project declares the same loop id.
    Duplicate {
        /// The id.
        id: &'a str,
    },
}

/// The reason when the project declares no loop definition, so there is nothing to bind (B1.2).
#[must_use]
pub fn missing_loop_definition_reason() -> &'static str {
    project::missing_loop_definition()
}

/// The sentence for one refused loop file of the open path. `file_name` is the file's own name; the duplicate sentence names the id
/// instead, as the `TypeScript` does.
#[must_use]
pub fn invalid_loop_reason(file_name: &str, problem: InvalidLoop<'_>) -> String {
    match problem {
        InvalidLoop::NotValidJson => discovery::not_valid_json(file_name),
        InvalidLoop::NotDefinition => discovery::not_loop_definition(file_name),
        InvalidLoop::RuntimeMismatch {
            loop_runtime,
            selected,
        } => discovery::runtime_mismatch(loop_runtime, selected),
        InvalidLoop::Validation(rejection) => validation_reason(file_name, rejection),
        InvalidLoop::Duplicate { id } => project::duplicate_in_project(id),
    }
}

/// The signals a rejection names, as the sentence lists them. A rule that names no signal is a rule of the loop configuration.
fn signals_named(rejection: &LoopRejection) -> String {
    match rejection {
        LoopRejection::IncompatibleUnits => "setpoint and processVariable".to_string(),
        LoopRejection::InvalidSignalDirection(signal)
        | LoopRejection::IncompatibleSignalType(signal)
        | LoopRejection::DuplicateSignalId(signal)
        | LoopRejection::DuplicateSignalRole(signal) => signal.clone(),
        LoopRejection::AmbiguousSignalAddress(first, second) => format!("{first} and {second}"),
        _ => "loop configuration".to_string(),
    }
}

/// The sentence for a loop file that fails a rule of its signals: units that do not match say so in their own words.
fn validation_reason(file_name: &str, rejection: &LoopRejection) -> String {
    let fields = signals_named(rejection);
    match rejection {
        LoopRejection::IncompatibleUnits => project::units_mismatch(file_name, &fields),
        _ => project::validation_failed(file_name, &fields, validation_reason_code(rejection)),
    }
}

#[cfg(test)]
#[path = "project_loop_tests.rs"]
mod tests;
