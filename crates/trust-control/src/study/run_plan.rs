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
    let tune_due = identify || !evidence.has_candidate;
    let validate_due = tune_due || !evidence.validation_has_result;
    // Tune, Validate and Apply do not apply to a loop of the C12 kinds, so a run of such a loop ends with Identify (C12).
    let controller_stages = evidence.controller_stages_apply;
    RunPlan {
        identify,
        tune: controller_stages && tune_due && through != RunThrough::Identify,
        validate: controller_stages && validate_due && through == RunThrough::Validate,
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

/// The sentence that asks for the manual method's settings (B8). A run stops on them before any stage runs, so no stage's failure
/// carries it.
pub const MANUAL_SETTINGS_NEEDED: &str = "Enter the manual settings to tune the controller.";

/// An input the engineer must give before a run can go on (B8), with the words that ask for it. The page shows `message` and writes
/// no sentence of its own for any input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeededInput {
    /// Which input the run waits for.
    pub needs: RunNeeds,
    /// The sentence that asks for it: the sentence of the refusal that stopped the run, so that the engine's own wording names what is
    /// missing (a limit, a requirement), or the manual settings' sentence.
    pub message: String,
    /// The form field the sentence concerns, when it concerns one: the field of the refusal (`durationSeconds` for a missing duration).
    pub field: Option<String>,
}

impl NeededInput {
    /// The input that a stage's failure asks for, with the failure's sentence and field; `None` for a failure that asks for none
    /// ([`needs_input`]).
    #[must_use]
    pub fn of_failure(failure: &Failure) -> Option<Self> {
        needs_input(failure).map(|needs| Self {
            needs,
            message: failure.message.clone(),
            field: failure.field.clone(),
        })
    }

    /// The manual method's settings, which a run asks for before any stage runs ([`MANUAL_SETTINGS_NEEDED`]).
    #[must_use]
    pub fn manual_settings() -> Self {
        Self {
            needs: RunNeeds::ManualSettings,
            message: MANUAL_SETTINGS_NEEDED.to_owned(),
            field: None,
        }
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
