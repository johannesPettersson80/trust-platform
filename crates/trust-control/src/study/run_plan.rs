//! The automatic progression of Control Studio (specification 35, B8 and §0.1a `controlStudio/run`): which stages a run executes,
//! and which input a stage that failed on it asks the engineer for (A11.2).
//!
//! A run starts from the earliest stage whose result is missing or no longer current (A8.4) and goes on through every later
//! stage up to the one it names. It never applies: a successful Validate ends the run at Apply (B8).

#![allow(clippy::module_name_repetitions)] // `RunPlan` names the module's subject.

use crate::failure::{Cause, Failure, NextAction};
use crate::messages::{RunNeeds, RunThrough};
use crate::stages::StageEvidence;

/// The stages a run executes, in their order (B8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunPlan {
    /// Identify runs its live campaign.
    pub identify: bool,
    /// Tune designs the candidate.
    pub tune: bool,
    /// Validate runs the model check and the actual-ST validation.
    pub validate: bool,
}

/// The stages a run executes, up to and including `through` (A8.4, B8). A stage runs when its result is missing or no longer
/// current, and so does every later stage, because each depends on the one before it. A result that is current is reused, and
/// a stage beyond `through` does not run.
#[must_use]
pub fn run_plan(evidence: &StageEvidence, through: RunThrough) -> RunPlan {
    let identify = !evidence.has_model;
    let tune = identify || !evidence.has_candidate;
    let validate = tune || !evidence.validation_has_result;
    RunPlan {
        identify,
        tune: tune && through != RunThrough::Identify,
        validate: validate && through == RunThrough::Validate,
    }
}

/// The evidence a run plans its stages from (A8.4). Identify plant identifies again (B8): the new model replaces the old one, and
/// the candidates and validations based on the old model are no longer current, so `run_plan` plans every stage after a missing
/// model. Without that request a current model is reused.
#[must_use]
pub fn planned_evidence(evidence: StageEvidence, identify_again: bool) -> StageEvidence {
    if identify_again {
        StageEvidence {
            has_model: false,
            ..evidence
        }
    } else {
        evidence
    }
}

/// The input the engineer must give for a stage that failed on it (B8): the limits, or a requirement that Automatic cannot fill
/// or that the engineer must correct. The failure's own next action says which (A11.2). Every other failure stops the run as a
/// failure, and this returns `None` for it.
#[must_use]
pub fn needs_input(failure: &Failure) -> Option<RunNeeds> {
    match (failure.cause, failure.next_action) {
        (Cause::Settings, NextAction::OpenLimits) => Some(RunNeeds::Limits),
        (Cause::Settings, NextAction::OpenRequirements) => Some(RunNeeds::Requirements),
        _ => None,
    }
}

#[cfg(test)]
#[path = "run_plan_tests.rs"]
mod tests;
