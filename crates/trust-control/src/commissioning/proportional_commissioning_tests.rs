#![allow(clippy::float_cmp)] // The reviewed values are exact decimals that the rules copy, so they are compared exactly.

use super::*;

use crate::recording::channels::ChannelQuality;

use super::super::fixture_loops::{first_order_model, proportional_loop, requirements};
use super::super::found_mode::found_mode_of;
use super::super::observation::ResponseSample;
use super::super::observation_assessment::{
    check_permitted_ranges, detached_refusal, not_held_refusal, observation_in_progress,
    recording_not_held, PermittedRange,
};
use super::super::parameter_status::RecordState;
use super::super::transfer::proportional_transfer_binding;
use super::super::transfer::TransferModeBinding;

fn numbers(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
    pairs
        .iter()
        .map(|(symbol, value)| ((*symbol).to_owned(), *value))
        .collect()
}

fn readings(pairs: &[(&str, TransferValue)]) -> BTreeMap<String, TransferValue> {
    pairs
        .iter()
        .map(|(symbol, value)| ((*symbol).to_owned(), *value))
        .collect()
}

/// A parameter record of the proportional fixture: `Main.Parameters.Kp` at 2, applied over the record at revision `a1`.
fn record(kp: f64, active: &str) -> ParameterStatus {
    ParameterStatus {
        supports_request_id: Some(true),
        transaction_id: Some("apply-1".to_owned()),
        program_revision: "program:1".to_owned(),
        binding_revision: "binding:1".to_owned(),
        active_revision: active.to_owned(),
        values: numbers(&[("Main.Parameters.Kp", kp)]),
        status: RecordState::Applied,
        previous_revision: None,
        previous_values: None,
        reason: None,
    }
}

fn expect_failure<T: std::fmt::Debug>(result: Result<T, Failure>) -> Failure {
    result.expect_err("the rule refuses this input")
}

/// The reviewed loop, its binding and the Manual mode value of the fixture: `Main.Auto` is false in Manual.
fn fixture() -> (LoopDefinition, TransferBinding) {
    let loop_definition = proportional_loop();
    let binding = proportional_transfer_binding(&loop_definition)
        .expect("the fixture's binding is proportional");
    (loop_definition, binding)
}

#[test]
fn the_target_is_the_operating_output_plus_the_reference_change() {
    let (_, binding) = fixture();
    let target =
        reviewed_target(&binding, &first_order_model(), &requirements()).expect("a target");
    assert_eq!(
        target, 3.0,
        "the operating output 2 plus the reviewed change 1"
    );
}

#[test]
fn the_criteria_judge_the_target_over_the_reviewed_duration_and_the_output_span() {
    let (loop_definition, binding) = fixture();
    let criteria = run_criteria(&loop_definition, &binding, &requirements(), 3.0);
    assert_eq!(criteria.target, 3.0);
    assert_eq!(criteria.duration_seconds, 4.0);
    assert_eq!(criteria.settling_time_seconds, 2.0);
    assert_eq!(criteria.maximum_error, 0.1);
    assert_eq!(criteria.task_period_ms, 10.0);
    assert_eq!(
        criteria.process_range,
        Range {
            minimum: 0.0,
            maximum: 100.0
        }
    );
    assert_eq!(
        criteria.output_limits,
        Range {
            minimum: 0.0,
            maximum: 100.0
        }
    );
    assert!(criteria.switching.is_none());
}

#[test]
fn the_result_names_the_setpoint_the_measurement_and_the_command() {
    let (loop_definition, _) = fixture();
    let channels = run_channels(&loop_definition).expect("the fixture has a setpoint");
    let ids: Vec<&str> = channels.iter().map(|channel| channel.id.as_str()).collect();
    assert_eq!(ids, ["LevelSP", "LevelPV", "ValveMV"]);
    assert!(channels.iter().all(|channel| channel.unit == "%"));
    assert_eq!(
        expected_settling_seconds(&loop_definition, &requirements()),
        Some(2.0)
    );
}

