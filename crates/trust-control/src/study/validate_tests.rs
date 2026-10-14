//! Tests of the Validate section (specification 35, B5, A8.4 and N8.5): the section's rules and its currency, the campaign rule,
//! the Apply rule, the refusals before a start, the composition of the result from the jobs of the campaign, and the stage
//! evidence. The engineer suite's loop, model and task are built as the campaign tests build them. The jobs are the answers
//! that an isolated run can give, so the composition is tested without a runtime.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::*;
use crate::model::decode_general_model;
use crate::stages::{apply_available, Stage, StageStatus};
use crate::study::Study;
use crate::tuning::candidate::{
    AntiWindup, CandidateSource, ControlPidParameters, DEFAULT_RATE_LIMIT_PER_S,
};
use crate::tuning::structure::Structure;
use crate::validation::engineer_verification_plan::{
    engineer_requirements, engineer_verification_plan, EngineerRequirements,
};
use crate::validation::isolated_run_protocol::IsolatedJobState;
use crate::validation::run_validation::Window;

const FIXTURE: &str = include_str!(
    "../../../../editors/vscode/src/test/fixtures/control-studio-engineer/control-loop.json"
);

/// The engineer fixture as the campaign tests build it: a first-order proportional loop with the PID law and the parameters
/// `Main.Kp`, `Main.Ki` and `Main.Kd`, each bounded from 0 to 100.
fn fixture_loop() -> LoopDefinition {
    let mut value: Value = crate::json::parse_text(FIXTURE).expect("the fixture is JSON");
    if let Some(fields) = value.as_object_mut() {
        fields.remove("analysis");
        fields.remove("analysisParameterSymbols");
    }
    value["controller"]["type"] = json!("Control.PID");
    value["controller"]["design"] = json!("pid");
    value["controller"]["law"] = json!({
        "kind": "pid",
        "parameterRecordSymbol": "Main.Parameters",
        "parameterRevisionSymbol": "Main.ParameterRevision",
        "proportionalGainSymbol": "Main.Kp",
        "integralGainSymbol": "Main.Ki",
        "derivativeGainSymbol": "Main.Kd"
    });
    value["parameterBinding"]["parameters"] = json!([
        { "symbol": "Main.Kp", "iec_type": "LREAL", "unit": "V/m", "convention": "proportional gain", "minimum": 0, "maximum": 100 },
        { "symbol": "Main.Ki", "iec_type": "LREAL", "unit": "V/(m*s)", "convention": "integral gain", "minimum": 0, "maximum": 100 },
        { "symbol": "Main.Kd", "iec_type": "LREAL", "unit": "V*s/m", "convention": "derivative gain", "minimum": 0, "maximum": 100 }
    ]);
    LoopDefinition::decode(&value.to_string()).expect("the fixture decodes")
}

/// The suite's campaign model: a first-order process with a 0.5 s delay, validated as `model-4`.
fn fixture_model() -> GeneralModel {
    decode_general_model(&json!({
        "kind": "transfer-function",
        "numerator": [1.5],
        "denominator": [4.0, 1.0],
        "delaySeconds": 0.5,
        "revision": "model-4",
        "source": "provided",
        "validation": "validated",
        "evidence": ["declared-fixture"],
        "inputs": [{ "id": "u", "unit": "V", "operatingRange": { "minimum": -20.0, "maximum": 20.0 } }],
        "outputs": [{ "id": "y", "unit": "m", "operatingRange": { "minimum": -20.0, "maximum": 20.0 } }]
    }))
    .expect("the fixture model decodes")
}

/// The suite's campaign task: a unit reference change, a 0.2 m error limit, a 0.1 m settling band, 15 s to settle, and a 30 s
/// validation.
fn fixture_task() -> EngineerRequirements {
    engineer_requirements(&json!({
        "referenceChange": 1.0,
        "maximumError": 0.2,
        "settlingBand": 0.1,
        "settlingTimeSeconds": 15.0,
        "durationSeconds": 30.0
    }))
    .expect("the fixture requirements decode")
}

/// The gains the parameter binding names, as the program holds them.
fn fixture_gains() -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("Main.Kp".to_owned(), 1.0),
        ("Main.Ki".to_owned(), 0.2),
        ("Main.Kd".to_owned(), 0.0),
    ])
}

/// The verification plan of the fixture.
fn plan_of(loop_definition: &LoopDefinition) -> VerificationPlan {
    engineer_verification_plan(
        loop_definition,
        &fixture_model(),
        &fixture_task(),
        &fixture_gains(),
        &BTreeMap::new(),
    )
    .expect("the fixture plan")
}

