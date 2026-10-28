#![allow(clippy::float_cmp)] // Whole milliseconds and reviewed plan values are compared exactly on purpose.

use serde_json::json;

use super::*;
use crate::experiment::{
    ControlEvidenceSource, ExperimentValue, PlanAuxiliarySignal, PlanSignal,
    RecordingRecoveryBinding,
};
use crate::identification::connected_record::connected_identification_record;
use crate::identification::model_comparison::ComparisonPresentation;
use crate::identification::record_fixtures::{fitted_model, setpoint_record, Row, Verdict};
use crate::identification::refusal::Refusal;
use crate::model::GeneralModel;
use crate::recording::fixtures::{binding, hold_loop, loop_definition, run, sample as scan};
use crate::signals::Quality;

/// One signal of a live sample of the fixture loop, good and in percent.
fn signal(symbol: &str, address: &str, value: f64) -> PlanSignal {
    PlanSignal {
        symbol: symbol.to_owned(),
        address: Some(address.to_owned()),
        value,
        unit: "%".to_owned(),
        quality: Quality::Good,
    }
}

/// A live sample of the fixture loop: the setpoint and the measurement at 45 %, the output at 30 %.
fn sample(source: EvidenceSource, mode: ControllerMode) -> ExperimentSample {
    ExperimentSample {
        scan: 7,
        source,
        controller_mode: mode,
        setpoint: Some(signal("LevelSP", "%ML0", 45.0)),
        process_variable: signal("LevelPV", "%IL0", 45.0),
        manipulated_variable: signal("ValveMV", "%QL0", 30.0),
        additional: Vec::new(),
    }
}

/// The steady baseline of the `TypeScript` campaign tests: 45 %, a 30 s window at one sample per second.
fn baseline() -> BaselineSummary {
    BaselineSummary {
        mean: 45.0,
        noise: 0.03,
        half_range: 0.05,
        spacing_seconds: 1.0,
        window_seconds: 30.0,
        input_mean: None,
        input_noise: None,
    }
}

/// The method Automatic chooses for a loop in Auto with a setpoint (B3.3).
fn setpoint_step() -> IdentificationMethod {
    IdentificationMethod {
        kind: ControlExperimentKind::SetpointStep,
        manual: false,
        automatic: true,
    }
}

/// A reviewed setpoint step of the fixture loop.
fn plan() -> ControlExperimentPlan {
    serde_json::from_value(json!({
        "id": "experiment-1", "source": "simulation", "kind": "setpoint_step", "affectedSignal": "LevelSP",
        "address": "%ML0", "valueType": "LREAL", "from": 45, "to": 47, "unit": "%", "minimum": 0, "maximum": 100,
        "durationMs": 570_000, "expectedScan": 7, "task": "$background", "stopConditions": ["Abort"],
        "restorationValue": 45, "requiresPhysicalApproval": false, "trajectory": [{ "atMs": 0, "value": 47 }]
    }))
    .expect("the plan decodes")
}

/// The owner of a recording of `loop_def` on a simulated target.
fn owner(loop_def: &LoopDefinition) -> RecordingRecoveryOwner {
    RecordingRecoveryOwner {
        schema_version: 1,
        recording_id: "recording-1".to_owned(),
        project_id: loop_def.project_id.clone(),
        runtime_id: loop_def.runtime_id.clone(),
        loop_id: loop_def.id.clone(),
        loop_revision: loop_def.revision.clone(),
        source: ControlEvidenceSource::Simulation,
        program_revision: Some("sha256:program".to_owned()),
        start_acknowledgement_unconfirmed: None,
        bindings: vec![RecordingRecoveryBinding {
            id: "processVariable".to_owned(),
            address: Some("%IL0".to_owned()),
            symbol: None,
            iec_type: "LREAL".to_owned(),
        }],
    }
}

/// A model fitted from one recording, with the validation and the output unit given.
fn model_result(
    validation: ModelValidation,
    output_unit: &str,
) -> AutomaticIdentificationResult<Verdict> {
    let mut model = fitted_model(
        vec!["sha256:recording".to_owned()],
        vec![0.5],
        vec![50.0, 1.0],
    );
    if let GeneralModel::TransferFunction { identity, .. } = &mut model {
        identity.validation = validation;
        identity.outputs[0].unit = output_unit.to_owned();
    }
    AutomaticIdentificationResult::Model {
        model: Box::new(model),
        runs: Vec::new(),
        fit: None,
        presentation: shown(),
    }
}

/// What the page shows of the model of these tests: a presentation in its units, without traces.
fn shown() -> Box<ComparisonPresentation> {
    Box::new(
        crate::json::decode_value(serde_json::json!({
            "equation": "G(s) = 1",
            "units": { "input": "%", "output": "degC", "gain": "degC/%", "time": "s" },
            "parameters": [], "poles": [], "zeros": [], "delaySeconds": 0.0,
            "uncertainty": { "label": "Provisional" }, "estimation": {}, "validation": {},
            "predictedResponse": [], "warnings": [],
            "validityRange": { "inputMinimum": 0.0, "inputMaximum": 100.0, "outputMinimum": 0.0, "outputMaximum": 100.0 }
        }))
        .expect("the presentation decodes"),
    )
}