#[test]
fn a_campaign_starts_once_and_a_recovery_review_never_starts_one() {
    let recovery_only = expect_failure(check_may_run(true, false));
    assert_eq!(recovery_only.next_action, NextAction::None);
    assert!(recovery_only.message.contains("recovery only"));
    assert!(expect_failure(check_may_run(false, true))
        .message
        .contains("already started"));
    assert!(check_may_run(false, false).is_ok());
}

#[test]
fn a_campaign_can_recover_only_with_its_transaction_and_request_identities() {
    assert!(check_recoverable(Some("apply-1"), Some(true)).is_ok());
    assert!(
        check_recoverable(None, Some(true)).is_err(),
        "no transaction identifier"
    );
    assert!(
        check_recoverable(Some(""), Some(true)).is_err(),
        "an empty identifier is none"
    );
    assert!(
        check_recoverable(Some("apply-1"), Some(false)).is_err(),
        "no request identities"
    );
    assert_eq!(
        expect_failure(check_recoverable(None, None)).cause,
        Cause::Runtime
    );
}

#[test]
fn a_proportional_auto_entry_needs_the_proportional_tracking_kind() {
    let kinds = ["parallel-pid-v1".to_owned(), "proportional".to_owned()];
    assert!(check_proportional_tracking(Some(kinds.as_slice())).is_ok());
    let without = ["parallel-pid-v1".to_owned()];
    let refused = expect_failure(check_proportional_tracking(Some(without.as_slice())));
    assert_eq!(refused.cause, Cause::ControllerCapability);
    assert!(expect_failure(check_proportional_tracking(None))
        .message
        .contains("P tracking"));
}

#[test]
fn a_program_that_changed_since_the_review_asks_for_a_new_validation() {
    assert!(check_program_unchanged("program:1", "program:1").is_ok());
    let changed = expect_failure(check_program_unchanged("program:1", "program:2"));
    assert_eq!(
        changed.next_action,
        NextAction::Validate,
        "B6.7: Candidate or validation no longer current"
    );
}

#[test]
fn the_run_starts_only_from_the_reviewed_record_mode_and_reference() {
    let (_, binding) = fixture();
    let reviewed = record(2.0, "a1");
    let current = record(2.0, "a1");
    let boundary = readings(&[
        ("Main.Auto", TransferValue::Boolean(false)),
        ("LevelSP", TransferValue::Number(3.0)),
    ]);
    assert!(
        check_reviewed_state(&reviewed, &current, Some(false), &binding, &boundary, 3.0).is_ok()
    );

    let moved_record = record(2.0, "a2");
    let refused = expect_failure(check_reviewed_state(
        &reviewed,
        &moved_record,
        Some(false),
        &binding,
        &boundary,
        3.0,
    ));
    assert_eq!(refused.next_action, NextAction::Validate);

    let auto = readings(&[
        ("Main.Auto", TransferValue::Boolean(true)),
        ("LevelSP", TransferValue::Number(3.0)),
    ]);
    assert!(
        check_reviewed_state(&reviewed, &current, Some(false), &binding, &auto, 3.0).is_err(),
        "the mode changed"
    );

    let moved_reference = readings(&[
        ("Main.Auto", TransferValue::Boolean(false)),
        ("LevelSP", TransferValue::Number(4.0)),
    ]);
    assert!(
        check_reviewed_state(
            &reviewed,
            &current,
            Some(false),
            &binding,
            &moved_reference,
            3.0
        )
        .is_err(),
        "the reference changed"
    );
}

#[test]
fn a_staged_gain_the_program_changed_is_named_and_refused() {
    let reviewed = numbers(&[("Main.Parameters.Kp", 2.0)]);
    assert!(check_staged_values(
        &reviewed,
        &readings(&[("Main.Parameters.Kp", TransferValue::Number(2.0))])
    )
    .is_ok());
    let changed = expect_failure(check_staged_values(
        &reviewed,
        &readings(&[("Main.Parameters.Kp", TransferValue::Number(3.0))]),
    ));
    assert!(
        changed.message.contains("Main.Parameters.Kp"),
        "A6.8 names the parameter: {}",
        changed.message
    );
    assert_eq!(changed.next_action, NextAction::OpenLoopSetup);
}

