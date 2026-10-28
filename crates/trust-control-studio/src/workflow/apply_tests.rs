//! Tests of the Apply workflow on the scripted plant runtime (`test_support`), which answers the runtime protocol annex as the runtime
//! does. They port the assertions of `control-studio-apply-process-time.test.ts` and `control-studio-commissioning-starting-point.test.ts`
//! that test the engine's sequence: the order of the writes, the checkpoint journaled before the first write and the one checkpoint per
//! loop, the mode a loop is applied from and returned to, the settings a failed requirement keeps, and the Restore a departure runs at
//! once. The wait for the starting point is tested in `apply/wait_tests.rs`, and the decisions themselves in trust-control
//! (`commissioning::*_tests`).

#![allow(clippy::float_cmp)] // The plant holds exact values, and the tests compare what the rules copy, exactly.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Value};
use trust_control::commissioning::adapters::controller_adapter;
use trust_control::commissioning::application_records::{ApplyOutcome, ReviewConfirmation};
use trust_control::commissioning::application_texts::{physical_approval_needed, review_changed};
use trust_control::commissioning::apply_texts::checkpoint_unresolved;
use trust_control::commissioning::checkpoint::{CheckpointStage, CommissioningCheckpoint};
use trust_control::commissioning::commissioning_result::CommissioningStatus;
use trust_control::commissioning::found_mode::FoundMode;
use trust_control::commissioning::pid_commissioning::PidRunProfile;
use trust_control::commissioning::starting_point::wait_cancelled;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::loop_definition::LoopDefinition;
use trust_control::study::Study;
use trust_control::tuning::candidate::{
    AntiWindup, CandidateSource, ControlPidCandidate, ControlPidParameters, LawCandidate,
};
use trust_control::tuning::structure::Structure;

use super::law::LawKind;
use super::review::{candidate_gains, checkpoint_to_retire, ApplyInputs};
use super::run::run_confirmed;
use super::test_support::{
    gains, inputs, instant, journal_file, model, own_p_loop_with_startup, own_pid_loop, pid_loop,
    proportional_loop, scratch, Plant, Report, Response, TARGET,
};
use super::{review_with, ApplyContext};
use crate::journal::{Journal, JournalEntry};
use crate::workflow::commissioning_journal::SharedJournal;