/// The target-response case of a plan: the plan's own requirements, with no events, overrides or effects.
fn nominal_case(plan: &VerificationPlan) -> CampaignCase {
    CampaignCase {
        id: "target-response".to_owned(),
        plan: plan.clone(),
        initial_overrides: BTreeMap::new(),
        events: Vec::new(),
        effects: Vec::new(),
        requirements: plan.requirements.clone(),
    }
}

/// A candidate record with the given proportional and integral gains, valid under `validate_parameters`.
fn sample_candidate(proportional: f64, integral: f64) -> ControlPidCandidate {
    ControlPidCandidate {
        applied: false,
        source: CandidateSource::Simc,
        structure: Structure::PI,
        parameters: ControlPidParameters {
            proportional_gain: proportional,
            integral_gain: integral,
            derivative_gain: 0.0,
            proportional_setpoint_weight: 1.0,
            derivative_setpoint_weight: 0.0,
            bias: 0.0,
            output_minimum: -10.0,
            output_maximum: 10.0,
            rate_minimum: -DEFAULT_RATE_LIMIT_PER_S,
            rate_maximum: DEFAULT_RATE_LIMIT_PER_S,
            tracking_time: 1.0,
            anti_windup: AntiWindup::BackCalculation,
        },
        measurement_filter_seconds: None,
    }
}

/// The Tune section that designed `candidate` on the fixture model, to `requirements`.
fn tune_section(candidate: ControlPidCandidate, requirements: TuningRequirements) -> TuneSection {
    TuneSection {
        method: None,
        model_revision: "model-4".to_owned(),
        requirements,
        candidate,
        reason: "Designed for the test.".to_owned(),
        closed_loop_time_seconds: None,
        attempts: Vec::new(),
        pins: None,
    }
}

/// The recording identifier of the model check of these tests.
fn recording_id() -> String {
    format!("rec-{}", "0".repeat(32))
}

/// A model check of the fixture model, with the given verdict.
fn model_check(status: RequirementStatus) -> ModelCheck {
    ModelCheck {
        status,
        model_revision: "model-4".to_owned(),
        recording_id: recording_id(),
        limit: 0.15,
        normalized_error: Some(0.04),
        reason: "The check measured its error against the limit.".to_owned(),
    }
}

/// One run's outcome, with no requirement results, on the project revision `rev-1`.
fn run_outcome(run_id: &str, status: RequirementStatus) -> RunOutcome {
    RunOutcome {
        run_id: run_id.to_owned(),
        project_revision: "rev-1".to_owned(),
        evidence: RunEvidence::StExecuted,
        status,
        requirements: Vec::new(),
    }
}

/// A scenario whose current settings passed and whose candidate has the given status.
fn scenario(id: &str, candidate: RequirementStatus) -> ScenarioOutcome {
    ScenarioOutcome {
        id: id.to_owned(),
        plant: "model model-4".to_owned(),
        current: run_outcome(&format!("{id}-current"), RequirementStatus::Pass),
        candidate: run_outcome(&format!("{id}-candidate"), candidate),
    }
}

/// A validation section that passed on the fixture model, with the candidate and requirements given.
fn passing_section(
    candidate: ControlPidCandidate,
    requirements: TuningRequirements,
) -> ValidateSection {
    ValidateSection {
        model_revision: "model-4".to_owned(),
        candidate,
        requirements,
        conditions: ValidationConditions {
            step_ms: 20,
            duration_ms: 30_000,
            record_every: 1,
            seed: 1,
            initial_values: BTreeMap::new(),
            observed_symbols: vec!["Main.Kp".to_owned()],
        },
        model_check: Some(model_check(RequirementStatus::Pass)),
        scenarios: vec![scenario("target-response", RequirementStatus::Pass)],
        status: RequirementStatus::Pass,
        apply_available: true,
    }
}

/// An isolated job's answer of the given state, on the given project revision, with no run.
fn job(status: IsolatedJobState, revision: &str, job_id: &str) -> IsolatedRunStatus {
    IsolatedRunStatus {
        job_id: job_id.to_owned(),
        project_revision: revision.to_owned(),
        status,
        cancel_requested: false,
        reason: None,
        source_basis: "loaded-runtime-sources".to_owned(),
        io_provenance: "isolated-simulation-no-physical-drivers".to_owned(),
        captured_sample_count: 0,
        run: None,
    }
}

