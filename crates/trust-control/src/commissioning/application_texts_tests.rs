//! Tests of Apply's and Restore's own refusals: each keeps its sentence, its cause and its typed next action (specification 35,
//! A11.1 and A11.2).

use super::*;

/// §0.1b Confirmation and annex E11.8: a confirmation whose identity the live runtime no longer gives is refused; the sentence
/// asks for a new review, and no typed action stands for one.
#[test]
fn a_changed_runtime_asks_for_a_new_review() {
    let failure = review_changed();
    assert_eq!(
        failure.message,
        "The runtime changed since the review; review again."
    );
    assert_eq!(failure.detail, None, "the sentence is the whole text");
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Runtime, NextAction::None)
    );
}

/// A2.5 and annex E10.2: a physical target without the engineer's approval is refused, and approving is the next action.
#[test]
fn a_physical_target_without_approval_asks_for_the_approval() {
    let failure = physical_approval_needed();
    assert_eq!(
        failure.message,
        "Physical approval is required for Apply or Restore on a physical target."
    );
    assert_eq!(
        failure.detail.as_deref(),
        Some("Physical approval is required for Apply or Restore on a physical target. Review the change on the page and approve it immediately before it starts.")
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Approval, NextAction::Approve)
    );
}

/// Annex E7.1 (`connectRuntimeParameters`, runtimeParameterConnection.ts): a loop without a parameter binding has no record to
/// read or write, and its setup is where one is declared.
#[test]
fn a_loop_without_a_parameter_binding_opens_its_setup() {
    let failure = parameter_binding_missing();
    assert_eq!(
        failure.message,
        "Declare the ST parameter binding before application."
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Settings, NextAction::OpenLoopSetup)
    );
}