/// Runs `action` with the context of an Apply of `loop_definition` on `plant`, its journal in `folder`, without a physical approval.
fn in_context<T>(
    loop_definition: &LoopDefinition,
    plant: &Plant,
    folder: &Path,
    action: impl FnOnce(&mut ApplyContext<'_>) -> T,
) -> T {
    let study = Study::new(&loop_definition.id, &loop_definition.revision);
    let connection = plant.connection();
    let mut journal = Journal::open(folder, &loop_definition.project_id, &loop_definition.id)
        .expect("the journal opens");
    let mut context = ApplyContext {
        loop_definition,
        study: &study,
        connection: &connection,
        journal: &mut journal,
        physical_approved: false,
        project_root: folder,
    };
    action(&mut context)
}

/// Reviews and confirms one Apply of `loop_definition` on `plant`, its journal in `folder` (B6.1 to B6.3, §0.1b Confirmation).
fn apply(
    loop_definition: &LoopDefinition,
    plant: &Plant,
    folder: &Path,
    inputs: &ApplyInputs,
) -> Result<ApplyOutcome, Failure> {
    plant.world().journal_file = Some(journal_file(folder, loop_definition));
    in_context(loop_definition, plant, folder, |context| {
        let review = review_with(context, inputs)?;
        let clock = || 0_u64;
        run_confirmed(
            context,
            inputs,
            &review.identity,
            &Report::default(),
            &instant(&clock),
        )
    })
}

/// Journals `checkpoint` at `stage` in the loop's journal in `folder`, as an earlier Apply left it (A8.5).
fn rejournal(
    loop_definition: &LoopDefinition,
    folder: &Path,
    checkpoint: &CommissioningCheckpoint,
    stage: CheckpointStage,
) {
    let mut journal = Journal::open(folder, &loop_definition.project_id, &loop_definition.id)
        .expect("the journal opens");
    let mut earlier = checkpoint.clone();
    earlier.stage = stage;
    SharedJournal::new(&mut journal)
        .write_checkpoint(&earlier)
        .expect("journaled");
}

/// The identifiers of the commissioning checkpoints the loop's journal holds, in order: one per loop at most (A8.5).
fn checkpoint_ids(loop_definition: &LoopDefinition, folder: &Path) -> Vec<String> {
    Journal::open(folder, &loop_definition.project_id, &loop_definition.id)
        .expect("the journal opens")
        .entries()
        .iter()
        .filter_map(|entry| match entry {
            JournalEntry::Commissioning { checkpoint } => Some(checkpoint.id.clone()),
            _ => None,
        })
        .collect()
}

/// The commissioning checkpoint the loop's journal holds, as a reopened engine reads it (A8.5).
fn journaled(loop_definition: &LoopDefinition, folder: &Path) -> Option<CommissioningCheckpoint> {
    Journal::open(folder, &loop_definition.project_id, &loop_definition.id)
        .expect("the journal opens")
        .entries()
        .iter()
        .find_map(|entry| match entry {
            JournalEntry::Commissioning { checkpoint } => Some(checkpoint.as_ref().clone()),
            _ => None,
        })
}

/// The kinds of the plant's transfers, in order: `transfer manual` or `transfer auto` (`world.applies` of the `TypeScript` fake).
fn transfers_of(log: &[String]) -> Vec<String> {
    log.iter()
        .filter(|entry| entry.starts_with("transfer "))
        .cloned()
        .collect()
}

/// B6.1 to B6.3, B6.5 (TS control-studio-apply-process-time, "a loop found in Auto starts from wherever its controller output is; in
/// Manual the output must be the validated one"): a PID loop found in Manual at its validated point (0 V, 2.004 m within 2 m ± 0.02 m) is
/// applied without a Manual staging. The record `Kp 2, Ki 0.5, Kd 0` is applied in Manual (E7.3), Auto is entered with the aligning
/// initialization, the recording starts before the target change to 3 m, and the response that follows the target passes with a recorded
/// error of 0 m. The settings stay applied: the checkpoint completes with its recording, and Restore and Keep in project are offered.
#[test]
fn a_pid_loop_found_in_manual_is_applied_and_keeps_its_settings() {
    let loop_definition = pid_loop();
    let plant = Plant::start(&loop_definition, false, Response::FollowsTarget);
    let folder = scratch("pid-manual");
    let outcome =
        apply(&loop_definition, &plant, &folder, &inputs(&gains(0.5), 0.0)).expect("applied");
    let result = &outcome.result;
    assert_eq!(
        result.status,
        CommissioningStatus::Passed,
        "{}",
        result.reason
    );
    assert_eq!(
        result.reason,
        "PID Auto commissioning passed: recorded target error 0 m; controller remains in Auto."
    );
    assert_eq!(result.found_mode, Some(FoundMode::Manual));
    assert_eq!(result.target, TARGET);
    assert_eq!(
        result.transfers.len(),
        2,
        "the Auto entry and the target change"
    );
    assert!(result.recovery_confirmed);
    assert_eq!(
        (
            outcome.restored,
            outcome.restore_available,
            outcome.keep_available
        ),
        (false, true, true)
    );
    assert!(
        !outcome.apply_available,
        "the settings stand, so Restore holds the position and Apply is not offered until a validation passes again (B7)"
    );
    {
        let world = plant.world();
        assert_eq!(
            world.log,
            [
                "parameters apply",
                "transfer auto",
                "recording start",
                "transfer auto",
                "recording stop",
                "recording release"
            ],
            "the recording starts before the target change, and is released once its samples are kept (E4.7)"
        );
        assert_eq!(
            (
                world.number("Main.Kp"),
                world.number("Main.Ki"),
                world.number("Main.Kd")
            ),
            (2.0, 0.5, 0.0)
        );
        assert!(world.in_auto(), "Apply leaves the loop in Auto (A6.2)");
        assert_eq!(world.number("Main.SP"), TARGET);
    }
    let checkpoint = journaled(&loop_definition, &folder)
        .expect("a completed checkpoint stays journaled for Restore (A8.5)");
    assert_eq!(checkpoint.stage, CheckpointStage::Completed);
    assert!(
        checkpoint.recording.is_none(),
        "the stopped recording's owner is cleared (E11.6)"
    );
    assert_eq!(checkpoint.recording_result, result.recording);
    assert!(result.recording.is_some());
    assert_eq!(checkpoint.found_mode, Some(FoundMode::Manual));
    assert_eq!(
        (
            outcome.checkpoint_id.as_str(),
            outcome.validation_identity.as_str()
        ),
        (checkpoint.id.as_str(), "validation:1"),
        "the receipt names the journaled checkpoint and the validation it was reviewed on (A8.1)"
    );
    assert_eq!(
        checkpoint_ids(&loop_definition, &folder),
        std::slice::from_ref(&checkpoint.id)
    );
    let _ = std::fs::remove_dir_all(&folder);
}

/// B6.2, A6.2 (TS control-studio-apply-process-time, "a loop found in Auto starts from wherever its controller output is"): a PID loop
/// found in Auto is staged into Manual with a transfer before its record is applied, since a parameter apply needs Manual (E7.3); then
/// Auto is entered and the target change observed. The result names the mode it found, Auto, and holds its three confirmed transfers.
#[test]
fn a_pid_loop_found_in_auto_is_staged_in_manual_before_its_record_is_applied() {
    let loop_definition = pid_loop();
    let plant = Plant::start(&loop_definition, true, Response::FollowsTarget);
    plant.world().numbers.insert("Main.MV".into(), 0.36);
    let folder = scratch("pid-auto");
    let outcome =
        apply(&loop_definition, &plant, &folder, &inputs(&gains(0.5), 0.0)).expect("applied");
    assert_eq!(
        outcome.result.status,
        CommissioningStatus::Passed,
        "{}",
        outcome.result.reason
    );
    assert!(
        !outcome.result.reason.contains("validated from"),
        "the running controller's output is not compared (B6.2)"
    );
    assert_eq!(outcome.result.found_mode, Some(FoundMode::Auto));
    assert_eq!(outcome.result.transfers.len(), 3);
    let world = plant.world();
    assert_eq!(
        world.log[..3],
        ["transfer manual", "parameters apply", "transfer auto"],
        "Manual first, then the record, then the Auto entry"
    );
    assert!(world.in_auto());
    drop(world);
    let _ = std::fs::remove_dir_all(&folder);
}

/// E11.2, A8.5: the checkpoint is journaled before the first write, and each transaction's owner before its dispatch. For a loop found
/// in Auto the first write is the Manual staging, and the journal already holds the checkpoint at the stage `transfer` with that
/// transfer's identifier; for a loop found in Manual it is the parameter apply, at the stage `parameters` with the parameter operation's
/// identifier.
#[test]
fn the_checkpoint_is_journaled_before_the_first_write() {
    for (found_in_auto, stage, owner) in [
        (true, "transfer", "transfer"),
        (false, "parameters", "parameterOperation"),
    ] {
        let loop_definition = pid_loop();
        let plant = Plant::start(&loop_definition, found_in_auto, Response::FollowsTarget);
        let folder = scratch(&format!("first-write-{stage}"));
        apply(&loop_definition, &plant, &folder, &inputs(&gains(0.5), 0.0)).expect("applied");
        let (journal, first_id) = plant.world().first_write.clone().expect("a write was made");
        let journal: Value = serde_json::from_str(
            &journal.expect("the journal existed when the first write arrived"),
        )
        .expect("the journal is JSON");
        let entry = &journal["entries"][0];
        assert_eq!(entry["kind"], "commissioning");
        assert_eq!(entry["checkpoint"]["stage"], stage);
        assert_eq!(
            entry["checkpoint"][owner]["id"],
            json!(first_id),
            "the first write's identity was journaled before it was dispatched"
        );
        let _ = std::fs::remove_dir_all(&folder);
    }
}

/// B6.3, B6.5, B6.7 (TS control-studio-apply-process-time, "a failed requirement keeps the reviewed settings applied, and Restore returns
/// the loop to Auto with the previous settings"): the measurement stays at 2.004 m after the target change to 3 m, so the target error
/// fails its limit of 0.1 m. The reviewed settings stay applied in Auto (Kp 2), nothing is restored, the failure offers Restore previous
/// settings, and the checkpoint completes, so Restore and Keep in project are offered.
#[test]
fn a_failed_requirement_keeps_the_settings_applied_and_offers_restore() {
    let loop_definition = pid_loop();
    let plant = Plant::start(&loop_definition, true, Response::Stays);
    let folder = scratch("requirement");
    let outcome =
        apply(&loop_definition, &plant, &folder, &inputs(&gains(0.5), 0.0)).expect("applied");
    let result = &outcome.result;
    assert_eq!(result.status, CommissioningStatus::Failed);
    assert!(
        result.reason.starts_with("Commissioning target error ")
            && result.reason.contains("exceeds 0.1."),
        "{}",
        result.reason
    );
    let failure = result.failure.as_ref().expect("the failed requirement");
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Settings, NextAction::Restore)
    );
    assert!(result.recovery_confirmed);
    assert_eq!(
        (
            outcome.restored,
            outcome.restore_available,
            outcome.keep_available
        ),
        (false, true, true)
    );
    {
        let world = plant.world();
        assert_eq!(
            world.number("Main.Kp"),
            2.0,
            "the reviewed gain stays applied"
        );
        assert!(world.in_auto());
        assert!(!world.log.iter().any(|entry| entry == "parameters rollback"));
    }
    assert_eq!(
        journaled(&loop_definition, &folder).map(|checkpoint| checkpoint.stage),
        Some(CheckpointStage::Completed)
    );
    let _ = std::fs::remove_dir_all(&folder);
}

