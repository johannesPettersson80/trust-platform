//! Tests of the guard flags of the Validate rows and of the requirements an answer names (specification 35, B5.4, B9.9 and A8.6):
//! a guard is a bounds or rate check that the plan generated, a declared requirement never is, and the answer names the requirements
//! the candidate was tuned to, whether it is answered at once or reopened.

use super::tests::{
    fixture_loop, fixture_model, fixture_task, job, nominal_case, plan_of, sample_candidate,
    tune_section,
};
use super::*;
use crate::validation::run_validation::Window;

/// B5.4, B9.9: each row says whether it is a guard. The plan's generated bounds are guards; a declared bounds check, a settling
/// time and a maximum error are not. A completed run's row takes the same flag from the same naming (`requirement_of`).
#[test]
fn the_rows_say_which_are_guards_and_a_declared_bounds_check_is_not_one() {
    let loop_definition = fixture_loop();
    let mut case = nominal_case(&plan_of(&loop_definition));
    case.requirements.push(RunRequirement {
        id: "load-minimum".to_owned(),
        channel: "Main.PV".to_owned(),
        window: Window {
            start: 0.0,
            end: 10.0,
        },
        check: RunCheck::Bounds {
            minimum: 33.0,
            maximum: 80.0,
        },
    });
    let failed = job(IsolatedJobState::Failed, "rev-1", "run-9");
    let outcome = job_outcome(
        &loop_definition,
        &case,
        &failed,
        "configured plant pl",
        false,
        None,
    )
    .expect("the outcome");
    let guard = |id: &str| {
        outcome
            .run
            .requirements
            .iter()
            .find(|requirement| requirement.id == id)
            .map(|requirement| requirement.guard)
    };
    assert_eq!(
        guard("process-bounds"),
        Some(true),
        "the plan generates the operating range"
    );
    assert_eq!(
        guard("fixed-parameter:Main.Kp"),
        Some(true),
        "the plan holds a fixed parameter"
    );
    assert_eq!(
        guard("response-deadline"),
        Some(false),
        "a settling time is the engineer's requirement"
    );
    assert_eq!(
        guard("target-error"),
        Some(false),
        "a maximum error is the engineer's requirement"
    );
    assert_eq!(
        guard("load-minimum"),
        Some(false),
        "a declared bounds check is the engineer's, never a guard"
    );

    let generated = case
        .plan
        .requirements
        .iter()
        .find(|requirement| requirement.id == "process-bounds")
        .expect("the plan's operating range");
    let result = RunRequirementResult {
        id: "process-bounds".to_owned(),
        kind: "bounds".to_owned(),
        status: RequirementStatus::Pass,
        value: Some(10.0),
        limit: None,
        unit: "m".to_owned(),
        reason: String::new(),
    };
    let row = requirement_of(
        &result,
        Naming::of(generated, &loop_definition, None, &case.plan),
        "configured plant pl",
        (None, None),
    );
    assert!(
        row.guard,
        "a completed run's generated bounds are guards too"
    );
}

/// B5.4, A8.6: the answer names the requirements the candidate was tuned to, which the validation judged against, and a reopened
/// loop names the same ones. The composed rows carry the guard flags of the plan.
#[test]
fn the_answer_names_the_candidate_requirements_it_judged_against() {
    let loop_definition = fixture_loop();
    let model = fixture_model();
    let tune = tune_section(sample_candidate(2.0, 0.5), fixture_task());
    let cases = [nominal_case(&plan_of(&loop_definition))];
    let current = job(IsolatedJobState::Failed, "rev-1", "run-1");
    let candidate = job(IsolatedJobState::Failed, "rev-1", "run-2");
    let plant = VerificationPlant::Model;
    let jobs = [CaseJobs {
        plant: &plant,
        current: &current,
        candidate: &candidate,
    }];
    let outcome = validate_outcome(&ValidationInput {
        loop_definition: &loop_definition,
        model: &model,
        tune: &tune,
        before: StageEvidence {
            has_model: true,
            has_candidate: true,
            ..StageEvidence::default()
        },
        cases: &cases,
        jobs: &jobs,
        qualification: ModelQualification::Qualified,
        plant_reason: None,
        entry: None,
    })
    .expect("composed");
    assert_eq!(
        outcome.result.requirements, tune.requirements,
        "the answer names the requirements the candidate was tuned to (B5.4)"
    );
    assert_eq!(
        outcome
            .section
            .result(outcome.result.stage_evidence)
            .requirements,
        tune.requirements,
        "a reopened loop names the same requirements (A8.6)"
    );
    let guards: Vec<&str> = outcome.result.scenarios[0]
        .candidate
        .requirements
        .iter()
        .filter(|row| row.guard)
        .map(|row| row.id.as_str())
        .collect();
    assert!(
        guards.contains(&"process-bounds") && !guards.contains(&"target-error"),
        "the composed rows keep the guard flags of the plan: {guards:?}"
    );
}
