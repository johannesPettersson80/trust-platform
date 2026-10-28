#![allow(clippy::float_cmp)] // The tests compare the values the rules copy, exactly, as the TypeScript tests do.

use serde_json::json;

use super::*;

use super::super::commissioning_result::{
    CommissioningStatus, TransferConfirmation, TransferTransaction,
};
use super::super::fixture_loops::{first_order_model, requirements};
use super::super::observation_assessment::JournalStage;
use super::super::parameter_application::ReviewKind;
use super::super::pid_apply_fixture::{
    accept_at, campaign, pid_loop, record, status, values, NOISE,
};
use super::super::pid_ending::RunOutcome;
use super::super::proportional_commissioning::RunEnding;
use super::super::transfer_tracking::TransferTracking;
use crate::loop_definition::Range;
use crate::validation::commissioning_assessment::CommissioningCriteria;

/// The criteria the saved campaign was judged by.
fn criteria() -> CommissioningCriteria {
    CommissioningCriteria {
        target: 3.0,
        duration_seconds: 4.0,
        settling_time_seconds: 2.0,
        maximum_error: 0.1,
        task_period_ms: 20.0,
        process_range: Range {
            minimum: -20.0,
            maximum: 20.0,
        },
        output_limits: Range {
            minimum: -20.0,
            maximum: 20.0,
        },
        switching: None,
    }
}

/// The parameter record a campaign left: the candidate `Kp 2, Ki 0.5, Kd 0` applied as `applied-1` over the original record at `Kp 1`.
fn saved_record() -> ParameterStatus {
    ParameterStatus {
        supports_request_id: Some(true),
        transaction_id: Some("apply-1".to_owned()),
        program_revision: "program".to_owned(),
        binding_revision: "parameters".to_owned(),
        active_revision: "applied-1".to_owned(),
        values: BTreeMap::from([
            ("Main.Kp".to_owned(), 2.0),
            ("Main.Ki".to_owned(), 0.5),
            ("Main.Kd".to_owned(), 0.0),
        ]),
        status: RecordState::Applied,
        previous_revision: Some("original".to_owned()),
        previous_values: Some(BTreeMap::from([
            ("Main.Kp".to_owned(), 1.0),
            ("Main.Ki".to_owned(), 0.0),
            ("Main.Kd".to_owned(), 0.0),
        ])),
        reason: None,
    }
}

/// The stored report of a Control.PID campaign that applied the candidate over the record at `Kp 1`, with its first transfer recorded
/// (`reviewRecovery`'s evidence), found in Auto or in Manual.
fn saved_report(found_in_auto: bool) -> CommissioningResult {
    let mut first = status(values(!found_in_auto, 0.0));
    first.transaction = Some(TransferTransaction {
        id: "transfer-1".to_owned(),
        program_revision: "program".to_owned(),
        binding_revision: "transfer".to_owned(),
        status: TransferState::Applied,
        reason: None,
        observed_values: Some(values(false, 0.0)),
        confirmation: Some(TransferConfirmation::OutputImage),
        initial_values: Some(values(!found_in_auto, 0.0)),
        applied_values: None,
        restored: None,
    });
    CommissioningResult {
        status: CommissioningStatus::Failed,
        reason: "Requirement failed.".to_owned(),
        recording: None,
        transfers: vec![first],
        recovery_confirmed: false,
        maximum_error: None,
        target: 3.0,
        parameters: Some(saved_record()),
        model_revision: "model:1".to_owned(),
        channels: Vec::new(),
        criteria: criteria(),
        gain_recovery: None,
        failure: None,
        found_mode: Some(if found_in_auto {
            FoundMode::Auto
        } else {
            FoundMode::Manual
        }),
    }
}