#[test]
fn a_record_the_program_changed_after_the_auto_entry_is_refused() {
    let reviewed = numbers(&[("Main.Parameters.Kp", 2.0)]);
    assert!(check_held_values("program:1", &reviewed, &record(2.0, "a1")).is_ok());
    let changed = expect_failure(check_held_values(
        "program:1",
        &reviewed,
        &record(5.0, "a1"),
    ));
    assert!(changed.message.contains("Main.Parameters.Kp"));
    assert_eq!(changed.next_action, NextAction::OpenLoopSetup);
    let mut other_program = record(2.0, "a1");
    other_program.program_revision = "program:2".to_owned();
    assert!(check_held_values("program:1", &reviewed, &other_program).is_err());
}

#[test]
fn a_manual_loop_must_hold_its_plant_input_within_the_auto_entry_allowance() {
    let (loop_definition, binding) = fixture();
    let model = first_order_model();
    let within = readings(&[
        ("ValvePosition", TransferValue::Number(0.1)),
        ("LevelPV", TransferValue::Number(2.0)),
    ]);
    assert!(check_starting_point(
        &loop_definition,
        &model,
        &requirements(),
        &binding,
        Some(FoundMode::Manual),
        &within,
        0.2
    )
    .is_ok());
    let beyond = readings(&[
        ("ValvePosition", TransferValue::Number(50.0)),
        ("LevelPV", TransferValue::Number(2.0)),
    ]);
    assert!(check_starting_point(
        &loop_definition,
        &model,
        &requirements(),
        &binding,
        Some(FoundMode::Manual),
        &beyond,
        0.2
    )
    .is_err());
}

#[test]
fn a_loop_found_in_auto_does_not_compare_its_plant_input() {
    let (loop_definition, binding) = fixture();
    let moved = readings(&[
        ("ValvePosition", TransferValue::Number(50.0)),
        ("LevelPV", TransferValue::Number(2.0)),
    ]);
    assert!(
        check_starting_point(
            &loop_definition,
            &first_order_model(),
            &requirements(),
            &binding,
            Some(FoundMode::Auto),
            &moved,
            0.2
        )
        .is_ok(),
        "B6.2: the running controller moves its output"
    );
}

#[test]
fn the_manual_staging_tracks_the_held_output_with_the_previous_reference() {
    let (loop_definition, binding) = fixture();
    let observed = readings(&[
        ("ValvePosition", TransferValue::Number(10.0)),
        ("LevelPV", TransferValue::Number(2.0)),
    ]);
    let step = manual_step(&loop_definition, &binding, &requirements(), &observed, 3.0)
        .expect("a staging step");
    assert_eq!(
        step.values,
        numbers(&[("LevelSP", 3.0), ("Main.ManualCommand", 10.0)])
    );
    assert!(
        (step.tolerance - 1e-4).abs() < 1e-12,
        "the exact check: 1e-6 of the 100 % span"
    );
    assert!(
        matches!(step.tracking, TransferTracking::Proportional(ref tracking) if !tracking.align_reference)
    );
}

#[test]
fn the_auto_entry_aligns_the_reference_to_the_held_output_with_the_noise_allowance() {
    let (loop_definition, binding) = fixture();
    let observed = readings(&[
        ("ValvePosition", TransferValue::Number(10.0)),
        ("LevelPV", TransferValue::Number(2.0)),
    ]);
    let gains = numbers(&[("Main.Parameters.Kp", 2.0)]);
    let step = auto_entry_step(
        &loop_definition,
        &binding,
        &requirements(),
        &gains,
        Some(0.05),
        &observed,
    )
    .expect("an Auto entry");
    // r = PV + u / Kp = 2 + 10 / 2 (C13.3).
    assert_eq!(step.values["LevelSP"], 7.0);
    assert_eq!(step.values["Main.ManualCommand"], 10.0);
    // The allowance of A6.4 for a P law: 2 |Kp| n, with n = 0.05, above the exact check.
    assert!(
        (step.tolerance - 0.2).abs() < 1e-12,
        "2 * 2 * 0.05 = 0.2, got {}",
        step.tolerance
    );
    assert!(
        matches!(step.tracking, TransferTracking::Proportional(ref tracking) if tracking.align_reference)
    );
}

