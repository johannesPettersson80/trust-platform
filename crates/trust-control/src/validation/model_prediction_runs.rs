//! The pure rules of a model comparison (`modelPredictionRuns.ts`; specification 35, §9 and B5.3). A comparison runs the
//! program's actual Structured Text on the selected model, once with the current settings and once with the candidate.
//! Two rules are kept here. The candidate is the first case of its campaign, the nominal `target-response` run. The
//! baseline and the candidate predictions must come from one project revision, or they do not compare.
//!
//! Left out (orchestration): building the baseline plan and the candidate's campaign from the loop, the verifier and its
//! ownership, the attempt records, and the cancellation. Those belong to the verification client (V3.2) and to the caller.

use crate::failure::{Cause, Failure, NextAction};
use crate::identification::connected_record::RuntimeVerificationResult;
use crate::validation::texts;

/// The candidate of a comparison: the first case of its campaign (`engineerVerificationCampaign(...)[0]`), which is the
/// nominal `target-response` run. `None` when the campaign holds no case.
#[must_use]
pub fn candidate_case<T>(cases: &[T]) -> Option<&T> {
    cases.first()
}

/// The baseline and the candidate predictions must carry one project revision (`compareOnModel`). A project that changed
/// while they were predicted gives runs that do not compare, so Validate again.
///
/// # Errors
/// The two predictions carry different project revisions.
pub fn same_project_revision<E, F>(
    baseline: &RuntimeVerificationResult<E>,
    candidate: &RuntimeVerificationResult<F>,
) -> Result<(), Failure> {
    if baseline.project_revision == candidate.project_revision {
        Ok(())
    } else {
        Err(Failure::new(
            &texts::project_changed_during_prediction(),
            Cause::Runtime,
            NextAction::Validate,
        ))
    }
}

#[cfg(test)]
#[path = "model_prediction_runs_tests.rs"]
mod tests;
