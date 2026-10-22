//! Tests of the Validate page's offers (specification 35, B5.4, B6.4, B6.5, B7, A8.6; the Validate page specification). The TypeScript
//! page held these rules as a state machine over the messages it received (`threePageApplication.ts` `takeValidation` and `position`);
//! no TypeScript test is ported, because the page now shows the engine's statement and decides nothing. Every expected value is worked
//! out from the sentences of B7 and B6.4 quoted beside it.

use std::collections::BTreeMap;

use serde_json::json;

use super::*;
use crate::commissioning::fixture_loops::{first_order_model, proportional_loop, requirements};
use crate::commissioning::found_mode::FoundMode;
use crate::commissioning::parameter_application::ParameterOwner;
use crate::commissioning::parameter_status::{ParameterStatus, RecordState};
use crate::commissioning::transfer::proportional_transfer_binding;
use crate::commissioning::transfer_tracking::{proportional_tracking, TransferTracking};
use crate::experiment::{ControlEvidenceSource, RecordingRecoveryBinding, RecordingRecoveryOwner};
use crate::study::apply::{ApplicationReceipt, ApplySection, RestorationReceipt};

/// The identity of the validation that the checkpoints of these tests applied.
const APPLIED: &str = "sha256:the-applied-validation";
/// The identity of a validation that passed after that Apply.
const NEWER: &str = "sha256:a-newer-validation";

/// The checkpoint of an Apply of the proportional loop at `stage`, which applied the validation [`APPLIED`]. With `recording`, it holds
/// the owner of an observation that the runtime may still record (A8.5).
fn checkpoint(stage: CheckpointStage, recording: bool) -> CommissioningCheckpoint {
    let loop_definition = proportional_loop();
    let binding =
        proportional_transfer_binding(&loop_definition).expect("the proportional binding");
    let tracking =
        proportional_tracking(&loop_definition, &binding, true).expect("the proportional tracking");
    CommissioningCheckpoint {
        id: "checkpoint:1".to_owned(),
        loop_revision: loop_definition.revision.clone(),
        stage,
        model: first_order_model(),
        task: requirements(),
        origin: ParameterStatus {
            supports_request_id: Some(true),
            transaction_id: None,
            program_revision: "program:1".to_owned(),
            binding_revision: "binding:1".to_owned(),
            active_revision: "active:1".to_owned(),
            values: BTreeMap::from([("Main.Parameters.Kp".to_owned(), 1.0)]),
            status: RecordState::Applied,
            previous_revision: None,
            previous_values: None,
            reason: None,
        },
        candidate: BTreeMap::from([("Main.Parameters.Kp".to_owned(), 2.0)]),
        parameter_operation: ParameterOwner {
            id: "parameter:1".to_owned(),
            program: "program:1".to_owned(),
            binding: "binding:1".to_owned(),
        },
        references: BTreeMap::new(),
        binding,
        recovery_tracking: TransferTracking::Proportional(tracking),
        tolerance: 0.1,
        replacement_program_revision: None,
        transfer: None,
        recording: recording.then(|| RecordingRecoveryOwner {
            schema_version: 1,
            recording_id: "recording:1".to_owned(),
            project_id: loop_definition.project_id.clone(),
            runtime_id: loop_definition.runtime_id.clone(),
            loop_id: loop_definition.id.clone(),
            loop_revision: loop_definition.revision.clone(),
            source: ControlEvidenceSource::Simulation,
            program_revision: Some("program:1".to_owned()),
            start_acknowledgement_unconfirmed: None,
            bindings: vec![RecordingRecoveryBinding {
                id: "processVariable".to_owned(),
                address: None,
                symbol: Some("Main.PV".to_owned()),
                iec_type: "LREAL".to_owned(),
            }],
        }),
        recording_result: None,
        found_mode: Some(FoundMode::Auto),
        process_noise: None,
        validation_identity: APPLIED.to_owned(),
        transfers: Vec::new(),
        schedule_origin: None,
    }
}