#[test]
fn a_validation_is_kept_with_its_model_check_recording_listed_and_read_back_whole() {
    let section = passing_section(sample_candidate(2.0, 0.5), fixture_task());
    section.check().expect("the section holds its rules");
    let mut study = Study::new("loop:tank", "sha256:abc");
    study.set_section(&section).expect("kept");
    assert_eq!(
        study.recordings,
        vec![recording_id()],
        "the check's recording is listed once"
    );
    assert_eq!(
        study.section::<ValidateSection>().expect("read"),
        Some(section.clone())
    );
    let text = study.encode().expect("encoded");
    let reopened = Study::decode(&text, "loop:tank").expect("decoded");
    assert_eq!(
        reopened.section::<ValidateSection>().expect("read"),
        Some(section)
    );
}

#[test]
fn a_section_whose_result_or_apply_rule_does_not_follow_from_its_runs_is_refused() {
    let mut wrong_status = passing_section(sample_candidate(2.0, 0.5), fixture_task());
    wrong_status
        .scenarios
        .push(scenario("declared", RequirementStatus::Fail));
    let refused = wrong_status.check().expect_err("a failed run cannot pass");
    assert!(
        refused.message.contains("overall result"),
        "{}",
        refused.message
    );
    let mut wrong_apply = passing_section(sample_candidate(2.0, 0.5), fixture_task());
    wrong_apply.model_check = Some(model_check(RequirementStatus::Fail));
    let refused = wrong_apply
        .check()
        .expect_err("a failed model check cannot authorize Apply");
    assert!(
        refused.message.contains("Apply rule"),
        "{}",
        refused.message
    );
}

#[test]
fn the_overall_result_follows_the_campaign_rule() {
    assert_eq!(
        overall_status(&[]),
        RequirementStatus::Inconclusive,
        "no case, no evidence"
    );
    assert_eq!(
        overall_status(&[
            scenario("a", RequirementStatus::Pass),
            scenario("b", RequirementStatus::Pass)
        ]),
        RequirementStatus::Pass
    );
    assert_eq!(
        overall_status(&[
            scenario("a", RequirementStatus::Pass),
            scenario("b", RequirementStatus::Inconclusive)
        ]),
        RequirementStatus::Inconclusive,
        "missing evidence never passes"
    );
    assert_eq!(
        overall_status(&[
            scenario("a", RequirementStatus::Fail),
            scenario("b", RequirementStatus::Inconclusive)
        ]),
        RequirementStatus::Fail
    );
}

#[test]
fn apply_needs_a_qualified_model_and_a_passed_candidate() {
    assert!(
        applies(None, RequirementStatus::Pass),
        "a model qualified before needs no check"
    );
    assert!(applies(
        Some(&model_check(RequirementStatus::Pass)),
        RequirementStatus::Pass
    ));
    assert!(!applies(
        Some(&model_check(RequirementStatus::Fail)),
        RequirementStatus::Pass
    ));
    assert!(!applies(
        Some(&model_check(RequirementStatus::Inconclusive)),
        RequirementStatus::Pass
    ));
    assert!(
        !applies(None, RequirementStatus::Inconclusive),
        "missing evidence never passes"
    );
}

#[test]
fn a_validation_is_current_only_for_its_model_candidate_and_requirements() {
    let model = fixture_model();
    let candidate = sample_candidate(2.0, 0.5);
    let requirements = fixture_task();
    let tune = tune_section(candidate, requirements.clone());
    let section = passing_section(candidate, requirements.clone());
    assert!(
        section.is_current(&model, &tune),
        "the same model, candidate and requirements"
    );
    let other_model = ValidateSection {
        model_revision: "model-3".to_owned(),
        ..section.clone()
    };
    assert!(
        !other_model.is_current(&model, &tune),
        "a new model invalidates it"
    );
    let mut changed = candidate;
    changed.parameters.proportional_gain = 3.0;
    let changed_tune = tune_section(changed, requirements.clone());
    assert!(
        !section.is_current(&model, &changed_tune),
        "a changed candidate invalidates it"
    );
    let mut other_requirements = requirements.clone();
    other_requirements.maximum_error = 0.3;
    let requirements_tune = tune_section(candidate, other_requirements);
    assert!(
        !section.is_current(&model, &requirements_tune),
        "changed requirements invalidate it"
    );
}

#[test]
fn check_before_start_needs_a_current_model_first() {
    let loop_definition = fixture_loop();
    let readiness: Result<String, Failure> = Ok("sha256:project".to_owned());
    let refused = check_before_start(&ValidateInputs {
        loop_definition: &loop_definition,
        identify: None,
        tune: None,
        readiness: &readiness,
    })
    .expect_err("no model");
    assert_eq!(refused.next_action, NextAction::Identify);
    assert_eq!(refused.cause, Cause::ModelOrData);
    assert!(
        refused.message.contains("no model for the loop"),
        "{}",
        refused.message
    );
}

