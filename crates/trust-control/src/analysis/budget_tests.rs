//! The budget of an analysis operation (A7): the time limit, checked between steps, and the cancellation of the operation.

use std::cell::Cell;
use std::time::Duration;

use super::{Budget, ANALYSIS_BUDGET, BUDGET_EXCEEDED};
use crate::failure::{Cause, NextAction};

#[test]
fn the_budget_is_ten_seconds_and_a_step_within_it_passes() {
    assert_eq!(ANALYSIS_BUDGET, Duration::from_secs(10));
    let now = Cell::new(Duration::from_millis(9_900));
    let elapsed = || now.get();
    let stop = || false;
    let budget = Budget::new(ANALYSIS_BUDGET, &elapsed, &stop);
    assert_eq!(budget.step(), Ok(()));
    // Exactly the limit is still within the budget; only more than the limit is past it.
    now.set(Duration::from_secs(10));
    assert_eq!(budget.step(), Ok(()));
}

#[test]
fn a_step_past_the_budget_fails_with_the_a7_sentence_and_no_next_action() {
    let now = Cell::new(Duration::from_millis(10_001));
    let elapsed = || now.get();
    let stop = || false;
    let budget = Budget::new(ANALYSIS_BUDGET, &elapsed, &stop);
    let failure = budget.step().expect_err("past the budget");
    assert_eq!(failure.message, BUDGET_EXCEEDED);
    assert_eq!(failure.message, "Analysis exceeded the 10 second budget.");
    assert_eq!(failure.cause, Cause::Settings);
    assert_eq!(failure.next_action, NextAction::None);
}

#[test]
fn a_cancelled_operation_stops_at_its_next_step_even_within_the_budget() {
    let now = Cell::new(Duration::ZERO);
    let elapsed = || now.get();
    let cancelled = Cell::new(false);
    let stop = || cancelled.get();
    let budget = Budget::new(ANALYSIS_BUDGET, &elapsed, &stop);
    assert_eq!(budget.step(), Ok(()));
    cancelled.set(true);
    let failure = budget.step().expect_err("cancelled");
    assert_eq!(failure.message, "Analysis was cancelled.");
    // The cancellation is checked before the time, so a cancelled step past the budget is reported as cancelled.
    now.set(Duration::from_secs(60));
    assert_eq!(
        budget.step().expect_err("cancelled and late").message,
        "Analysis was cancelled."
    );
}