/// B6.3, B6.4, B6.7, A6.2 (TS control-studio-apply-process-time, "a failed PID Apply that found the loop in Auto returns it to Auto, with
/// the previous gain restored"): the measurement leaves its permitted range of 0 m to 10 m at 25 m. The recording is stopped and kept, and
/// the one Restore runs at once: the record returns to Kp 1 and the loop to Auto, the mode Apply found it in. The reason names the
/// departure, the failure is the departure's own (model or data, none), and the confirmed checkpoint is cleared.
#[test]
fn a_departure_from_the_permitted_range_restores_the_previous_settings_at_once() {
    let loop_definition = pid_loop();
    let plant = Plant::start(&loop_definition, true, Response::Departs(25.0));
    let folder = scratch("departure");
    let outcome =
        apply(&loop_definition, &plant, &folder, &inputs(&gains(0.5), 0.0)).expect("applied");
    let result = &outcome.result;
    assert_eq!(result.status, CommissioningStatus::Failed);
    assert_eq!(
        result.reason,
        "Main.PV left its permitted range (25 m outside 0 m..10 m); the previous settings were restored."
    );
    let failure = result.failure.as_ref().expect("the departure");
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::ModelOrData, NextAction::None)
    );
    assert!(result.recovery_confirmed);
    assert!(
        result.recording.is_some(),
        "the samples up to the departure are kept"
    );
    assert_eq!(
        (
            outcome.restored,
            outcome.restore_available,
            outcome.keep_available
        ),
        (true, false, false)
    );
    {
        let world = plant.world();
        assert_eq!(world.number("Main.Kp"), 1.0, "the previous gain is back");
        assert!(
            world.in_auto(),
            "the loop returns to the mode Apply found it in"
        );
        let stop = world.log.iter().position(|entry| entry == "recording stop");
        let rollback = world
            .log
            .iter()
            .position(|entry| entry == "parameters rollback");
        assert!(
            stop.is_some() && rollback.is_some() && stop < rollback,
            "{:?}",
            world.log
        );
        assert_eq!(
            transfers_of(&world.log).last().map(String::as_str),
            Some("transfer auto"),
            "the last write returns the loop to Auto"
        );
    }
    assert!(
        journaled(&loop_definition, &folder).is_none(),
        "a confirmed Restore clears the checkpoint (A8.5)"
    );
    assert!(
        outcome.checkpoint_id.starts_with("commissioning-") && outcome.checkpoint_id.len() == 46,
        "the outcome still names the cleared checkpoint for its receipt: {}",
        outcome.checkpoint_id
    );
    assert_eq!(outcome.validation_identity, "validation:1");
    let _ = std::fs::remove_dir_all(&folder);
}

