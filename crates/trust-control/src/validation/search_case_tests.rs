use std::collections::BTreeMap;

use super::*;
use crate::loop_definition::LoopDefinition;
use crate::tuning::search::{SearchBounds, SearchCandidate, SearchTrial};

/// Lesson 89's control loop: the finite search over the notch's frequency and quality (C5).
const LESSON_89_LOOP: &str = include_str!(
    "../../../../examples/control/07-classical-stability-and-frequency-design/89-mechanical-resonance/control-loop.json"
);

/// Lesson 89's chosen trial as a search candidate: 2 Hz at a quality of 3 (C5, lesson 89's grid).
fn chosen(parameters: BTreeMap<String, f64>) -> SearchCandidate {
    SearchCandidate {
        parameters: parameters.clone(),
        selected: 0,
        trials: vec![SearchTrial {
            parameters,
            attempted: true,
            bounds: Some(SearchBounds::Met),
            score: Some(0.5),
            reason: None,
        }],
        score_unit: "rad²·s".to_owned(),
    }
}

#[test]
fn the_chosen_trial_of_lesson_89_is_its_whole_write_set() {
    let loop_definition =
        LoopDefinition::decode(LESSON_89_LOOP).expect("lesson 89's loop file decodes");
    let trial = BTreeMap::from([
        ("P1.NotchFrequencyHz".to_owned(), 2.0),
        ("P1.NotchQuality".to_owned(), 3.0),
    ]);
    let search = chosen(trial.clone());
    assert_eq!(
        candidate_coefficients(&loop_definition, &search)
            .expect("every bound parameter is in the trial"),
        &trial
    );
}

#[test]
fn a_trial_without_a_bound_parameter_is_refused_with_its_next_action() {
    let loop_definition =
        LoopDefinition::decode(LESSON_89_LOOP).expect("lesson 89's loop file decodes");
    let search = chosen(BTreeMap::from([("P1.NotchFrequencyHz".to_owned(), 2.0)]));
    let failure = candidate_coefficients(&loop_definition, &search)
        .expect_err("the notch quality is not in the trial");
    assert!(
        failure.message.contains("P1.NotchQuality"),
        "{}",
        failure.message
    );
    assert_eq!(failure.next_action, NextAction::Tune);
    assert_eq!(failure.cause, Cause::Settings);
}
