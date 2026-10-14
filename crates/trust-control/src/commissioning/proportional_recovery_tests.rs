#![allow(clippy::float_cmp)] // The gains and references are exact decimals that the rules copy, so they are compared exactly.

use super::*;

use super::super::commissioning_result::{CommissioningChannel, TransferState};
use super::super::fixture_loops::{first_order_model, proportional_loop, requirements};
use super::super::gain_recovery::GainRecoveryContext;
use super::super::transfer::proportional_transfer_binding;
use super::super::transfer_tracking::TransferTracking;
use crate::loop_definition::Range;
use crate::validation::commissioning_assessment::CommissioningCriteria;

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

fn criteria() -> CommissioningCriteria {
    CommissioningCriteria {
        target: 3.0,
        duration_seconds: 4.0,
        settling_time_seconds: 2.0,
        maximum_error: 0.1,
        task_period_ms: 10.0,
        process_range: Range {
            minimum: 0.0,
            maximum: 100.0,
        },
        output_limits: Range {
            minimum: 0.0,
            maximum: 100.0,
        },
        switching: None,
    }
}

/// The original set of the campaign: `Main.Parameters.Kp` at 1, in force at revision `original-rev`.
fn original() -> RecoveryOrigin {
    RecoveryOrigin {
        program_revision: "program:1".to_owned(),
        binding_revision: "binding:1".to_owned(),
        active_revision: "original-rev".to_owned(),
        values: numbers(&[("Main.Parameters.Kp", 1.0)]),
    }
}

fn candidate() -> BTreeMap<String, f64> {
    numbers(&[("Main.Parameters.Kp", 2.0)])
}

/// The Auto transfer of the campaign: its reference was 3 and the loop was in Manual at the boundary.
fn transfer() -> TransferStatus {
    TransferStatus {
        program_revision: "program:1".to_owned(),
        binding_revision: "binding:1".to_owned(),
        values: BTreeMap::new(),
        runtime_time_ns: None,
        status: TransferState::Applied,
        reason: None,
        supported_tracking: Some(vec!["proportional".to_owned()]),
        transaction: Some(TransferTransaction {
            id: "transfer:1".to_owned(),
            program_revision: "program:1".to_owned(),
            binding_revision: "binding:1".to_owned(),
            status: TransferState::Applied,
            reason: None,
            observed_values: Some(readings(&[("Main.Auto", TransferValue::Boolean(false))])),
            confirmation: Some(TransferConfirmation::OutputImage),
            initial_values: Some(readings(&[("LevelSP", TransferValue::Number(3.0))])),
            applied_values: None,
            restored: None,
        }),
    }
}

/// A passed campaign whose recovery was never confirmed away: the record a Restore can use.
fn passed_report() -> CommissioningResult {
    CommissioningResult {
        status: CommissioningStatus::Passed,
        reason:
            "Auto commissioning passed: recorded target error 0.2 degC; controller remains in Auto."
                .to_owned(),
        recording: None,
        transfers: vec![transfer()],
        recovery_confirmed: true,
        maximum_error: Some(0.2),
        target: 3.0,
        parameters: None,
        model_revision: "model:1".to_owned(),
        channels: vec![
            CommissioningChannel {
                id: "LevelSP".to_owned(),
                unit: "%".to_owned(),
            },
            CommissioningChannel {
                id: "LevelPV".to_owned(),
                unit: "%".to_owned(),
            },
            CommissioningChannel {
                id: "ValveMV".to_owned(),
                unit: "%".to_owned(),
            },
        ],
        criteria: criteria(),
        gain_recovery: Some(GainRecoveryContext {
            origin: original(),
            candidate: candidate(),
        }),
        failure: None,
        found_mode: Some(FoundMode::Manual),
    }
}

/// The running parameter record after the campaign's Apply: the candidate in force over the original revision.
fn applied_record() -> ParameterStatus {
    ParameterStatus {
        supports_request_id: Some(true),
        transaction_id: Some("apply-1".to_owned()),
        program_revision: "program:1".to_owned(),
        binding_revision: "binding:1".to_owned(),
        active_revision: "candidate-rev".to_owned(),
        values: candidate(),
        status: RecordState::Applied,
        previous_revision: Some("original-rev".to_owned()),
        previous_values: Some(numbers(&[("Main.Parameters.Kp", 1.0)])),
        reason: None,
    }
}