/// C13, E11.10 (TS control-studio-apply-process-time, "enters Auto ... within the A6.4 allowance"): a proportional loop found in Auto is
/// staged into Manual, its gain Kp 2 applied with Ki and Kd as explicit zeros, and Auto entered with the reference aligned to the held
/// output: r = 2.004 m + 0 V / 2 = 2.004 m. The reviewed target change follows as its own transfer, and the passed result keeps the gain
/// recovery context of the original record.
#[test]
fn a_proportional_loop_found_in_auto_is_aligned_staged_and_applied() {
    let loop_definition = proportional_loop();
    let plant = Plant::start(&loop_definition, true, Response::FollowsTarget);
    let folder = scratch("proportional");
    let outcome =
        apply(&loop_definition, &plant, &folder, &inputs(&gains(0.0), 0.0)).expect("applied");
    let result = &outcome.result;
    assert_eq!(
        result.status,
        CommissioningStatus::Passed,
        "{}",
        result.reason
    );
    assert_eq!(
        result.reason,
        "Auto commissioning passed: recorded target error 0 m; controller remains in Auto."
    );
    assert_eq!(result.transfers.len(), 3);
    let entry = result.transfers[1]
        .transaction
        .as_ref()
        .and_then(|transaction| transaction.applied_values.clone())
        .expect("the Auto entry's applied values");
    assert_eq!(entry["Main.SP"], 2.004, "r = PV + u / Kp (C13.3)");
    let recovery = result
        .gain_recovery
        .as_ref()
        .expect("the gain recovery context");
    assert_eq!(recovery.origin.values["Main.Kp"], 1.0);
    assert_eq!(
        recovery.candidate,
        BTreeMap::from([
            ("Main.Kd".to_owned(), 0.0),
            ("Main.Ki".to_owned(), 0.0),
            ("Main.Kp".to_owned(), 2.0)
        ])
    );
    assert_eq!(plant.world().number("Main.Kp"), 2.0);
    let _ = std::fs::remove_dir_all(&folder);
}

/// §0.1b Confirmation, E11.8: a confirmation that names another identity than the one the live runtime gives is refused before anything
/// is written ("The runtime changed since the review; review again."), and nothing is journaled.
#[test]
fn a_confirmation_of_another_review_is_refused_and_writes_nothing() {
    let loop_definition = pid_loop();
    let plant = Plant::start(&loop_definition, false, Response::FollowsTarget);
    let folder = scratch("identity");
    let study = Study::new(&loop_definition.id, &loop_definition.revision);
    let connection = plant.connection();
    let mut journal = Journal::open(&folder, &loop_definition.project_id, &loop_definition.id)
        .expect("the journal opens");
    let mut context = ApplyContext {
        loop_definition: &loop_definition,
        study: &study,
        connection: &connection,
        journal: &mut journal,
        physical_approved: false,
        project_root: &folder,
    };
    let clock = || 0_u64;
    let refused = run_confirmed(
        &mut context,
        &inputs(&gains(0.5), 0.0),
        "sha256:another-review",
        &Report::default(),
        &instant(&clock),
    )
    .expect_err("another review is refused");
    assert_eq!(refused, review_changed());
    assert_eq!(plant.world().log, [] as [&str; 0]);
    assert!(journaled(&loop_definition, &folder).is_none());
    let _ = std::fs::remove_dir_all(&folder);
}

