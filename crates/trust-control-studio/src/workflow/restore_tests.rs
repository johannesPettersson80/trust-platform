// Restore previous settings on the scripted runtime of `restore/test_runtime.rs` (specification 35, B6.4, B6.7, A6.2, A8.5; annex E7.5,
// E11.4): the review, which writes nothing, the one Restore from the journaled checkpoint, its refusals, and the steps it does not repeat.
// Each test names the TypeScript test it ports and the section it checks.

#![allow(clippy::float_cmp)] // The tests compare the values the scripted runtime holds, exactly, as the TypeScript tests do.

use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use serde_json::{json, Value};
use trust_control::commissioning::application_texts::physical_approval_needed;
use trust_control::commissioning::commissioning_result::{
    CommissioningResult, CommissioningStatus,
};
use trust_control::commissioning::found_mode::FoundMode;
use trust_control::commissioning::parameter_status::RecordState;
use trust_control::commissioning::texts::{pid_commission_ownership_changed, restore_unavailable};
use trust_control::failure::Cause;
use trust_control::json::decode_value;
use trust_control::loop_definition::Range;
use trust_control::messages::OperationUpdate;
use trust_control::recording::Completeness;
use trust_control::study::apply::ApplicationReceipt;
use trust_control::validation::commissioning_assessment::CommissioningCriteria;

use super::test_runtime::{
    candidate_gains, gains, held_run, origin_gains, recording_owner, setup, Kind, Setup, World,
    NOISE, PARAMETER_BINDING, PROGRAM, TRANSFER_BINDING,
};
use super::*;
use crate::journal::Journal;
use crate::operation::{Operations, UpdateSink};
use crate::runtime_client::fake_runtime::FakeRuntime;

/// An update sink that drops every update: the tests read the outcome itself.
struct Quiet;

impl UpdateSink for Quiet {
    fn send(&self, _update: OperationUpdate) {}
}

