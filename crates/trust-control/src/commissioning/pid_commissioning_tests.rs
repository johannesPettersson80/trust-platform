#![allow(clippy::float_cmp)] // The tests compare the values the rules copy, exactly, as the TypeScript tests do.

use serde_json::json;

use super::*;

use super::super::adapters::controller_adapter;
use super::super::commissioning_result::CommissioningStatus;
use super::super::fixture_loops::{first_order_model, library_pid_loop, requirements};
use super::super::observation::{AuxiliaryValue, ResponseSample};
use super::super::observation_assessment::response_covers_duration;
use super::super::observation_assessment::JournalStage;
use super::super::pid_apply_fixture::{
    accept_at, campaign, pid_loop, record, refusal, secondary_signal, status, values, NOISE,
};
use super::super::proportional_commissioning::Recovery;
use crate::recording::channels::ChannelQuality;

#[test]
fn a_loop_found_in_auto_does_not_compare_its_plant_input() {
    // TS control-studio-apply-process-time, "a loop found in Auto starts from wherever its controller output is" (B6.2).
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let observed = status(values(false, 0.36));
    assert!(accept_at(&loop_definition, &model, &task, None, &observed).is_ok());
}

#[test]
fn a_loop_found_in_manual_holds_its_plant_input_at_the_validated_point() {
    // TS control-studio-apply-process-time, "in Manual the output must be the validated one" (B6.2).
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let observed = status(values(true, 0.36));
    let failure = refusal(accept_at(&loop_definition, &model, &task, None, &observed));
    assert!(
        failure
            .message
            .contains("Main.MV is 0.36 V, but the settings were validated from 0 V"),
        "{}",
        failure.message
    );
}

#[test]
fn v4_6_a_manual_apply_accepts_a_plant_input_within_the_auto_entry_allowance_and_refuses_one_beyond_it(
) {
    // TS control-studio-apply-process-time, "in Manual a plant input within the Auto-entry allowance for the measured noise counts as the
    // validated one" (B6.2, A6.4). The candidate (Kp 2, Ki 0.5) allows Kp·2n + Ki·Δt·(|e| + n), about 0.2005 V around the validated 0 V.
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let within = status(values(true, 0.15));
    assert!(accept_at(&loop_definition, &model, &task, Some(NOISE), &within).is_ok());
    let beyond = status(values(true, 0.36));
    let failure = refusal(accept_at(
        &loop_definition,
        &model,
        &task,
        Some(NOISE),
        &beyond,
    ));
    assert!(
        failure
            .message
            .contains("Main.MV is 0.36 V, but the settings were validated from 0 V ± 0.2"),
        "{}",
        failure.message
    );
    // B6.7: a plant input away from its validated point needs nothing until it is back.
    assert_eq!(failure.next_action, NextAction::None);
}

#[test]
fn the_auto_entry_allowance_is_the_a6_4_bound_of_the_pid_and_it_aligns_the_output() {
    // TS control-studio-apply-process-time, "enters Auto ... within the A6.4 allowance for the measured noise" (the first Auto entry):
    // Kp·2n + Ki·Δt·(|e| + n) with e = 2 − 2.004 and Δt = 20 ms.
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let campaign = campaign(&loop_definition, &model, &task);
    let manual = status(values(true, 0.0));
    let step = campaign
        .auto_entry_step(&manual)
        .expect("the Auto entry builds");
    let expected = 2.0 * 2.0 * NOISE + 0.5 * 0.02 * (0.004 + NOISE);
    assert!(
        (step.tolerance - expected).abs() < 1e-9,
        "{}",
        step.tolerance
    );
    assert!(
        matches!(step.tracking, TransferTracking::ParallelPid(ref tracking) if tracking.align_output)
    );
}