/// A2.2, E9.5, E11.5, B6.4: a program reloaded between the review and its confirmation is another runtime: its revision is part of the
/// review identity, so the run refuses it as changed instead of writing to it as the reviewed one, and nothing is journaled.
#[test]
fn a_runtime_reloaded_since_the_review_is_not_treated_as_the_reviewed_one() {
    let loop_definition = pid_loop();
    let plant = Plant::start(&loop_definition, false, Response::FollowsTarget);
    let folder = scratch("reloaded");
    let study = Study::new(&loop_definition.id, &loop_definition.revision);
    let connection = plant.connection();
    let mut journal = Journal::open(&folder, &loop_definition.project_id, &loop_definition.id)
        .expect("the journal opens");
    let mut context = ApplyContext {
        loop_definition: &loop_definition,
        study: &study,
        connection: &connection,
        journal: &mut journal,
        physical_approved: false,
        project_root: &folder,
    };
    let inputs = inputs(&gains(0.5), 0.0);
    let review = review_with(&mut context, &inputs).expect("reviewed");
    plant.world().program = "program-2".into();
    let clock = || 0_u64;
    let refused = run_confirmed(
        &mut context,
        &inputs,
        &review.identity,
        &Report::default(),
        &instant(&clock),
    )
    .expect_err("the reloaded runtime is refused");
    assert_eq!(refused, review_changed());
    assert_eq!(plant.world().log, [] as [&str; 0]);
    assert!(journaled(&loop_definition, &folder).is_none());
    let _ = std::fs::remove_dir_all(&folder);
}

/// A8.5, E11.7, V4 decision 5: the checkpoint an earlier Apply left decides how a new Apply starts, and the journal holds one checkpoint
/// per loop. Completed, the checkpoint does not block (`checkpoint_to_retire` gives nothing to retire), and the next Apply replaces it
/// with its own; at the unresolved stage `transfer` on the running program `program-1` it blocks with "An earlier Apply of this loop is
/// unresolved: its checkpoint <id> stopped at the transfer stage." and the next action Restore; at the stage `retired` it is a leftover
/// that blocks nothing, and the next Apply clears it.
#[test]
fn a_completed_checkpoint_is_replaced_an_unresolved_one_blocks_and_a_retired_one_is_cleared() {
    let loop_definition = pid_loop();
    let folder = scratch("replaced");
    let first = Plant::start(&loop_definition, false, Response::FollowsTarget);
    apply(&loop_definition, &first, &folder, &inputs(&gains(0.5), 0.0)).expect("applied");
    let completed = journaled(&loop_definition, &folder).expect("completed");
    let decided = |stage: CheckpointStage| {
        rejournal(&loop_definition, &folder, &completed, stage);
        let journal = Journal::open(&folder, &loop_definition.project_id, &loop_definition.id)
            .expect("the journal opens");
        checkpoint_to_retire(&journal, &completed.origin)
    };
    assert_eq!(
        decided(CheckpointStage::Completed),
        Ok(None),
        "a completed checkpoint does not block"
    );
    let blocked = decided(CheckpointStage::Transfer).expect_err("an unresolved checkpoint blocks");
    assert_eq!(blocked, checkpoint_unresolved(&completed.id, "transfer"));
    assert_eq!(blocked.next_action, NextAction::Restore);
    assert_eq!(
        decided(CheckpointStage::Retired),
        Ok(None),
        "a retired leftover blocks nothing"
    );
    let second = Plant::start(&loop_definition, false, Response::FollowsTarget);
    apply(
        &loop_definition,
        &second,
        &folder,
        &inputs(&gains(0.5), 0.0),
    )
    .expect("applied again");
    let replacing = checkpoint_ids(&loop_definition, &folder);
    assert_eq!(
        replacing.len(),
        1,
        "the retired leftover is cleared: one checkpoint per loop"
    );
    assert_ne!(replacing[0], completed.id);
    let third = Plant::start(&loop_definition, false, Response::FollowsTarget);
    apply(&loop_definition, &third, &folder, &inputs(&gains(0.5), 0.0)).expect("applied over it");
    let replaced = checkpoint_ids(&loop_definition, &folder);
    assert_eq!(
        replaced.len(),
        1,
        "the completed checkpoint is replaced: one checkpoint per loop"
    );
    assert_ne!(replaced[0], replacing[0]);
    let _ = std::fs::remove_dir_all(&folder);
}