/// R1 (B5.4, B7): the open answer and the Validate answer state Apply by this one rule: both ask `ApplicationOffers::of` with the
/// checkpoint the journal holds, the validation of the current candidate that passed and the validation the newest Apply applied
/// (`application_state` in the engine). A waiting checkpoint makes both false, whatever the validation; a passed validation of the
/// current candidate with no Apply recorded makes both true. The wrong behaviour caught: the Validate answer's Apply ignoring a waiting
/// checkpoint while the open answer refuses it, so the page shows Apply for a loop whose earlier Apply is unresolved.
#[test]
fn apply_availability_is_one_rule_for_open_and_validate() {
    let unresolved = checkpoint(CheckpointStage::Transfer, false);
    let waiting = JournaledApply::of(Some(&unresolved), false);
    assert_eq!(waiting, JournaledApply::Waiting { resumable: false });
    let applied = newest_applied_validation(Some(&unresolved), None);
    // The open answer passes the study's validation; the Validate answer passes the validation it has just kept. Both are a passed
    // validation of the current candidate, here NEWER, and both meet the same journal.
    for passed in [Some(APPLIED), Some(NEWER)] {
        assert!(
            !ApplicationOffers::of(waiting, passed, applied, true).apply,
            "a waiting checkpoint holds Apply back (B6.7, E11.7)"
        );
    }
    let none = JournaledApply::of(None, false);
    assert_eq!(none, JournaledApply::None);
    assert!(
        ApplicationOffers::of(none, Some(NEWER), None, false).apply,
        "a passed validation of the current candidate with no Apply recorded offers Apply (B5.4)"
    );
    assert!(
        !ApplicationOffers::of(none, None, None, false).apply,
        "no passed validation, no Apply (B5.4)"
    );
    // A checkpoint whose Apply or Restore runs in this engine is followed to its end; no second Apply is offered beside it.
    let running = JournaledApply::of(Some(&unresolved), true);
    assert_eq!(running, JournaledApply::InProgress);
    assert!(!apply_offered(running, Some(NEWER), applied));
}

/// B7: "While an Apply's settings stand (its checkpoint completed and not restored or retired), Apply is not offered for the validation
/// that was applied, also after Keep in project; a candidate that passes validation after the Apply ... may be applied". The wrong
/// behaviour caught: a reopened loop offering a second Apply of the applied validation, or refusing Apply to a newer one.
#[test]
fn a_standing_apply_holds_back_apply_only_for_the_validation_it_applied() {
    let completed = checkpoint(CheckpointStage::Completed, false);
    let standing = JournaledApply::of(Some(&completed), false);
    assert_eq!(standing, JournaledApply::Standing);
    let applied = newest_applied_validation(Some(&completed), None);
    assert_eq!(
        applied,
        Some(APPLIED),
        "the checkpoint keeps the validation"
    );
    assert!(
        !apply_offered(standing, Some(APPLIED), applied),
        "the applied validation"
    );
    assert!(
        apply_offered(standing, Some(NEWER), applied),
        "a validation that passed after the Apply"
    );
    assert!(
        !apply_offered(standing, None, applied),
        "no passed validation"
    );
    // A standing Apply is not unresolved: an operation that runs beside it does not change that (B7).
    assert_eq!(JournaledApply::of(Some(&completed), true), standing);
}

/// A8.5, B6.4: a checkpoint that is closed grants nothing; an unresolved one waits for its Restore, or for its Resume while the runtime
/// may still record its observation (A8.6: "A journaled observation that the runtime may still record is offered as Resume"). The wrong
/// behaviour caught: a recovered or retired checkpoint offering Restore, or an observation without its recording offered as Resume.
#[test]
fn a_journaled_checkpoint_is_standing_waiting_or_closed_by_its_stage() {
    for stage in [CheckpointStage::Recovered, CheckpointStage::Retired] {
        let closed = checkpoint(stage, true);
        assert_eq!(
            JournaledApply::of(Some(&closed), false),
            JournaledApply::None,
            "{stage:?}"
        );
    }
    for stage in [
        CheckpointStage::Prepared,
        CheckpointStage::Parameters,
        CheckpointStage::Transfer,
    ] {
        let unresolved = checkpoint(stage, false);
        assert_eq!(
            JournaledApply::of(Some(&unresolved), false),
            JournaledApply::Waiting { resumable: false },
            "{stage:?}"
        );
    }
    let observing = checkpoint(CheckpointStage::Recording, true);
    assert_eq!(
        JournaledApply::of(Some(&observing), false),
        JournaledApply::Waiting { resumable: true }
    );
    let stopped = checkpoint(CheckpointStage::Recording, false);
    assert_eq!(
        JournaledApply::of(Some(&stopped), false),
        JournaledApply::Waiting { resumable: false },
        "a recording that is no longer journaled cannot be followed"
    );
}