#[test]
fn a_start_refused_before_any_experiment_existed_ends_the_run_and_every_other_answer_is_reconciled()
{
    let started = StartRead::Answer(json!({
        "experiment_id": "experiment-1", "status": "running", "restoration": "pending", "symbol": "LevelSP"
    }));
    assert_eq!(
        start_outcome("experiment-1", &started),
        StartOutcome::Started
    );
    let another = StartRead::Answer(json!({
        "experiment_id": "experiment-2", "status": "running", "restoration": "pending", "symbol": "LevelSP"
    }));
    assert_eq!(
        start_outcome("experiment-1", &another),
        StartOutcome::Unknown,
        "an answer for another experiment never confirms the start"
    );
    let refused = StartRead::Refused {
        code: Some("outside_limits".to_owned()),
        text: "The value 120 lies outside 0 to 100.".to_owned(),
    };
    let StartOutcome::Refused(failure) = start_outcome("experiment-1", &refused) else {
        panic!("a refusal before any experiment existed ends the run");
    };
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Runtime, NextAction::OpenLimits)
    );
    for read in [
        StartRead::Refused {
            code: Some("transaction_pending".to_owned()),
            text: "A transaction is pending.".to_owned(),
        },
        StartRead::Refused {
            code: None,
            text: "Refused.".to_owned(),
        },
        StartRead::Unreadable("the start acknowledgement was lost".to_owned()),
    ] {
        assert_eq!(
            start_outcome("experiment-1", &read),
            StartOutcome::Unknown,
            "{read:?}"
        );
    }
}

#[test]
fn an_unknown_start_follows_an_experiment_the_runtime_holds_and_says_why_otherwise() {
    let running = ExperimentIdentity::Running {
        state: json!({ "status": "running", "restoration": "pending" }),
    };
    assert_eq!(start_reconciliation(&running), StartReconciliation::Follow);
    let ended = ExperimentIdentity::Terminal {
        state: json!({ "status": "completed", "restoration": "restored" }),
    };
    assert_eq!(start_reconciliation(&ended), StartReconciliation::Follow);
    let absent = ExperimentIdentity::Absent {
        reason: "No experiment holds this identifier.".to_owned(),
    };
    let StartReconciliation::Absent(failure) = start_reconciliation(&absent) else {
        panic!("an absent identity never ran");
    };
    assert_eq!(
        failure.detail.as_deref(),
        Some("The experiment start was not confirmed, and the runtime holds no experiment with its identity, so none ran. No experiment holds this identifier.")
    );
    let unresolved = ExperimentIdentity::Unresolved {
        reason: "The status of experiment experiment-1 could not be read: transport lost."
            .to_owned(),
    };
    let StartReconciliation::Unresolved(failure) = start_reconciliation(&unresolved) else {
        panic!("an unreadable status stays unresolved");
    };
    assert_eq!(
        failure.detail.as_deref(),
        Some("The experiment start was not confirmed and its status cannot be read. The status of experiment experiment-1 could not be read: transport lost. The start is not sent again.")
    );
}

#[test]
fn the_step_holds_the_maximum_after_the_baseline_less_one_window_and_a_hold_under_a_second_is_refused(
) {
    // A 600 s maximum, a recording of the 300 s baseline budget and the maximum, and a 30 s window (5 % of the maximum).
    let window = js_round(baseline().window_seconds * 1_000.0);
    let hold = step_hold_ms(600_000, 900_000, window, 30.0, 30.0, 1);
    assert_eq!(hold, 570_000.0);
    assert_eq!(
        step_hold_ms(600_000, 900_000, window, 200.0, 200.0, 1),
        570_000.0,
        "a long wait for a steady start never shortens the step"
    );
    assert_eq!(
        step_hold_ms(600_000, 900_000, window, 400.0, 400.0, 1),
        470_000.0,
        "the step ends within the recording"
    );
    assert_eq!(hold_refusal(hold), None);
    let refusal = hold_refusal(500.0).expect("half a second is too short");
    assert_eq!(
        refusal.detail.as_deref(),
        Some("The maximum experiment duration leaves no time to hold the step after the baseline and its restoration (1 s free). Open Limits… for a longer maximum duration.")
    );
    assert_eq!(
        (refusal.cause, refusal.next_action),
        (Cause::Settings, NextAction::OpenLimits)
    );
    let none_left = hold_refusal(-3_000.0).expect("no time is too short");
    assert!(
        none_left.message.contains("(0 s free)"),
        "{}",
        none_left.message
    );
}

#[test]
fn the_recording_follows_a_declared_settling_time_or_a_tenth_of_the_maximum() {
    assert_eq!(
        recording_settling_ms(600_000, None),
        Some(60_000),
        "a tenth of a 600 s maximum"
    );
    assert_eq!(recording_settling_ms(600_000, Some(12.5)), Some(12_500));
    assert_eq!(
        recording_settling_ms(600_000, Some(0.000_4)),
        None,
        "under a millisecond the sample limit alone sets the record period"
    );
    assert_eq!(recording_settling_ms(600_000, Some(f64::NAN)), None);
}

#[test]
fn the_model_check_uses_the_next_seed_and_the_plan_keeps_its_low_32_bits() {
    let mut loop_def = loop_definition();
    assert_eq!(
        step_seed(&loop_def, false),
        1,
        "a loop without an experiment has seed 1"
    );
    loop_def.experiment = Some(
        serde_json::from_value(json!({
            "execution": "control-studio", "kind": "mv_step", "amplitude": 5, "durationSeconds": 600,
            "seed": 4_294_967_295_u64
        }))
        .expect("the experiment decodes"),
    );
    assert_eq!(step_seed(&loop_def, false), u32::MAX);
    assert_eq!(
        step_seed(&loop_def, true),
        0,
        "2^32 keeps no low bits; the plan then uses 1"
    );
}

