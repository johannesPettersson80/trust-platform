use serde_json::{json, Value};

use super::*;
use crate::loop_definition::{ControllerProject, ParameterBinding};
use crate::stages::StageEvidence;

/// Lesson 89's loop file: its search law is the grid that these tests read.
const LESSON_89_LOOP: &str = include_str!(
    "../../../../editors/vscode/media/examples/control/07-classical-stability-and-frequency-design/89-mechanical-resonance/control-loop.json"
);

fn lesson_89() -> Value {
    serde_json::from_str(LESSON_89_LOOP).expect("lesson 89's loop file is JSON")
}

/// The unit of J for lesson 89, whose process variable is an angle in radians.
fn unit() -> String {
    "rad²·s".to_owned()
}

fn met(score: f64) -> SearchTrial {
    SearchTrial {
        parameters: BTreeMap::new(),
        attempted: true,
        bounds: Some(SearchBounds::Met),
        score: Some(score),
        reason: None,
    }
}

fn with_bounds(bounds: SearchBounds, score: Option<f64>) -> SearchTrial {
    SearchTrial {
        bounds: Some(bounds),
        score,
        ..met(0.0)
    }
}

fn not_attempted() -> SearchTrial {
    SearchTrial::planned(BTreeMap::new(), None)
}

#[test]
fn lesson_89_grid_has_eight_trials_in_listed_order() {
    let project = ControllerProject::from_value(&lesson_89()["controller"]["project"])
        .expect("lesson 89's search law decodes");
    assert!(project.is_search_law());
    assert_eq!(trial_count(&project.axes, &BTreeMap::new()), 8);
    let combinations = grid(&project.axes, &BTreeMap::new());
    let order: Vec<(f64, f64)> = combinations
        .iter()
        .map(|trial| (trial["P1.NotchFrequencyHz"], trial["P1.NotchQuality"]))
        .collect();
    assert_eq!(
        order,
        vec![
            (0.0, 1.0),
            (0.0, 3.0),
            (2.0, 1.0),
            (2.0, 3.0),
            (3.0, 1.0),
            (3.0, 3.0),
            (4.0, 1.0),
            (4.0, 3.0),
        ]
    );
    assert!(check_trial_count(8, project.maximum_candidates).is_ok());
    assert!(
        check_trial_count(9, 8).is_err(),
        "more trials than the budget"
    );
    assert!(
        check_trial_count(65, 100).is_err(),
        "more trials than A7 allows"
    );
}

#[test]
fn a_pinned_axis_is_not_part_of_the_grid() {
    let project = ControllerProject::from_value(&lesson_89()["controller"]["project"])
        .expect("lesson 89's search law decodes");
    let pins = BTreeMap::from([("P1.NotchFrequencyHz".to_owned(), 3.0)]);
    assert_eq!(trial_count(&project.axes, &pins), 2);
    let combinations = grid(&project.axes, &pins);
    assert_eq!(combinations.len(), 2);
    assert!(combinations
        .iter()
        .all(|trial| (trial["P1.NotchFrequencyHz"] - 3.0).abs() < 1e-9));
}

#[test]
fn a_real_value_is_rounded_to_single_precision_before_the_trial_runs() {
    let mut binding = lesson_89()["parameterBinding"].clone();
    binding["parameters"][0]["iec_type"] = json!("REAL");
    let binding: ParameterBinding =
        serde_json::from_value(binding).expect("the binding decodes with a REAL field");
    let observed = BTreeMap::from([
        ("P1.NotchFrequencyHz".to_owned(), 0.0),
        ("P1.NotchQuality".to_owned(), 1.0),
    ]);
    let combination = BTreeMap::from([
        ("P1.NotchFrequencyHz".to_owned(), 0.1),
        ("P1.NotchQuality".to_owned(), 3.0),
    ]);
    let settings = trial_settings(&binding, &observed, &BTreeMap::new(), &combination)
        .expect("every bound parameter has a value");
    assert_eq!(settings["P1.NotchFrequencyHz"], f64::from(0.1_f32));
    assert_eq!(settings["P1.NotchQuality"], 3.0);
}