/// B6.4, A8.5 (the restarted runtime of `reviewInterruptedRollback`): an earlier Apply's checkpoint left at the unresolved stage
/// `transfer` on the program `program-1` does not block an Apply on the reloaded program `program-2`, since nothing of it can be restored
/// there. The review writes nothing and leaves that checkpoint journaled at its stage; the run retires it before it journals its own and
/// passes, so the journal holds one checkpoint: the new one, completed.
#[test]
fn an_unresolved_checkpoint_of_a_reloaded_program_is_let_pass_by_the_review_and_retired_by_the_run()
{
    let loop_definition = pid_loop();
    let folder = scratch("reloaded");
    let first = Plant::start(&loop_definition, false, Response::FollowsTarget);
    apply(&loop_definition, &first, &folder, &inputs(&gains(0.5), 0.0)).expect("applied");
    let earlier = journaled(&loop_definition, &folder).expect("completed");
    assert_eq!(earlier.origin.program_revision, "program-1");
    rejournal(
        &loop_definition,
        &folder,
        &earlier,
        CheckpointStage::Transfer,
    );
    let reloaded = Plant::start(&loop_definition, false, Response::FollowsTarget);
    reloaded.world().program = "program-2".into();
    let inputs = inputs(&gains(0.5), 0.0);
    let outcome = in_context(&loop_definition, &reloaded, &folder, |context| {
        let review = review_with(context, &inputs).expect("the review lets it pass");
        assert_eq!(
            journaled(&loop_definition, &folder).map(|left| (left.id, left.stage)),
            Some((earlier.id.clone(), CheckpointStage::Transfer)),
            "the review writes nothing"
        );
        let clock = || 0_u64;
        run_confirmed(
            context,
            &inputs,
            &review.identity,
            &Report::default(),
            &instant(&clock),
        )
    })
    .expect("applied on the reloaded program");
    assert_eq!(
        outcome.result.status,
        CommissioningStatus::Passed,
        "{}",
        outcome.result.reason
    );
    let ids = checkpoint_ids(&loop_definition, &folder);
    assert_eq!(
        ids,
        std::slice::from_ref(&outcome.checkpoint_id),
        "the earlier checkpoint is retired: one checkpoint per loop"
    );
    assert_ne!(ids[0], earlier.id);
    assert_eq!(
        journaled(&loop_definition, &folder).map(|journaled| journaled.stage),
        Some(CheckpointStage::Completed)
    );
    let _ = std::fs::remove_dir_all(&folder);
}

/// E11.7, A8.5, B6.7: an earlier Apply's checkpoint left at the unresolved stage `transfer` on the running program `program-1` refuses the
/// review and its confirmation alike with "An earlier Apply of this loop is unresolved: its checkpoint <id> stopped at the transfer stage."
/// and the next action Restore. Nothing is written, and the checkpoint stays journaled at its stage for Restore.
#[test]
fn an_unresolved_checkpoint_of_the_running_program_blocks_the_review_and_the_run() {
    let loop_definition = pid_loop();
    let folder = scratch("unresolved");
    let first = Plant::start(&loop_definition, false, Response::FollowsTarget);
    apply(&loop_definition, &first, &folder, &inputs(&gains(0.5), 0.0)).expect("applied");
    let earlier = journaled(&loop_definition, &folder).expect("completed");
    rejournal(
        &loop_definition,
        &folder,
        &earlier,
        CheckpointStage::Transfer,
    );
    let plant = Plant::start(&loop_definition, false, Response::FollowsTarget);
    let inputs = inputs(&gains(0.5), 0.0);
    let (reviewed, confirmed) = in_context(&loop_definition, &plant, &folder, |context| {
        let reviewed = review_with(context, &inputs);
        let clock = || 0_u64;
        let confirmed = run_confirmed(
            context,
            &inputs,
            "any review",
            &Report::default(),
            &instant(&clock),
        );
        (reviewed, confirmed)
    });
    let refusal = checkpoint_unresolved(&earlier.id, "transfer");
    assert_eq!(reviewed.expect_err("the review is refused"), refusal);
    assert_eq!(confirmed.expect_err("the confirmation is refused"), refusal);
    assert_eq!(refusal.next_action, NextAction::Restore);
    assert_eq!(plant.world().log, [] as [&str; 0]);
    assert_eq!(
        journaled(&loop_definition, &folder).map(|left| (left.id, left.stage)),
        Some((earlier.id.clone(), CheckpointStage::Transfer))
    );
    let _ = std::fs::remove_dir_all(&folder);
}

/// A2.4, A2.5, §0.1b (TS control-studio-validate-apply, "uses revision-guarded existing runtime requests and separate physical
/// approval"): on a physical target the review is answered and marked physical, since the engineer approves the reviewed write; its
/// confirmation without that approval is refused before anything is written or journaled.
#[test]
fn a_physical_target_is_reviewed_and_its_confirmation_needs_the_approval() {
    let loop_definition = pid_loop();
    let plant = Plant::start(&loop_definition, false, Response::FollowsTarget);
    plant.world().physical = true;
    let folder = scratch("physical");
    let inputs = inputs(&gains(0.5), 0.0);
    let refused = in_context(&loop_definition, &plant, &folder, |context| {
        let review = review_with(context, &inputs).expect("a physical target is reviewed");
        assert!(review.physical);
        assert_eq!(review.confirmation, ReviewConfirmation::Approval);
        let clock = || 0_u64;
        run_confirmed(
            context,
            &inputs,
            &review.identity,
            &Report::default(),
            &instant(&clock),
        )
    })
    .expect_err("the confirmation needs the approval");
    assert_eq!(refused, physical_approval_needed());
    assert_eq!(plant.world().log, [] as [&str; 0]);
    assert!(journaled(&loop_definition, &folder).is_none());
    let _ = std::fs::remove_dir_all(&folder);
}