#[test]
fn a_physical_target_and_a_downward_relay_are_refused_before_anything_is_recorded() {
    let loop_def = loop_definition();
    let physical = sample(EvidenceSource::PhysicalPlant, ControllerMode::Auto);
    let refusal = campaign_refusal(&loop_def, setpoint_step(), Some(&physical))
        .expect("a physical target is refused");
    assert_eq!(refusal.message, PHYSICAL_TARGET_TEXT);
    assert_eq!(
        (refusal.cause, refusal.next_action),
        (Cause::Approval, NextAction::None)
    );
    let simulated = sample(EvidenceSource::SimulatedPlant, ControllerMode::Manual);
    let relay = IdentificationMethod {
        kind: ControlExperimentKind::Relay,
        manual: true,
        automatic: false,
    };
    assert_eq!(
        campaign_refusal(&loop_def, relay, Some(&simulated)),
        None,
        "a relay that starts upward runs"
    );
    let mut downward = loop_definition();
    downward.experiment = Some(
        serde_json::from_value(json!({
            "execution": "control-studio", "kind": "relay", "amplitude": 5, "relayDirection": "negative",
            "durationSeconds": 600, "seed": 1
        }))
        .expect("the experiment decodes"),
    );
    assert_eq!(
        campaign_refusal(&downward, relay, Some(&simulated)).map(|failure| failure.message),
        Some(NEGATIVE_RELAY_TEXT.to_owned())
    );
    assert!(offers_setpoint_step(
        &loop_def,
        Some(&sample(
            EvidenceSource::SimulatedPlant,
            ControllerMode::Auto
        ))
    ));
    assert!(!offers_setpoint_step(&loop_def, Some(&simulated)));
    assert!(!offers_setpoint_step(&loop_def, None));
}

#[test]
fn a_project_scripted_loop_with_a_setpoint_is_never_offered_a_setpoint_step() {
    // C12: the project owns the experiment and Control Studio writes no setpoint, so a wait for settling names no setpoint step,
    // even on a loop that names its setpoint.
    let mut observed = loop_definition();
    observed.experiment = Some(
        serde_json::from_value(json!({
            "execution": "project-scripted", "kind": "observation", "amplitude": 0,
            "durationSeconds": 120, "seed": 1
        }))
        .expect("the experiment decodes"),
    );
    let auto = sample(EvidenceSource::SimulatedPlant, ControllerMode::Auto);
    assert!(
        observed.signals.setpoint.is_some(),
        "the fixture names its setpoint"
    );
    assert!(!offers_setpoint_step(&observed, Some(&auto)));
    assert!(offers_setpoint_step(&loop_definition(), Some(&auto)));
}

#[test]
fn a_live_write_on_a_physical_target_needs_the_engineers_approval() {
    assert_eq!(
        live_write_refusal(EvidenceSource::SimulatedPlant, IDENTIFY_OPERATION),
        None
    );
    let refusal =
        live_write_refusal(EvidenceSource::PhysicalPlant, IDENTIFY_OPERATION).expect("refused");
    assert_eq!(
        refusal.message,
        "Physical approval is required for automatic identification campaign on a physical target."
    );
    assert_eq!(
        (refusal.cause, refusal.next_action),
        (Cause::Approval, NextAction::Approve)
    );
}

#[test]
fn an_approved_physical_campaign_passes_its_refusal_and_an_unapproved_one_does_not() {
    let loop_def = loop_definition();
    let physical = sample(EvidenceSource::PhysicalPlant, ControllerMode::Auto);
    assert_eq!(
        approved_campaign_refusal(&loop_def, setpoint_step(), Some(&physical), true),
        None,
        "one approval covers the reviewed campaign on a physical target"
    );
    let refusal = approved_campaign_refusal(&loop_def, setpoint_step(), Some(&physical), false)
        .expect("a physical target without approval is refused");
    assert_eq!(refusal.message, PHYSICAL_TARGET_TEXT);
    assert_eq!(
        campaign_refusal(&loop_def, setpoint_step(), Some(&physical))
            .map(|failure| failure.message),
        Some(PHYSICAL_TARGET_TEXT.to_owned()),
        "a run without an approval is refused as before"
    );
}

#[test]
fn an_approved_physical_setpoint_step_is_planned_on_the_physical_target_and_an_unapproved_one_is_not(
) {
    let loop_def = loop_definition();
    let physical = sample(EvidenceSource::PhysicalPlant, ControllerMode::Auto);
    let planned = approved_step_plan(
        &loop_def,
        Some(&physical),
        setpoint_step(),
        &baseline(),
        570_000.0,
        false,
        true,
        None,
    )
    .expect("the approved physical setpoint step is planned");
    assert_eq!(planned.source, PlanSource::Physical);
    assert!(
        planned.requires_physical_approval,
        "the start must carry the runtime's approval"
    );
    assert!(
        approved_step_plan(
            &loop_def,
            Some(&physical),
            setpoint_step(),
            &baseline(),
            570_000.0,
            false,
            false,
            None,
        )
        .is_err(),
        "a physical plan without the approval is refused"
    );
}

#[test]
fn a_start_whose_review_changed_is_refused_with_no_next_action() {
    let refusal = review_changed_refusal();
    assert_eq!(
        refusal.message,
        "The campaign changed since it was reviewed: its loop, its mode or its settings differ."
    );
    assert_eq!(refusal.detail.as_deref(), Some(REVIEW_CHANGED_TEXT));
    assert_eq!(
        (refusal.cause, refusal.next_action),
        (Cause::Approval, NextAction::None)
    );
}

#[test]
fn a_hold_completes_only_when_its_command_is_restored_after_its_planned_end() {
    let finished = json!({ "status": "completed", "restoration": "restored", "stop_reason": "finished_early" });
    assert_eq!(hold_end(&finished), HoldEnd::Completed);
    assert!(ended_and_restored(&finished));
    let aborted =
        json!({ "status": "aborted", "restoration": "restored", "stop_reason": "user_abort" });
    let HoldEnd::Stopped(stopped) = hold_end(&aborted) else {
        panic!("an aborted hold stopped early");
    };
    assert_eq!(
        stopped.message,
        "The experiment stopped before its hold ended: it was aborted."
    );
    assert!(
        ended_and_restored(&aborted),
        "an aborted experiment that restored has ended"
    );
    let unrestored = json!({ "status": "failed_to_restore", "restoration": "failed", "stop_reason": "duration_complete" });
    assert_eq!(
        hold_end(&unrestored),
        HoldEnd::Unrestored(Failure::new(
            UNRESTORED_TEXT,
            Cause::Unclassified,
            NextAction::None
        ))
    );
    assert!(!ended_and_restored(&unrestored));
    assert!(
        !ended_and_restored(
            &json!({ "status": "completed", "restoration": "pending", "stop_reason": "duration_complete" })
        ),
        "a pending restoration has not ended"
    );
}

