//! The sentences of the Apply workflow's own refusals and of the result it reports after reopening (specification 35, B6.1 to B6.3,
//! B6.7, A3.2, A6.7, A8.5, A8.6 and A11.1; annex E4.6 and E11.5). Each refusal is a whole failure with its cause and typed next action
//! (A11.2), so that the run and its reattachment refuse in the same words; each result sentence names the values involved (A11.1). The
//! sentences of the commissioning rules themselves are in `texts`, and those of the review and its confirmation in `application_texts`.

use crate::failure::{Cause, Failure, NextAction};

/// Apply is not available because the study holds no validation of its candidate that is current for its model, its candidate and its
/// requirements and that passed (B5.4, A8.4). B6.7 names the next action: "Candidate or validation no longer current: Validate".
#[must_use]
pub fn validation_not_current() -> Failure {
    Failure::new(
        "The candidate or its validation is no longer current, so Apply is not available. Validate the controller again before applying.",
        Cause::Settings,
        NextAction::Validate,
    )
}

/// The loop's newest Apply already applied the study's validation (B7; the Validate page specification: "A new candidate must pass
/// validation before this action can offer Apply for that candidate"): whether its settings stand, were restored or were retired, Apply
/// waits for a validation that passes after it.
#[must_use]
pub fn validation_already_applied() -> Failure {
    Failure::new(
        "Apply already applied this validation, so Apply is not available for it again. Validate the controller again before applying.",
        Cause::Settings,
        NextAction::Validate,
    )
}

/// An earlier Apply of the loop is unresolved on the running program (A8.5, B6.4, E11.7): its journaled checkpoint `checkpoint` stopped at
/// the stage `stage`, so a new Apply would write over operations whose outcome the journal still owns, and new work cannot bypass their
/// owners. Restore previous settings resolves it first.
#[must_use]
pub fn checkpoint_unresolved(checkpoint: &str, stage: &str) -> Failure {
    Failure::new(
        &format!(
            "An earlier Apply of this loop is unresolved: its checkpoint {checkpoint} stopped at the {stage} stage. Restore previous settings first, so that its writes are resolved before a new Apply."
        ),
        Cause::Runtime,
        NextAction::Restore,
    )
}

/// The runtime reports no clock in its transfer status, so a wait for the starting point cannot measure process time (A3.2;
/// `runtimeClockNs` of `runtimeDuration.ts`).
#[must_use]
pub fn runtime_clock_missing() -> Failure {
    Failure::new(
        "The runtime does not report its clock in the transfer status.",
        Cause::Runtime,
        NextAction::None,
    )
}

/// The runtime ended the recording of an Apply observation (annex E4.6, state `failed`: a scan reset or a runtime boundary, A6.7), with
/// the runtime's reason when it gives one (`followAppliedResponse` of `applyObservation.ts`). The runtime's new identity is resolved
/// before any further live action (A6.7, E11.5).
#[must_use]
pub fn recording_ended(reason: Option<&str>) -> Failure {
    let reason = reason
        .filter(|reason| !reason.is_empty())
        .map_or_else(String::new, |reason| format!(": {reason}"));
    Failure::new(
        &format!(
            "The runtime ended the recording{reason}. Connect the runtime again before any further live action."
        ),
        Cause::Runtime,
        NextAction::ConnectRuntime,
    )
}

/// A reattachment that finds no observation in the loop's journal (A8.5, A8.6; `resumeApplyObservation` of `applyObservation.ts`): no
/// unresolved checkpoint holds a recording that the runtime may still record.
#[must_use]
pub fn no_observation_to_resume() -> Failure {
    Failure::new(
        "The commissioning journal holds no applied response to resume.",
        Cause::Unclassified,
        NextAction::None,
    )
}

/// The reason of an Apply observation that passed after the engine reattached it (A8.6, B6.3; `settlePass` of
/// `engineerActions/recovery.ts`): the recorded target error, to five fraction digits, in the process unit.
#[must_use]
pub fn reattach_passed(maximum_error: f64, process_unit: &str) -> String {
    let error = crate::tuning::texts::engineering(maximum_error, Some(5));
    let unit = crate::tuning::texts::unit_suffix(process_unit);
    format!("Auto commissioning passed after reopening: recorded target error {error}{unit}.")
}

#[cfg(test)]
#[path = "apply_texts_tests.rs"]
mod tests;