#[test]
fn a_manual_staging_writes_the_observed_states_with_the_transfer_allowance_and_keeps_them() {
    // The Manual staging writes the states the runtime holds (pidTransferStates) with the transfer allowance and no alignment (E6.4).
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let campaign = campaign(&loop_definition, &model, &task);
    let observed = status(values(false, 0.0));
    let step = campaign.manual_step(&observed).expect("the staging builds");
    assert_eq!(step.values.get("Main.SP"), Some(&2.0));
    assert_eq!(step.values.get("Main.ManualCommand"), Some(&0.0));
    assert_eq!(
        step.tolerance,
        transfer_tolerance(&loop_definition, None).expect("the transfer tolerance")
    );
    assert!(
        matches!(step.tracking, TransferTracking::ParallelPid(ref tracking) if !tracking.align_output)
    );
}

#[test]
fn the_target_change_sets_the_reviewed_target_with_the_target_allowance_and_does_not_align() {
    // The target change holds the reviewed target as the reference (C1, "Bumpless entry"; B6.3); its allowance is the target allowance.
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let campaign = campaign(&loop_definition, &model, &task);
    let entered = status(values(false, 0.0));
    let step = campaign
        .target_step(&entered)
        .expect("the target change builds");
    assert_eq!(
        step.values.get("Main.SP"),
        Some(&3.0),
        "the operating output 2 plus the reference change 1"
    );
    assert_eq!(step.values.get("Main.Integral"), Some(&0.0));
    assert_eq!(
        Some(step.tolerance),
        transfer_target_allowance(campaign.binding())
    );
    assert!(
        matches!(step.tracking, TransferTracking::ParallelPid(ref tracking) if !tracking.align_output)
    );
}

#[test]
fn a_decimal_target_on_a_real_setpoint_is_held_as_the_runtime_holds_it() {
    // TS control-studio-apply-process-time, "a decimal target on a REAL setpoint is sent and assessed as the runtime holds it" (C0).
    let loop_definition = pid_loop("REAL", json!([]));
    let model = first_order_model();
    let mut task = requirements();
    task.reference_change = 0.1 - 2.0;
    let campaign = campaign(&loop_definition, &model, &task);
    assert_eq!(
        campaign.target().expect("the target builds"),
        f64::from(0.1_f32)
    );
}

#[test]
fn the_reviewed_record_mode_and_reference_must_hold_before_the_run_starts() {
    // The run's check of the reviewed record, mode and reference before it starts (execute; B6.2, B6.7).
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let campaign = campaign(&loop_definition, &model, &task);
    let boundary = values(false, 0.0);
    assert!(campaign
        .check_reviewed_held(&record(1.0, 0.0, 0.0, "original"), &boundary)
        .is_ok());
    let changed_gain = campaign.check_reviewed_held(&record(1.5, 0.0, 0.0, "original"), &boundary);
    assert_eq!(refusal(changed_gain).next_action, NextAction::Validate);
    let mut changed_mode = boundary.clone();
    changed_mode.insert("Main.Manual".to_owned(), TransferValue::Boolean(true));
    assert!(campaign
        .check_reviewed_held(&record(1.0, 0.0, 0.0, "original"), &changed_mode)
        .is_err());
    let mut changed_reference = boundary;
    changed_reference.insert("Main.SP".to_owned(), TransferValue::Number(2.5));
    assert!(campaign
        .check_reviewed_held(&record(1.0, 0.0, 0.0, "original"), &changed_reference)
        .is_err());
}

#[test]
fn a_program_that_changed_since_the_review_asks_for_a_new_validation() {
    // The program the review read must be the one that the run's first reading shows (B6.7: Validate).
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let mut observed = status(values(false, 0.0));
    observed.program_revision = "program-2".to_owned();
    let failure = refusal(accept_at(&loop_definition, &model, &task, None, &observed));
    assert_eq!(
        failure.message,
        "The program changed during PID commissioning review."
    );
    assert_eq!(failure.next_action, NextAction::Validate);
}