#[test]
fn cancel_aborts_a_running_experiment_confirms_an_ended_one_and_leaves_an_absent_one() {
    let running = ExperimentIdentity::Running {
        state: json!({ "status": "running", "restoration": "pending" }),
    };
    assert_eq!(cancel_step(&running), CancelStep::Abort);
    let ended = ExperimentIdentity::Terminal {
        state: json!({ "status": "aborted", "restoration": "restored" }),
    };
    assert_eq!(cancel_step(&ended), CancelStep::Confirm);
    let absent = ExperimentIdentity::Absent {
        reason: "No experiment holds this identifier.".to_owned(),
    };
    assert_eq!(cancel_step(&absent), CancelStep::Nothing);
    let unresolved = ExperimentIdentity::Unresolved {
        reason: "The status of experiment experiment-1 could not be read: transport lost."
            .to_owned(),
    };
    assert_eq!(
        cancel_step(&unresolved),
        CancelStep::Unconfirmed(restoration_unconfirmed())
    );
}

#[test]
fn the_journaled_experiment_is_its_plan_with_its_recording_and_a_foreign_recording_is_refused() {
    let loop_def = loop_definition();
    let recovery = experiment_recovery(&loop_def, &plan(), &owner(&loop_def))
        .expect("the record decodes for its loop");
    assert_eq!(
        (recovery.schema_version, recovery.experiment_id.as_str()),
        (2, "experiment-1")
    );
    let mut foreign = owner(&loop_def);
    foreign.loop_revision = "sha256:loop-2".to_owned();
    let refusal = experiment_recovery(&loop_def, &plan(), &foreign)
        .expect_err("another revision is outside the loop's scope");
    assert_eq!(refusal.message, EXPERIMENT_RECOVERY_TEXT);
}

#[test]
fn a_model_fitted_from_the_runs_own_recording_is_provisional_and_must_qualify_for_the_loop() {
    let provisional = model_result(ModelValidation::Unvalidated, "%");
    assert!(is_provisional(&provisional));
    assert!(!is_provisional(&model_result(
        ModelValidation::Validated,
        "%"
    )));
    assert_eq!(qualification(&loop_definition(), &provisional), Ok(()));
    let other_unit = model_result(ModelValidation::Unvalidated, "degC");
    let failure =
        qualification(&loop_definition(), &other_unit).expect_err("the output unit differs");
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::ModelOrData, NextAction::Identify)
    );
}

#[test]
fn a_missing_output_limit_opens_the_limits_and_a_model_that_misses_them_is_identified_again() {
    // B3.9: a missing or invalid limit is refused as the precondition refuses it, and Limits opens there. A model whose input
    // does not meet the actuator range that valid limits give is identified again.
    let mut without_maximum = loop_definition();
    without_maximum.limits.output_maximum = None;
    let failure = qualification(
        &without_maximum,
        &model_result(ModelValidation::Unvalidated, "%"),
    )
    .expect_err("the output maximum is missing");
    assert_eq!(
        failure.message,
        "The plant input limits are missing: limits.outputMaximum must be set, with the minimum below the maximum."
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Settings, NextAction::OpenLimits)
    );
    let mut elsewhere = loop_definition();
    elsewhere.limits.output_minimum = Some(200.0);
    elsewhere.limits.output_maximum = Some(300.0);
    let failure = qualification(&elsewhere, &model_result(ModelValidation::Unvalidated, "%"))
        .expect_err("the model's input does not meet the limits");
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::ModelOrData, NextAction::Identify)
    );
}

#[test]
fn a_record_whose_evaluation_does_not_pass_is_refused() {
    let channels = [("LevelPV", "%"), ("ValveMV", "%")];
    let rows: Vec<Row> = vec![
        (0.0, 10, vec![Some(45.0), Some(30.0)]),
        (1.0, 20, vec![Some(45.0), Some(30.0)]),
    ];
    let passed = setpoint_record("recording-1", &channels, &rows, RequirementStatus::Pass);
    assert_eq!(evaluation_refusal(&passed), None);
    let failed = setpoint_record("recording-1", &channels, &rows, RequirementStatus::Fail);
    // The message is the first sentence; the detail is the whole text (`Failure::new`).
    assert_eq!(
        evaluation_refusal(&failed).and_then(|failure| failure.detail),
        Some(format!(
            "{EVALUATION_TEXT} The recording is kept and is not qualified for a fit."
        ))
    );
}

#[test]
fn a_fit_without_a_model_keeps_each_attempt_and_a_refusal_keeps_its_sentence() {
    let attempts = RecordedFitError::StateModelRequired {
        attempts: vec![
            "Step-response fit: the response did not settle.".to_owned(),
            "Excitation fit: no form met its error limit.".to_owned(),
        ],
    };
    let failure = fit_failure(&loop_definition(), &attempts);
    assert_eq!(
        failure.message,
        "Step-response fit: the response did not settle."
    );
    assert_eq!(
        failure.detail.as_deref(),
        Some("Step-response fit: the response did not settle. Excitation fit: no form met its error limit.")
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::ModelOrData, NextAction::ChooseMethod)
    );
    let refused = fit_failure(
        &loop_definition(),
        &RecordedFitError::Refused(Refusal::new(
            "The response is smaller than 20 times the measured noise.",
        )),
    );
    assert_eq!(
        refused.message,
        "The response is smaller than 20 times the measured noise."
    );
}