#[test]
fn a_zero_gain_cannot_align_the_reference() {
    let (loop_definition, binding) = fixture();
    let observed = readings(&[
        ("ValvePosition", TransferValue::Number(10.0)),
        ("LevelPV", TransferValue::Number(2.0)),
    ]);
    let gains = numbers(&[("Main.Parameters.Kp", 0.0)]);
    assert!(auto_entry_step(
        &loop_definition,
        &binding,
        &requirements(),
        &gains,
        Some(0.05),
        &observed
    )
    .is_err());
}

#[test]
fn the_target_step_holds_the_target_with_the_target_change_allowance() {
    let (loop_definition, binding) = fixture();
    let observed = readings(&[
        ("ValvePosition", TransferValue::Number(10.0)),
        ("LevelPV", TransferValue::Number(2.0)),
    ]);
    let step = target_step(&loop_definition, &binding, &observed, 5.0).expect("a target step");
    assert_eq!(step.values["LevelSP"], 5.0);
    assert_eq!(step.values["Main.ManualCommand"], 10.0);
    assert!(
        (step.tolerance - 100.0).abs() < 1e-12,
        "the actuator span equals the output span"
    );
}

#[test]
fn an_observation_is_in_progress_only_while_its_checkpoint_journals_both_its_start_and_its_recording(
) {
    // The TypeScript test `control-studio-apply-reattach.test.ts`: "an observation is in progress only while its checkpoint journals
    // both the observation and the recording identity (A8.5)". Arguments: stage, observation journalled, recording identity journalled.
    assert!(observation_in_progress(JournalStage::Recording, true, true));
    assert!(observation_in_progress(JournalStage::Transfer, true, true));
    assert!(
        !observation_in_progress(JournalStage::Transfer, false, true),
        "before the observation starts, the reviewed recovery applies"
    );
    assert!(
        !observation_in_progress(JournalStage::Recording, true, false),
        "without a recording identity nothing can be reattached"
    );
    assert!(
        !observation_in_progress(JournalStage::Completed, true, true),
        "a completed observation is not followed again"
    );
}

#[test]
fn a_recording_the_runtime_no_longer_holds_is_not_held_and_offers_restore() {
    // The TypeScript test `reopening follows ...`: "a recording the runtime no longer holds is reported as not held" (A8.6).
    let attempt = attempt_of_refusal(not_held_refusal("Recording identity is unavailable."));
    let RunAttempt::Refused(failure) = attempt else {
        panic!("a recording that is not held is a refused run");
    };
    assert_eq!(failure.next_action, NextAction::Restore);
    assert!(
        failure.message.contains("no longer holds the recording"),
        "{}",
        failure.message
    );
    assert!(
        recording_not_held("recording_unavailable"),
        "the runtime's code for a lost recording"
    );
    assert!(
        !recording_not_held("transport lost"),
        "any other failure still fails the recovery"
    );
}

#[test]
fn a_closed_panel_during_a_reattached_observation_reports_no_outcome() {
    // The TypeScript test: "closing the panel detaches a reattached observation" (A10.4): nothing is restored, stored or reported.
    let attempt = attempt_of_refusal(detached_refusal());
    assert_eq!(attempt, RunAttempt::Detached);
    assert!(settle_run(attempt, false, Recovery::NotAttempted, true, "degC", None).is_none());
}

#[test]
fn a_failed_requirement_at_reopening_is_completed_and_offers_restore() {
    // The TypeScript test: "a failed requirement at reopening stores the response as completed, and offers Restore previous settings".
    let failure = Failure::new(
        "Commissioning settling: the response did not settle.",
        Cause::Settings,
        NextAction::Restore,
    );
    let ending = settle_run(
        RunAttempt::RequirementFailed(failure),
        false,
        Recovery::NotAttempted,
        true,
        "degC",
        None,
    )
    .expect("a finished run");
    assert_eq!(ending.status, CommissioningStatus::Failed);
    assert_eq!(
        ending.stage,
        JournalStage::Completed,
        "the response is stored as completed"
    );
    assert!(ending.awaiting_restore);
    assert!(
        ending.recovery_confirmed,
        "the reviewed gain stays applied, so nothing is unconfirmed"
    );
    assert_eq!(
        ending.failure.map(|failure| failure.next_action),
        Some(NextAction::Restore)
    );
}