#[test]
fn a_runtime_without_the_structure_tracking_refuses_the_run_with_the_kind_it_needs() {
    // The refusal names the tracking kind of the structure, as the TypeScript does (E6.3).
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let mut first = status(values(false, 0.0));
    first.supported_tracking = Some(vec!["proportional".to_owned()]);
    let setup = ReviewSetup {
        loop_definition: &loop_definition,
        model: &model,
        task: &task,
        profile: PidRunProfile::ParallelPid,
        review: super::super::pid_apply_fixture::review(),
        reference_hold: None,
        process_noise: None,
    };
    let failure = refusal(PidStart::begin(setup, &first));
    assert_eq!(
        failure.message,
        "The runtime needs parallel-pid-v1 state tracking before Auto commissioning."
    );
    assert_eq!(failure.cause, Cause::ControllerCapability);
}

#[test]
fn a_run_starts_once() {
    // The guard against a second start (run; A6.6).
    assert!(check_not_started(false).is_ok());
    assert_eq!(
        refusal(check_not_started(true)).message,
        "The reviewed controller commissioning campaign has already started."
    );
}

#[test]
fn a_passed_run_names_its_label_and_its_error_in_the_process_unit_and_stays_in_auto() {
    // The reason of a passed run names the structure (execute; B6.3, C1).
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let campaign = campaign(&loop_definition, &model, &task);
    let ending = campaign
        .settle(
            RunAttempt::Passed {
                maximum_error: 0.001_234_56,
            },
            false,
            Recovery::NotAttempted,
            None,
        )
        .expect("a passed run ends");
    assert_eq!(ending.status, CommissioningStatus::Passed);
    assert_eq!(
        ending.reason,
        "PID Auto commissioning passed: recorded target error 0.00123 m; controller remains in Auto."
    );
    assert!(ending.recovery_confirmed);
    assert_eq!(ending.stage, JournalStage::Completed);
}

#[test]
fn a_departure_from_the_permitted_range_is_named_and_the_settings_are_restored() {
    // TS control-studio-apply-process-time, "a failed PID Apply that found the loop in Auto returns it to Auto, with the previous gain
    // restored" (B6.3, B6.7): the reason names the departure, and the failure is the departure's own.
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let campaign = campaign(&loop_definition, &model, &task);
    let departure = Failure::new(
        "Main.PV left its permitted range (25 m outside 0 m..10 m)",
        Cause::ModelOrData,
        NextAction::None,
    );
    let ending = campaign
        .settle(
            RunAttempt::PermittedRangeLeft(departure),
            false,
            Recovery::Confirmed,
            None,
        )
        .expect("a departure ends the run");
    assert_eq!(ending.status, CommissioningStatus::Failed);
    assert_eq!(
        ending.reason,
        "Main.PV left its permitted range (25 m outside 0 m..10 m); the previous settings were restored."
    );
    let failure = ending.failure.expect("the departure is the failure");
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::ModelOrData, NextAction::None)
    );
    assert!(ending.recovery_confirmed);
    assert_eq!(ending.stage, JournalStage::Recovered);
}

#[test]
fn a_departure_whose_recovery_is_unconfirmed_offers_restore_again() {
    // B6.7: "Restore not confirmed: Restore again; dependent actions stay blocked until confirmed".
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let campaign = campaign(&loop_definition, &model, &task);
    let departure = Failure::new(
        "Main.PV left its permitted range (25 m outside 0 m..10 m)",
        Cause::ModelOrData,
        NextAction::None,
    );
    let ending = campaign
        .settle(
            RunAttempt::PermittedRangeLeft(departure),
            false,
            Recovery::Unconfirmed("communication lost".to_owned()),
            None,
        )
        .expect("a departure ends the run");
    assert!(!ending.recovery_confirmed);
    // The outcome sentence carries no full stop of its own, as the TypeScript's `withRecoveryOutcome` leaves it.
    assert!(
        ending
            .reason
            .ends_with("Recovery unconfirmed: communication lost"),
        "{}",
        ending.reason
    );
    assert_eq!(
        ending.failure.map(|failure| failure.next_action),
        Some(NextAction::Restore)
    );
    assert_eq!(ending.stage, JournalStage::Transfer);
}

