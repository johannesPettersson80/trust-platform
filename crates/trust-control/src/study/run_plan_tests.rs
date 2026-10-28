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
fn a_loop_without_a_controller_law_ends_its_run_with_identify() {
    // C12: Tune, Validate and Apply do not apply, so the run takes Identify alone, whatever the stage it names.
    let observed = StageEvidence {
        controller_stages_apply: false,
        ..evidence(false, false, false)
    };
    for through in [RunThrough::Tune, RunThrough::Validate] {
        assert_eq!(
            run_plan(&observed, through),
            plan(true, false, false),
            "{through:?}"
        );
    }
    let with_model = StageEvidence {
        controller_stages_apply: false,
        ..evidence(true, false, false)
    };
    assert_eq!(
        run_plan(&with_model, RunThrough::Validate),
        plan(false, false, false),
        "a current model needs no Identify, and no controller stage follows it"
    );
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
fn an_input_a_stage_asks_for_carries_the_stage_s_own_sentence_and_field() {
    // B8, B3.2: the run asks for an input in the sentence of the refusal that stopped it, and names the refusal's form field, so that the
    // page shows the engine's wording under the right field and writes none. A generic sentence per code would hide what is missing.
    let duration = Failure::new(
        "Set the maximum experiment duration in Limits before identification.",
        Cause::Settings,
        NextAction::OpenLimits,
    )
    .with_field("durationSeconds");
    let asked = NeededInput::of_failure(&duration).expect("a limit is an input");
    assert_eq!(asked.needs, RunNeeds::Limits);
    assert_eq!(
        asked.message,
        "Set the maximum experiment duration in Limits before identification."
    );
    assert_eq!(asked.field.as_deref(), Some("durationSeconds"));
    let requirement = Failure::new(
        "Enter the maximum settling time. The design needs it.",
        Cause::Settings,
        NextAction::OpenRequirements,
    );
    let asked = NeededInput::of_failure(&requirement).expect("a requirement is an input");
    assert_eq!(asked.needs, RunNeeds::Requirements);
    assert_eq!(
        asked.message, "Enter the maximum settling time.",
        "the failure's message is its first sentence; the rest is its detail"
    );
    assert_eq!(asked.field, None, "a refusal of no field names none");
}

#[test]
fn the_manual_settings_are_asked_for_in_the_engine_s_sentence() {
    // B8: the run asks for the manual method's settings before any stage runs, so no refusal carries the sentence; it is the engine's.
    let asked = NeededInput::manual_settings();
    assert_eq!(asked.needs, RunNeeds::ManualSettings);
    assert_eq!(
        asked.message,
        "Enter the manual settings to tune the controller."
    );
    assert_eq!(asked.field, None);
}

#[test]
fn a_failure_that_asks_for_no_input_gives_no_needed_input() {
    // Every other failure stops the run as a failure (A11.2): it must not be turned into a request for an input.
    let infeasible = Failure::new(
        "No structure meets the requirements.",
        Cause::TaskInfeasible,
        NextAction::OpenRequirements,
    );
    assert_eq!(NeededInput::of_failure(&infeasible), None);
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