#[test]
fn a_validation_that_has_not_run_is_not_a_stage_result() {
    let evidence = stage_evidence(&fixture_loop(), None, None, None, true);
    assert!(!evidence.validation_has_result);
    assert!(!apply_available(&evidence));
}

#[test]
fn a_run_that_did_not_complete_gives_inconclusive_requirements_with_their_reason() {
    let loop_definition = fixture_loop();
    let case = nominal_case(&plan_of(&loop_definition));
    let failed = job(IsolatedJobState::Failed, "rev-1", "run-9");
    let outcome = job_outcome(
        &loop_definition,
        &case,
        &failed,
        "configured plant pl",
        false,
    )
    .expect("the outcome");
    assert!(outcome.evaluation.is_none(), "no run, no evaluation");
    assert_eq!(outcome.run.status, RequirementStatus::Inconclusive);
    assert_eq!(outcome.run.run_id, "run-9");
    assert_eq!(outcome.run.requirements.len(), case.requirements.len());
    for requirement in &outcome.run.requirements {
        assert_eq!(requirement.status, RequirementStatus::Inconclusive);
        assert_eq!(
            requirement.reason,
            texts::run_not_completed("failed", failed.reason.as_deref())
        );
        assert_eq!(requirement.source, "configured plant pl");
    }
}

/// B5.3, C0: the candidate's run holds the candidate's values, so a fixed parameter it overrode is checked against them;
/// the other requirements, and the parameters it did not override, keep the plan's values.
#[test]
fn a_fixed_parameter_the_run_overrode_is_checked_against_the_value_it_held() {
    let window = Window {
        start: 0.0,
        end: 10.0,
    };
    let fixed = |symbol: &str, value: f64| RunRequirement {
        id: format!("{FIXED_PARAMETER_PREFIX}{symbol}"),
        channel: symbol.to_owned(),
        window,
        check: RunCheck::Bounds {
            minimum: value,
            maximum: value,
        },
    };
    let requirements = vec![fixed("P1.Kp", 1.0), fixed("P1.Ki", 0.2)];
    let parameters = BTreeMap::from([("P1.Kp".to_owned(), 3.5)]);
    let held = held_parameters(&requirements, &parameters);
    assert_eq!(held, vec![fixed("P1.Kp", 3.5), fixed("P1.Ki", 0.2)]);
    assert_eq!(
        held_parameters(&requirements, &BTreeMap::new()),
        requirements
    );
}

#[test]
fn the_jobs_of_a_campaign_share_one_program_revision() {
    let plant = VerificationPlant::Model;
    let same = [
        job(IsolatedJobState::Failed, "rev-1", "a"),
        job(IsolatedJobState::Failed, "rev-1", "b"),
    ];
    let jobs = [CaseJobs {
        plant: &plant,
        current: &same[0],
        candidate: &same[1],
    }];
    assert!(one_revision(&jobs));
    let mixed = [
        job(IsolatedJobState::Failed, "rev-1", "a"),
        job(IsolatedJobState::Failed, "rev-2", "b"),
    ];
    let jobs = [CaseJobs {
        plant: &plant,
        current: &mixed[0],
        candidate: &mixed[1],
    }];
    assert!(
        !one_revision(&jobs),
        "a program that changed during the campaign cannot compare"
    );
    assert!(one_revision(&[]), "no job, nothing to compare");
}

#[test]
fn the_failure_names_the_model_check_then_a_failed_requirement_then_the_missing_evidence() {
    let mut scenarios = vec![scenario("target-response", RequirementStatus::Fail)];
    scenarios[0]
        .candidate
        .requirements
        .push(RequirementOutcome {
            id: "settling-time".to_owned(),
            status: RequirementStatus::Fail,
            value: Some(20.0),
            limit: Some(15.0),
            unit: "s".to_owned(),
            source: "model model-4".to_owned(),
            reason: "The response settles at 20 s, after the 15 s limit.".to_owned(),
        });
    let failing_check = model_check(RequirementStatus::Fail);
    let by_model = failure_of(
        Some(&failing_check),
        RequirementStatus::Fail,
        &scenarios,
        &[],
    );
    assert_eq!(by_model.next_action, NextAction::Identify);
    assert_eq!(by_model.cause, Cause::ModelOrData);
    assert!(by_model.message.contains("check"), "{}", by_model.message);
    let by_requirement = failure_of(None, RequirementStatus::Fail, &scenarios, &[]);
    assert_eq!(by_requirement.next_action, NextAction::Tune);
    assert_eq!(by_requirement.cause, Cause::Settings);
    assert!(
        by_requirement.message.contains("settles at 20 s"),
        "{}",
        by_requirement.message
    );
    let missing = failure_of(
        None,
        RequirementStatus::Inconclusive,
        &[scenario("declared", RequirementStatus::Inconclusive)],
        &[None],
    );
    assert_eq!(missing.next_action, NextAction::None);
    assert_eq!(missing.cause, Cause::Runtime, "a run did not complete");
    assert!(
        missing.message.contains("No requirement"),
        "{}",
        missing.message
    );
}

