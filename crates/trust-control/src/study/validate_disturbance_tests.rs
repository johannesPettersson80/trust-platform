//! The measured-disturbance case of a feedforward's Validate (specification 35, C8) on lesson 84 and the supplied two-input heater: the case
//! is a row beside the single-loop cases; the result follows from its row, so a failing row ends Apply and names Tune; and the stored
//! section keeps the row and refuses a result that does not follow from it.

#![allow(clippy::float_cmp)] // The allowance is the literal the test sets, compared exactly on purpose.

use std::collections::BTreeMap;

use super::tests::job;
use super::*;
use crate::messages::TuneRequest;
use crate::study::current_model::{CurrentModel, CurrentModelSource};
use crate::study::tune::{tune_held, TuneInputs};
use crate::tuning::objective::TuningObjective;
use crate::tuning::structures::fixtures::{heater_model, requirements};
use crate::validation::engineer_verification_campaign::engineer_verification_campaign;

/// Lesson 84 as the shipped file declares it.
const LESSON_84: &str = include_str!(
    "../../../../editors/vscode/media/examples/control/14-integrated-capstones/84-independent/control-loop.json"
);

/// The sentence a refusal shows in full: its detail when it says more than its first sentence.
fn text_of(failure: &Failure) -> String {
    failure
        .detail
        .clone()
        .unwrap_or_else(|| failure.message.clone())
}

fn lesson_84() -> LoopDefinition {
    LoopDefinition::decode(LESSON_84).expect("lesson 84 decodes")
}

/// The test's task: a 1 °C change, a 0.3 °C allowance at 40 s, a 0.1 °C band and an 80 s test, balanced.
fn task() -> TuningRequirements {
    let mut task = requirements(1.0, 0.3, 0.1, 40.0, 80.0);
    task.tuning_objective = Some(TuningObjective::Balanced);
    task
}

/// The request of lesson 84's Tune, Automatic.
fn request() -> TuneRequest {
    TuneRequest {
        project_root: "/projects/control-capstone-independent".to_owned(),
        runtime_id: "simulator".to_owned(),
        loop_id: "control-capstone-independent".to_owned(),
        storage_folder: "/state/trust".to_owned(),
        method: None,
        choice: None,
        structure: None,
        pins: None,
        settings: None,
        connection: None,
    }
}

/// Lesson 84's Tune section on the supplied heater, with the feedforward's PI and its `K_ff`, lead and lag.
fn tuned() -> TuneSection {
    let loop_definition = lesson_84();
    let model = CurrentModel {
        source: CurrentModelSource::Supplied,
        model: heater_model(),
        identify: None,
    };
    let request = request();
    let inputs = TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&model),
        previous: None,
        request: &request,
        relay: None,
        pins: None,
    };
    tune_held(&inputs, Some(&task()), None)
        .expect("the feedforward is designed")
        .section
}

/// Validates lesson 84's single-loop cases with the supplied heater, and the measured-disturbance case beside them.
/// Every job of a case has not completed, so each scenario row is Inconclusive, and the result is the disturbance row's (N8.5).
fn validate_with(section: &TuneSection) -> ValidateOutcome {
    let loop_definition = lesson_84();
    let model = heater_model();
    let observed = BTreeMap::from([
        ("P1.Parameters.ProportionalGain".to_owned(), 2.0),
        ("P1.Parameters.IntegralGain".to_owned(), 0.5),
        ("P1.Parameters.DerivativeGain".to_owned(), 0.0),
        ("P1.FeedforwardGain".to_owned(), 0.5),
        ("P1.LeadS".to_owned(), 8.0),
        ("P1.LagS".to_owned(), 3.0),
    ]);
    // The campaign takes the loop and its model as the study holds them: every case runs on the two-input model (C8, Validate).
    let cases = engineer_verification_campaign(
        &loop_definition,
        &model,
        &section.requirements,
        &observed,
        &BTreeMap::new(),
    )
    .expect("the campaign is built");
    let runs: Vec<(IsolatedRunStatus, IsolatedRunStatus)> = cases
        .iter()
        .map(|_| {
            (
                job(IsolatedJobState::Failed, "rev-1", "job-current"),
                job(IsolatedJobState::Failed, "rev-1", "job-candidate"),
            )
        })
        .collect();
    let plant = VerificationPlant::Model;
    let jobs: Vec<CaseJobs<'_>> = runs
        .iter()
        .map(|(current, candidate)| CaseJobs {
            plant: &plant,
            current,
            candidate,
        })
        .collect();
    validate_outcome(&ValidationInput {
        loop_definition: &loop_definition,
        model: &model,
        tune: section,
        before: StageEvidence::default(),
        cases: &cases,
        jobs: &jobs,
        qualification: ModelQualification::Qualified,
        plant_reason: None,
        entry: None,
    })
    .expect("the validation composes")
}

