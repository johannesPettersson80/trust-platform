//! Whether an analysis answer is still current (specification 35, B9.9 and A8.4). The page holds the answers it was given; the engine
//! says which of them no longer belong to the model it analyses now, so the page keeps no rule of its own for when an answer is out of
//! date and drops exactly the answers the engine lists.

use crate::messages::{AnalysisPage, AnalysisTool, HeldAnswer};

/// Whether the tool's answer is calculated from the study's model (B9.9): every tool but the declared analyses (`rootLocus` and `routh`),
/// which read the loop's declared plant, and the measured ones (`measuredFrequency` and `measuredPaths`), which read a recording. Only
/// these answers go out of date when the model changes.
#[must_use]
pub fn reads_study_model(tool: AnalysisTool) -> bool {
    !matches!(
        tool,
        AnalysisTool::RootLocus
            | AnalysisTool::Routh
            | AnalysisTool::MeasuredFrequency
            | AnalysisTool::MeasuredPaths
    )
}

/// The tools of the answers in `held` that are no longer current for the model the engine analyses now, `model` being its revision
/// (`None` when the study holds no model). An answer of a tool that reads the model is current only while it was calculated for that
/// revision; an answer that names no revision cannot be known to be current. An answer of a tool that does not read the model stays
/// current whatever the model is. The tools come in the order of `held`.
#[must_use]
pub fn stale_answers(held: &[HeldAnswer], model: Option<&str>) -> Vec<AnalysisTool> {
    held.iter()
        .filter(|answer| reads_study_model(answer.tool) && answer.model.as_deref() != model)
        .map(|answer| answer.tool)
        .collect()
}

/// The tools whose answer the study records (`study.analysis`) that are no longer current for `model`, the revision of the study's
/// current model now (`None` when it holds none): the analysis the engine recorded with the revision it calculated for, when that tool
/// reads the model and the model has changed since (B9.9, A8.4). An analysis that is only selected names no revision and is not stale:
/// nothing was calculated. This is the list an inputs answer carries as `stale`.
#[must_use]
pub fn stale_shown(shown: Option<&AnalysisPage>, model: Option<&str>) -> Vec<AnalysisTool> {
    let Some(shown) = shown else {
        return Vec::new();
    };
    let Some(calculated_for) = shown.model_revision.as_deref() else {
        return Vec::new();
    };
    let answer = HeldAnswer {
        tool: shown.tool,
        model: Some(calculated_for.to_owned()),
    };
    stale_answers(&[answer], model)
}

#[cfg(test)]
#[path = "currency_tests.rs"]
mod tests;