/// A2.4, A2.5, B9.3: the review says how it is confirmed, so the page decides nothing from the target's kind: the press of Apply
/// confirms it on a simulated target, and the engineer's approval on a physical one.
#[test]
fn a_review_says_whether_the_press_or_the_approval_confirms_it() {
    for (physical, name, expected) in [
        (false, "confirmation-simulated", ReviewConfirmation::Press),
        (true, "confirmation-physical", ReviewConfirmation::Approval),
    ] {
        let loop_definition = pid_loop();
        let plant = Plant::start(&loop_definition, false, Response::FollowsTarget);
        plant.world().physical = physical;
        let folder = scratch(name);
        let inputs = inputs(&gains(0.5), 0.0);
        let review = in_context(&loop_definition, &plant, &folder, |context| {
            review_with(context, &inputs)
        })
        .expect("the target is reviewed");
        assert_eq!((review.physical, review.confirmation), (physical, expected));
        let _ = std::fs::remove_dir_all(&folder);
    }
}

/// B6.2 (TS control-studio-commissioning-starting-point, "waits outside the settling band with its progress, and Abort ends the wait
/// without writing anything"): a cancel before the starting point is reached ends the run with "Apply cancelled while waiting for the
/// process to settle; nothing was written." and the next action Validate, and nothing is written or journaled.
#[test]
fn a_cancel_while_waiting_writes_nothing_and_journals_nothing() {
    let loop_definition = pid_loop();
    let plant = Plant::start(&loop_definition, false, Response::FollowsTarget);
    let folder = scratch("cancelled");
    let study = Study::new(&loop_definition.id, &loop_definition.revision);
    let connection = plant.connection();
    let mut journal = Journal::open(&folder, &loop_definition.project_id, &loop_definition.id)
        .expect("the journal opens");
    let mut context = ApplyContext {
        loop_definition: &loop_definition,
        study: &study,
        connection: &connection,
        journal: &mut journal,
        physical_approved: false,
        project_root: &folder,
    };
    let inputs = inputs(&gains(0.5), 0.0);
    let review = review_with(&mut context, &inputs).expect("reviewed");
    let report = Report::default();
    report.cancel();
    let clock = || 0_u64;
    let refused = run_confirmed(
        &mut context,
        &inputs,
        &review.identity,
        &report,
        &instant(&clock),
    )
    .expect_err("cancelled");
    assert_eq!(refused, wait_cancelled());
    assert_eq!(refused.next_action, NextAction::Validate);
    assert_eq!(plant.world().log, [] as [&str; 0]);
    assert!(journaled(&loop_definition, &folder).is_none());
    let _ = std::fs::remove_dir_all(&folder);
}

/// C0 ("Tune MUST keep the exact mapped record ... Validate runs that record, and Apply writes it"; `materialize(design.candidate,
/// observed)`): the candidate's gains are written to the gains its law maps, the terms the law does not declare stay neutral, a law
/// without back-calculation anti-windup takes no tracking gain, and the record keeps every other running value. A P candidate writes its
/// integral and derivative gains as the explicit zeros the record holds (C13).
#[test]
fn the_candidate_is_written_as_the_record_its_adapter_maps() {
    let candidate = ControlPidCandidate {
        applied: false,
        source: CandidateSource::Simc,
        structure: Structure::PI,
        parameters: ControlPidParameters {
            proportional_gain: 2.0,
            integral_gain: 0.5,
            derivative_gain: 0.0,
            proportional_setpoint_weight: 1.0,
            derivative_setpoint_weight: 0.0,
            bias: 0.0,
            output_minimum: -20.0,
            output_maximum: 20.0,
            rate_minimum: -1e6,
            rate_maximum: 1e6,
            tracking_time: 4.0,
            anti_windup: AntiWindup::BackCalculation,
        },
        measurement_filter_seconds: None,
    };
    let observed = BTreeMap::from([
        ("Main.Kp".to_owned(), 1.0),
        ("Main.Ki".to_owned(), 0.0),
        ("Main.Kd".to_owned(), 0.0),
    ]);
    let pid = pid_loop();
    let gains = candidate_gains(&pid, &LawCandidate::Pid(candidate));
    assert_eq!(gains.back_calculation_gain, None);
    assert_eq!(
        controller_adapter(&pid)
            .expect("the PID adapter")
            .materialize(&gains, &observed)
            .expect("the PID record"),
        BTreeMap::from([
            ("Main.Kd".to_owned(), 0.0),
            ("Main.Ki".to_owned(), 0.5),
            ("Main.Kp".to_owned(), 2.0)
        ])
    );
    let proportional = proportional_loop();
    let p_candidate = ControlPidCandidate {
        structure: Structure::P,
        parameters: ControlPidParameters {
            integral_gain: 0.0,
            ..candidate.parameters
        },
        ..candidate
    };
    assert_eq!(
        controller_adapter(&proportional)
            .expect("the connected P")
            .materialize(
                &candidate_gains(&proportional, &LawCandidate::Pid(p_candidate)),
                &observed,
            )
            .expect("the P record"),
        BTreeMap::from([
            ("Main.Kd".to_owned(), 0.0),
            ("Main.Ki".to_owned(), 0.0),
            ("Main.Kp".to_owned(), 2.0)
        ])
    );
}