#[test]
fn a_permitted_range_departure_is_named_with_its_values_and_offers_no_action() {
    // The TypeScript test: "a departure from the permitted range stores the samples ..., then raises the departure" (B6.3, B6.7).
    let sample = ResponseSample {
        elapsed_seconds: 0.0,
        scan: 0,
        controller_in_auto: true,
        setpoint: Some(5.0),
        process_value: Some(12.5),
        applied_output: Some(2.0),
        quality: ChannelQuality::Good,
        additional: Vec::new(),
    };
    let range = PermittedRange {
        symbol: "P1.RoomC".to_owned(),
        unit: "degC".to_owned(),
        minimum: 0.0,
        maximum: 10.0,
        auxiliary_id: None,
    };
    let Err(ObservationRefusal::PermittedRangeLeft(failure)) =
        check_permitted_ranges(&[sample], &[range])
    else {
        panic!("the process value 12.5 leaves the range 0..10");
    };
    assert_eq!(
        failure.message,
        "P1.RoomC left its permitted range (12.5 °C outside 0 °C..10 °C)"
    );
    assert_eq!(failure.next_action, NextAction::None);
}

#[test]
fn a_departure_names_the_departure_and_the_restored_settings() {
    let failure = Failure::new(
        "P1.RoomC left its permitted range (12.5 °C outside 0 °C..10 °C)",
        Cause::ModelOrData,
        NextAction::None,
    );
    let ending = settle_run(
        RunAttempt::PermittedRangeLeft(failure),
        false,
        Recovery::Confirmed,
        false,
        "degC",
        None,
    )
    .expect("a finished run");
    assert_eq!(ending.reason, "P1.RoomC left its permitted range (12.5 °C outside 0 °C..10 °C); the previous settings were restored.");
    assert_eq!(ending.status, CommissioningStatus::Failed);
    assert_eq!(
        ending.failure.map(|failure| failure.next_action),
        Some(NextAction::None)
    );
    assert_eq!(ending.stage, JournalStage::Recovered);
}

#[test]
fn a_refusal_with_a_restored_recovery_names_the_mode_it_returned_to() {
    let failure = Failure::new(
        "The transfer was refused.",
        Cause::Runtime,
        NextAction::None,
    );
    let ending = settle_run(
        RunAttempt::Refused(failure),
        false,
        Recovery::Confirmed,
        true,
        "degC",
        None,
    )
    .expect("a finished run");
    assert_eq!(
        ending.reason,
        "The transfer was refused. Recovery confirmed in Auto; previous gains restored."
    );
    assert!(ending.recovery_confirmed);
    assert_eq!(ending.stage, JournalStage::Recovered);
}

#[test]
fn an_unconfirmed_recovery_keeps_the_journal_at_the_transfer_and_offers_restore_again() {
    let failure = Failure::new(
        "The transfer was refused.",
        Cause::Runtime,
        NextAction::None,
    );
    let ending = settle_run(
        RunAttempt::Refused(failure),
        false,
        Recovery::Unconfirmed("the runtime did not answer".to_owned()),
        false,
        "degC",
        None,
    )
    .expect("a finished run");
    assert!(!ending.recovery_confirmed);
    assert!(
        ending
            .reason
            .ends_with("Recovery unconfirmed: the runtime did not answer"),
        "{}",
        ending.reason
    );
    assert_eq!(
        ending.failure.map(|failure| failure.next_action),
        Some(NextAction::Restore),
        "B6.7: Restore again"
    );
    assert_eq!(ending.stage, JournalStage::Transfer);
}

#[test]
fn a_cancelled_run_is_cancelled_and_names_its_recovery() {
    let ending = settle_run(
        RunAttempt::Cancelled,
        true,
        Recovery::Confirmed,
        false,
        "degC",
        None,
    )
    .expect("a finished run");
    assert_eq!(ending.status, CommissioningStatus::Cancelled);
    assert_eq!(
        ending.reason,
        "Commissioning cancelled. Recovery confirmed in Manual; previous gains restored."
    );
    assert!(ending.failure.is_none(), "a cancellation is not a failure");
    assert_eq!(ending.stage, JournalStage::Recovered);
}