/// The rollback review of a saved campaign: the record the campaign left, with the values that a Restore writes back (`Kp 1`).
fn rollback_review(report: &CommissioningResult) -> ParameterReview {
    let observed = report
        .parameters
        .clone()
        .expect("the saved campaign keeps its record");
    ParameterReview {
        transaction_id: "rollback-1".to_owned(),
        values: observed
            .previous_values
            .clone()
            .expect("the saved record keeps its previous values"),
        observed,
        kind: ReviewKind::Rollback {
            previous_revision: "original".to_owned(),
        },
    }
}

/// Begins the recovery of a saved campaign on the apply fixture's loop, model and requirements.
fn begin<'a>(
    loop_definition: &'a LoopDefinition,
    model: &'a GeneralModel,
    task: &'a EngineerRequirements,
    report: &'a CommissioningResult,
) -> Result<RecoveryStart<'a>, Failure> {
    RecoveryStart::begin(RecoverySetup {
        loop_definition,
        model,
        task,
        profile: PidRunProfile::ParallelPid,
        review: rollback_review(report),
        report,
    })
}

/// The transfer identity a saved campaign's recovery reads now: the transfer the report recorded, not pending.
fn current_transfer() -> TransferStatus {
    status(values(false, 0.0))
}

#[test]
fn a_saved_campaign_recovers_from_the_original_reference_it_recorded() {
    // TS control-studio-apply-process-time, "a Restore reviewed later from Apply's report returns a loop found in Auto to Auto" (A6.2,
    // B6.4): the original reference comes from the first transfer's recorded values, and the rollback values are the restore values.
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let report = saved_report(true);
    let start =
        begin(&loop_definition, &model, &task, &report).expect("the saved campaign is reviewed");
    let campaign = start
        .accept(&current_transfer())
        .expect("the transfer identity holds");
    assert_eq!(campaign.references().get("Main.SP"), Some(&2.0));
    assert_eq!(campaign.previous_reference(), 2.0);
    assert!(campaign.found_in_auto());
    assert_eq!(campaign.restore_values().get("Main.Kp"), Some(&1.0));
    assert!(campaign
        .rollback_description()
        .contains("and return to Auto"));
}

#[test]
fn a_saved_campaign_that_recovered_in_manual_has_no_rollback_left() {
    // The assertRecoveryNeeded refusal of a campaign already recovered in Manual (B6.4).
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let mut done = saved_report(true);
    done.recovery_confirmed = true;
    if let Some(transaction) = done
        .transfers
        .first_mut()
        .and_then(|transfer| transfer.transaction.as_mut())
    {
        transaction.observed_values = Some(values(true, 0.0));
    }
    let binding = PidRunProfile::ParallelPid
        .binding(&loop_definition)
        .expect("the binding fits the loop");
    assert_eq!(
        assert_recovery_needed(&done, &binding.mode).err().map(|failure| failure.message),
        Some("This controller commissioning campaign is already recovered in Manual; no further rollback is needed.".to_owned())
    );
    let refused = begin(&loop_definition, &model, &task, &done)
        .err()
        .expect("the refusal");
    assert_eq!(refused.next_action, NextAction::None);
}

#[test]
fn a_saved_campaign_is_restored_only_on_the_model_and_runtime_it_was_reviewed_on() {
    // The stored campaign must match the reviewed model and runtime (reviewRecovery; B6.4).
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let mut other_model = saved_report(true);
    other_model.model_revision = "model:0".to_owned();
    let refused = begin(&loop_definition, &model, &task, &other_model)
        .err()
        .expect("the refusal");
    assert_eq!(
        refused.message,
        "The saved controller campaign does not match the reviewed runtime and rollback values."
    );
    assert_eq!(refused.cause, Cause::Settings);
}

#[test]
fn a_recovery_refuses_a_transfer_whose_identity_changed_or_whose_result_is_pending() {
    // The transfer a Restore reads must be the one the report recorded, and not pending (reviewRecovery; B6.4).
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let report = saved_report(true);
    let start =
        begin(&loop_definition, &model, &task, &report).expect("the saved campaign is reviewed");
    let mut pending = current_transfer();
    pending.status = TransferState::Pending;
    let refused = start
        .accept(&pending)
        .err()
        .expect("a pending transfer is refused");
    assert_eq!(
        refused.message,
        "Controller transfer identity changed or its result remains pending."
    );
    let start =
        begin(&loop_definition, &model, &task, &report).expect("the saved campaign is reviewed");
    let mut moved = current_transfer();
    moved.program_revision = "program-2".to_owned();
    assert!(
        start.accept(&moved).is_err(),
        "a reloaded program is refused"
    );
}