/// C2 (`LawKind::of`): a served program's PI, PD or PID law runs its declared states, as a Control.PID does, so its profile is the declared
/// one and carries its family's label. A program's P law is the connected P on its variable (C13).
#[test]
fn a_served_pid_law_runs_its_declared_profile_and_a_program_p_law_the_connected_p() {
    let plant_model = model();
    let pid = own_pid_loop();
    let values = BTreeMap::from([
        ("Main.Kp".to_owned(), 2.0),
        ("Main.Ki".to_owned(), 0.5),
        ("Main.Kd".to_owned(), 0.0),
    ]);
    let adapter = controller_adapter(&pid).expect("the served PID law");
    let kind = LawKind::of(&adapter, &plant_model, &values).expect("the law of the served PID");
    assert!(
        matches!(&kind, LawKind::Pid(PidRunProfile::Declared(profile)) if profile.label == "PID"),
        "{kind:?}"
    );
    let proportional = own_p_loop_with_startup();
    let adapter = controller_adapter(&proportional).expect("the served P law");
    let kind = LawKind::of(
        &adapter,
        &plant_model,
        &BTreeMap::from([("Main.Kp".to_owned(), 2.0)]),
    )
    .expect("the law of the served P");
    assert_eq!(kind, LawKind::Proportional);
}

/// C2 Apply (B6.1, B6.2, E7.3): a served program's PID law is applied as one entry. Its three seed gains are written in Manual before the
/// transfer, and the one transfer to Auto carries its declared states with the mode, so the states and the mode change together.
#[test]
fn a_served_pid_law_writes_its_seed_gains_then_enters_auto_with_its_declared_states() {
    let loop_definition = own_pid_loop();
    let plant = Plant::start(&loop_definition, false, Response::FollowsTarget);
    let folder = scratch("own-pid");
    let outcome =
        apply(&loop_definition, &plant, &folder, &inputs(&gains(0.5), 0.0)).expect("applied");
    let result = &outcome.result;
    assert_eq!(
        result.status,
        CommissioningStatus::Passed,
        "{}",
        result.reason
    );
    let entry = result.transfers[0]
        .transaction
        .as_ref()
        .and_then(|transaction| transaction.applied_values.clone())
        .expect("the Auto entry's applied values");
    assert!(
        entry.contains_key("Main.Integral") && entry.contains_key("Main.PreviousError"),
        "the declared states enter with the mode: {entry:?}"
    );
    {
        let world = plant.world();
        assert!(
            world
                .log
                .starts_with(&["parameters apply".to_owned(), "transfer auto".to_owned()]),
            "the gains are written in Manual before the entry: {:?}",
            world.log
        );
        assert_eq!(
            (
                world.number("Main.Kp"),
                world.number("Main.Ki"),
                world.number("Main.Kd")
            ),
            (2.0, 0.5, 0.0)
        );
        assert!(world.in_auto(), "Apply leaves the loop in Auto (A6.2)");
    }
    let _ = std::fs::remove_dir_all(&folder);
}

/// C13 and C1 (B6.2, E11.10): a program's P law is applied as the connected P. Its entry transfer writes the reference r = PV + u/Kp with
/// the startup-done flag TRUE and the mode, so the flag is set by the transaction that enters Auto.
#[test]
fn a_program_p_law_applies_the_connected_p_entry_with_its_startup_flag() {
    let loop_definition = own_p_loop_with_startup();
    let plant = Plant::start(&loop_definition, false, Response::FollowsTarget);
    let folder = scratch("own-p-startup");
    let outcome =
        apply(&loop_definition, &plant, &folder, &inputs(&gains(0.0), 0.0)).expect("applied");
    let result = &outcome.result;
    assert_eq!(
        result.status,
        CommissioningStatus::Passed,
        "{}",
        result.reason
    );
    let entry = result.transfers[0]
        .transaction
        .as_ref()
        .and_then(|transaction| transaction.applied_values.clone())
        .expect("the Auto entry's applied values");
    assert_eq!(entry["Main.SP"], 2.004, "r = PV + u / Kp (C13.3)");
    assert_eq!(
        entry["Main.Started"], 1.0,
        "the startup-done flag is written TRUE by the entry"
    );
    {
        let world = plant.world();
        assert_eq!(world.number("Main.Kp"), 2.0);
        assert_eq!(world.number("Main.Started"), 1.0);
        assert!(world.in_auto(), "Apply leaves the loop in Auto (A6.2)");
    }
    let _ = std::fs::remove_dir_all(&folder);
}