// N2.6, B3.9: a record without an input change is refused as the missing nonzero step, and another method, a larger step or a
// longer duration is the fix. Any other refusal keeps its sentence and offers no action.
#[test]
fn a_record_without_an_input_change_offers_another_method_and_another_refusal_offers_none() {
    let missing = fit_failure(
        &loop_definition(),
        &RecordedFitError::Refused(Refusal::new(MISSING_NONZERO_STEP)),
    );
    assert_eq!(
        missing.message,
        "The recording is missing a nonzero step: the plant input never changes, so there is no input change to fit."
    );
    assert_eq!(missing.detail.as_deref(), Some(MISSING_NONZERO_STEP));
    assert_eq!(
        (missing.cause, missing.next_action),
        (Cause::Unclassified, NextAction::ChooseMethod)
    );
    let other = fit_failure(
        &loop_definition(),
        &RecordedFitError::Refused(Refusal::new(
            "The response is smaller than 20 times the measured noise.",
        )),
    );
    assert_eq!(other.next_action, NextAction::None);
}

// C12, B3.9: a project-scripted loop takes the project's scenario whatever the method, so another method is refused there. Its
// missing nonzero step therefore opens the loop setup, to check the project's scenario, and not the method choice.
#[test]
fn a_project_scripted_loop_without_an_input_change_opens_its_setup_instead_of_another_method() {
    let mut scripted = loop_definition();
    scripted.experiment = Some(
        serde_json::from_value(json!({
            "execution": "project-scripted",
            "kind": "observation",
            "amplitude": 0.0,
            "durationSeconds": 30.0,
            "seed": 1
        }))
        .expect("the project-scripted experiment decodes"),
    );
    let missing = fit_failure(
        &scripted,
        &RecordedFitError::Refused(Refusal::new(MISSING_NONZERO_STEP)),
    );
    assert_eq!(
        (missing.cause, missing.next_action),
        (Cause::Unclassified, NextAction::OpenLoopSetup)
    );
    // No adequate model (B3.9): the same rule, because another method is refused on a project-scripted loop (C12).
    let inadequate = fit_failure(
        &scripted,
        &RecordedFitError::StateModelRequired {
            attempts: vec!["Excitation fit: no form is within the limit.".to_owned()],
        },
    );
    assert_eq!(
        (inadequate.cause, inadequate.next_action),
        (Cause::ModelOrData, NextAction::OpenLoopSetup)
    );
    let ordinary = fit_failure(
        &loop_definition(),
        &RecordedFitError::StateModelRequired {
            attempts: vec!["Excitation fit: no form is within the limit.".to_owned()],
        },
    );
    assert_eq!(ordinary.next_action, NextAction::ChooseMethod);
}

#[test]
fn acquisition_settings_that_do_not_decode_are_refused_with_limits_as_the_fix() {
    let refusal = checked_acquisition_settings(
        &loop_definition(),
        &json!({
            "method": "automatic", "durationSeconds": 0.5, "amplitude": 0, "reuseRecordings": false
        }),
    )
    .expect_err("half a second is outside 1 s to 24 h");
    assert_eq!(refusal.message, ACQUISITION_SETTINGS_TEXT);
    assert_eq!(refusal.next_action, NextAction::OpenLimits);
    let settings = checked_acquisition_settings(
        &loop_definition(),
        &json!({
            "method": "automatic", "durationSeconds": 600, "amplitude": 0, "reuseRecordings": false
        }),
    )
    .expect("valid settings");
    assert_eq!(settings.duration_seconds, 600.0);
}

// B3.4 step 1, B3.9: a loop that declares no duration gives its settings a duration of zero, and the refusal names the missing
// field, with Limits as the next action. Settings that give a duration are the engineer's; a loop that declares one keeps the
// settings' sentence for a zero duration.
#[test]
fn settings_with_no_duration_on_a_loop_that_declares_none_name_the_missing_field() {
    let zero = json!({
        "method": "automatic", "durationSeconds": 0.0, "amplitude": 0, "reuseRecordings": false
    });
    let refusal = checked_acquisition_settings(&loop_definition(), &zero)
        .expect_err("no duration is declared or given");
    assert_eq!(
        refusal.message,
        "The maximum experiment duration is missing: experiment.durationSeconds must be set, between 1 s and 24 h."
    );
    assert_eq!(
        (refusal.cause, refusal.next_action),
        (Cause::Settings, NextAction::OpenLimits)
    );
    let given = json!({
        "method": "automatic", "durationSeconds": 600, "amplitude": 0, "reuseRecordings": false
    });
    let settings = checked_acquisition_settings(&loop_definition(), &given)
        .expect("the settings give the duration");
    assert_eq!(settings.duration_seconds, 600.0);
    let declared = checked_acquisition_settings(&hold_loop(), &zero)
        .expect_err("the loop declares 45.1 s, so the settings' own sentence stands");
    assert_eq!(declared.message, ACQUISITION_SETTINGS_TEXT);
    assert_eq!(declared.next_action, NextAction::OpenLimits);
}

// B3.2: a range the loop does not allow is refused under the field of Limits that holds it, not under the generic sentence.
#[test]
fn a_range_the_loop_does_not_allow_is_refused_under_its_field_on_identify() {
    let refusal = checked_acquisition_settings(
        &loop_definition(),
        &json!({
            "method": "automatic", "durationSeconds": 600, "amplitude": 0, "reuseRecordings": false,
            "inputRange": { "minimum": 0, "maximum": 150 }
        }),
    )
    .expect_err("150 % is beyond the declared 100 %");
    assert_eq!(refusal.field.as_deref(), Some("inputMaximum"));
    assert_eq!(refusal.next_action, NextAction::OpenLimits);
}

#[test]
fn the_step_plan_is_the_reviewed_setpoint_step_held_for_the_hold() {
    let loop_def = loop_definition();
    let live = sample(EvidenceSource::SimulatedPlant, ControllerMode::Auto);
    let reviewed = step_plan(
        &loop_def,
        Some(&live),
        setpoint_step(),
        &baseline(),
        570_000.0,
        false,
        None,
    )
    .expect("a setpoint step in Auto");
    assert_eq!(reviewed.kind, ControlExperimentKind::SetpointStep);
    assert_eq!(reviewed.source, PlanSource::Simulation);
    assert_eq!(reviewed.duration_ms, 570_000.0);
    let refusal = step_plan(
        &loop_def,
        None,
        setpoint_step(),
        &baseline(),
        570_000.0,
        false,
        None,
    )
    .expect_err("no sample, no step");
    assert_eq!(
        refusal.message,
        "Fresh synchronized SP, PV, and MV are required."
    );
}