#[test]
fn a_campaign_already_recovered_in_manual_has_no_rollback_left() {
    let report = passed_report();
    let refused = assert_recovery_needed(&report, "Main.Auto", false);
    assert!(
        refused.is_err(),
        "a confirmed recovery that reached Manual is not repeated"
    );
    let mut unconfirmed = passed_report();
    unconfirmed.recovery_confirmed = false;
    assert!(
        assert_recovery_needed(&unconfirmed, "Main.Auto", false).is_ok(),
        "an unconfirmed recovery may still be made"
    );
    let mut in_auto = passed_report();
    if let Some(transaction) = in_auto.transfers[0].transaction.as_mut() {
        transaction.observed_values =
            Some(readings(&[("Main.Auto", TransferValue::Boolean(true))]));
    }
    assert!(
        assert_recovery_needed(&in_auto, "Main.Auto", false).is_ok(),
        "a loop still in Auto needs its rollback"
    );
}

#[test]
fn a_record_that_cannot_be_recovered_is_refused_with_its_reason() {
    let report = passed_report();
    let first = report.transfers[0].transaction.clone();
    assert!(check_historical_record(&report, "model:1", first.as_ref()).is_ok());

    let refused = check_historical_record(&report, "model:other", first.as_ref());
    assert!(
        refused.is_err(),
        "the model the campaign was reviewed on must be the one named"
    );

    let mut failed = passed_report();
    failed.status = CommissioningStatus::Failed;
    assert!(
        check_historical_record(&failed, "model:1", first.as_ref()).is_err(),
        "a failed campaign with its recovery confirmed is refused"
    );

    let mut legacy = passed_report();
    legacy.gain_recovery = None;
    assert!(
        check_historical_record(&legacy, "model:1", first.as_ref()).is_err(),
        "no gain record and no saved origin"
    );
    legacy.parameters = Some(applied_record());
    assert!(
        check_historical_record(&legacy, "model:1", first.as_ref()).is_ok(),
        "a saved origin is enough"
    );

    let mut no_reference = first.clone();
    if let Some(transaction) = no_reference.as_mut() {
        transaction.initial_values = None;
    }
    assert!(
        check_historical_record(&passed_report(), "model:1", no_reference.as_ref()).is_err(),
        "the original reference is needed"
    );
}

#[test]
fn the_running_record_is_the_one_the_campaign_left_or_the_restore_is_refused() {
    let report = passed_report();
    let review = numbers(&[("Main.Parameters.Kp", 1.0)]);
    assert!(check_historical_identity(&report, &applied_record(), &review).is_ok());

    let mut wrong_origin = passed_report();
    if let Some(context) = wrong_origin.gain_recovery.as_mut() {
        context
            .origin
            .values
            .insert("Main.Parameters.Kp".to_owned(), 2.0);
    }
    let refused = check_historical_identity(&wrong_origin, &applied_record(), &review);
    assert!(
        refused.is_err(),
        "a forged original gain cannot be restored"
    );
    assert_eq!(refused.expect_err("refused").next_action, NextAction::None);

    let mut moved = applied_record();
    moved.values.insert("Main.Parameters.Kp".to_owned(), 5.0);
    assert!(
        check_historical_identity(&report, &moved, &review).is_err(),
        "another change is never overwritten"
    );
}

#[test]
fn the_original_reference_is_read_as_it_was_recorded() {
    let loop_definition = proportional_loop();
    let binding = proportional_transfer_binding(&loop_definition).expect("the fixture binds");
    let first = transfer()
        .transaction
        .expect("the transfer has a transaction");
    assert_eq!(
        historical_reference(&binding, &first).expect("the reference was recorded"),
        3.0
    );

    let mut missing = first.clone();
    missing.initial_values = Some(readings(&[("Other", TransferValue::Number(3.0))]));
    assert!(
        historical_reference(&binding, &missing).is_err(),
        "no recorded value for the reference"
    );
}

#[test]
fn a_gain_transaction_is_reviewed_again_when_it_no_longer_matches() {
    let restored = ParameterStatus {
        active_revision: "original-rev".to_owned(),
        values: numbers(&[("Main.Parameters.Kp", 1.0)]),
        previous_revision: None,
        previous_values: None,
        ..applied_record()
    };
    assert!(
        check_gain_transaction(&original(), &candidate(), &restored).is_ok(),
        "the original set is in force"
    );
    assert!(
        check_gain_transaction(&original(), &candidate(), &applied_record()).is_ok(),
        "the candidate is over the original revision"
    );

    let mut pending = applied_record();
    pending.status = RecordState::Pending;
    let refused = check_gain_transaction(&original(), &candidate(), &pending);
    assert_eq!(
        refused.expect_err("pending").next_action,
        NextAction::Restore
    );
    let mut elsewhere = applied_record();
    elsewhere.previous_revision = Some("another-rev".to_owned());
    assert!(check_gain_transaction(&original(), &candidate(), &elsewhere).is_err());
}

