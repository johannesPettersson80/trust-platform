//! What the parameter tool of a C12 loop offers, and how its write ended (specification 35, C12): the step the tool's own button runs
//! next, the actions the engine takes now, and the outcome of a write with its sentence. The engine decides each from its own state, the
//! loop's claim and journal and the review an answer gives; the page renders them and keeps no phase of its own.

#![allow(clippy::module_name_repetitions)] // The types name the tool they describe: the parameter tool.

use schemars::JsonSchema;
use serde::Serialize;

use crate::commissioning::parameter_application::applied_reason;
use crate::failure::Failure;

/// One action of the parameter tool (C12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ParameterToolStep {
    /// Read the values the runtime holds (`inspect`). It writes nothing, so the engine takes it in every state.
    Inspect,
    /// Review a write of the values the engineer typed (`review`).
    Review,
    /// Confirm the review the answer gives (`confirm`): the engine applies its values, or restores the values from before when the
    /// review is a Restore's.
    Apply,
    /// Review a Restore of the values the runtime retains from before the last write (`reviewRestore`, E7.5).
    Restore,
}

/// What the parameter tool offers after an answer (C12): the step its own button runs next, and every action the engine takes now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ParameterToolOffer {
    /// The step the tool's own button runs next: `inspect` while the settings are frozen, `apply` after a review, and `review`
    /// otherwise.
    pub next_step: ParameterToolStep,
    /// The actions the engine takes now, in the tool's order: `inspect` always; `review` and `restore` while the settings are free; and
    /// `apply` after a review, which it confirms.
    pub available: Vec<ParameterToolStep>,
}

/// How a parameter write ended (C12, E7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ParameterWriteStatus {
    /// The runtime reported the written values after the owning task executed.
    Applied,
    /// The values were not written: the runtime refused the write or did not report the written values, or the record changed since
    /// the review.
    Failed,
}

/// The offer of the parameter tool (C12) from the engine's state. The settings are free when no operation of the engine works on the
/// loop and its journal holds no unfinished operation; while they are not (a project test holds them, A8.5), the tool can only read them.
/// `review_given` says the answer is a review, whose identity the confirmation names (E11.8).
#[must_use]
pub fn parameter_tool_offer(settings_free: bool, review_given: bool) -> ParameterToolOffer {
    use ParameterToolStep::{Apply, Inspect, Restore, Review};
    if !settings_free {
        return ParameterToolOffer {
            next_step: Inspect,
            available: vec![Inspect],
        };
    }
    if review_given {
        return ParameterToolOffer {
            next_step: Apply,
            available: vec![Inspect, Review, Apply, Restore],
        };
    }
    ParameterToolOffer {
        next_step: Review,
        available: vec![Inspect, Review, Restore],
    }
}

/// The tool's first offer, which the open answer's project test form names (C12): before any answer of `controlStudio/parameters`, the
/// tool's step is Inspect, which reads only and which the engine takes in every state, and nothing else is offered until the engine
/// answers it.
#[must_use]
pub fn first_tool_offer() -> ParameterToolOffer {
    ParameterToolOffer {
        next_step: ParameterToolStep::Inspect,
        available: vec![ParameterToolStep::Inspect],
    }
}

/// How a write that was sent ended, with the sentence the page shows (C12, E7.4): applied, with the sentence of what the runtime's record
/// shows (an Apply's reviewed values, or a Restore's previous values, observed after the task executed), when the runtime confirmed the
/// written values; otherwise failed, with the failure's whole text.
#[must_use]
pub fn write_outcome(
    rollback: bool,
    confirmed: Result<(), &Failure>,
) -> (ParameterWriteStatus, String) {
    match confirmed {
        Ok(()) => (ParameterWriteStatus::Applied, applied_reason(rollback)),
        Err(failure) => (
            ParameterWriteStatus::Failed,
            failure
                .detail
                .clone()
                .unwrap_or_else(|| failure.message.clone()),
        ),
    }
}