#[test]
fn the_fit_starts_one_window_before_the_baseline_end_and_never_before_the_recording() {
    assert_eq!(
        fit_window(92.0, 30.0),
        FitWindow {
            fit_from_seconds: 62.0,
            baseline_end_seconds: 92.0
        }
    );
    assert_eq!(fit_window(12.0, 30.0).fit_from_seconds, 0.0);
}

// B3.4, N1.3 and B3.6: a refit starts where the live fit started. The live run takes its window from the baseline's end and
// length, its connected record writes the window into the request, and the stored window reads the same window back.
#[test]
fn the_stored_fit_window_is_the_window_the_live_fit_used() {
    let recorded = run(
        vec![
            binding("setpoint", "%ML0", "LREAL"),
            binding("processVariable", "%IL0", "LREAL"),
            binding("manipulatedVariable", "%QL0", "LREAL"),
        ],
        vec![
            scan(
                11,
                Some(2_000_000_000),
                &[Some(45.0), Some(45.0), Some(30.0)],
                &[Quality::Good; 3],
            ),
            scan(
                12,
                Some(2_010_000_000),
                &[Some(45.0), Some(45.5), Some(31.0)],
                &[Quality::Good; 3],
            ),
        ],
    );
    for (baseline_end, window_seconds) in [(42.5, 30.0), (20.0, 30.0), (0.0, 30.0), (10.0, 0.0)] {
        let live = fit_window(baseline_end, window_seconds);
        let record = connected_identification_record(
            &loop_definition(),
            &recorded,
            None,
            &plan(),
            live.fit_from_seconds,
            Some(live.baseline_end_seconds),
            |_, _| Ok::<_, Failure>(Verdict(RequirementStatus::Pass)),
        )
        .expect("the complete recording is a record");
        assert_eq!(
            stored_fit_window(&record.request),
            live,
            "a baseline ending at {baseline_end} s after a {window_seconds} s window"
        );
    }
}

// A8.4 and A8.5: nothing starts beside a journaled operation, and the refusal names it.
#[test]
fn identify_is_refused_while_the_loops_journal_holds_an_operation() {
    assert_eq!(unfinished_operation_refusal(&[]), None);
    let refusal = unfinished_operation_refusal(&["recording-7", "experiment-1"]).expect("refused");
    assert_eq!(refusal.message, "This loop has an unfinished operation.");
    assert_eq!(
        refusal.detail.as_deref(),
        Some("This loop has an unfinished operation. Wait for it to end, or resume or cancel it. Journaled: recording-7, experiment-1.")
    );
    assert_eq!(
        (refusal.cause, refusal.next_action),
        (Cause::Unclassified, NextAction::Resume)
    );
}

// §0.1b, D6: a Cancel names an operation that the loop's journal holds; any other is refused with the identifier.
#[test]
fn a_cancel_of_an_operation_the_journal_does_not_hold_is_refused_by_its_identifier() {
    let failure = not_journaled("tank", "operation-1");
    assert_eq!(
        failure.message,
        "Operation operation-1 is not journaled for loop tank."
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Settings, NextAction::None)
    );
}

// E11.1: a live action, or a resume, without a runtime connection asks for the runtime to be connected first.
#[test]
fn a_live_action_without_a_runtime_connection_asks_to_connect_the_runtime() {
    let refusal = runtime_required();
    assert_eq!(
        refusal.message,
        "Start the runtime before running a live action."
    );
    assert_eq!(refusal.detail, None);
    assert_eq!(
        (refusal.cause, refusal.next_action),
        (Cause::Runtime, NextAction::ConnectRuntime)
    );
}

// The engine's own wiring failures carry their sentences and no action: no input of the user causes them (Cause::Implementation).
#[test]
fn the_wiring_failures_of_the_engine_keep_their_sentences_and_their_implementation_cause() {
    let wiring = [
        (
            model_check_instead_of_model(),
            "The identification recorded a model check instead of fitting a model.",
        ),
        (
            resumed_without_experiment(),
            "The resumed campaign's journal named no experiment, so its method is unknown.",
        ),
        (
            accepted_model_from_one_record(),
            "A fit of one recording gave an accepted model, which takes two independent recordings.",
        ),
        (
            model_not_from_one_record(),
            "The model was not fitted from one recording, so the study cannot keep the request it was taken with.",
        ),
        (
            recording_not_kept("rec-7"),
            "The recording rec-7 was not kept in the study.",
        ),
    ];
    for (failure, sentence) in wiring {
        assert_eq!(failure.message, sentence);
        assert_eq!(
            (failure.cause, failure.next_action),
            (Cause::Implementation, NextAction::None),
            "{sentence}"
        );
    }
}

