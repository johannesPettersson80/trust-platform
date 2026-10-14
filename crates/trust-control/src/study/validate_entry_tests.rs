//! Tests of the Manual-to-Auto entry case in the Validate composition (specification 35, B5.3, B5.4, B5.5 and B9.9): the case joins the
//! campaign rule, a failing first output fails the validation with the reason of its row, a validation that runs no case answers none,
//! every row is a guard, and a loop whose Auto entry cannot be planned still validates its other cases and answers its case
//! Inconclusive with the refusal. The fixtures are the campaign tests' own.

use std::collections::BTreeMap;

use super::tests::{
    fixture_loop, fixture_model, fixture_task, job, nominal_case, plan_of, sample_candidate,
    tune_section,
};
use super::*;
use crate::identification::connected_record::ValidationChannel;
use crate::signals::Quality;
use crate::validation::initialization_case::{plan_entry_case, EntryCase};
use crate::validation::isolated_run_protocol::{
    IsolatedRunConditions, IsolatedRunObservation, IsolatedRunRecord, IsolatedRunSample,
    IsolatedRunValue,
};
use crate::validation::run_validation::Window;

/// The output channel that the fixture entry case judges: the first output after Auto entry.
const OUTPUT: &str = "Main.Out";

/// A row of the entry case with the given status: a guard, named as the case names its first output (B9.9).
fn entry_row(status: RequirementStatus) -> RequirementOutcome {
    RequirementOutcome {
        id: "transfer-continuity".to_owned(),
        label: "First output after Auto entry".to_owned(),
        channel: RequirementChannel {
            role: "Output".to_owned(),
            unit: "V".to_owned(),
            symbol: OUTPUT.to_owned(),
        },
        limit_side: LimitSide::AtMost,
        status,
        value: None,
        limit: None,
        unit: "V".to_owned(),
        source: "Model".to_owned(),
        reason: "The first output is outside its allowance.".to_owned(),
        automatic: false,
        guard: true,
        measured_range: None,
        limit_range: None,
    }
}

/// The entry case with one row of the given status, under the table's name for the case.
fn entry_with(status: RequirementStatus) -> InitializationOutcome {
    InitializationOutcome {
        label: "Manual-to-Auto entry".to_owned(),
        status,
        requirements: vec![entry_row(status)],
    }
}

/// A campaign scenario whose current and candidate runs both pass.
fn passing_scenario() -> ScenarioOutcome {
    let run = RunOutcome {
        run_id: "run-1".to_owned(),
        project_revision: "rev-1".to_owned(),
        evidence: RunEvidence::StExecuted,
        status: RequirementStatus::Pass,
        requirements: Vec::new(),
    };
    ScenarioOutcome {
        id: "target-response".to_owned(),
        plant: "Model".to_owned(),
        label: "Target response".to_owned(),
        current: run.clone(),
        candidate: run,
    }
}

/// The entry case of the fixture: its first output must stay between 0.9 V and 1.1 V over the last 0.2 s of its 0.4 s run.
fn entry_case() -> CampaignCase {
    let requirements = vec![RunRequirement {
        id: "transfer-continuity".to_owned(),
        channel: OUTPUT.to_owned(),
        window: Window {
            start: 0.2,
            end: 0.4,
        },
        check: RunCheck::Bounds {
            minimum: 0.9,
            maximum: 1.1,
        },
    }];
    CampaignCase {
        id: "manual-to-auto-entry".to_owned(),
        plan: plan_of(&fixture_loop()),
        initial_overrides: BTreeMap::new(),
        events: Vec::new(),
        effects: Vec::new(),
        requirements,
    }
}

