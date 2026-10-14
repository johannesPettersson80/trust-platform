//! The sentences of Apply's and Restore's own refusals (specification 35, §0.1b Confirmation, A2.5 and B6; annex E7.1 and
//! E11.8). Each is a whole failure with its cause and typed next action (A11.1, A11.2), so that both front ends and every
//! workflow that refuses for the same reason refuse in the same words.

use crate::failure::{Cause, Failure, NextAction};

/// The confirmation names a review identity that the live runtime no longer gives (§0.1b Confirmation, E11.8): a revision of
/// the parameter record, the mode, the values to write, the transfer binding or the study's validation changed since the
/// review, so nothing is written. No typed next action reviews (A11.2), so the action is none and the sentence asks for the new
/// review, which the same action without a review identity produces.
#[must_use]
pub fn review_changed() -> Failure {
    Failure::new(
        "The runtime changed since the review; review again.",
        Cause::Runtime,
        NextAction::None,
    )
}

/// The target is physical and the request does not carry the engineer's approval, which every live write on a physical target
/// needs immediately before it starts (A2.5, E10.2). The approval is given on the page; the command line has none, so it
/// refuses a physical target with this failure (§0.1b).
#[must_use]
pub fn physical_approval_needed() -> Failure {
    Failure::new(
        "Physical approval is required for Apply or Restore on a physical target. Review the change on the page and approve it immediately before it starts.",
        Cause::Approval,
        NextAction::Approve,
    )
}

/// The loop declares no parameter binding, so there is no parameter record to read or write (`connectRuntimeParameters`,
/// runtimeParameterConnection.ts; E7.1). The engine's `RuntimeClient::parameter_values` refuses in the same words.
#[must_use]
pub fn parameter_binding_missing() -> Failure {
    Failure::new(
        "Declare the ST parameter binding before application.",
        Cause::Settings,
        NextAction::OpenLoopSetup,
    )
}

#[cfg(test)]
#[path = "application_texts_tests.rs"]
mod tests;