/// B6.4: "Restore previous settings is offered while the loop's journal holds the Apply's checkpoint"; B6.5 and B7: Keep in project is
/// offered while an Apply's settings stand and the receipts allow it, never after a Restore; A8.6: Resume only for an observation the
/// runtime may still record. The wrong behaviour caught: Keep in project offered beside a Restore that waits, which would keep values the
/// runtime has not confirmed.
#[test]
fn restore_keep_and_resume_follow_the_journaled_checkpoint() {
    let completed = checkpoint(CheckpointStage::Completed, false);
    let standing = ApplicationOffers::of(
        JournaledApply::of(Some(&completed), false),
        None,
        Some(APPLIED),
        true,
    );
    assert_eq!(
        (standing.restore, standing.keep, standing.resume),
        (true, true, false)
    );
    let kept = ApplicationOffers::of(
        JournaledApply::of(Some(&completed), false),
        None,
        Some(APPLIED),
        false,
    );
    assert_eq!(
        (kept.restore, kept.keep),
        (true, false),
        "the receipts say the values are kept"
    );
    let unresolved = checkpoint(CheckpointStage::Parameters, false);
    let waiting = ApplicationOffers::of(
        JournaledApply::of(Some(&unresolved), false),
        None,
        Some(APPLIED),
        true,
    );
    assert_eq!(
        (waiting.restore, waiting.keep, waiting.resume),
        (true, false, false)
    );
    let observing = checkpoint(CheckpointStage::Recording, true);
    let resumable = ApplicationOffers::of(
        JournaledApply::of(Some(&observing), false),
        None,
        Some(APPLIED),
        true,
    );
    assert_eq!(
        (resumable.restore, resumable.keep, resumable.resume),
        (true, false, true),
        "Restore stays offered beside the Resume (A8.6)"
    );
    let running = ApplicationOffers::of(
        JournaledApply::of(Some(&observing), true),
        None,
        Some(APPLIED),
        true,
    );
    assert!(
        !running.resume,
        "an observation this engine follows is not resumed again"
    );
    let nothing = ApplicationOffers::of(JournaledApply::None, None, None, true);
    assert_eq!(
        (nothing.restore, nothing.keep, nothing.resume),
        (false, false, false)
    );
}

/// R2 (B7): "The position holds Apply while the engine offers it". The wrong behaviour caught: Restore or Keep in project taking the
/// position from an Apply that a validation after the Apply offers.
#[test]
fn primary_action_is_apply_while_apply_is_offered() {
    for (restore, keep) in [(false, false), (true, false), (true, true)] {
        assert_eq!(
            primary_action(true, restore, keep),
            Some(PrimaryAction::Apply),
            "restore {restore}, keep {keep}"
        );
    }
    let completed = checkpoint(CheckpointStage::Completed, false);
    let offers = ApplicationOffers::of(
        JournaledApply::of(Some(&completed), false),
        Some(NEWER),
        Some(APPLIED),
        true,
    );
    assert_eq!(offers.primary_action(), Some(PrimaryAction::Apply));
}

/// R2 (B7): "and otherwise Restore previous settings while the engine offers that". The wrong behaviour caught: a waiting checkpoint
/// leaving the position empty, so that the page offers no way to resolve it.
#[test]
fn primary_action_is_restore_when_only_restore_is_offered() {
    assert_eq!(
        primary_action(false, true, false),
        Some(PrimaryAction::Restore)
    );
    let unresolved = checkpoint(CheckpointStage::Transfer, false);
    let waiting = ApplicationOffers::of(
        JournaledApply::of(Some(&unresolved), false),
        Some(APPLIED),
        Some(APPLIED),
        true,
    );
    assert_eq!(waiting.primary_action(), Some(PrimaryAction::Restore));
    assert_eq!(
        primary_action(false, false, false),
        None,
        "nothing offered, nothing holds the position"
    );
}