#[test]
fn squared_error_integral_is_the_trapezoidal_sum_by_hand() {
    // ½·(1² + 0²)·0.5 = 0.25 and ½·(0² + 2²)·0.5 = 1.0, so J = 1.25.
    let samples = [(0.0, 1.0), (0.5, 0.0), (1.0, 2.0)];
    let score = squared_error_integral(&samples).expect("three samples give J");
    assert!((score - 1.25).abs() <= 1e-12, "{score}");
    assert_eq!(squared_error_integral(&[(0.0, 1.0)]), None);
    assert_eq!(squared_error_integral(&[(0.0, 1.0), (0.0, 1.0)]), None);
    assert_eq!(squared_error_integral(&[(0.0, 1.0), (1.0, f64::NAN)]), None);
}

#[test]
fn a_failing_bound_does_not_qualify_and_missing_evidence_never_does() {
    let trials = [
        met(1.0),
        with_bounds(SearchBounds::NotMet, Some(0.1)),
        with_bounds(SearchBounds::MissingEvidence, Some(0.05)),
        met(2.0),
    ];
    assert_eq!(select(&trials), Some(0));
    let only_failing = [
        with_bounds(SearchBounds::NotMet, Some(0.1)),
        with_bounds(SearchBounds::MissingEvidence, None),
        not_attempted(),
    ];
    assert_eq!(select(&only_failing), None);
}

#[test]
fn equal_scores_pick_the_first_listed_trial() {
    assert_eq!(select(&[met(1.0), met(1.0), met(1.0)]), Some(0));
    assert_eq!(
        select(&[
            with_bounds(SearchBounds::NotMet, Some(0.5)),
            met(1.0),
            met(1.0)
        ]),
        Some(1)
    );
}

#[test]
fn bounds_are_met_only_when_every_case_passes() {
    assert_eq!(
        bounds_of(&[RequirementStatus::Pass, RequirementStatus::Pass]),
        SearchBounds::Met
    );
    assert_eq!(
        bounds_of(&[RequirementStatus::Pass, RequirementStatus::Fail]),
        SearchBounds::NotMet
    );
    assert_eq!(
        bounds_of(&[RequirementStatus::Pass, RequirementStatus::Inconclusive]),
        SearchBounds::MissingEvidence
    );
    assert_eq!(bounds_of(&[]), SearchBounds::MissingEvidence);
}

#[test]
fn a_cancelled_search_keeps_the_attempted_trials_and_marks_the_rest() {
    let mut trials: Vec<SearchTrial> = (0..8).map(|_| not_attempted()).collect();
    let scores = [4.0, 3.0, 2.0, 1.0, 5.0, 6.0, 7.0, 8.0];
    let stop = run_trials(
        &mut trials,
        |index| (index == 3).then_some(SearchStop::Cancelled),
        |index| {
            Ok(TrialEvidence {
                bounds: SearchBounds::Met,
                score: Some(scores[index]),
            })
        },
    );
    assert_eq!(stop, Some(SearchStop::Cancelled));
    let answer = answer_of(stop, trials, unit());
    assert_eq!(answer.status, SearchStatus::Cancelled);
    assert_eq!(answer.selected, Some(2), "the best of the attempted trials");
    let attempted: Vec<bool> = answer.trials.iter().map(|trial| trial.attempted).collect();
    assert_eq!(
        attempted,
        vec![true, true, true, false, false, false, false, false]
    );
    assert!(answer.trials[3..]
        .iter()
        .all(|trial| trial.bounds.is_none() && trial.score.is_none()));
    assert_eq!(
        answer.candidate_to_keep(),
        None,
        "a cancelled search keeps no candidate"
    );
}

#[test]
fn a_revision_change_stops_the_search_at_the_trial_that_saw_it() {
    let mut trials: Vec<SearchTrial> = (0..5).map(|_| not_attempted()).collect();
    let stop = run_trials(
        &mut trials,
        |_| None,
        |index| {
            if index == 2 {
                Err(SearchStop::ProgramChanged)
            } else {
                Ok(TrialEvidence {
                    bounds: SearchBounds::Met,
                    score: Some(1.0),
                })
            }
        },
    );
    assert_eq!(stop, Some(SearchStop::ProgramChanged));
    assert!(trials[0].attempted && trials[1].attempted && trials[2].attempted);
    assert_eq!(trials[2].bounds, Some(SearchBounds::MissingEvidence));
    assert_eq!(trials[2].score, None);
    assert!(trials[3..].iter().all(|trial| !trial.attempted));
    assert_eq!(
        answer_of(stop, trials, unit()).status,
        SearchStatus::ProgramChanged
    );
}