#[test]
fn a_loop_found_in_auto_returns_to_auto_with_the_noise_allowance_when_it_is_restored_in_session() {
    // V4.6 regression, as the TypeScript apply tests check it ("enters Auto, and returns to Auto after a failed Apply, within the A6.4
    // allowance for the measured noise"): the return to Auto carries the restored record's allowance, 2·Kp·n with Kp 1 and no integral term.
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let in_session = accept_at(
        &loop_definition,
        &model,
        &task,
        Some(NOISE),
        &status(values(false, 0.0)),
    )
    .expect("the in-session run is reviewed");
    let entry = in_session
        .found_mode_return(&status(values(true, 0.0)))
        .expect("the return builds")
        .expect("a loop found in Auto re-enters Auto");
    assert!(
        (entry.tolerance - 2.0 * 1.0 * NOISE).abs() < 1e-12,
        "{}",
        entry.tolerance
    );
    assert!(
        matches!(entry.tracking, TransferTracking::ParallelPid(ref tracking) if tracking.align_output)
    );
    assert_eq!(entry.values.get("Main.SP"), Some(&2.0));
}

#[test]
fn a_loop_found_in_auto_re_enters_auto_when_a_saved_restore_reads_it_in_manual() {
    // TS control-studio-apply-process-time, "a Restore reviewed later from Apply's report returns a loop found in Auto to Auto" (A6.2): the
    // saved campaign's Restore re-enters Auto. The saved review holds no measured noise, so the return carries no noise term.
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let report = saved_report(true);
    let start =
        begin(&loop_definition, &model, &task, &report).expect("the saved campaign is reviewed");
    let campaign = start
        .accept(&current_transfer())
        .expect("the transfer identity holds");
    let entry = campaign.found_mode_return(&status(values(true, 0.0)));
    assert!(entry.expect("the return builds").is_some());
}

#[test]
fn a_loop_found_in_manual_stays_in_manual_when_it_is_restored() {
    // TS control-studio-apply-process-time, "a failed PID Apply that found the loop in Manual leaves it in Manual" (A6.2).
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let report = saved_report(false);
    let start =
        begin(&loop_definition, &model, &task, &report).expect("the saved campaign is reviewed");
    let campaign = start
        .accept(&current_transfer())
        .expect("the transfer identity holds");
    assert!(!campaign.found_in_auto());
    assert!(campaign
        .found_mode_return(&status(values(true, 0.0)))
        .expect("the return builds")
        .is_none());
}

#[test]
fn the_manual_transfer_of_a_recovery_is_written_only_while_the_mode_or_a_reference_differs() {
    // The recovery writes the Manual states of the reviewed reference and the output's current value as the manual command (C1; B6.4). A
    // state that differs while the mode and the reference hold needs no write: the PID checks only the mode and the references.
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let fresh = campaign(&loop_definition, &model, &task);
    let mut held = values(true, 0.0);
    held.insert("Main.Integral".to_owned(), TransferValue::Number(0.3));
    assert!(fresh
        .recovery_manual_step(&status(held))
        .expect("the check reads")
        .is_none());
    let in_auto = values(false, 0.36);
    let step = fresh
        .recovery_manual_step(&status(in_auto))
        .expect("the recovery step builds")
        .expect("the loop in Auto is written back to Manual");
    assert_eq!(
        step.values.get("Main.SP"),
        Some(&2.0),
        "the reviewed reference"
    );
    assert_eq!(
        step.values.get("Main.ManualCommand"),
        Some(&0.36),
        "the output's current value"
    );
    assert!(
        matches!(step.tracking, TransferTracking::ParallelPid(ref tracking) if !tracking.align_output)
    );
}