#[test]
fn lesson_84_validates_with_its_single_loop_rows_and_its_measured_disturbance_row() {
    let section = tuned();
    let outcome = validate_with(&section);
    assert!(
        !outcome.result.scenarios.is_empty(),
        "the single-loop cases are rows of the result"
    );
    let case = outcome
        .result
        .disturbance_case
        .as_ref()
        .expect("the measured-disturbance case is a row of the result");
    let [check] = case.requirements.as_slice() else {
        panic!("the case has one row: {case:?}");
    };
    assert_eq!(check.limit, Some(section.requirements.maximum_error));
    assert_eq!(case.status, check.status);
    let value = check
        .value
        .expect("the case measured its largest deviation");
    let expected = if value <= section.requirements.maximum_error {
        RequirementStatus::Pass
    } else {
        RequirementStatus::Fail
    };
    assert_eq!(check.status, expected, "{check:?}");
    assert_eq!(
        outcome.section.disturbance_case.as_ref(),
        Some(case),
        "the stored section keeps the row"
    );
    // The scenario rows have no evidence, so the candidate's result is the disturbance row's, and Apply waits for a passing run.
    let overall = if case.status == RequirementStatus::Fail {
        RequirementStatus::Fail
    } else {
        RequirementStatus::Inconclusive
    };
    assert_eq!(outcome.result.status, overall);
    assert!(!outcome.result.apply_available);
}

#[test]
fn a_failing_measured_disturbance_case_ends_apply_and_names_tune() {
    let mut section = tuned();
    // No deviation fits a zero allowance, so the case fails on the model (C8, B5.4).
    section.requirements.maximum_error = 0.0;
    let outcome = validate_with(&section);
    let case = outcome
        .result
        .disturbance_case
        .as_ref()
        .expect("the measured-disturbance case is a row of the result");
    assert_eq!(case.status, RequirementStatus::Fail);
    assert_eq!(outcome.result.status, RequirementStatus::Fail);
    assert!(!outcome.result.apply_available, "a failed case ends Apply");
    let failure = outcome
        .failure
        .expect("a failed case names why Apply stops");
    assert!(matches!(failure.cause, Cause::Settings));
    assert!(matches!(failure.next_action, NextAction::Tune));
    assert!(
        text_of(&failure).contains("leaves the error allowance"),
        "{}",
        text_of(&failure)
    );
}

#[test]
fn the_stored_section_refuses_a_disturbance_result_that_does_not_follow_from_its_row() {
    let outcome = validate_with(&tuned());
    outcome
        .section
        .check()
        .expect("the section holds its rules");
    let mut tampered = outcome.section.clone();
    let case = tampered
        .disturbance_case
        .as_mut()
        .expect("the section keeps the case");
    let row = case.requirements[0].status;
    case.status = if row == RequirementStatus::Pass {
        RequirementStatus::Fail
    } else {
        RequirementStatus::Pass
    };
    assert!(
        tampered.check().is_err(),
        "a case whose result contradicts its row is not kept"
    );
}
