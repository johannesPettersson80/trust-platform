//! The progress sentences of the Apply workflow's operations (specification 35, B6.1, B6.3, A3.6 and A8.6; A11.1): what an Apply shows
//! while it commissions and records the response, and what a reattachment shows. The refusals and the results are trust-control's
//! (`commissioning::apply_texts`, `commissioning::texts`, `commissioning::application_texts`).

use trust_control::tuning::texts::engineering;

/// The progress of a confirmed Apply once the starting point is reached (`confirmReviewedOperation` of
/// `engineerActions/application.ts`, B6.1).
pub const COMMISSIONING: &str =
    "Commissioning the reviewed controller and recording its response. Cancel remains available.";

/// The progress of a reattachment to an Apply observation (`reattachApplyObservation` of `engineerActions/recovery.ts`, A8.6).
pub const REATTACHING: &str = "Reattaching to the response the runtime is recording.";

/// The failure of a run whose ending the rules did not give: only a panel that detached its observation ends so, and the engine has
/// no panel, so this is a defect of the engine (A11.3).
pub const NO_ENDING: &str = "The Apply ended without an outcome to report.";

/// The failure of a Validate section that cannot be written as JSON for its identity: a defect of the engine (A11.3), with the error
/// in words.
#[must_use]
pub fn validation_unreadable(error: &str) -> String {
    format!("The study's validation could not be read for the review identity: {error}.")
}

/// The progress of a recorded response (`followAppliedResponse` of `applyObservation.ts`; B6.3, A3.6): the process time elapsed of
/// the reviewed duration, and the process time remaining, each in s.
#[must_use]
pub fn recording_progress(elapsed_ms: f64, duration_ms: f64) -> String {
    let remaining_ms = (duration_ms - elapsed_ms).max(0.0);
    format!(
        "Recording the response: {} s of {} s elapsed, {} s of process time remaining.",
        seconds(elapsed_ms),
        seconds(duration_ms),
        seconds(remaining_ms)
    )
}

/// A duration in ms as the sentences show it, in s (`seconds` of `applyObservation.ts`).
fn seconds(milliseconds: f64) -> String {
    engineering(milliseconds / 1000.0, None)
}
