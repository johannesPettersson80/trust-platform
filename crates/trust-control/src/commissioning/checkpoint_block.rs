//! The block of a loop's stages while an Apply's checkpoint waits (specification 35, B6.4, B6.7, A8.6; annex E11.5, E11.7). An Apply or
//! a Restore that stopped before its end leaves writes that the runtime has not confirmed, and new work cannot bypass their owner: until
//! Restore previous settings resolves them, or the Resume of an interrupted observation follows the Apply to its end, the stages whose
//! action writes or changes the study wait. The stage statuses carry the block's reason, so the block holds after a reopen
//! (`stages::blocked_stage_availability`), and the engine refuses Identify, Tune, Validate, Reset, the imports and a change of the study
//! inputs with that reason and the next action Restore; an Apply is refused with the next action Restore by its own review
//! (`apply_texts::checkpoint_unresolved`).

use super::application_offers::JournaledApply;
use crate::failure::{Cause, Failure, NextAction};

/// The reason of the block of an Apply or a Restore that stopped before the runtime confirmed its end (B6.7: "dependent actions stay
/// blocked until confirmed").
pub const UNRESOLVED_APPLY: &str =
    "An Apply or a Restore of this loop stopped before the runtime confirmed its end. Restore \
     previous settings first: until the runtime confirms it, the loop's other actions wait.";

/// The reason of the block of an Apply that was interrupted during its observation, which the runtime may still record (A8.6).
pub const INTERRUPTED_OBSERVATION: &str =
    "An Apply of this loop was interrupted during its observation. Resume it to see its \
     result, or restore previous settings: until then, the loop's other actions wait.";

/// The reason of the block that `journaled` holds the loop under, or `None` when nothing holds it: only a checkpoint that waits blocks,
/// while an Apply whose settings stand does not hold back Identify, Tune or Validate (B7), and one that runs is followed to its end.
#[must_use]
pub const fn block_reason(journaled: JournaledApply) -> Option<&'static str> {
    match journaled {
        JournaledApply::Waiting { resumable: false } => Some(UNRESOLVED_APPLY),
        JournaledApply::Waiting { resumable: true } => Some(INTERRUPTED_OBSERVATION),
        JournaledApply::None | JournaledApply::Standing | JournaledApply::InProgress => None,
    }
}

/// The refusal of an action that the block holds (B6.7, E11.7): the block's reason, with the next action Restore previous settings.
/// `None` when nothing holds the loop.
#[must_use]
pub fn block_refusal(journaled: JournaledApply) -> Option<Failure> {
    block_reason(journaled).map(|reason| Failure::new(reason, Cause::Runtime, NextAction::Restore))
}

#[cfg(test)]
#[path = "checkpoint_block_tests.rs"]
mod tests;
