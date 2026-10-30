use std::collections::BTreeMap;

use super::*;
use crate::loop_definition::LoopDefinition;
use crate::model::decode_general_model;
use crate::stages::StageEvidence;
use crate::study::current_model::{CurrentModel, CurrentModelSource};
use crate::study::tune::stage_evidence;
use crate::study::StudySection;
use crate::tuning::candidate::LawCandidate;
use crate::tuning::search::{answer_of, SearchBounds, SearchTrial};

/// Lesson 89's loop and its supplied model, the loop the search tunes on the selected model.
const LESSON_89_LOOP: &str = include_str!(
    "../../../../../examples/control/07-classical-stability-and-frequency-design/89-mechanical-resonance/control-loop.json"
);
const LESSON_89_MODEL: &str = include_str!(
    "../../../../../examples/control/07-classical-stability-and-frequency-design/89-mechanical-resonance/flexible-axis-model.json"
);

fn requirements() -> TuningRequirements {
    TuningRequirements {
        channel_requirements: None,
        reference_change: 1.0,
        maximum_error: 0.02,
        settling_band: 0.01,
        settling_time_seconds: 4.0,
        duration_seconds: 8.0,
        constant_input_disturbance: None,
        measurement_noise_band: None,
        gain_uncertainty_fraction: None,
        transfer_tolerance: Some(1.5),
        maximum_overshoot_percent: None,
        tuning_method: None,
        tuning_objective: None,
        automatic_criteria: None,
    }
}

fn candidate() -> SearchCandidate {
    let trial = |quality: f64, bounds: SearchBounds, score: f64| SearchTrial {
        parameters: BTreeMap::from([("P1.NotchQuality".to_owned(), quality)]),
        attempted: true,
        bounds: Some(bounds),
        score: Some(score),
        reason: None,
    };
    let trials = vec![
        trial(1.0, SearchBounds::NotMet, 0.1),
        trial(3.0, SearchBounds::Met, 0.4),
        SearchTrial::planned(BTreeMap::from([("P1.NotchQuality".to_owned(), 5.0)]), None),
    ];
    SearchCandidate {
        parameters: BTreeMap::from([("P1.NotchQuality".to_owned(), 3.0)]),
        selected: 1,
        trials,
        score_unit: "rad²·s".to_owned(),
    }
}

#[test]
fn the_search_section_keeps_its_candidate_and_checks() {
    let section = search_section("sha256:model".to_owned(), requirements(), candidate(), None);
    assert!(section.check().is_ok());
    assert!(matches!(
        section.law_candidate(),
        Some(LawCandidate::Search(kept)) if kept.selected == 1
    ));
    assert!(section.reason.contains("attempted 2 of 3 trials"));
}

#[test]
fn the_reopened_study_holds_the_search_candidate_only_while_it_has_a_current_candidate() {
    let mut study = Study::new("control-mechanical-resonance", "sha256:loop");
    study
        .set_section(&search_section(
            "sha256:model".to_owned(),
            requirements(),
            candidate(),
            None,
        ))
        .expect("the search section is kept");
    let current = StageEvidence {
        has_candidate: true,
        ..StageEvidence::default()
    };
    assert_eq!(
        study_search_candidate(&study, current).expect("the study reads"),
        Some(candidate()),
        "a current candidate reopens with its trials and the unit of J"
    );
    let stale = StageEvidence {
        has_candidate: false,
        ..StageEvidence::default()
    };
    assert_eq!(
        study_search_candidate(&study, stale).expect("the study reads"),
        None,
        "without a current candidate, reopening shows none"
    );
}

#[test]
fn after_the_candidate_is_kept_the_final_answer_says_the_study_has_one() {
    let loop_definition =
        LoopDefinition::decode(LESSON_89_LOOP).expect("lesson 89's loop file decodes");
    let model =
        decode_general_model(&serde_json::from_str(LESSON_89_MODEL).expect("the model is JSON"))
            .expect("lesson 89's supplied model decodes");
    let current = CurrentModel {
        source: CurrentModelSource::Supplied,
        model,
        identify: None,
    };
    let section = search_section(
        current.model.identity().revision.clone(),
        requirements(),
        candidate(),
        None,
    );
    let mut study = Study::new(&loop_definition.id, &loop_definition.revision);
    study
        .set_section(&section)
        .expect("the search section is kept");
    let kept = study
        .section::<TuneSection>()
        .expect("the study reads")
        .expect("the kept section is there");
    let evidence = stage_evidence(&loop_definition, Some(&current), Some(&kept), false);
    assert!(
        evidence.has_candidate,
        "a candidate designed on the study's current model is the study's current candidate"
    );
    let answer = answer_of(None, candidate().trials, "rad²·s".to_owned()).with_stages(evidence);
    let value = serde_json::to_value(&answer).expect("the answer serializes");
    assert_eq!(value["status"], "completed");
    assert_eq!(value["stageEvidence"]["hasCandidate"], true, "{value}");
    assert_eq!(value["scoreUnit"], "rad²·s");
}