#[test]
fn the_parameter_record_is_checked_for_its_owner_before_a_restore_writes() {
    // A record another operation owns is not overwritten (recover; A6.8, B6.4).
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let fresh = campaign(&loop_definition, &model, &task);
    assert!(fresh
        .check_ownership(&record(1.0, 0.0, 0.0, "original"), None)
        .is_ok());
    let applied = BTreeMap::from([
        ("Main.Kp".to_owned(), 2.0),
        ("Main.Ki".to_owned(), 0.5),
        ("Main.Kd".to_owned(), 0.0),
    ]);
    let mut ours = record(2.0, 0.5, 0.0, "applied-1");
    ours.values = applied;
    ours.previous_revision = Some("original".to_owned());
    assert!(fresh.check_ownership(&ours, Some("applied-1")).is_ok());
    let mut pending = record(1.0, 0.0, 0.0, "original");
    pending.status = RecordState::Pending;
    let refused = fresh
        .check_ownership(&pending, None)
        .expect_err("a pending record is refused");
    assert_eq!(
        refused.message,
        "Parameter ownership changed; recovery will not overwrite another operation's values."
    );
    assert_eq!(refused.next_action, NextAction::Restore);
    assert!(fresh
        .check_ownership(&record(3.0, 0.0, 0.0, "other"), None)
        .is_err());
}

#[test]
fn the_program_and_the_parameter_record_are_checked_before_and_after_the_manual_transfer() {
    // A reloaded program is not written to, and a pending record is not overwritten (recover; A6.7, A6.8).
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let fresh = campaign(&loop_definition, &model, &task);
    let mut reloaded = current_transfer();
    reloaded.program_revision = "program-2".to_owned();
    let refused = fresh
        .check_program(&reloaded)
        .expect_err("a reloaded program is refused");
    assert_eq!(
        refused.message,
        "Program identity changed; PID recovery cannot write to the new program."
    );
    assert!(fresh.check_program(&current_transfer()).is_ok());
    let mut pending = record(1.0, 0.0, 0.0, "original");
    pending.status = RecordState::Pending;
    let refused = fresh
        .check_parameters_settled(&pending)
        .expect_err("a pending record is unresolved");
    assert_eq!(
        refused.message,
        "Controller parameter recovery is unresolved or its identity changed."
    );
    assert!(fresh
        .check_parameters_settled(&record(1.0, 0.0, 0.0, "original"))
        .is_ok());
}

#[test]
fn a_rollback_is_written_only_while_the_values_in_force_differ_and_must_match_the_campaign() {
    // The rollback writes the values a Restore keeps, and the runtime's rollback must hold them (recover; B6.4).
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let fresh = campaign(&loop_definition, &model, &task);
    assert!(!fresh.rollback_needed(&record(1.0, 0.0, 0.0, "original")));
    assert!(fresh.rollback_needed(&record(2.0, 0.5, 0.0, "applied-1")));
    let retained = ParameterReview {
        transaction_id: "rollback-1".to_owned(),
        observed: record(2.0, 0.5, 0.0, "applied-1"),
        values: BTreeMap::from([
            ("Main.Kp".to_owned(), 1.0),
            ("Main.Ki".to_owned(), 0.0),
            ("Main.Kd".to_owned(), 0.0),
        ]),
        kind: ReviewKind::Rollback {
            previous_revision: "original".to_owned(),
        },
    };
    assert!(fresh.check_rollback(&retained).is_ok());
    let mut other = retained;
    other.values.insert("Main.Kp".to_owned(), 1.5);
    let refused = fresh
        .check_rollback(&other)
        .expect_err("a different rollback is refused");
    assert_eq!(
        refused.message,
        "The retained controller rollback no longer matches this campaign."
    );
    assert_eq!(refused.next_action, NextAction::Restore);
}

