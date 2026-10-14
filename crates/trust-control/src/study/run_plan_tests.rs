//! Tests of the automatic progression's order and of the inputs it asks for (specification 35, B8 and A11.2).

use super::*;
use crate::messages::RunNeeds;
use crate::stages::StageEvidence;

fn evidence(has_model: bool, has_candidate: bool, validated: bool) -> StageEvidence {
    StageEvidence {
        has_model,
        has_candidate,
        validation_has_result: validated,
        ..StageEvidence::default()
    }
}

fn plan(identify: bool, tune: bool, validate: bool) -> RunPlan {
    RunPlan {
        identify,
        tune,
        validate,
    }
}

#[test]
fn from_scratch_through_identify_runs_identify_only() {
    assert_eq!(
        run_plan(&evidence(false, false, false), RunThrough::Identify),
        plan(true, false, false)
    );
}

#[test]
fn from_scratch_through_tune_runs_identify_then_tune() {
    assert_eq!(
        run_plan(&evidence(false, false, false), RunThrough::Tune),
        plan(true, true, false),
        "Identify, then Tune; Validate waits for its own name"
    );
}

#[test]
fn from_scratch_through_validate_runs_all_three_stages() {
    assert_eq!(
        run_plan(&evidence(false, false, false), RunThrough::Validate),
        plan(true, true, true)
    );
}

#[test]
fn with_a_current_model_through_validate_runs_tune_and_validate_only() {
    assert_eq!(
        run_plan(&evidence(true, false, false), RunThrough::Validate),
        plan(false, true, true)
    );
}

#[test]
fn with_a_candidate_and_no_validation_through_validate_runs_validate_only() {
    assert_eq!(
        run_plan(&evidence(true, true, false), RunThrough::Validate),
        plan(false, false, true)
    );
}

#[test]
fn current_results_are_reused_and_a_run_through_tune_stops_at_tune() {
    assert_eq!(
        run_plan(&evidence(true, true, true), RunThrough::Validate),
        plan(false, false, false),
        "every result is current: nothing runs"
    );
    assert_eq!(
        run_plan(&evidence(true, true, false), RunThrough::Tune),
        plan(false, false, false),
        "the candidate is current, so Tune is not run again"
    );
}

#[test]
fn a_stage_that_is_not_current_runs_with_every_stage_after_it() {
    assert_eq!(
        run_plan(&evidence(false, true, true), RunThrough::Validate),
        plan(true, true, true),
        "a new model makes the candidate and the validation depend on something new"
    );
}

#[test]
fn a_failure_on_a_limit_or_a_requirement_names_that_input() {
    let limits = Failure::new("Set the limits.", Cause::Settings, NextAction::OpenLimits);
    assert_eq!(needs_input(&limits), Some(RunNeeds::Limits));
    let requirements = Failure::new(
        "Enter a requirement.",
        Cause::Settings,
        NextAction::OpenRequirements,
    );
    assert_eq!(needs_input(&requirements), Some(RunNeeds::Requirements));
}

#[test]
fn every_other_failure_stops_the_run_as_a_failure() {
    let infeasible = Failure::new(
        "No structure meets the requirements.",
        Cause::TaskInfeasible,
        NextAction::OpenRequirements,
    );
    assert_eq!(
        needs_input(&infeasible),
        None,
        "an infeasible requirement is a failure"
    );
    let waiting = Failure::new("Wait.", Cause::Runtime, NextAction::Wait);
    assert_eq!(needs_input(&waiting), None);
    let unnamed = Failure::new("Stopped.", Cause::Settings, NextAction::None);
    assert_eq!(needs_input(&unnamed), None);
}

#[test]
fn identify_plant_identifies_again_though_a_model_is_current_and_other_runs_reuse_it() {
    let current = StageEvidence {
        has_model: true,
        has_candidate: true,
        validation_has_result: true,
        validation_passed: true,
        ..StageEvidence::default()
    };
    assert_eq!(
        run_plan(&current, RunThrough::Identify),
        RunPlan {
            identify: false,
            tune: false,
            validate: false,
        },
        "a current model is reused"
    );
    assert_eq!(
        run_plan(&planned_evidence(current, true), RunThrough::Identify),
        RunPlan {
            identify: true,
            tune: false,
            validate: false,
        },
        "Identify plant runs a new campaign and stops on Identify (B8)"
    );
    assert_eq!(
        run_plan(&planned_evidence(current, true), RunThrough::Validate),
        RunPlan {
            identify: true,
            tune: true,
            validate: true,
        },
        "a new model retires the candidate and the validation that depend on it (A8.4)"
    );
    assert_eq!(planned_evidence(current, false), current);
}