/// R2: once an Apply's settings stand, Keep in project is the primary action while it is offered (the order the page held before the
/// engine stated it: Apply while offered, then Keep in project once offered, else Restore; B7 is silent on Keep), and Restore previous
/// settings keeps the position beside it. The wrong behaviour caught: Keep in project hidden behind Restore after a successful Apply.
#[test]
fn primary_action_is_keep_once_keep_is_offered_after_apply() {
    let completed = checkpoint(CheckpointStage::Completed, false);
    // The end of an Apply offers no second Apply (`apply_offered_after`); the reopened loop states the same for the applied validation.
    let reopened = ApplicationOffers::of(
        JournaledApply::of(Some(&completed), false),
        Some(APPLIED),
        Some(APPLIED),
        true,
    );
    assert_eq!(
        (reopened.apply, reopened.restore, reopened.keep),
        (false, true, true)
    );
    assert_eq!(reopened.primary_action(), Some(PrimaryAction::Keep));
    assert_eq!(
        primary_action(apply_offered_after(ReviewedAction::Apply), true, true),
        Some(PrimaryAction::Keep)
    );
    let words: Vec<serde_json::Value> = [
        PrimaryAction::Apply,
        PrimaryAction::Restore,
        PrimaryAction::Keep,
    ]
    .iter()
    .map(|action| serde_json::to_value(action).expect("a word"))
    .collect();
    assert_eq!(words, ["apply", "restore", "keep"], "the words of the wire");
}

/// B7 and the Validate page specification: an Apply or a Restore that has ended does not offer Apply; only a Validate that passes
/// afterwards does.
#[test]
fn no_application_offers_a_second_apply_until_a_new_validation_passes() {
    for ended in [ReviewedAction::Apply, ReviewedAction::Restore] {
        assert!(!apply_offered_after(ended), "{ended:?}");
    }
}

/// R2 (B9.4: "Apply needs a candidate whose current validation passed"; B7: "while an Apply's settings stand, Apply is not offered for
/// the validation that was applied"): in an answer that states the offers, the Apply stage can start exactly when Apply is offered. A
/// passed validation of the current candidate with nothing journaled offers Apply, and its stage can start; while the Apply of that same
/// validation stands, Apply is not offered and its stage cannot start, and Validate still can. The wrong behaviour caught: an Apply stage
/// that says Apply can start while the engine offers no Apply, so that the page, which enables Apply from the stage, offers it (N7).
#[test]
fn the_apply_stage_starts_exactly_when_apply_is_offered() {
    let evidence = StageEvidence {
        loop_bound: true,
        has_model: true,
        has_candidate: true,
        validation_has_result: true,
        validation_passed: true,
        controller_stages_apply: true,
        ..StageEvidence::default()
    };
    let available = |stages: &[StageAvailability], stage: Stage| {
        stages
            .iter()
            .find(|item| item.stage == stage)
            .map(|item| item.available)
    };
    let free = offered_stages(
        &evidence,
        None,
        apply_offered(JournaledApply::None, Some(APPLIED), None),
    );
    assert_eq!(available(&free, Stage::Apply), Some(true));
    let held = offered_stages(
        &evidence,
        None,
        apply_offered(JournaledApply::Standing, Some(APPLIED), Some(APPLIED)),
    );
    assert_eq!(available(&held, Stage::Apply), Some(false));
    assert_eq!(available(&held, Stage::Validate), Some(true));
}

/// The study's receipts of an Apply of `checkpoint:1` that applied [`APPLIED`], and of the Restore that the runtime confirmed for it
/// (A8.1, B6.4). `settings_applied` says whether the Apply's settings stood when it ended: they did before an engineer's Restore, and did
/// not when the Apply restored the previous settings at once (B6.3), which keeps its restoration receipt beside its own.
fn restored_receipts(settings_applied: bool) -> ApplySection {
    let status = if settings_applied { "passed" } else { "failed" };
    let result = serde_json::from_value(json!({
        "status": status,
        "reason": "The response was assessed.",
        "transfers": [],
        "recoveryConfirmed": !settings_applied,
        "target": 38.0,
        "modelRevision": "model:1",
        "channels": [],
        "criteria": {
            "target": 38.0,
            "durationSeconds": 60.0,
            "settlingTimeSeconds": 30.0,
            "maximumError": 0.2,
            "taskPeriodMs": 100.0,
            "processRange": { "minimum": 0.0, "maximum": 80.0 },
            "outputLimits": { "minimum": 0.0, "maximum": 100.0 }
        }
    }))
    .expect("a stored result");
    ApplySection {
        applications: vec![ApplicationReceipt {
            checkpoint: "checkpoint:1".to_owned(),
            validation_identity: APPLIED.to_owned(),
            settings_applied,
            result,
        }],
        restorations: vec![RestorationReceipt {
            checkpoint: "checkpoint:1".to_owned(),
            restored: true,
            retired: false,
            reason: "The previous settings were confirmed back.".to_owned(),
            mode: Some(FoundMode::Auto),
        }],
        keeps: Vec::new(),
    }
}