/// A journal folder of the test's own, removed when the test ends.
struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("trust-cs-restore-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The loop's journal in `folder`, opened as the engine opens it for one operation (A8.3).
fn open_journal(folder: &Folder, setup: &Setup) -> Journal {
    Journal::open(
        folder.path(),
        &setup.loop_definition.project_id,
        &setup.loop_definition.id,
    )
    .expect("the journal opens")
}

/// Journals the checkpoint as Apply does before its first write and as each dispatch rewrites it (A8.5, E11.2); the journal is then
/// closed, as a stopped engine leaves it.
fn journal_checkpoint(folder: &Folder, setup: &Setup) {
    let mut journal = open_journal(folder, setup);
    SharedJournal::new(&mut journal)
        .write_checkpoint(&setup.checkpoint)
        .expect("the checkpoint is journaled");
}

/// The checkpoint the journal holds now, read as a restarted engine reads it.
fn journaled(folder: &Folder, setup: &Setup) -> Option<CommissioningCheckpoint> {
    SharedJournal::new(&mut open_journal(folder, setup)).checkpoint()
}

/// The one Restore (`restore_with_approval`) from the journaled checkpoint, as Apply's failure path calls it, with no physical approval.
fn restore(fake: &FakeRuntime, folder: &Folder, setup: &Setup) -> Result<RestoreOutcome, Failure> {
    let mut journal = open_journal(folder, setup);
    let shared = SharedJournal::new(&mut journal);
    let checkpoint = shared.checkpoint().expect("the checkpoint is journaled");
    let mut client =
        RuntimeClient::connect(&fake.connection(None)).expect("the scripted runtime answers");
    restore_with_approval(
        &mut client,
        &setup.loop_definition,
        &shared,
        checkpoint,
        false,
        &|_: &str| {},
    )
}

/// The review of a Restore (`review_restore`), in the context the method gives it.
fn review(
    fake: &FakeRuntime,
    folder: &Folder,
    setup: &Setup,
    study: &Study,
) -> Result<ApplicationReview, Failure> {
    let mut journal = open_journal(folder, setup);
    let connection = fake.connection(None);
    let mut context = ApplyContext {
        loop_definition: &setup.loop_definition,
        study,
        connection: &connection,
        journal: &mut journal,
        physical_approved: false,
        project_root: folder.path(),
    };
    review_restore(&mut context)
}

/// The confirmed Restore as the method runs it (`run_restore`, on the operation's own thread), with the review identity `identity`;
/// `cancel_first` cancels the operation before it begins.
fn confirm(
    fake: &FakeRuntime,
    folder: &Folder,
    setup: &Setup,
    identity: &str,
    cancel_first: bool,
) -> Result<RestoreOutcome, Failure> {
    let operations = Operations::new(Arc::new(Quiet));
    let (outcome, received) = mpsc::channel();
    let (go, wait) = mpsc::channel::<()>();
    let loop_definition = setup.loop_definition.clone();
    let connection = fake.connection(None);
    let root = folder.path().to_path_buf();
    let identity = identity.to_owned();
    let started = operations.start("restore", move |operation| {
        wait.recv().map_err(|_| cancelled())?;
        let mut journal = Journal::open(&root, &loop_definition.project_id, &loop_definition.id)?;
        let study = Study::new(&loop_definition.id, &loop_definition.revision);
        let mut context = ApplyContext {
            loop_definition: &loop_definition,
            study: &study,
            connection: &connection,
            journal: &mut journal,
            physical_approved: false,
            project_root: &root,
        };
        let _ = outcome.send(run_restore(&mut context, operation, &identity));
        Ok(Value::Null)
    });
    if cancel_first {
        operations
            .cancel(&started.operation_id)
            .expect("the operation is known");
    }
    go.send(()).expect("the operation waits for its start");
    received
        .recv_timeout(Duration::from_secs(60))
        .expect("the Restore ends")
}

/// The write and cancel requests the runtime received: none for a review or a refusal (V4 decision 1).
fn writes(fake: &FakeRuntime) -> Vec<Value> {
    fake.requests()
        .into_iter()
        .filter(|request| {
            matches!(
                request["type"].as_str(),
                Some(
                    "control.transfer.apply"
                        | "control.transfer.cancel"
                        | "control.parameters.apply"
                        | "control.parameters.rollback"
                        | "control.parameters.cancel"
                        | "control.recording.stop"
                        | "control.recording.release"
                )
            )
        })
        .collect()
}

/// The parameters of each transfer apply the runtime received, oldest first.
fn transfers(fake: &FakeRuntime) -> Vec<Value> {
    fake.requests_of("control.transfer.apply")
        .into_iter()
        .map(|request| request["params"].clone())
        .collect()
}

/// The record Apply left, as its receipt holds it: the candidate in force under `active-candidate`, the origin retained (E7.2).
fn applied_record(kind: Kind) -> ParameterStatus {
    ParameterStatus {
        supports_request_id: Some(true),
        transaction_id: Some("apply-1".to_owned()),
        program_revision: PROGRAM.to_owned(),
        binding_revision: PARAMETER_BINDING.to_owned(),
        active_revision: "active-candidate".to_owned(),
        values: candidate_gains(kind),
        status: RecordState::Applied,
        previous_revision: Some("active-origin".to_owned()),
        previous_values: Some(origin_gains(kind)),
        reason: None,
    }
}

/// A study whose Apply section holds the receipt of the checkpoint's Apply, with the record that Apply left (A8.1).
fn study_with_application(setup: &Setup, parameters: ParameterStatus) -> Study {
    let mut study = Study::new(&setup.loop_definition.id, &setup.loop_definition.revision);
    let criteria = CommissioningCriteria {
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
    };
    let result = CommissioningResult {
        status: CommissioningStatus::Passed,
        reason: "PID Auto commissioning passed: recorded target error 0.01 m; controller remains in Auto."
            .to_owned(),
        recording: None,
        transfers: Vec::new(),
        recovery_confirmed: true,
        maximum_error: Some(0.01),
        target: 3.0,
        parameters: Some(parameters),
        model_revision: "model:1".to_owned(),
        channels: Vec::new(),
        criteria,
        gain_recovery: None,
        failure: None,
        found_mode: Some(FoundMode::Auto),
    };
    let section = ApplySection {
        applications: vec![ApplicationReceipt {
            checkpoint: setup.checkpoint.id.clone(),
            validation_identity: setup.checkpoint.validation_identity.clone(),
            settings_applied: true,
            result,
        }],
        restorations: Vec::new(),
        keeps: Vec::new(),
    };
    study
        .set_section(&section)
        .expect("the Apply section holds");
    study
}

/// A study without an Apply section.
fn empty_study(setup: &Setup) -> Study {
    Study::new(&setup.loop_definition.id, &setup.loop_definition.revision)
}

/// The requests that end Apply's operations and roll the record back, as a Restore of `kind`'s checkpoint sends them. The owned
/// operations end by their identities, never by a replay (E11.4); the rollback names the retained revision and the record it replaces,
/// and no values of its own (control-studio-parameter-application.test.ts, "rolls back the exact retained values and confirms them
/// after execution", E7.5).
fn assert_rollback_and_cancels(fake: &FakeRuntime, kind: Kind) {
    assert_eq!(
        fake.requests_of("control.parameters.cancel")[0]["params"]["transaction_id"],
        "apply-1"
    );
    assert_eq!(
        fake.requests_of("control.transfer.cancel")[0]["params"]["transaction_id"],
        "target-1"
    );
    let rollbacks = fake.requests_of("control.parameters.rollback");
    assert_eq!(rollbacks.len(), 1);
    let rollback = &rollbacks[0]["params"];
    assert_eq!(rollback["previous_revision"], "active-origin");
    assert_eq!(rollback["expected_active_revision"], "active-candidate");
    let expected: BTreeMap<String, f64> =
        decode_value(rollback["expected_values"].clone()).expect("the expected values decode");
    assert_eq!(expected, candidate_gains(kind));
    assert!(
        rollback.get("values").is_none(),
        "a rollback names no values"
    );
}

/// Checklist V4.6, the regression at the workflow level, porting control-studio-found-mode.test.ts "a Control.PID loop found in Auto
/// returns to Auto through its declared initialization, allowing Apply's measured noise" (B6.4, A6.2, A6.4, A8.5): the checkpoint as
/// Apply journals it, the engine restarted (the journal closed and opened again), then `review_restore` and `run_restore`. Expected: the
/// record back at Kp 1, Ki 0, Kd 0 under active-origin, the loop in Auto, two transfers (Manual, then Auto through the declared-state
/// tracking aligned to the output, allowing 2·Kp·n = 0.1), the owned operations ended by their identities, and no checkpoint left.
#[test]
fn a_control_pid_loop_found_in_auto_returns_to_auto_after_an_engine_restart_with_the_noise_allowance(
) {
    let setup = setup(
        Kind::Library,
        Some(FoundMode::Auto),
        CheckpointStage::Completed,
    );
    let folder = Folder::new("library-restart");
    journal_checkpoint(&folder, &setup);
    let (fake, world) = World::applied(&setup, true).serve();

    let review = review(&fake, &folder, &setup, &empty_study(&setup)).expect("the review holds");
    assert_eq!(review.action, ReviewedAction::Restore);
    assert_eq!(review.found_mode, Some(FoundMode::Auto));
    assert!(!review.physical, "a simulated target");
    assert!(
        review.identity.starts_with("sha256:"),
        "{}",
        review.identity
    );
    assert_eq!(
        review.description,
        "Return to Manual at the current output and restore the previous settings. Then the loop returns to Auto, the mode it was found in."
    );
    let rows: Vec<(&str, f64, f64)> = review
        .parameters
        .iter()
        .map(|row| (row.symbol.as_str(), row.running, row.next))
        .collect();
    assert_eq!(
        rows,
        [
            ("Main.Parameters.Kp", 2.0, 1.0),
            ("Main.Parameters.Ki", 0.5, 0.0),
            ("Main.Parameters.Kd", 0.0, 0.0)
        ]
    );
    assert_eq!(writes(&fake), [] as [Value; 0], "the review writes nothing");

    let outcome = confirm(&fake, &folder, &setup, &review.identity, false)
        .expect("the runtime confirms the Restore");
    assert!(outcome.restored);
    assert_eq!(outcome.mode, Some(FoundMode::Auto));
    assert_eq!(
        outcome.reason,
        "Previous values restored and observed after ST execution; the loop returned to Auto."
    );
    assert_eq!(outcome.parameters.values, origin_gains(Kind::Library));
    assert_eq!(outcome.parameters.active_revision, "active-origin");
    {
        let world = world.lock().expect("the scripted runtime");
        assert!(
            world.in_auto(),
            "the loop returns to the mode it was found in"
        );
        assert_eq!(world.parameter_values(), origin_gains(Kind::Library));
    }
    let transfers = transfers(&fake);
    assert_eq!(transfers.len(), 3, "{transfers:?}");
    assert_eq!(
        transfers[0]["manual"], true,
        "the first write returns the loop to Manual"
    );
    assert_eq!(transfers[0]["tracking"]["align_output"], false);
    assert_eq!(
        transfers[1]["manual"], false,
        "the second write enters Auto"
    );
    assert_eq!(
        transfers[1]["tracking"]["kind"], "declared-state-v1",
        "the entry goes through the declared initialization, as Apply's does"
    );
    assert_eq!(
        transfers[1]["tracking"]["align_output"], true,
        "the entry starts from the current output"
    );
    let tolerance = transfers[1]["maximum_output_change"]
        .as_f64()
        .expect("the allowance is a number");
    assert!(
        (tolerance - 2.0 * 1.0 * NOISE).abs() < 1e-12,
        "A6.4 gives {}, the entry allows {tolerance}",
        2.0 * NOISE
    );
    // The entry aligned the reference to the measurement; the last write returns the reference Apply found (B6.4).
    assert_reference_step(&transfers[2], &setup);
    assert_rollback_and_cancels(&fake, Kind::Library);
    assert_eq!(
        journaled(&folder, &setup),
        None,
        "a confirmed Restore ends the checkpoint"
    );
}

/// control-studio-found-mode.test.ts, "a PID loop found in Auto returns to Auto once its previous settings are back" (A6.2, B6.4): Kp back
/// at 1, the loop in Auto, the last write enters Auto through the parallel PID tracking aligned to the output, and the reason says so.
#[test]
fn a_pid_loop_found_in_auto_returns_to_auto_once_its_previous_settings_are_back() {
    let setup = setup(
        Kind::Pid,
        Some(FoundMode::Auto),
        CheckpointStage::Parameters,
    );
    let folder = Folder::new("pid-auto");
    journal_checkpoint(&folder, &setup);
    let (fake, world) = World::applied(&setup, true).serve();
    let outcome = restore(&fake, &folder, &setup).expect("the runtime confirms the Restore");
    {
        let world = world.lock().expect("the scripted runtime");
        assert_eq!(world.number("Main.Kp"), 1.0, "the previous gain is back");
        assert!(
            world.in_auto(),
            "the loop returns to the mode it was found in"
        );
    }
    let transfers = transfers(&fake);
    let last = transfers.last().expect("the Restore wrote transfers");
    assert_eq!(last["manual"], false, "the last write enters Auto");
    assert_eq!(last["tracking"]["kind"], "parallel-pid-v1");
    assert_eq!(last["tracking"]["align_output"], true);
    assert_eq!(
        outcome.reason,
        "Previous values restored and observed after ST execution; the loop returned to Auto."
    );
    assert_eq!(outcome.mode, Some(FoundMode::Auto));
    assert_eq!(
        outcome.recording, None,
        "the checkpoint retains no recording"
    );
    assert!(
        !outcome.restore_available && !outcome.keep_available,
        "a confirmed Restore offers neither Restore nor Keep in project (B6.4, B6.5)"
    );
    assert_eq!(journaled(&folder, &setup), None);
}

/// A8.6, B6.4: an opened loop offers Restore while its journal holds the Apply's checkpoint, and stops offering it once a confirmed
/// Restore clears the checkpoint. Keep in project follows the study's receipts, so a study without an Apply offers none.
#[test]
fn an_opened_loop_offers_restore_only_while_the_journal_holds_the_checkpoint() {
    let setup = setup(
        Kind::Pid,
        Some(FoundMode::Auto),
        CheckpointStage::Parameters,
    );
    let folder = Folder::new("offers");
    journal_checkpoint(&folder, &setup);
    let (fake, _world) = World::applied(&setup, true).serve();
    let journal = open_journal(&folder, &setup);
    let offers = crate::commands::apply_offers(
        journal.entries(),
        &trust_control::study::apply::ApplySection::default(),
    );
    assert!(offers.restore, "the journaled checkpoint offers Restore");
    assert!(!offers.keep, "no receipt kept, so no Keep in project");
    assert!(
        !offers.observation_resumable,
        "a checkpoint without a recording has no observation to follow"
    );
    drop(journal);
    restore(&fake, &folder, &setup).expect("the runtime confirms the Restore");
    let journal = open_journal(&folder, &setup);
    let offers = crate::commands::apply_offers(
        journal.entries(),
        &trust_control::study::apply::ApplySection::default(),
    );
    assert!(
        !offers.restore,
        "the confirmed Restore cleared the checkpoint"
    );
}

/// A2.5 and B9.3: the Restore's review names each signal it returns with the limits the loop declares for it, names its restoration and
/// stop conditions, and has no duration, because a Restore records nothing.
#[test]
fn a_restore_review_names_each_signal_with_its_declared_limits_and_no_duration() {
    let setup = setup(
        Kind::Pid,
        Some(FoundMode::Auto),
        CheckpointStage::Parameters,
    );
    let folder = Folder::new("approval");
    journal_checkpoint(&folder, &setup);
    let (fake, _world) = World::applied(&setup, true).serve();
    let review = review(&fake, &folder, &setup, &empty_study(&setup)).expect("the review holds");
    let approval = &review.approval;
    assert_eq!(approval.loop_id, setup.loop_definition.id);
    assert_eq!(approval.duration_seconds, None, "a Restore records nothing");
    assert_eq!(approval.signals.len(), review.parameters.len());
    for (signal, parameter) in approval.signals.iter().zip(&review.parameters) {
        assert_eq!(signal.symbol, parameter.symbol);
        assert_eq!(
            (signal.running, signal.next),
            (parameter.running, parameter.next)
        );
        let declared = setup
            .loop_definition
            .parameter_binding
            .iter()
            .flat_map(|binding| binding.parameters.iter())
            .find(|declared| declared.symbol == signal.symbol)
            .expect("the record declares every signal the Restore writes");
        assert_eq!(
            (signal.minimum, signal.maximum),
            (declared.minimum, declared.maximum)
        );
    }
    assert!(approval.restoration.starts_with("The parameter record"));
    assert_eq!(approval.stop_conditions.len(), 1);
}

/// B6.4, B6.7: a Restore that the runtime refuses answers no outcome, so no availability is shown for it, and the checkpoint stays
/// journaled for another try.
#[test]
fn a_refused_restore_answers_no_outcome_and_keeps_the_checkpoint_for_another_try() {
    let setup = setup(
        Kind::Pid,
        Some(FoundMode::Auto),
        CheckpointStage::Parameters,
    );
    let folder = Folder::new("unconfirmed");
    journal_checkpoint(&folder, &setup);
    let mut world = World::applied(&setup, true);
    world.program = "program-2".to_owned();
    let (fake, _world) = world.serve();
    review(&fake, &folder, &setup, &empty_study(&setup))
        .expect_err("Restore is unavailable after a runtime change");
    assert!(
        journaled(&folder, &setup).is_some(),
        "the checkpoint stays journaled for another try (B6.7)"
    );
}

/// control-studio-found-mode.test.ts, "a proportional loop found in Auto returns to Auto once its previous gain is back" (A6.2, C13.3):
/// Kp back at 1, the loop in Auto, and the entry aligns the reference to r = PV + u/Kp = 2.004 + 0/1 with the Manual command at the held
/// output, 0 V.
#[test]
fn a_proportional_loop_found_in_auto_returns_to_auto_aligned_to_the_current_output_at_its_previous_gain(
) {
    let setup = setup(
        Kind::Proportional,
        Some(FoundMode::Auto),
        CheckpointStage::Parameters,
    );
    let folder = Folder::new("proportional-auto");
    journal_checkpoint(&folder, &setup);
    let (fake, world) = World::applied(&setup, true).serve();
    let outcome = restore(&fake, &folder, &setup).expect("the runtime confirms the Restore");
    let transfers = transfers(&fake);
    assert_eq!(
        transfers.len(),
        3,
        "Manual, the Auto entry and the reference step: {transfers:?}"
    );
    // The entry aligned the reference to r = PV + u/Kp; the last write returns the reference Apply found (B6.4, C13 step 8).
    assert_reference_step(&transfers[2], &setup);
    let entry = &transfers[1];
    assert_eq!(entry["manual"], false, "the second write enters Auto");
    assert_eq!(entry["tracking"]["kind"], "proportional");
    assert_eq!(entry["tracking"]["align_reference"], true);
    let values: BTreeMap<String, f64> =
        decode_value(entry["values"].clone()).expect("the entry's values decode");
    assert_eq!(
        values,
        BTreeMap::from([
            ("Main.SP".to_owned(), 2.004 + 0.0 / 1.0),
            ("Main.ManualCommand".to_owned(), 0.0),
        ])
    );
    {
        let world = world.lock().expect("the scripted runtime");
        assert_eq!(world.number("Main.Kp"), 1.0, "the previous gain is back");
        assert!(world.in_auto());
    }
    assert_eq!(outcome.mode, Some(FoundMode::Auto));
}

/// control-studio-found-mode.test.ts, "a loop found in Manual stays in Manual, and no Auto entry is written" (A6.2, B6.4): the loop is in
/// Manual with its reference at the value Apply found, so the Restore writes no transfer at all, only the rollback of E7.5.
#[test]
fn a_loop_found_in_manual_with_its_reference_held_is_restored_without_any_transfer() {
    let setup = setup(
        Kind::Pid,
        Some(FoundMode::Manual),
        CheckpointStage::Completed,
    );
    let folder = Folder::new("pid-manual");
    journal_checkpoint(&folder, &setup);
    let (fake, world) = World::applied(&setup, false).serve();
    let outcome = restore(&fake, &folder, &setup).expect("the runtime confirms the Restore");
    assert_eq!(transfers(&fake), [] as [Value; 0], "no transfer is written");
    assert_eq!(fake.requests_of("control.parameters.rollback").len(), 1);
    {
        let world = world.lock().expect("the scripted runtime");
        assert_eq!(world.number("Main.Kp"), 1.0, "the previous gain is back");
        assert!(!world.in_auto(), "the loop stays in Manual");
    }
    assert_eq!(outcome.mode, Some(FoundMode::Manual));
    assert_eq!(
        outcome.reason,
        "Previous values restored and observed after ST execution; the loop stays in Manual."
    );
}

/// control-studio-found-mode.test.ts, "a checkpoint written before the mode was journalled stays in Manual and says that the mode is
/// unknown" (A6.2): the review says so, the Restore leaves the loop in Manual, and its reason names the unknown mode.
#[test]
fn a_checkpoint_without_its_found_mode_is_restored_in_manual_and_says_the_mode_is_unknown() {
    let setup = setup(Kind::Pid, None, CheckpointStage::Parameters);
    let folder = Folder::new("pid-unknown");
    journal_checkpoint(&folder, &setup);
    let (fake, world) = World::applied(&setup, true).serve();
    let review = review(&fake, &folder, &setup, &empty_study(&setup)).expect("the review holds");
    assert_eq!(review.found_mode, None);
    assert!(
        review
            .description
            .ends_with("The mode the loop was found in is unknown, so it stays in Manual."),
        "{}",
        review.description
    );
    let outcome = restore(&fake, &folder, &setup).expect("the runtime confirms the Restore");
    {
        let world = world.lock().expect("the scripted runtime");
        assert_eq!(world.number("Main.Kp"), 1.0, "the previous gain is back");
        assert!(!world.in_auto(), "the restore leaves the loop in Manual");
    }
    assert!(
        transfers(&fake)
            .iter()
            .all(|transfer| transfer["manual"] == true),
        "no Auto entry is written"
    );
    assert!(
        outcome
            .reason
            .contains("the mode it was found in is unknown"),
        "{}",
        outcome.reason
    );
    assert_eq!(outcome.mode, Some(FoundMode::Manual));
}

/// B6.4 (`reviewInterruptedRollback`, engineerActions/recovery.ts): the runtime runs program-2 now, not the program-1 Apply wrote to. The
/// review refuses, naming both programs, with a new validation as the next action; the confirmation refuses the same way; nothing is
/// written, and the checkpoint stays journaled as history.
#[test]
fn a_runtime_restarted_since_apply_makes_restore_unavailable_and_keeps_the_checkpoint_journaled() {
    let setup = setup(Kind::Pid, Some(FoundMode::Auto), CheckpointStage::Completed);
    let folder = Folder::new("replaced");
    journal_checkpoint(&folder, &setup);
    let mut world = World::applied(&setup, true);
    world.program = "program-2".to_owned();
    let (fake, _world) = world.serve();
    let refusal =
        review(&fake, &folder, &setup, &empty_study(&setup)).expect_err("Restore is unavailable");
    assert_eq!(
        (refusal.cause, refusal.next_action),
        (Cause::Runtime, NextAction::Validate)
    );
    assert!(
        refusal.message.starts_with(
            "Restore is unavailable: the runtime was restarted or its program reloaded since Apply"
        ),
        "{}",
        refusal.message
    );
    assert!(
        refusal
            .message
            .contains("program revision program-1 at Apply, program-2 now"),
        "{}",
        refusal.message
    );
    let refusal = confirm(&fake, &folder, &setup, "sha256:reviewed-before", false)
        .expect_err("the confirmation refuses too");
    assert_eq!(refusal.next_action, NextAction::Validate);
    assert_eq!(writes(&fake), [] as [Value; 0], "nothing is written");
    assert_eq!(
        journaled(&folder, &setup).map(|checkpoint| (checkpoint.id, checkpoint.stage)),
        Some(("checkpoint-1".to_owned(), CheckpointStage::Completed)),
        "the checkpoint stays journaled as history"
    );
}

/// B6.4 (`restoreUnavailableReason`, restoreAvailability.ts): another write replaced the settings Apply wrote (Kp 3 under active-other).
/// With the Apply's receipt the review refuses with the reason of `restore_availability`; without it, the ownership check refuses
/// (`assertParameterOwner`). Nothing is written, and the checkpoint stays journaled.
#[test]
fn a_record_another_write_replaced_since_apply_makes_restore_unavailable() {
    let setup = setup(Kind::Pid, Some(FoundMode::Auto), CheckpointStage::Completed);
    let folder = Folder::new("replaced-record");
    journal_checkpoint(&folder, &setup);
    let mut world = World::applied(&setup, true);
    world.write_numbers(&gains(Kind::Pid, 3.0, 0.5, 0.0));
    world.active = "active-other".to_owned();
    world.previous = Some(("active-candidate".to_owned(), candidate_gains(Kind::Pid)));
    world.parameter_transaction = Some("someone-else".to_owned());
    let (fake, _world) = world.serve();
    let study = study_with_application(&setup, applied_record(Kind::Pid));
    let refusal = review(&fake, &folder, &setup, &study).expect_err("Restore is unavailable");
    assert_eq!(
        refusal,
        Failure::new(&restore_unavailable(), Cause::Settings, NextAction::None)
    );
    let refusal = review(&fake, &folder, &setup, &empty_study(&setup))
        .expect_err("another operation owns the record");
    assert_eq!(
        refusal,
        Failure::new(
            &pid_commission_ownership_changed(),
            Cause::Runtime,
            NextAction::None
        )
    );
    assert_eq!(writes(&fake), [] as [Value; 0], "nothing is written");
    assert!(journaled(&folder, &setup).is_some());
}

/// interruptedCommissioningRecovery.ts `review` (B6.4, E11.7): a pending parameter transaction that is not the checkpoint's own, and a
/// pending transfer of another owner, each refuse before anything is written.
#[test]
fn a_parameter_or_transfer_transaction_another_operation_holds_pending_refuses_the_review() {
    let setup = setup(Kind::Pid, Some(FoundMode::Auto), CheckpointStage::Completed);
    let folder = Folder::new("pending");
    journal_checkpoint(&folder, &setup);

    let mut world = World::applied(&setup, true);
    world.parameter_state = "pending";
    world.parameter_transaction = Some("someone-else".to_owned());
    let (fake, _world) = world.serve();
    let refusal = review(&fake, &folder, &setup, &empty_study(&setup))
        .expect_err("another parameter transaction is pending");
    assert_eq!(refusal.message, pid_commission_ownership_changed());
    assert_eq!(writes(&fake), [] as [Value; 0]);

    let mut world = World::applied(&setup, true);
    world.transfer_state = "pending";
    world.transfer_transaction = Some(json!({
        "id": "transfer-9", "program_revision": PROGRAM, "binding_revision": TRANSFER_BINDING, "status": "pending",
        "reason": null, "observed_values": null, "confirmation": null
    }));
    let (fake, _world) = world.serve();
    let refusal = review(&fake, &folder, &setup, &empty_study(&setup))
        .expect_err("another transfer is pending");
    assert_eq!(
        refusal.message,
        "Another program or transfer owns the current runtime; recovery cannot overwrite it."
    );
    assert_eq!(writes(&fake), [] as [Value; 0]);
}

/// B6.4 ("Restored is shown only after runtime confirmation"), E7.4: the runtime reports the accepted rollback pending for two status
/// reads; the Restore waits and confirms it from the third, which reports it applied under active-origin.
#[test]
fn the_rollback_is_confirmed_only_once_the_runtime_reports_it_applied() {
    let setup = setup(
        Kind::Pid,
        Some(FoundMode::Manual),
        CheckpointStage::Completed,
    );
    let folder = Folder::new("confirmation");
    journal_checkpoint(&folder, &setup);
    let mut world = World::applied(&setup, false);
    world.held_rollback_reads = 2;
    let (fake, _world) = world.serve();
    let outcome = restore(&fake, &folder, &setup).expect("the runtime confirms the Restore");
    assert!(outcome.restored);
    assert_eq!(outcome.parameters.active_revision, "active-origin");
    assert_eq!(outcome.parameters.values, origin_gains(Kind::Pid));
    let requests = fake.requests();
    let sent = requests
        .iter()
        .position(|request| request["type"] == "control.parameters.rollback")
        .expect("the rollback is sent");
    let reads = requests[sent..]
        .iter()
        .filter(|request| request["type"] == "control.parameters.status")
        .count();
    assert!(
        reads >= 3,
        "two pending reads and the applied one, {reads} read"
    );
}

/// B6.4 ("A step the runtime already confirmed is not repeated"), B6.7 ("Restore not confirmed: Restore again"), and
/// control-studio-apply-reattach.test.ts "a retained recording the runtime no longer holds is stored as incomplete evidence, so the
/// recovery goes on" (stopRecovered, A8.6): the checkpoint holds a recording the runtime no longer holds, and the runtime rejects the
/// first Auto entry. The first Restore returns the record, keeps the recording as incomplete evidence, and fails with Restore again, the
/// checkpoint journaling the rollback's and the rejected entry's identities. The second writes only the Auto entry.
#[test]
fn a_restore_interrupted_after_its_rollback_resumes_without_repeating_a_confirmed_step() {
    let mut setup = setup(Kind::Pid, Some(FoundMode::Auto), CheckpointStage::Recording);
    setup.checkpoint.recording = Some(recording_owner(&setup.loop_definition));
    let folder = Folder::new("resume");
    journal_checkpoint(&folder, &setup);
    let mut world = World::applied(&setup, true);
    world.rejected_auto_entries = 1;
    let (fake, world) = world.serve();

    let failure = restore(&fake, &folder, &setup).expect_err("the runtime rejects the Auto entry");
    assert_eq!(failure.next_action, NextAction::Restore);
    let checkpoint = journaled(&folder, &setup).expect("the checkpoint stays journaled");
    let rollback = fake.requests_of("control.parameters.rollback");
    assert_eq!(
        Some(checkpoint.parameter_operation.id.as_str()),
        rollback[0]["params"]["transaction_id"].as_str(),
        "the rollback's identity was journaled before it was sent"
    );
    let written = transfers(&fake);
    assert_eq!(written.len(), 2, "Manual, then the rejected entry");
    assert_eq!(
        checkpoint.transfer.as_ref().map(|owner| owner.id.as_str()),
        written[1]["transaction_id"].as_str(),
        "the rejected entry's identity was journaled before it was sent"
    );
    assert_eq!(checkpoint.stage, CheckpointStage::Transfer);
    let kept = checkpoint
        .recording_result
        .as_ref()
        .expect("the recording is kept in the checkpoint");
    assert_eq!(kept.acquisition.completeness, Completeness::Incomplete);
    assert_eq!(kept.samples.len(), 0);
    assert!(
        kept.acquisition
            .reason
            .starts_with("The runtime no longer holds this recording."),
        "{}",
        kept.acquisition.reason
    );
    {
        let world = world.lock().expect("the scripted runtime");
        assert_eq!(world.number("Main.Kp"), 1.0, "the previous gain is back");
        assert!(
            !world.in_auto(),
            "the rejected entry left the loop in Manual"
        );
    }

    let outcome = restore(&fake, &folder, &setup).expect("the second Restore is confirmed");
    assert_eq!(outcome.mode, Some(FoundMode::Auto));
    let evidence = outcome
        .recording
        .as_ref()
        .expect("the outcome hands over the evidence the checkpoint kept");
    assert_eq!(evidence.recording_id, "recording-1");
    assert_eq!(evidence.acquisition.completeness, Completeness::Incomplete);
    assert_eq!(
        fake.requests_of("control.parameters.rollback").len(),
        1,
        "the confirmed rollback is not repeated"
    );
    assert_eq!(
        fake.requests_of("control.recording.stop").len(),
        1,
        "the kept recording is not stopped again"
    );
    assert_eq!(
        fake.requests_of("control.recording.release").len(),
        0,
        "a recording the runtime does not hold is not released"
    );
    let written = transfers(&fake);
    assert_eq!(
        written.len(),
        3,
        "the second Restore writes only the Auto entry"
    );
    assert_eq!(written[2]["manual"], false);
    assert!(world.lock().expect("the scripted runtime").in_auto());
    assert_eq!(journaled(&folder, &setup), None);
}

/// E4.7, E11.6 (interruptedCommissioningRecovery.ts `rollback`, runtimeRecording.ts): a recording the checkpoint holds without its result
/// is stopped, its run is kept and handed over in the outcome (`recording-1`, as the runtime stopped it), and its buffer is released only
/// after the stop.
#[test]
fn a_recording_the_checkpoint_holds_is_stopped_before_its_buffer_is_released() {
    let mut setup = setup(
        Kind::Pid,
        Some(FoundMode::Manual),
        CheckpointStage::Recording,
    );
    let owner = recording_owner(&setup.loop_definition);
    setup.checkpoint.recording = Some(owner.clone());
    let folder = Folder::new("recording");
    journal_checkpoint(&folder, &setup);
    let mut world = World::applied(&setup, false);
    world
        .recordings
        .insert(owner.recording_id.clone(), held_run(&owner));
    let (fake, world) = world.serve();
    let outcome = restore(&fake, &folder, &setup).expect("the runtime confirms the Restore");
    assert_eq!(
        outcome
            .recording
            .as_ref()
            .map(|run| (run.recording_id.as_str(), run.acquisition.reason.as_str())),
        Some(("recording-1", "Stopped by the Restore.")),
        "the outcome hands over the run the runtime stopped"
    );
    let requests = fake.requests();
    let stop = requests
        .iter()
        .position(|request| request["type"] == "control.recording.stop")
        .expect("the recording is stopped");
    let release = requests
        .iter()
        .position(|request| request["type"] == "control.recording.release")
        .expect("its buffer is released");
    assert!(
        stop < release,
        "the run is kept before the buffer is released"
    );
    assert!(world
        .lock()
        .expect("the scripted runtime")
        .recordings
        .is_empty());
}

/// control-studio-apply-reattach.test.ts, the second half of "a retained recording the runtime no longer holds is stored as incomplete
/// evidence" ("any other failure still fails the recovery"; stopRecovered, E11.4): the runtime receives the stop and its answer is lost.
/// The Restore fails with connecting the runtime as the next action, nothing is kept as evidence, nothing is released, and the
/// checkpoint stays journaled with the confirmed rollback, so that Restore is tried again (B6.7).
#[test]
fn a_recording_stop_that_fails_otherwise_fails_the_restore_and_keeps_the_checkpoint() {
    let mut setup = setup(
        Kind::Pid,
        Some(FoundMode::Manual),
        CheckpointStage::Recording,
    );
    setup.checkpoint.recording = Some(recording_owner(&setup.loop_definition));
    let folder = Folder::new("lost-stop");
    journal_checkpoint(&folder, &setup);
    let mut world = World::applied(&setup, false);
    world.lost_stops = true;
    let (fake, _world) = world.serve();
    let failure = restore(&fake, &folder, &setup).expect_err("the stop is not confirmed");
    assert_eq!(failure.next_action, NextAction::ConnectRuntime);
    let checkpoint = journaled(&folder, &setup).expect("the checkpoint stays journaled");
    assert_eq!(
        checkpoint.recording_result, None,
        "nothing is kept as evidence"
    );
    assert_eq!(
        checkpoint.stage,
        CheckpointStage::Parameters,
        "the confirmed rollback is journaled"
    );
    assert_eq!(fake.requests_of("control.recording.release").len(), 0);
}

/// A2.5, E10.2: a Restore without the engineer's approval refuses on a physical target before any write, the cancels included; the review
/// reports the target as physical.
#[test]
fn a_physical_target_without_approval_refuses_the_restore_before_anything_is_written() {
    let setup = setup(Kind::Pid, Some(FoundMode::Auto), CheckpointStage::Completed);
    let folder = Folder::new("physical");
    journal_checkpoint(&folder, &setup);
    let mut world = World::applied(&setup, true);
    world.physical = true;
    let (fake, _world) = world.serve();
    let review = review(&fake, &folder, &setup, &empty_study(&setup)).expect("the review holds");
    assert!(review.physical, "the review shows the physical target");
    assert_eq!(
        restore(&fake, &folder, &setup),
        Err(physical_approval_needed())
    );
    assert_eq!(writes(&fake), [] as [Value; 0], "nothing is written");
}

/// §0.1b Confirmation, E11.8: the identity recomputed from the live runtime is not the one the confirmation names.
#[test]
fn a_confirmation_that_names_another_review_identity_writes_nothing() {
    let setup = setup(Kind::Pid, Some(FoundMode::Auto), CheckpointStage::Completed);
    let folder = Folder::new("stale");
    journal_checkpoint(&folder, &setup);
    let (fake, _world) = World::applied(&setup, true).serve();
    let refusal = confirm(&fake, &folder, &setup, "sha256:stale", false)
        .expect_err("the runtime changed since the review");
    assert_eq!(refusal, review_changed());
    assert_eq!(writes(&fake), [] as [Value; 0], "nothing is written");
    assert!(journaled(&folder, &setup).is_some());
}

/// E11.10, B6.4: a Cancel before the Restore begins ends it with nothing written; once it writes, it goes on until the runtime confirms
/// or refuses.
#[test]
fn a_restore_cancelled_before_its_first_step_writes_nothing() {
    let setup = setup(Kind::Pid, Some(FoundMode::Auto), CheckpointStage::Completed);
    let folder = Folder::new("cancelled");
    journal_checkpoint(&folder, &setup);
    let (fake, _world) = World::applied(&setup, true).serve();
    let review = review(&fake, &folder, &setup, &empty_study(&setup)).expect("the review holds");
    let refusal =
        confirm(&fake, &folder, &setup, &review.identity, true).expect_err("cancelled first");
    assert_eq!(refusal, cancelled());
    assert_eq!(writes(&fake), [] as [Value; 0], "nothing is written");
    assert!(journaled(&folder, &setup).is_some());
}

/// A Restore's reference step after its Auto entry (B6.4; C13 step 8): it keeps the loop in Auto, carries the tracking of a target
/// change (no alignment), and writes each reference Apply found at its found value.
fn assert_reference_step(step: &Value, setup: &Setup) {
    assert_eq!(
        step["manual"], false,
        "the reference step keeps the loop in Auto"
    );
    let tracking = &step["tracking"];
    assert!(
        tracking["align_output"] == false || tracking["align_reference"] == false,
        "a reference step does not align: {tracking}"
    );
    let values: BTreeMap<String, f64> =
        decode_value(step["values"].clone()).expect("the step's values decode");
    for (symbol, found) in &setup.checkpoint.references {
        assert_eq!(
            values.get(symbol),
            Some(found),
            "the step writes {symbol} back at {found}"
        );
    }
}