#[test]
fn a_refusal_is_recovered_to_the_mode_the_loop_was_found_in() {
    // TS control-studio-apply-process-time, "a failed PID Apply that found the loop in Manual leaves it in Manual" (A6.2, A6.3): the
    // recovered sentence names the mode the loop is confirmed in.
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let in_manual = accept_at(
        &loop_definition,
        &model,
        &task,
        None,
        &status(values(true, 0.0)),
    )
    .expect("the manual reading is accepted");
    let refused = || Failure::new("Staging refused.", Cause::Runtime, NextAction::None);
    let ending = in_manual
        .settle(
            RunAttempt::Refused(refused()),
            false,
            Recovery::Confirmed,
            None,
        )
        .expect("a refusal ends the run");
    assert_eq!(
        ending.reason,
        "Staging refused. Recovery confirmed in Manual; previous controller parameters restored."
    );
    let in_auto = campaign(&loop_definition, &model, &task);
    let ending = in_auto
        .settle(
            RunAttempt::Refused(refused()),
            false,
            Recovery::Confirmed,
            None,
        )
        .expect("a refusal ends the run");
    assert_eq!(
        ending.reason,
        "Staging refused. Recovery confirmed in Auto; previous controller parameters restored."
    );
}

#[test]
fn a_failed_requirement_keeps_the_settings_applied_and_offers_restore() {
    // TS control-studio-apply-process-time, "a failed requirement keeps the reviewed settings applied" (B6.3): the run awaits Restore.
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let campaign = campaign(&loop_definition, &model, &task);
    let requirement = Failure::new(
        "Requirement failed: settling band.",
        Cause::Settings,
        NextAction::Restore,
    );
    let ending = campaign
        .settle(
            RunAttempt::RequirementFailed(requirement),
            false,
            Recovery::NotAttempted,
            None,
        )
        .expect("a failed requirement ends the run");
    assert_eq!(ending.status, CommissioningStatus::Failed);
    assert!(ending.awaiting_restore);
    assert!(ending.recovery_confirmed);
    assert_eq!(
        ending
            .failure
            .map(|failure| (failure.cause, failure.next_action)),
        Some((Cause::Settings, NextAction::Restore))
    );
    assert_eq!(ending.stage, JournalStage::Completed);
}

#[test]
fn a_failed_requirement_whose_settings_cannot_be_read_is_unconfirmed_and_names_the_read() {
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let campaign = campaign(&loop_definition, &model, &task);
    let requirement = Failure::new(
        "Requirement failed: settling band.",
        Cause::Settings,
        NextAction::Restore,
    );
    let ending = campaign
        .settle(
            RunAttempt::RequirementFailed(requirement),
            false,
            Recovery::Unconfirmed("communication lost".to_owned()),
            None,
        )
        .expect("a failed requirement ends the run");
    assert!(!ending.recovery_confirmed);
    assert!(
        ending
            .reason
            .ends_with("The applied settings could not be read: communication lost"),
        "{}",
        ending.reason
    );
    assert!(ending.awaiting_restore);
}

#[test]
fn a_cancelled_run_is_cancelled_and_names_its_recovery() {
    // The cancel names its recovery (execute's catch; A10.4, B6.4).
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let campaign = campaign(&loop_definition, &model, &task);
    let ending = campaign
        .settle(RunAttempt::Cancelled, true, Recovery::Confirmed, None)
        .expect("a cancel ends the run");
    assert_eq!(ending.status, CommissioningStatus::Cancelled);
    assert_eq!(
        ending.reason,
        "Controller commissioning cancelled. Recovery confirmed in Auto; previous controller parameters restored."
    );
    assert!(
        ending.failure.is_none(),
        "a cancel carries no failure of its own"
    );
}