#[test]
fn a_validation_without_passed_runs_is_inconclusive_and_says_why() {
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
    })
    .expect("composed");
    assert_eq!(outcome.section.status, RequirementStatus::Inconclusive);
    assert!(!outcome.result.apply_available);
    assert!(!outcome.section.apply_available);
    assert_eq!(outcome.result.scenarios.len(), 1);
    assert!(
        outcome.section.check().is_ok(),
        "the section it composes reads back"
    );
    let failure = outcome
        .failure
        .expect("Apply is not available, so the failure says why");
    assert_eq!(failure.cause, Cause::Runtime);
    assert_eq!(failure.next_action, NextAction::None);
    let validate = outcome
        .result
        .stages
        .iter()
        .find(|item| item.stage == Stage::Validate)
        .expect("the Validate stage");
    assert_eq!(
        validate.status,
        StageStatus::NeedsData,
        "missing evidence needs data"
    );
}

#[test]
fn a_failed_model_check_stops_apply_and_names_identify() {
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
        qualification: ModelQualification::Checked(model_check(RequirementStatus::Fail)),
        plant_reason: None,
    })
    .expect("composed");
    assert_eq!(
        outcome.result.model_check,
        Some(model_check(RequirementStatus::Fail))
    );
    assert!(!outcome.result.apply_available);
    let failure = outcome.failure.expect("the failed check stops Apply");
    assert_eq!(failure.next_action, NextAction::Identify);
    assert_eq!(failure.cause, Cause::ModelOrData);
    assert!(outcome.section.check().is_ok());
    let recording = recording_id();
    assert_eq!(
        outcome.section.recording_ids(),
        vec![recording.as_str()],
        "the check's recording is kept with the section"
    );
}

#[test]
fn the_composition_refuses_jobs_that_do_not_match_the_campaign_or_ran_on_two_revisions() {
    let loop_definition = fixture_loop();
    let model = fixture_model();
    let tune = tune_section(sample_candidate(2.0, 0.5), fixture_task());
    let cases = [nominal_case(&plan_of(&loop_definition))];
    let plant = VerificationPlant::Model;
    let no_jobs = validate_outcome(&ValidationInput {
        loop_definition: &loop_definition,
        model: &model,
        tune: &tune,
        before: StageEvidence::default(),
        cases: &cases,
        jobs: &[],
        qualification: ModelQualification::Qualified,
        plant_reason: None,
    })
    .expect_err("one case, no job");
    assert_eq!(no_jobs.cause, Cause::Implementation);
    let current = job(IsolatedJobState::Failed, "rev-1", "run-1");
    let candidate = job(IsolatedJobState::Failed, "rev-2", "run-2");
    let jobs = [CaseJobs {
        plant: &plant,
        current: &current,
        candidate: &candidate,
    }];
    let changed = validate_outcome(&ValidationInput {
        loop_definition: &loop_definition,
        model: &model,
        tune: &tune,
        before: StageEvidence::default(),
        cases: &cases,
        jobs: &jobs,
        qualification: ModelQualification::Qualified,
        plant_reason: None,
    })
    .expect_err("two program revisions");
    assert_eq!(changed.next_action, NextAction::Validate);
    assert!(changed.message.contains("changed"), "{}", changed.message);
}

#[test]
fn the_result_names_why_the_model_replaced_the_configured_plant_and_omits_the_reason_otherwise() {
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
    let reason = "The configured plant cannot show the response from its operating point.";
    let replaced = validate_outcome(&ValidationInput {
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
        plant_reason: Some(reason.to_owned()),
    })
    .expect("composed");
    assert_eq!(replaced.result.plant_reason.as_deref(), Some(reason));
    let kept = validate_outcome(&ValidationInput {
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
    })
    .expect("composed");
    assert_eq!(kept.result.plant_reason, None);
    let serialized = serde_json::to_value(&kept.result).expect("the result serializes");
    assert!(serialized.get("plantReason").is_none(), "{serialized}");
}