#[test]
fn a_passed_run_reports_its_error_in_the_process_unit_and_stays_in_auto() {
    let ending = settle_run(
        RunAttempt::Passed { maximum_error: 0.2 },
        false,
        Recovery::NotAttempted,
        true,
        "degC",
        None,
    )
    .expect("a finished run");
    assert_eq!(ending.status, CommissioningStatus::Passed);
    assert_eq!(
        ending.reason,
        "Auto commissioning passed: recorded target error 0.2 degC; controller remains in Auto."
    );
    assert!(ending.failure.is_none() && ending.recovery_confirmed);
    assert_eq!(ending.stage, JournalStage::Completed);
}

#[test]
fn a_recording_stop_not_confirmed_is_said_and_unconfirms_the_recovery() {
    let passed = settle_run(
        RunAttempt::Passed { maximum_error: 0.2 },
        false,
        Recovery::NotAttempted,
        true,
        "degC",
        Some("the stop timed out"),
    )
    .expect("a finished run");
    assert!(!passed.recovery_confirmed);
    assert!(
        passed.reason.ends_with(
            "Recording stop unconfirmed: the stop timed out. Cancel retries the owned Stop."
        ),
        "{}",
        passed.reason
    );
    assert_eq!(
        passed.stage,
        JournalStage::Completed,
        "a passed run stays completed"
    );

    let failure = Failure::new(
        "The transfer was refused.",
        Cause::Runtime,
        NextAction::None,
    );
    let refused = settle_run(
        RunAttempt::Refused(failure),
        false,
        Recovery::Confirmed,
        false,
        "degC",
        Some("the stop timed out"),
    )
    .expect("a finished run");
    assert_eq!(
        refused.stage,
        JournalStage::Transfer,
        "without a confirmed recovery the journal stays at the transfer"
    );
}

#[test]
fn a_failure_offers_restore_again_when_it_has_none_of_its_own() {
    let again = restore_again(None, "Recovery unconfirmed: x");
    assert_eq!(again.next_action, NextAction::Restore);
    assert_eq!(again.cause, Cause::Runtime);
    let kept = restore_again(
        Some(Failure::new(
            "Keep me.",
            Cause::Settings,
            NextAction::Validate,
        )),
        "unused",
    );
    assert_eq!(kept.message, "Keep me.");
    assert_eq!(kept.next_action, NextAction::Restore);
}

#[test]
fn the_fixture_loop_is_in_manual_when_its_mode_reads_its_manual_value() {
    // The mode binding of the fixture is `Main.Auto`, whose Manual value is false.
    let (_, binding) = fixture();
    assert_eq!(
        binding.mode,
        TransferModeBinding {
            symbol: "Main.Auto".to_owned(),
            manual_value: false
        }
    );
    assert_eq!(
        found_mode_of(Some(false), binding.mode.manual_value),
        Some(FoundMode::Manual)
    );
    assert_eq!(
        found_mode_of(Some(true), binding.mode.manual_value),
        Some(FoundMode::Auto)
    );
}

#[test]
fn a_loop_outside_manual_is_staged_and_the_review_refuses_a_zero_gain() {
    let (loop_definition, binding) = fixture();
    assert!(stages_manual(
        &binding,
        &readings(&[("Main.Auto", TransferValue::Boolean(true))])
    ));
    assert!(!stages_manual(
        &binding,
        &readings(&[("Main.Auto", TransferValue::Boolean(false))])
    ));
    let observed = readings(&[
        ("ValvePosition", TransferValue::Number(10.0)),
        ("LevelPV", TransferValue::Number(2.0)),
    ]);
    assert_eq!(
        aligned_reference(&loop_definition, &binding, &observed, 2.0).expect("a reference"),
        7.0,
        "r = PV + u / Kp"
    );
    assert!(
        aligned_reference(&loop_definition, &binding, &observed, 0.0).is_err(),
        "a zero gain cannot align"
    );
}
