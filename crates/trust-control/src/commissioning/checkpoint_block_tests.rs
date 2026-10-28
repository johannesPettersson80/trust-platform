//! Tests of the block of a loop's stages while an Apply's checkpoint waits for its Restore (specification 35, B6.4, B6.7, A8.6; annex
//! E11.7). The TypeScript page held this block as a lock of its own (`threePageRecovery.ts`), armed only by a failure the page received
//! in its session; the engine states it from the journal, so a reopened loop holds it too (`a_waiting_checkpoint_blocks_after_a_reopen`
//! in the engine's `application_state` tests).

use super::*;
use crate::loop_definition::NO_CONTROLLER_LAW;
use crate::stages::{blocked_stage_availability, stage_availability, Stage, StageEvidence};

/// The evidence of a study whose candidate passed its current validation: without a block, every stage of B9.4 can start.
fn passed() -> StageEvidence {
    StageEvidence {
        has_model: true,
        has_recording: true,
        has_candidate: true,
        validation_has_result: true,
        validation_passed: true,
        ..StageEvidence::default()
    }
}

/// R3 (B6.7: "Restore not confirmed | Restore again; dependent actions stay blocked until confirmed"; E11.7: "Dependent commands ...
/// stay locked until the operation is reconciled"): a waiting checkpoint holds every stage whose action writes or changes the study,
/// and each names the block's reason, while its status still follows the evidence. Monitor only observes. The refusal of a held action
/// is the same reason with the next action Restore. The wrong behaviour caught: a block that leaves Identify, Tune, Validate or Apply
/// available, that drops its reason, or that names a next action other than Restore.
#[test]
fn a_waiting_checkpoint_blocks_every_stage_with_its_reason() {
    let waiting = JournaledApply::Waiting { resumable: false };
    let reason = block_reason(waiting).expect("a waiting checkpoint blocks");
    assert_eq!(reason, UNRESOLVED_APPLY);
    let evidence = passed();
    let blocked = blocked_stage_availability(&evidence, Some(reason));
    let free = stage_availability(&evidence);
    for stage in [
        Stage::Test,
        Stage::Identify,
        Stage::Tune,
        Stage::Validate,
        Stage::Apply,
    ] {
        let held = blocked
            .iter()
            .find(|item| item.stage == stage)
            .expect("every stage");
        let unheld = free
            .iter()
            .find(|item| item.stage == stage)
            .expect("every stage");
        assert!(unheld.available, "{stage:?} starts without the block");
        assert!(!held.available, "{stage:?} waits for the Restore");
        assert_eq!(held.reason, Some(UNRESOLVED_APPLY), "{stage:?}");
        assert_eq!(
            held.status, unheld.status,
            "{stage:?} keeps the status of its evidence"
        );
    }
    let monitor = blocked
        .iter()
        .find(|item| item.stage == Stage::Monitor)
        .expect("Monitor");
    assert!(monitor.available, "observing writes nothing");
    assert_eq!(monitor.reason, None);
    let wire = serde_json::to_value(blocked[2]).expect("the Identify stage");
    assert_eq!(wire["stage"], "Identify", "{wire}");
    assert_eq!(wire["available"], serde_json::json!(false), "{wire}");
    assert_eq!(wire["reason"], UNRESOLVED_APPLY, "{wire}");
    let refusal = block_refusal(waiting).expect("a held action is refused");
    assert_eq!(refusal.next_action, NextAction::Restore);
    assert_eq!(
        refusal.message,
        "An Apply or a Restore of this loop stopped before the runtime confirmed its end."
    );
    assert_eq!(refusal.detail.as_deref(), Some(UNRESOLVED_APPLY));
    // C12: a stage that does not apply keeps its own reason under a block.
    let lawless = StageEvidence {
        controller_stages_apply: false,
        ..evidence
    };
    let tune = blocked_stage_availability(&lawless, Some(reason))
        .into_iter()
        .find(|item| item.stage == Stage::Tune)
        .expect("Tune");
    assert_eq!(tune.reason, Some(NO_CONTROLLER_LAW));
}

/// B7: "A standing Apply's checkpoint is not an unfinished operation: it does not hold back Identify, Tune or Validate", and an Apply or
/// Restore that this engine runs is followed to its end, not blocked. An interrupted observation blocks with its own reason, which
/// names its Resume, and the next action Restore (A8.6). The wrong behaviour caught: a completed Apply that locks the loop, or no block at
/// all with no checkpoint.
#[test]
fn only_a_checkpoint_that_waits_blocks_the_stages() {
    for free in [
        JournaledApply::None,
        JournaledApply::Standing,
        JournaledApply::InProgress,
    ] {
        assert_eq!(block_reason(free), None, "{free:?}");
        assert!(block_refusal(free).is_none(), "{free:?}");
    }
    assert_eq!(
        blocked_stage_availability(&passed(), None),
        stage_availability(&passed()),
        "no block, the stages as the evidence gives them"
    );
    let interrupted = JournaledApply::Waiting { resumable: true };
    assert_eq!(block_reason(interrupted), Some(INTERRUPTED_OBSERVATION));
    let refusal = block_refusal(interrupted).expect("refused");
    assert_eq!(refusal.next_action, NextAction::Restore);
    assert_eq!(
        refusal.message,
        "An Apply of this loop was interrupted during its observation."
    );
}