// B3.4 step 1, B3.9: a loop without a plant input range opens Limits at the field, and a step is never planned on it.
#[test]
fn a_step_on_a_loop_without_a_plant_input_range_is_refused_with_open_limits() {
    let mut without_minimum = loop_definition();
    without_minimum.limits.output_minimum = None;
    let refusal = plant_input_range(&without_minimum).expect_err("the minimum is missing");
    assert_eq!(
        refusal.message,
        "The plant input limits are missing: limits.outputMinimum must be set, with the minimum below the maximum."
    );
    assert_eq!(
        (refusal.cause, refusal.next_action),
        (Cause::Settings, NextAction::OpenLimits)
    );
    let planned = step_plan(
        &without_minimum,
        Some(&sample(
            EvidenceSource::SimulatedPlant,
            ControllerMode::Auto,
        )),
        setpoint_step(),
        &baseline(),
        30_000.0,
        false,
        None,
    );
    assert_eq!(planned.expect_err("no step is planned"), refusal);
    // A range whose minimum is not below its maximum is the same refusal: the step has no range to size itself in.
    let mut unordered = loop_definition();
    unordered.limits.output_minimum = Some(100.0);
    let planned = step_plan(
        &unordered,
        Some(&sample(
            EvidenceSource::SimulatedPlant,
            ControllerMode::Auto,
        )),
        setpoint_step(),
        &baseline(),
        30_000.0,
        false,
        None,
    );
    assert_eq!(
        planned.expect_err("no step is planned").message,
        "The plant input limits are missing: limits.outputMinimum and limits.outputMaximum must be set, with the minimum below the maximum."
    );
}

// Section 0.1b and the layer contract's D6: a hold that the engineer's Cancel aborted ends as the cancellation, so the verb
// ends as interrupted (130); a hold that stopped for another reason keeps its failure (1).
#[test]
fn a_hold_stopped_by_the_engineers_cancel_is_the_cancellation() {
    let aborted =
        json!({ "status": "aborted", "restoration": "restored", "stop_reason": "user_abort" });
    let HoldEnd::Stopped(stopped) = hold_end(&aborted) else {
        panic!("an aborted hold stopped early");
    };
    assert_eq!(
        stopped_hold_outcome(true, stopped.clone()),
        identification_cancelled()
    );
    assert_eq!(stopped_hold_outcome(false, stopped.clone()), stopped);
}

// A8.5, B3.9: one operation per loop at a time in one engine; the refusal names the running operation and waits for it to end.
#[test]
fn a_loop_with_an_operation_running_refuses_another_and_names_it_and_offers_to_wait() {
    let refusal = loop_busy_refusal("Identify plant");
    assert_eq!(
        refusal.message,
        "Identify plant is running in this engine on this loop."
    );
    assert_eq!(
        refusal.detail.as_deref(),
        Some("Identify plant is running in this engine on this loop. Wait for it to end, then try again.")
    );
    assert_eq!(
        (refusal.cause, refusal.next_action),
        (Cause::Unclassified, NextAction::Wait)
    );
}

/// The tank with two plant inputs (C7, C8), declared as lesson 84 declares them: the valve, which is the manipulated variable, and a
/// measured load (`P1.LoadKw`, 0 to 50 kW) stepped through its own Manual command. Each input has an identification step of 10.
fn two_input_loop() -> LoopDefinition {
    serde_json::from_value(json!({
        "schemaVersion": 2, "id": "heater", "name": "Heater", "revision": "sha256:heater",
        "projectId": "project", "runtimeId": "runtime",
        "controller": {
            "instance": "P1.Controller", "type": "Control.PID", "revision": "sha256:controller",
            "action": "direct", "taskPeriodMs": 100.0,
            "manualOutputSignal": "P1.ManualPct", "manualOutputType": "LREAL"
        },
        "observedSampleTiming": { "nominalPeriodMs": 100.0 },
        "signals": {
            "setpoint": { "symbol": "LevelSP", "iecType": "LREAL", "direction": "read", "unit": "%",
                "range": { "minimum": 0.0, "maximum": 100.0 } },
            "processVariable": { "symbol": "LevelPV", "iecType": "LREAL", "direction": "read", "unit": "%",
                "range": { "minimum": 0.0, "maximum": 100.0 } },
            "manipulatedVariable": { "symbol": "ValveMV", "iecType": "LREAL", "direction": "write", "unit": "%",
                "range": { "minimum": 0.0, "maximum": 100.0 } },
            "additional": [{
                "id": "load", "channel": "load", "role": "disturbance", "symbol": "P1.LoadKw", "iecType": "LREAL",
                "direction": "read", "unit": "kW", "range": { "minimum": 0.0, "maximum": 50.0 }
            }]
        },
        "limits": { "outputMinimum": 0.0, "outputMaximum": 100.0 },
        "experiment": { "execution": "control-studio", "kind": "mv_step", "amplitude": 10.0,
            "durationSeconds": 600.0, "seed": 1 },
        "channels": {
            "inputs": [
                { "modelChannelId": "valve", "signalId": "manipulatedVariable", "manualOutputSignal": "P1.ManualPct",
                  "manualOutputType": "LREAL", "actuator": { "kind": "continuous", "minimum": 0.0, "maximum": 100.0 },
                  "experimentStep": 10.0 },
                { "modelChannelId": "load", "signalId": "load", "manualOutputSignal": "P1.LoadKw",
                  "manualOutputType": "LREAL", "actuator": { "kind": "continuous", "minimum": 0.0, "maximum": 50.0 },
                  "experimentStep": 10.0 }
            ],
            "outputs": [{ "modelChannelId": "level", "signalId": "processVariable", "setpointSignalId": "setpoint" }]
        }
    }))
    .expect("the two-input loop decodes")
}

/// A live sample of the two-input loop in Manual: the valve at 30 %, the level at 45 % and the measured load at 20 kW.
fn two_input_sample() -> ExperimentSample {
    let mut live = sample(EvidenceSource::SimulatedPlant, ControllerMode::Manual);
    live.additional.push(PlanAuxiliarySignal {
        id: "load".to_owned(),
        signal: PlanSignal {
            symbol: "P1.LoadKw".to_owned(),
            address: None,
            value: 20.0,
            unit: "kW".to_owned(),
            quality: Quality::Good,
        },
    });
    live
}

/// The output-step method the two-input loop declares.
fn output_step() -> IdentificationMethod {
    IdentificationMethod {
        kind: ControlExperimentKind::MvStep,
        manual: true,
        automatic: false,
    }
}

