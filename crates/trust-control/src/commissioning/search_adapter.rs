//! The finite search of a program's law as a controller adapter (specification 35, C5 and A6.4): the search law's declared states, which its
//! Auto entry starts as a program's own PID's do, and the reviewed Auto-entry tolerance that the entry needs, since a search has no
//! first-output formula. The search itself is `tuning::search`; its trials are isolated runs of the study's model (C5).

use std::collections::BTreeMap;

use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{ControllerProject, LoopDefinition};
use crate::model::GeneralModel;
use crate::validation::engineer_verification_plan::EngineerRequirements;

use super::declared_initialization::declared_initialization_plan;
use super::entry_allowance::REVIEWED_TOLERANCE_REQUIRED;
use super::initialization_plan::StartPlan;

/// The search law of a loop (C5): its `controller.project` block, when that block is the search-law form (tuning axes, no seed).
#[must_use]
pub fn search_law(loop_definition: &LoopDefinition) -> Option<&ControllerProject> {
    loop_definition
        .controller
        .project
        .as_ref()
        .filter(|project| project.is_search_law())
}

/// The start plan of a search law's Auto entry (C5, A6.4): the declared plan of its states, which needs the reviewed Auto-entry tolerance.
/// A search law has no first-output formula, so the entry is refused without a reviewed tolerance, and the reviewed value is the allowance.
///
/// # Errors
/// The task has no reviewed Auto-entry tolerance; or the declared plan does not fit the loop.
pub fn initialization_plan(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    task: &EngineerRequirements,
    values: &BTreeMap<String, f64>,
    process_noise: Option<f64>,
    project: &ControllerProject,
) -> Result<StartPlan, Failure> {
    if task.transfer_tolerance.is_none() {
        return Err(Failure::new(
            REVIEWED_TOLERANCE_REQUIRED,
            Cause::Settings,
            NextAction::OpenRequirements,
        ));
    }
    declared_initialization_plan(
        loop_definition,
        model,
        task,
        values,
        &project.initialization,
        process_noise,
    )
}

#[cfg(test)]
#[path = "search_adapter_tests.rs"]
mod tests;
