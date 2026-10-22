//! Tests of the currency of the analysis answers the page holds (specification 35, B9.9 and A8.4).

use super::{reads_study_model, stale_answers, stale_shown};
use crate::messages::{AnalysisInputs, AnalysisPage, AnalysisTool, HeldAnswer};

fn held(tool: AnalysisTool, model: Option<&str>) -> HeldAnswer {
    HeldAnswer {
        tool,
        model: model.map(str::to_owned),
    }
}

#[test]
fn an_analysis_answer_is_not_current_after_the_model_changes() {
    // B9.9, A8.4: a pole-zero map calculated for model `fit-1` describes that model. When the study's model is `fit-2` the answer is
    // stale and the page drops it; keeping it would show the poles of a plant that is no longer the study's.
    let answers = [
        held(AnalysisTool::PoleZero, Some("fit-1")),
        held(AnalysisTool::Frequency, Some("fit-2")),
    ];
    assert_eq!(
        stale_answers(&answers, Some("fit-2")),
        vec![AnalysisTool::PoleZero],
        "only the answer calculated for another model is stale"
    );
    assert_eq!(
        stale_answers(&answers, Some("fit-1")),
        vec![AnalysisTool::Frequency],
        "the model that changed back makes the other answer stale"
    );
}

#[test]
fn an_answer_for_the_models_revision_stays_current() {
    // Over-asking must cost no answer: when the page asks again after an event that did not change the model, every answer survives.
    let answers = [
        held(AnalysisTool::PoleZero, Some("fit-1")),
        held(AnalysisTool::StateStructure, Some("fit-1")),
        held(AnalysisTool::InitialState, Some("fit-1")),
    ];
    assert_eq!(
        stale_answers(&answers, Some("fit-1")),
        Vec::<AnalysisTool>::new()
    );
}

#[test]
fn a_study_without_a_model_leaves_no_model_answer_current() {
    // A8.4: a reset study holds no model, so an answer calculated for one is stale, and so is an answer that names no model, which cannot
    // be known to be current.
    let answers = [
        held(AnalysisTool::PoleZero, Some("fit-1")),
        held(AnalysisTool::LeadLag, None),
    ];
    assert_eq!(
        stale_answers(&answers, None),
        vec![AnalysisTool::PoleZero],
        "an answer calculated for a model is stale when the study holds none"
    );
    assert_eq!(
        stale_answers(&[held(AnalysisTool::LeadLag, None)], Some("fit-1")),
        vec![AnalysisTool::LeadLag],
        "an answer that names no model is not known to be current for this one"
    );
}

#[test]
fn the_declared_and_the_measured_answers_do_not_depend_on_the_model() {
    // B9.9, B10.2: a root locus and a Routh reading come from the loop's declared plant, and the measured responses from a recording, so a
    // new model leaves them as they are. Clearing them with the model would throw away a calculation that is still right.
    for tool in [
        AnalysisTool::RootLocus,
        AnalysisTool::Routh,
        AnalysisTool::MeasuredFrequency,
        AnalysisTool::MeasuredPaths,
    ] {
        assert!(!reads_study_model(tool), "{tool:?}");
        assert!(
            stale_answers(&[held(tool, Some("fit-1"))], Some("fit-2")).is_empty(),
            "{tool:?}"
        );
        assert!(
            stale_answers(&[held(tool, None)], None).is_empty(),
            "{tool:?}"
        );
    }
    for tool in [
        AnalysisTool::PoleZero,
        AnalysisTool::Frequency,
        AnalysisTool::Feedback,
        AnalysisTool::LeadLag,
        AnalysisTool::InitialState,
        AnalysisTool::StateStructure,
        AnalysisTool::RelativeGain,
    ] {
        assert!(reads_study_model(tool), "{tool:?}");
    }
}

fn shown(tool: AnalysisTool, model: Option<&str>) -> AnalysisPage {
    AnalysisPage {
        tool,
        inputs: AnalysisInputs::default(),
        model_revision: model.map(str::to_owned),
    }
}

#[test]
fn the_recorded_analysis_is_stale_only_when_it_was_calculated_for_another_model() {
    // B9.9, A8.4: the engine records the analysis it answers with the revision of the model it used, and an inputs answer names it when
    // the study's model is another one. Worked by hand: a pole-zero map calculated for `fit-1` describes that plant.
    assert_eq!(
        stale_shown(
            Some(&shown(AnalysisTool::PoleZero, Some("fit-1"))),
            Some("fit-2")
        ),
        vec![AnalysisTool::PoleZero],
        "the study's model is another one"
    );
    assert_eq!(
        stale_shown(Some(&shown(AnalysisTool::PoleZero, Some("fit-1"))), None),
        vec![AnalysisTool::PoleZero],
        "a reset study holds no model"
    );
    assert!(
        stale_shown(
            Some(&shown(AnalysisTool::PoleZero, Some("fit-1"))),
            Some("fit-1")
        )
        .is_empty(),
        "the same model leaves the analysis current"
    );
    // An analysis that was only selected names no model: nothing was calculated, so nothing is out of date, and none recorded says none.
    let none = Vec::<AnalysisTool>::new();
    assert_eq!(
        stale_shown(Some(&shown(AnalysisTool::PoleZero, None)), Some("fit-2")),
        none
    );
    assert_eq!(stale_shown(None, Some("fit-2")), none);
    // A declared or measured analysis does not read the model, so a new model leaves it as it is.
    assert_eq!(
        stale_shown(
            Some(&shown(AnalysisTool::RootLocus, Some("fit-1"))),
            Some("fit-2")
        ),
        none
    );
}