/// The entry run's record: the output at each sample time, over a run of 0.4 s.
fn record(samples: &[(f64, f64)]) -> IsolatedRunRecord {
    IsolatedRunRecord {
        id: "entry-record".to_owned(),
        evidence: RunEvidence::StExecuted,
        parameters: BTreeMap::new(),
        conditions: IsolatedRunConditions {
            initial_state: BTreeMap::new(),
            events: Vec::new(),
            duration_seconds: 0.4,
            sample_times_seconds: samples.iter().map(|(time, _)| *time).collect(),
            seed: 1,
            model_revision: "model-4".to_owned(),
            program_revision: "sha256:program".to_owned(),
            parameter_schema_revision: "sha256:schema".to_owned(),
            record_every: None,
            model_provenance: None,
            simulation_config_revision: None,
            feedback_relay: None,
        },
        channels: vec![ValidationChannel {
            id: OUTPUT.to_owned(),
            unit: "V".to_owned(),
        }],
        samples: samples
            .iter()
            .zip(0_u64..)
            .map(|((time, value), sequence)| IsolatedRunSample {
                time_seconds: *time,
                sequence,
                values: BTreeMap::from([(
                    OUTPUT.to_owned(),
                    IsolatedRunValue {
                        value: Some(*value),
                        quality: Quality::Good,
                    },
                )]),
            })
            .collect(),
    }
}

/// The entry run's job, completed with its record.
fn completed(record: IsolatedRunRecord) -> IsolatedRunStatus {
    IsolatedRunStatus {
        job_id: "entry-1".to_owned(),
        project_revision: "rev-1".to_owned(),
        status: IsolatedJobState::Completed,
        cancel_requested: false,
        reason: None,
        source_basis: "loaded-runtime-sources".to_owned(),
        io_provenance: "isolated-simulation-no-physical-drivers".to_owned(),
        captured_sample_count: 0,
        run: Some(record),
    }
}

/// The validation of the fixture loop whose campaign did not run, so that the entry case alone decides its status. The entry case is
/// run as `entry_job`, refused as `refusal`, or absent when neither is given.
fn validation_with_entry(
    entry_job: Option<&IsolatedRunStatus>,
    refusal: Option<&Failure>,
) -> ValidateOutcome {
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
    let case = entry_case();
    let entry = match (entry_job, refusal) {
        (Some(run), _) => Some(EntryJob::Ran {
            case: &case,
            job: run,
            plant: &plant,
        }),
        (None, Some(refused)) => Some(EntryJob::Refused(refused)),
        (None, None) => None,
    };
    validate_outcome(&ValidationInput {
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
        entry,
    })
    .expect("the validation is composed")
}

#[test]
fn the_entry_case_joins_the_campaign_rule_so_apply_needs_it_to_pass() {
    let scenarios = [passing_scenario()];
    assert_eq!(
        overall_status(&scenarios, None),
        RequirementStatus::Pass,
        "a validation without the case keeps the campaign rule"
    );
    assert_eq!(
        overall_status(&scenarios, Some(&entry_with(RequirementStatus::Pass))),
        RequirementStatus::Pass
    );
    assert_eq!(
        overall_status(&scenarios, Some(&entry_with(RequirementStatus::Fail))),
        RequirementStatus::Fail,
        "a failed first output fails the validation"
    );
    assert_eq!(
        overall_status(
            &scenarios,
            Some(&entry_with(RequirementStatus::Inconclusive))
        ),
        RequirementStatus::Inconclusive,
        "a case that did not run never passes"
    );
}

#[test]
fn a_failing_first_output_fails_the_validation_with_the_reason_of_its_row() {
    // The output is 5 V over the window of the first output, outside its 0.9 V to 1.1 V allowance.
    let entry = completed(record(&[
        (0.0, 1.0),
        (0.1, 1.0),
        (0.2, 5.0),
        (0.3, 5.0),
        (0.4, 5.0),
    ]));
    let outcome = validation_with_entry(Some(&entry), None);
    let answered = outcome
        .result
        .initialization
        .expect("the entry case is answered");
    assert_eq!(
        answered.label, "Manual-to-Auto entry",
        "the table names the case, so a failing transfer row is told apart from a scenario's rows"
    );
    assert_eq!(answered.status, RequirementStatus::Fail);
    assert_eq!(outcome.section.status, RequirementStatus::Fail);
    assert!(!outcome.result.apply_available && !outcome.section.apply_available);
    let failed = answered
        .requirements
        .iter()
        .find(|row| row.status == RequirementStatus::Fail)
        .expect("the first output row fails");
    assert_eq!(failed.label, "First output after Auto entry");
    let failure = outcome
        .failure
        .expect("Apply is not available, so the failure says why");
    assert!(
        failure.message.contains(&failed.reason),
        "the failure carries the reason of the failed row: {}",
        failure.message
    );
    assert_eq!(failure.cause, Cause::Implementation);
    assert_eq!(failure.next_action, NextAction::OpenLoopSetup);
    assert!(outcome.section.check().is_ok(), "the section reads back");
}