// C7, C8: a campaign of several plant inputs steps them in input order. The valve is stepped on the loop itself; the load is stepped
// through its own Manual command, with the address the runtime's I/O list gives it and its own identification step.
#[test]
fn a_two_input_campaign_steps_the_valve_then_the_load_each_through_its_own_command() {
    let loop_definition = two_input_loop();
    let io = json!({ "inputs": [], "outputs": [], "memory": [{ "name": "p1.loadkw", "address": "%MD4" }] });
    let steps = input_steps(&loop_definition, &io).expect("two inputs step in order");
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0].input_symbol, "ValveMV");
    assert!(steps[0].manipulated);
    assert_eq!(steps[1].input_symbol, "P1.LoadKw");
    assert!(!steps[1].manipulated);
    let live = two_input_sample();
    let valve_sample =
        step_sample(&steps[0], Some(live.clone())).expect("the valve is the loop's own input");
    let first = step_plan(
        &steps[0].loop_definition,
        valve_sample.as_ref(),
        output_step(),
        &baseline(),
        300_000.0,
        false,
        None,
    )
    .expect("the valve steps");
    assert_eq!(first.affected_signal, "P1.ManualPct");
    assert_eq!(
        first.address, None,
        "the loop's own command has no address here"
    );
    let load_sample =
        step_sample(&steps[1], Some(live)).expect("the load is measured, so it is its sample");
    let second = step_plan(
        &steps[1].loop_definition,
        load_sample.as_ref(),
        output_step(),
        &baseline(),
        300_000.0,
        false,
        None,
    )
    .expect("the load steps through its own command");
    assert_eq!(second.affected_signal, "P1.LoadKw");
    assert_eq!(second.address.as_deref(), Some("%MD4"));
    assert_eq!(second.value_type, "LREAL");
    assert_eq!((second.minimum, second.maximum), (0.0, 50.0));
    let ExperimentValue::Number(from) = second.from else {
        panic!("the load starts from a number");
    };
    let ExperimentValue::Number(to) = second.to else {
        panic!("the load moves to a number");
    };
    assert_eq!((from, to), (20.0, 30.0), "the load's own step of 10 kW");
    assert_eq!(
        steps[0].loop_definition.experiment.as_ref().map(|e| e.seed),
        Some(1),
        "the first input keeps the loop's seed"
    );
    assert_eq!(
        steps[1].loop_definition.experiment.as_ref().map(|e| e.seed),
        Some(1 + 7_919),
        "the second input has its own seed"
    );
    assert!(several_channels(&loop_definition));
}

#[test]
fn a_load_without_a_manual_address_is_stepped_by_name_and_a_missing_measurement_is_refused() {
    let loop_definition = two_input_loop();
    let steps = input_steps(
        &loop_definition,
        &json!({ "inputs": [], "outputs": [], "memory": [] }),
    )
    .expect("the load steps by name");
    assert_eq!(
        steps[1].loop_definition.controller.manual_output_address,
        None
    );
    let refusal = step_sample(
        &steps[1],
        Some(sample(
            EvidenceSource::SimulatedPlant,
            ControllerMode::Manual,
        )),
    )
    .expect_err("the load has no current measurement");
    assert_eq!(refusal.message, "No current measured input for P1.LoadKw.");
}

#[test]
fn the_steps_of_several_inputs_share_the_time_left_and_a_single_input_is_stepped_as_it_is() {
    // Two inputs share the 600 s maximum after a 10 s baseline wait and the 30 s window, within a 620 s recording.
    let window = js_round(baseline().window_seconds * 1_000.0);
    assert_eq!(
        step_hold_ms(600_000, 620_000, window, 30.0, 40.0, 2),
        260_000.0,
        "half of what is left, less one window"
    );
    assert_eq!(
        step_hold_ms(600_000, 620_000, window, 30.0, 40.0, 1),
        550_000.0,
        "one input has all the time left"
    );
    let plain = loop_definition();
    let steps = input_steps(&plain, &json!({})).expect("one input steps as it is");
    assert_eq!(steps.len(), 1);
    assert!(steps[0].manipulated);
    assert_eq!(steps[0].loop_definition, plain);
    assert!(!several_channels(&plain));
}

#[test]
fn a_manual_address_is_the_first_entry_of_its_name_and_none_without_an_address() {
    let io = json!({
        "inputs": [{ "name": "P1.LoadKw", "address": "%IX1" }],
        "outputs": [],
        "memory": [{ "name": "P1.LoadKw", "address": "%MD4" }, { "name": "P1.Other" }]
    });
    assert_eq!(manual_address(&io, "p1.loadkw").as_deref(), Some("%IX1"));
    assert_eq!(manual_address(&io, "P1.Other"), None, "no address");
    assert_eq!(manual_address(&io, "P1.Missing"), None, "no entry");
    assert_eq!(manual_address(&Value::Null, "P1.LoadKw"), None, "no list");
}

// C7, C8: a campaign runs the method Identify chose for the loop, unless every input declares its own step; then it runs manual-output
// steps. A resumed campaign plans its remaining inputs by the same method.
#[test]
fn a_campaign_runs_the_loops_method_unless_every_input_has_its_own_step() {
    let setpoint = IdentificationMethod {
        kind: ControlExperimentKind::SetpointStep,
        manual: false,
        automatic: true,
    };
    let declared = two_input_loop();
    assert_eq!(
        campaign_method(&declared, setpoint),
        output_step(),
        "both inputs declare their own step, so the campaign steps each in Manual"
    );
    let mut undeclared = two_input_loop();
    undeclared.experiment = None;
    assert_eq!(
        campaign_method(&undeclared, setpoint),
        setpoint,
        "without an experiment no input has its own step, so the loop's method runs"
    );
    let chosen = IdentificationMethod {
        kind: ControlExperimentKind::MvStep,
        manual: true,
        automatic: true,
    };
    assert!(campaign_method(&undeclared, chosen).automatic);
    assert!(
        !campaign_method(&undeclared, setpoint).manual,
        "a setpoint step runs in Auto"
    );
}