#[test]
fn a_gain_that_changed_while_the_manual_transfer_was_reviewed_is_refused() {
    let review = applied_record();
    assert!(check_gain_unchanged(
        &review,
        &readings(&[("Main.Parameters.Kp", TransferValue::Number(2.0))])
    )
    .is_ok());
    let changed = check_gain_unchanged(
        &review,
        &readings(&[("Main.Parameters.Kp", TransferValue::Number(3.0))]),
    );
    assert_eq!(
        changed.expect_err("changed").next_action,
        NextAction::Restore
    );
}

#[test]
fn the_program_a_recovery_writes_to_is_the_campaign_s() {
    assert!(check_program_identity("program:1", "program:1").is_ok());
    let reloaded = check_program_identity("program:1", "program:2");
    assert_eq!(
        reloaded.expect_err("a new program").next_action,
        NextAction::None
    );
}

#[test]
fn a_manual_transfer_is_written_only_when_the_loop_or_a_state_differs() {
    let loop_definition = proportional_loop();
    let binding = proportional_transfer_binding(&loop_definition).expect("the fixture binds");
    let observed = readings(&[
        ("ValvePosition", TransferValue::Number(10.0)),
        ("LevelPV", TransferValue::Number(2.0)),
    ]);
    let step = super::super::proportional_commissioning::manual_step(
        &loop_definition,
        &binding,
        &requirements(),
        &observed,
        3.0,
    )
    .expect("a Manual step");
    let in_manual = readings(&[
        ("Main.Auto", TransferValue::Boolean(false)),
        ("LevelSP", TransferValue::Number(3.0)),
        ("Main.ManualCommand", TransferValue::Number(10.0)),
    ]);
    assert!(!manual_transfer_needed(&binding, &step, &in_manual));

    let in_auto = readings(&[
        ("Main.Auto", TransferValue::Boolean(true)),
        ("LevelSP", TransferValue::Number(3.0)),
        ("Main.ManualCommand", TransferValue::Number(10.0)),
    ]);
    assert!(
        manual_transfer_needed(&binding, &step, &in_auto),
        "the loop is still in Auto"
    );

    let moved_command = readings(&[
        ("Main.Auto", TransferValue::Boolean(false)),
        ("LevelSP", TransferValue::Number(3.0)),
        ("Main.ManualCommand", TransferValue::Number(5.0)),
    ]);
    assert!(
        manual_transfer_needed(&binding, &step, &moved_command),
        "a state differs from the Manual step"
    );
}

#[test]
fn the_parameter_rollback_is_skipped_once_the_original_set_is_in_force() {
    let restored = ParameterStatus {
        active_revision: "original-rev".to_owned(),
        values: numbers(&[("Main.Parameters.Kp", 1.0)]),
        ..applied_record()
    };
    assert_eq!(parameter_already_restored(&original(), &restored), Ok(true));
    assert_eq!(
        parameter_already_restored(&original(), &applied_record()),
        Ok(false),
        "the candidate is still in force"
    );
    let mut pending = applied_record();
    pending.status = RecordState::Pending;
    assert!(
        parameter_already_restored(&original(), &pending).is_err(),
        "a pending transaction is inspected first"
    );
}

#[test]
fn the_rollback_review_must_return_to_the_original_set_from_the_candidate() {
    let review = original().values;
    assert!(check_rollback_review(&original(), &candidate(), &applied_record(), &review).is_ok());
    let mut unlinked = applied_record();
    unlinked.previous_revision = None;
    assert!(check_rollback_review(&original(), &candidate(), &unlinked, &review).is_err());
}