#[test]
fn the_rows_of_the_entry_case_are_guards_and_a_stored_case_must_keep_them() {
    let entry = completed(record(&[
        (0.0, 1.0),
        (0.1, 1.0),
        (0.2, 1.0),
        (0.3, 1.0),
        (0.4, 1.0),
    ]));
    let outcome = validation_with_entry(Some(&entry), None);
    let answered = outcome
        .result
        .initialization
        .expect("the entry case is answered");
    assert_eq!(answered.status, RequirementStatus::Pass);
    assert!(
        answered.requirements.iter().all(|row| row.guard),
        "every row of the case is a guard (B9.9)"
    );
    let mut stored = outcome.section.clone();
    if let Some(entry) = stored.initialization.as_mut() {
        for row in &mut entry.requirements {
            row.guard = false;
        }
    }
    assert!(
        stored.check().is_err(),
        "a stored case whose rows are not guards is refused"
    );
}

#[test]
fn a_validation_that_runs_no_entry_case_answers_none() {
    let outcome = validation_with_entry(None, None);
    assert_eq!(outcome.result.initialization, None);
    assert_eq!(outcome.section.initialization, None);
    assert!(outcome.section.check().is_ok());
}

#[test]
fn a_loop_whose_auto_entry_cannot_be_planned_still_validates_its_other_cases() {
    // The fixture declares no initialization for its PID, so Auto is blocked (C1): the case is refused, and the other cases still judge.
    let refusal = plan_entry_case(
        &fixture_loop(),
        &fixture_model(),
        &fixture_task(),
        &BTreeMap::new(),
        0.0,
    )
    .expect_err("the loop has no Auto entry to plan");
    let outcome = validation_with_entry(None, Some(&refusal));
    assert_eq!(
        outcome.result.scenarios.len(),
        1,
        "the target-response case was judged"
    );
    let answered = outcome
        .result
        .initialization
        .clone()
        .expect("the refused case is answered");
    assert_eq!(answered.label, "Manual-to-Auto entry");
    assert_eq!(answered.status, RequirementStatus::Inconclusive);
    assert_eq!(answered.requirements.len(), 1, "the case has one row");
    let row = &answered.requirements[0];
    assert!(row.guard, "the row is a guard (B9.9)");
    assert_eq!(row.status, RequirementStatus::Inconclusive);
    assert_eq!(row.reason, refusal.message, "the row names the refusal");
    assert_eq!(
        outcome.result.status,
        RequirementStatus::Inconclusive,
        "the overall result is at best Inconclusive"
    );
    assert!(
        !outcome.result.apply_available && !outcome.section.apply_available,
        "Apply is unavailable"
    );
    assert_eq!(
        outcome.failure,
        Some(refusal.clone()),
        "the failure is the refusal, with its cause and next action (B5.5)"
    );
    let reopened = outcome.section.result(outcome.result.stage_evidence);
    assert_eq!(
        reopened.failure.map(|failure| failure.message),
        Some(refusal.message.clone()),
        "a reopened validation names the refusal"
    );
    assert!(outcome.section.check().is_ok(), "the section reads back");
}

#[test]
fn the_entry_request_carries_the_candidate_and_the_channels_with_their_units() {
    let loop_definition = fixture_loop();
    let entry = EntryCase {
        case: nominal_case(&plan_of(&loop_definition)),
        extra_observations: vec![IsolatedRunObservation {
            symbol: "Main.Mode".to_owned(),
            unit: "1".to_owned(),
        }],
    };
    let request = entry
        .request(
            &loop_definition,
            &fixture_model(),
            &sample_candidate(2.0, 0.5),
            None,
            "entry-job",
            "rev-1",
        )
        .expect("the request is built");
    assert_eq!(request.request_id, "entry-job");
    assert!(
        request
            .observations
            .iter()
            .any(|observation| observation.symbol == "Main.Mode" && observation.unit == "1"),
        "the mode channel is observed with its unit"
    );
    assert!(
        !request.parameter_overrides.is_empty(),
        "the candidate's record is written as parameter overrides"
    );
}
