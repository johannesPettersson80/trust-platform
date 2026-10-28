//! The Tune section of a finished finite search (specification 35, C5, B4.1 and A8.1): the search candidate, kept unapplied with the
//! requirements it was designed to and the model revision it was designed on. The search is Automatic (no method), so the section names
//! no formula, and it keeps no `Control.PID` or program-law candidate: exactly one candidate of the law is present.

use crate::failure::Failure;
use crate::stages::StageEvidence;
use crate::study::tune::TuneSection;
use crate::study::Study;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::search::SearchCandidate;
use crate::tuning::settings_pins::ControllerSettingsPins;

/// The finite-search candidate that `controlStudio/open` holds for a study (C5, A8.6): the Tune section's search candidate, given only when
/// the stage evidence says the study has a current candidate (A8.4). This is the rule `identify_result::study_candidate` applies to the
/// Control.PID candidate, so a reopened study shows the same candidate state for every law.
///
/// # Errors
/// The study's Tune section cannot be read.
pub fn study_search_candidate(
    study: &Study,
    evidence: StageEvidence,
) -> Result<Option<SearchCandidate>, Failure> {
    if !evidence.has_candidate {
        return Ok(None);
    }
    Ok(study
        .section::<TuneSection>()?
        .and_then(|section| section.search_candidate))
}

/// The Tune section that keeps a finished search's candidate (C5). The `reason` says how the candidate was chosen (B4.4).
#[must_use]
pub fn search_section(
    model_revision: String,
    requirements: TuningRequirements,
    candidate: SearchCandidate,
    pins: Option<ControllerSettingsPins>,
) -> TuneSection {
    let reason = search_reason(&candidate);
    TuneSection {
        method: None,
        model_revision,
        requirements,
        candidate: None,
        project_candidate: None,
        state_candidate: None,
        search_candidate: Some(candidate),
        on_off_candidate: None,
        group_candidate: None,
        reason,
        closed_loop_time_seconds: None,
        attempts: Vec::new(),
        pins,
    }
}

/// The sentence that says how the search chose its candidate (B4.4): the trials it attempted, and the rule that chose the selected one.
#[must_use]
pub fn search_reason(candidate: &SearchCandidate) -> String {
    let attempted = candidate
        .trials
        .iter()
        .filter(|trial| trial.attempted)
        .count();
    format!(
        "The finite search attempted {attempted} of {} trials; the selected trial met every requirement and had the smallest error after the settling time among them (C5).",
        candidate.trials.len()
    )
}

#[cfg(test)]
#[path = "search_record_tests.rs"]
mod tests;