#[test]
fn a_loop_found_in_auto_returns_to_auto_with_the_noise_allowance() {
    // V4.6, the proportional form: Restore of a loop found in Auto returns it to Auto, aligned to the restored gain, allowed the
    // noise of its set (A6.4: 2 |Kp| n = 2 * 2 * 0.05 = 0.2).
    let loop_definition = proportional_loop();
    let binding = proportional_transfer_binding(&loop_definition).expect("the fixture binds");
    let current = readings(&[
        ("Main.Auto", TransferValue::Boolean(false)),
        ("ValvePosition", TransferValue::Number(10.0)),
        ("LevelPV", TransferValue::Number(2.0)),
    ]);
    let restored = numbers(&[("Main.Parameters.Kp", 2.0)]);
    let entry = found_mode_return(
        &loop_definition,
        &binding,
        &requirements(),
        Some(FoundMode::Auto),
        &current,
        &restored,
        Some(0.05),
    )
    .expect("the entry builds")
    .expect("a loop found in Auto re-enters Auto");
    assert_eq!(entry.values["LevelSP"], 7.0, "r = PV + u / Kp = 2 + 10 / 2");
    assert_eq!(entry.values["Main.ManualCommand"], 10.0);
    assert!(
        (entry.tolerance - 0.2).abs() < 1e-12,
        "the noise allowance of the restored set"
    );
    assert!(
        matches!(entry.tracking, TransferTracking::Proportional(ref tracking) if tracking.align_reference)
    );

    let stays = found_mode_return(
        &loop_definition,
        &binding,
        &requirements(),
        Some(FoundMode::Manual),
        &current,
        &restored,
        Some(0.05),
    )
    .expect("no entry is built");
    assert!(stays.is_none(), "a loop found in Manual stays in Manual");

    let in_auto = readings(&[("Main.Auto", TransferValue::Boolean(true))]);
    let already = found_mode_return(
        &loop_definition,
        &binding,
        &requirements(),
        Some(FoundMode::Auto),
        &in_auto,
        &restored,
        Some(0.05),
    )
    .expect("no entry is built");
    assert!(already.is_none(), "the loop is already in Auto");
}

#[test]
fn a_restored_result_names_the_mode_it_returned_to_and_clears_its_failure() {
    let mut failed = passed_report();
    failed.status = CommissioningStatus::Failed;
    failed.failure = Some(Failure::new(
        "The transfer was refused.",
        Cause::Runtime,
        NextAction::Validate,
    ));
    let restored = restored_result(failed, true);
    assert!(restored.recovery_confirmed);
    assert!(
        restored.failure.is_none(),
        "a confirmed Restore clears the failure"
    );
    assert_eq!(
        restored.reason,
        "Recovery confirmed in Auto; previous gains restored."
    );
}

#[test]
fn an_unconfirmed_restore_fails_the_run_and_offers_restore_again() {
    let unconfirmed = restore_failed(passed_report(), "the runtime did not answer");
    assert_eq!(unconfirmed.status, CommissioningStatus::Failed);
    assert!(!unconfirmed.recovery_confirmed);
    assert_eq!(
        unconfirmed.reason,
        "Recovery unconfirmed: the runtime did not answer"
    );
    assert_eq!(
        unconfirmed.failure.map(|failure| failure.next_action),
        Some(NextAction::Restore)
    );
}

#[test]
fn a_cancel_keeps_a_completed_record_and_names_its_recovery() {
    assert!(
        cancel_keeps_result(&passed_report()),
        "a passed campaign whose recovery is confirmed is kept as it is"
    );
    let mut unconfirmed = passed_report();
    unconfirmed.recovery_confirmed = false;
    assert!(!cancel_keeps_result(&unconfirmed));

    let recovered = cancel_recovered(unconfirmed, false);
    assert!(recovered.recovery_confirmed);
    assert_eq!(
        recovered.reason,
        "Commissioning cancelled. Recovery confirmed in Manual with the previous gain set."
    );

    let stuck = cancel_unconfirmed(passed_report(), "x");
    assert!(!stuck.recovery_confirmed);
    assert_eq!(stuck.reason, "Recovery unconfirmed: x");
}

#[test]
fn a_closed_panel_adds_its_unconfirmed_recovery_to_the_reason() {
    let mut cancelled = passed_report();
    cancelled.status = CommissioningStatus::Cancelled;
    cancelled.reason = "Commissioning cancelled.".to_owned();
    let closed = close_unconfirmed(cancelled, "the panel was not answered");
    assert!(!closed.recovery_confirmed);
    assert_eq!(
        closed.reason,
        "Commissioning cancelled. Close recovery unconfirmed: the panel was not answered"
    );
}

#[test]
fn the_fixture_model_and_requirements_still_describe_the_campaign() {
    // The fixture model's operating output is 2, and the requirements' reference change is 1: the target of the campaign is 3.
    assert_eq!(
        first_order_model()
            .identity()
            .operating_point
            .as_ref()
            .map(|point| point.outputs[0]),
        Some(2.0)
    );
    assert_eq!(requirements().reference_change, 1.0);
}