/// B7: "The end of an Apply and the end of a Restore answer all three: neither end offers Apply (`applyAvailable` is false), because the
/// Apply's settings stand or it restored the previous settings at once, and a candidate must pass validation after the application
/// before Apply is offered for it", and "One rule decides `applyAvailable` in every answer that states it, `controlStudio/open`,
/// `controlStudio/inputs` and `controlStudio/validate` alike" (each asks `ApplicationOffers::of` through the engine's
/// `ApplicationState`); B6.4: a retired checkpoint offers no Apply "until a new validation passes". After a confirmed Restore the
/// journal holds no checkpoint, the study's receipts name the Apply of APPLIED and its Restore, and APPLIED is still the study's passed
/// validation. Every answer then states what the Restore's end stated: no Apply, no Restore, no Keep and nothing in the position. A
/// validation that passed after the Restore (NEWER) is offered. The same holds after an Apply that restored the previous settings at once,
/// and when the journal's history is the Apply's only record: a retired checkpoint, and the recovered checkpoint of an Apply interrupted
/// before its receipt was kept (B6.4). The journal's checkpoint is the newest Apply's, so it names the applied validation before any
/// receipt does. The wrong behaviour caught: a reopened loop, an inputs change or a Validate answer offering Apply again for the
/// validation that the Restore undid, while the Restore's end offered none.
#[test]
fn after_a_confirmed_restore_every_answer_gives_the_same_apply_offer() {
    for (settings_applied, ended) in [
        (true, ReviewedAction::Restore),
        (false, ReviewedAction::Apply),
    ] {
        let receipts = restored_receipts(settings_applied);
        let applied = newest_applied_validation(None, receipts.newest_validation());
        assert_eq!(
            applied,
            Some(APPLIED),
            "the receipts name the applied validation"
        );
        let answered = ApplicationOffers::of(
            JournaledApply::of(None, false),
            Some(APPLIED),
            applied,
            receipts.keep_available(),
        );
        assert_eq!(answered.apply, apply_offered_after(ended), "{ended:?}");
        assert_eq!(
            (
                answered.apply,
                answered.restore,
                answered.keep,
                answered.primary_action()
            ),
            (false, false, false, None),
            "{ended:?}: the confirmed restoration closed the Apply's settings"
        );
        assert!(
            ApplicationOffers::of(JournaledApply::None, Some(NEWER), applied, false).apply,
            "{ended:?}: a validation that passed after the restoration may be applied"
        );
    }
    for stage in [CheckpointStage::Retired, CheckpointStage::Recovered] {
        let history = checkpoint(stage, false);
        let journaled = JournaledApply::of(Some(&history), false);
        let applied = newest_applied_validation(Some(&history), None);
        assert_eq!(
            ApplicationOffers::of(journaled, Some(APPLIED), applied, false).apply,
            apply_offered_after(ReviewedAction::Restore),
            "{stage:?}"
        );
        assert!(
            ApplicationOffers::of(journaled, Some(NEWER), applied, false).apply,
            "{stage:?}: a validation that passed after the Restore"
        );
    }
    let newer = CommissioningCheckpoint {
        validation_identity: NEWER.to_owned(),
        ..checkpoint(CheckpointStage::Completed, false)
    };
    assert_eq!(
        newest_applied_validation(Some(&newer), Some(APPLIED)),
        Some(NEWER),
        "an Apply whose receipt is not kept yet is the newest"
    );
}