#[test]
fn a_recovery_needs_a_write_only_when_the_run_applied_a_record_or_owns_a_transfer() {
    // A run that applied nothing and owns no transfer has nothing to restore (cancel, recover; B6.4).
    assert!(!recovery_needed(false, false));
    assert!(recovery_needed(true, false));
    assert!(recovery_needed(false, true));
}

#[test]
fn the_restore_result_clears_the_failure_and_names_the_mode_it_confirms() {
    // TS control-studio-apply-process-time, "Restore brings back the previous gain" (B6.4): the result keeps the reviewed record's values
    // and reports the mode the loop returned to. The result is built from the campaign's own ending.
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let fresh = campaign(&loop_definition, &model, &task);
    let failed = fresh
        .result(
            RunEnding {
                status: CommissioningStatus::Failed,
                reason: "Requirement failed.".to_owned(),
                failure: Some(Failure::new(
                    "Requirement failed.",
                    Cause::Settings,
                    NextAction::Restore,
                )),
                recovery_confirmed: true,
                awaiting_restore: true,
                stage: JournalStage::Completed,
            },
            RunOutcome {
                recording: None,
                transfers: Vec::new(),
                maximum_error: None,
                parameters: None,
            },
        )
        .expect("the result builds");
    assert_eq!(failed.model_revision, "model:1");
    assert_eq!(failed.target, 3.0);
    assert_eq!(failed.found_mode, Some(FoundMode::Auto));
    assert_eq!(failed.channels.len(), 3);
    let restored = fresh.restore_confirmed(failed, record(1.0, 0.0, 0.0, "original"), Vec::new());
    assert!(restored.recovery_confirmed);
    assert!(restored.failure.is_none());
    assert_eq!(
        restored.reason,
        "Controller recovery confirmed in Auto; previous parameters restored."
    );
    assert_eq!(
        restored
            .parameters
            .map(|parameters| parameters.active_revision),
        Some("original".to_owned())
    );
}

#[test]
fn a_cancel_names_its_confirmed_recovery_and_an_unresolved_one_offers_restore_again() {
    // The cancel names its recovery and keeps the failure (cancel; B6.4, B6.7).
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let fresh = campaign(&loop_definition, &model, &task);
    let result = fresh
        .result(
            RunEnding {
                status: CommissioningStatus::Cancelled,
                reason: "Controller commissioning cancelled.".to_owned(),
                failure: None,
                recovery_confirmed: false,
                awaiting_restore: false,
                stage: JournalStage::Transfer,
            },
            RunOutcome {
                recording: None,
                transfers: Vec::new(),
                maximum_error: None,
                parameters: None,
            },
        )
        .expect("the result builds");
    let confirmed = fresh.cancel_confirmed(
        result.clone(),
        None,
        record(1.0, 0.0, 0.0, "original"),
        Vec::new(),
    );
    assert!(confirmed.recovery_confirmed);
    assert_eq!(
        confirmed.reason,
        "Controller recovery and recording stop confirmed in Auto."
    );
    let unresolved = cancel_unresolved(result, "boom");
    assert!(!unresolved.recovery_confirmed);
    assert_eq!(
        unresolved.reason,
        "Controller recovery remains unresolved: boom"
    );
    assert_eq!(
        unresolved.failure.map(|failure| failure.next_action),
        Some(NextAction::Restore)
    );
}

#[test]
fn a_recovery_review_is_refused_for_a_loop_it_cannot_restore_from_its_settings_alone() {
    // A saved campaign whose first transfer recorded no original values cannot be restored (C1 "Apply and Restore").
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let mut report = saved_report(true);
    if let Some(transaction) = report
        .transfers
        .first_mut()
        .and_then(|transfer| transfer.transaction.as_mut())
    {
        transaction.initial_values = None;
    }
    let refused = begin(&loop_definition, &model, &task, &report)
        .err()
        .expect("the refusal");
    assert_eq!(
        refused.message,
        "The saved controller campaign does not match the reviewed runtime and rollback values."
    );
}