#[test]
fn a_refused_trial_is_never_run() {
    let mut trials = vec![
        SearchTrial::planned(BTreeMap::new(), Some("refused".to_owned())),
        not_attempted(),
    ];
    let mut ran = Vec::new();
    let stop = run_trials(
        &mut trials,
        |_| None,
        |index| {
            ran.push(index);
            Ok(TrialEvidence {
                bounds: SearchBounds::Met,
                score: Some(1.0),
            })
        },
    );
    assert_eq!(stop, None);
    assert_eq!(ran, vec![1]);
    assert!(!trials[0].attempted);
    assert_eq!(trials[0].reason.as_deref(), Some("refused"));
}

#[test]
fn a_completed_search_keeps_its_selected_candidate_and_checks_it() {
    let mut trials: Vec<SearchTrial> = (0..3).map(|_| not_attempted()).collect();
    for (trial, quality) in trials.iter_mut().zip([1.0, 2.0, 3.0]) {
        trial.parameters = BTreeMap::from([("P1.NotchQuality".to_owned(), quality)]);
    }
    let scores = [2.0, 1.0, 3.0];
    let stop = run_trials(
        &mut trials,
        |_| None,
        |index| {
            Ok(TrialEvidence {
                bounds: SearchBounds::Met,
                score: Some(scores[index]),
            })
        },
    );
    let answer = answer_of(stop, trials, unit());
    assert_eq!(answer.status, SearchStatus::Completed);
    assert_eq!(answer.message, None);
    let candidate = answer
        .candidate_to_keep()
        .expect("a completed search keeps its candidate");
    assert_eq!(candidate.selected, 1);
    assert_eq!(candidate.parameters["P1.NotchQuality"], 2.0);
    assert_eq!(candidate.score_unit, "rad²·s");
    assert!(candidate.check().is_ok());
}

#[test]
fn the_answer_has_the_shape_the_tune_page_reads() {
    let mut trials = vec![met(0.25), not_attempted()];
    trials[0].parameters = BTreeMap::from([("P1.NotchFrequencyHz".to_owned(), 0.0)]);
    trials[1].parameters = BTreeMap::from([("P1.NotchFrequencyHz".to_owned(), 2.0)]);
    let answer = answer_of(Some(SearchStop::BudgetExhausted), trials, unit());
    let value = serde_json::to_value(&answer).expect("the answer serializes");
    assert_eq!(value["status"], "budgetExhausted");
    assert_eq!(value["selected"], 0);
    assert_eq!(value["scoreUnit"], "rad²·s");
    assert_eq!(value["trials"][0]["attempted"], true);
    assert_eq!(value["trials"][0]["bounds"], "met");
    assert_eq!(value["trials"][0]["score"], 0.25);
    assert_eq!(value["trials"][1]["attempted"], false);
    assert!(value["trials"][1].get("bounds").is_none());
    assert!(value["message"].is_string());
    assert!(
        value.get("stages").is_none(),
        "the stages come once the evidence is given"
    );
}

#[test]
fn the_unit_of_j_is_the_process_variable_unit_squared_times_seconds() {
    assert_eq!(score_unit_of("rad"), "rad²·s");
    assert_eq!(score_unit_of("degC"), "°C²·s");
    assert_eq!(score_unit_of(" N*m "), "N·m²·s");
}

#[test]
fn the_answer_carries_the_stages_of_the_evidence_it_is_given() {
    let evidence = StageEvidence {
        loop_bound: true,
        has_model: true,
        has_candidate: true,
        ..StageEvidence::default()
    };
    let answer = answer_of(None, vec![met(0.25)], unit()).with_stages(evidence);
    let value = serde_json::to_value(&answer).expect("the answer serializes");
    assert_eq!(value["stageEvidence"]["hasCandidate"], true);
    assert_eq!(
        value["stages"],
        serde_json::to_value(stage_availability(&evidence)).expect("the stages serialize")
    );
    assert_eq!(value["scoreUnit"], "rad²·s");
}