#[test]
fn a_recording_stop_that_is_not_confirmed_is_said_and_unconfirms_the_recovery() {
    // A recording stop that is not confirmed is named (execute's finally; A8.5).
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let campaign = campaign(&loop_definition, &model, &task);
    let ending = campaign
        .settle(
            RunAttempt::Passed { maximum_error: 0.5 },
            false,
            Recovery::NotAttempted,
            Some("the runtime is busy"),
        )
        .expect("a passed run ends");
    assert!(!ending.recovery_confirmed);
    assert!(
        ending.reason.ends_with(
            "Recording stop unconfirmed: the runtime is busy. Cancel retries the owned Stop."
        ),
        "{}",
        ending.reason
    );
    assert_eq!(
        ending.stage,
        JournalStage::Completed,
        "a passed run stays completed"
    );
}

#[test]
fn closing_an_observing_campaign_detaches_it_and_nothing_is_restored() {
    // TS control-studio-apply-process-time, "closing during the observation detaches it at once" and "settling an observing campaign never
    // aborts it" (A8.5, A10.4): a detached observation has no outcome here, and the outcome that comes later keeps Restore offered.
    assert_eq!(close_while_observing(true), CloseAction::Detach);
    assert_eq!(close_while_observing(false), CloseAction::Cancel);
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let campaign = campaign(&loop_definition, &model, &task);
    assert!(campaign
        .settle(RunAttempt::Detached, false, Recovery::NotAttempted, None)
        .is_none());
    let requirement = Failure::new("Requirement failed.", Cause::Settings, NextAction::Restore);
    let outcome = campaign
        .settle(
            RunAttempt::RequirementFailed(requirement),
            false,
            Recovery::NotAttempted,
            None,
        )
        .expect("the outcome of the observation is reported");
    assert_eq!(
        outcome.failure.map(|failure| failure.next_action),
        Some(NextAction::Restore)
    );
}

#[test]
fn the_run_records_the_setpoint_the_measurement_the_command_and_every_additional_signal() {
    // The channels a run records are the setpoint, the measurement, the command and the loop's
    // additional signals (C1, C13.8). The proportional run records the first three.
    let loop_definition = pid_loop("LREAL", secondary_signal());
    let model = first_order_model();
    let task = requirements();
    let campaign = campaign(&loop_definition, &model, &task);
    let channels = campaign.channels().expect("the channels build");
    let names: Vec<(&str, &str)> = channels
        .iter()
        .map(|channel| (channel.id.as_str(), channel.unit.as_str()))
        .collect();
    assert_eq!(
        names,
        vec![
            ("Main.SP", "m"),
            ("Main.PV", "m"),
            ("Main.MV", "V"),
            ("P2.Pressure", "bar")
        ]
    );
}

#[test]
fn the_description_and_the_rollback_description_name_the_structure_and_its_mode() {
    // The description and the rollback description (description, rollbackDescription; B6.2, B6.4).
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let campaign = campaign(&loop_definition, &model, &task);
    assert_eq!(
        campaign.description().expect("the description builds"),
        "Apply the reviewed PID settings and start Auto at 3 m. Monitor for 4 s."
    );
    assert_eq!(
        campaign.rollback_description(),
        "Return to Manual at the current applied output, restore reference 2 m, then restore the previous PID record and return to Auto. Confirm recovery on the reviewed runtime."
    );
}

#[test]
fn the_run_records_the_mode_the_loop_was_found_in() {
    // TS control-studio-apply-process-time, "a Restore reviewed later from Apply's report returns a loop found in Auto to Auto" (A6.2).
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let in_auto = campaign(&loop_definition, &model, &task);
    assert_eq!(in_auto.found_mode(), Some(FoundMode::Auto));
    assert!(in_auto.found_in_auto());
    let in_manual = accept_at(
        &loop_definition,
        &model,
        &task,
        None,
        &status(values(true, 0.0)),
    )
    .expect("the manual reading is accepted");
    assert_eq!(in_manual.found_mode(), Some(FoundMode::Manual));
}

#[test]
fn the_response_starts_at_the_first_auto_scan_at_the_target_and_covers_the_duration_from_there() {
    // TS control-studio-apply-process-time, "the response is recorded for the reviewed duration from the adopted target, so the target change
    // never shortens it" (B6.3): the response starts 2 s in, at the target 7, and its recording covers 4 s from there.
    let sample = |seconds: f64| ResponseSample {
        elapsed_seconds: seconds,
        scan: 0,
        controller_in_auto: true,
        setpoint: Some(if seconds >= 2.0 { 7.0 } else { 5.0 }),
        process_value: Some(5.0),
        applied_output: None,
        quality: ChannelQuality::Good,
        additional: Vec::<AuxiliaryValue>::new(),
    };
    let samples: Vec<ResponseSample> = (0..=8).map(|second| sample(f64::from(second))).collect();
    assert_eq!(response_start_seconds(&samples, 7.0), Some(2.0));
    assert!(response_covers_duration(Some(2.0), 6.0, 4000.0));
    assert!(!response_covers_duration(Some(2.0), 5.9, 4000.0));
    assert_eq!(response_start_seconds(&samples, 9.0), None);
}

#[test]
fn the_observation_request_holds_the_reviewed_duration_at_the_target_and_expects_the_settling_time()
{
    // TS control-studio-apply-process-time, "records the applied response at a record period that follows the requirements' settling time"
    // (A4.2, B6.3).
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let request = campaign(&loop_definition, &model, &task)
        .response_request()
        .expect("the request builds");
    assert_eq!(request.duration_ms, 4000.0);
    assert_eq!(request.target, 3.0);
    assert_eq!(request.expected_settling_seconds, Some(2.0));
}

#[test]
fn the_starting_point_compares_the_plant_input_only_for_a_loop_found_in_manual() {
    // The wait compares the plant input that the found mode compares, at the allowance of the reading (B6.2, A6.4).
    let loop_definition = pid_loop("LREAL", json!([]));
    let model = first_order_model();
    let task = requirements();
    let campaign = campaign(&loop_definition, &model, &task);
    let in_auto = status(values(false, 0.36));
    assert_eq!(
        campaign
            .start
            .starting_point(&in_auto.values)
            .expect("the point builds")
            .input_symbols,
        [] as [String; 0]
    );
    let in_manual = status(values(true, 0.36));
    let point = campaign
        .start
        .starting_point(&in_manual.values)
        .expect("the point builds");
    assert_eq!(point.input_symbols, ["Main.MV".to_owned()]);
    assert!(point.input_tolerance > 0.0);
}

#[test]
fn the_declared_structure_takes_its_own_label_and_tracking_kind() {
    // A loop whose adapter declares states (C1, `control-pid-v1`) commissions as the declared structure, labelled `Control.PID`.
    let loop_definition = library_pid_loop();
    let model = first_order_model();
    let adapter = controller_adapter(&loop_definition).expect("the library PID is adapted");
    let declared = adapter
        .declared_profile(&model, &BTreeMap::new())
        .expect("the declared profile builds");
    let profile = PidRunProfile::from_adapter(declared);
    assert_eq!(profile.label(), "Control.PID");
    assert_eq!(profile.tracking_kind(), "declared-state-v1");
    assert_eq!(
        PidRunProfile::from_adapter(None),
        PidRunProfile::ParallelPid
    );
    assert_eq!(PidRunProfile::ParallelPid.label(), "PID");
    assert_eq!(
        PidRunProfile::ParallelPid.tracking_kind(),
        "parallel-pid-v1"
    );
}
