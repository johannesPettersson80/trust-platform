//! The rest of a loop found in Auto before a Manual method's mode change (specification 35, B3.4 step 1). Every expected value
//! follows from the rule of the steady baseline (B3.4 step 2): with a 200 s maximum duration and readings 0.1 s of process time
//! apart, the window is the longer of twenty readings (2 s) and 5 % of the maximum (10 s), so 10 s; the budget is the longest of
//! five minutes, a fifth of the maximum (40 s) and two windows (20 s), so 300 s. A signal is steady when its trend moves it across
//! the window by no more than four times its noise (from successive differences) or 0.001 % of its range.

use serde_json::json;

use super::{RestStep, RestTracker};
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::LoopSignal;

/// Lesson 30's water, read in 0 to 80 degC.
fn water() -> LoopSignal {
    serde_json::from_value(
        json!({ "symbol": "P1.TemperatureC", "iecType": "LREAL", "direction": "read",
        "unit": "degC", "range": { "minimum": 0, "maximum": 80 } }),
    )
    .expect("the water's signal")
}

/// Lesson 30's heater, the controller output a transfer holds, in 0 to 100 %.
fn heater() -> LoopSignal {
    serde_json::from_value(
        json!({ "symbol": "P1.AppliedPowerPct", "iecType": "LREAL", "direction": "write",
        "unit": "%", "range": { "minimum": 0, "maximum": 100 } }),
    )
    .expect("the heater's signal")
}

/// A tracker for a 200 s maximum duration and a 100 ms task, with no declared settling time.
fn tracker(offer_setpoint_step: bool) -> RestTracker {
    RestTracker::new(
        &water(),
        &heater(),
        200_000,
        100.0,
        None,
        offer_setpoint_step,
    )
}

/// The process time of reading `index`, 0.1 s apart, exactly.
fn at(index: u32) -> f64 {
    f64::from(index) / 10.0
}

/// Feeds readings `from` to `to` of `water` and `heater` (functions of process time) and returns each step's answer.
fn feed(
    tracker: &mut RestTracker,
    from: u32,
    to: u32,
    water: impl Fn(f64) -> f64,
    heater: impl Fn(f64) -> f64,
) -> Vec<Result<RestStep, Failure>> {
    (from..=to)
        .map(|index| {
            let seconds = at(index);
            tracker.feed(seconds, water(seconds), heater(seconds))
        })
        .collect()
}

fn whole_text(failure: &Failure) -> &str {
    failure.detail.as_deref().unwrap_or(&failure.message)
}

fn progress_texts(steps: &[Result<RestStep, Failure>]) -> Vec<String> {
    steps
        .iter()
        .filter_map(|step| match step {
            Ok(RestStep::Waiting { progress }) => progress.clone(),
            _ => None,
        })
        .collect()
}

#[test]
fn a_loop_at_rest_in_auto_rests_once_one_baseline_window_is_full() {
    // Catches a rest judged on fewer readings than the baseline's window, and one that never ends for a loop at rest.
    let mut rest = tracker(false);
    let steps = feed(&mut rest, 0, 99, |_| 34.993, |_| 74.965);
    assert!(
        steps
            .iter()
            .all(|step| matches!(step, Ok(RestStep::Waiting { .. }))),
        "before 10 s the window is not full"
    );
    assert_eq!(rest.feed(10.0, 34.993, 74.965), Ok(RestStep::Rested));
}

#[test]
fn a_controller_output_still_moving_holds_the_wait_while_the_measurement_is_flat() {
    // Lesson 30 (30 October 2026): Identify took the loop to Manual while the controller was still moving the heater, and the
    // water settled open loop at 35.05 °C, away from the 34.99 °C reference. Catches a rest judged on the measurement alone.
    let mut rest = tracker(false);
    let steps = feed(
        &mut rest,
        0,
        100,
        |_| 34.99,
        |seconds| 75.36 - 0.02 * seconds,
    );
    assert!(matches!(steps.last(), Some(Ok(RestStep::Waiting { .. }))));
    // The heater moves 0.2 % across the 10 s window, beyond four times its noise of 0.0014 % (steps of 0.002 %): 1.2 %/min.
    assert_eq!(
        progress_texts(&steps).last().map(String::as_str),
        Some("Waiting for the loop to settle in Auto (P1.AppliedPowerPct changing 1.2 %/min)…")
    );
}

#[test]
fn a_measurement_still_rising_holds_the_wait_and_is_named() {
    // The water rises 0.01 degC/s, 0.6 degC/min, while the heater is held: catches a rest judged on the output alone.
    let mut rest = tracker(false);
    let steps = feed(
        &mut rest,
        0,
        100,
        |seconds| 34.2 + 0.01 * seconds,
        |_| 75.25,
    );
    assert!(matches!(steps.last(), Some(Ok(RestStep::Waiting { .. }))));
    assert_eq!(
        progress_texts(&steps).last().map(String::as_str),
        Some("Waiting for the loop to settle in Auto (P1.TemperatureC changing 0.6 degC/min)…")
    );
}

#[test]
fn a_progress_is_shown_at_most_every_ten_seconds_of_process_time() {
    // Readings at 0 s, 10 s and 20 s carry a progress; the 248 readings between them carry none.
    let mut rest = tracker(false);
    let steps = feed(
        &mut rest,
        0,
        250,
        |seconds| 34.2 + 0.01 * seconds,
        |_| 75.25,
    );
    assert_eq!(progress_texts(&steps).len(), 3);
}

#[test]
fn a_loop_still_moving_after_the_budget_is_refused_with_the_rate_and_the_offer() {
    // The budget is 300 s: the reading at 299.9 s still waits, the one at 300 s refuses. Without the offer the next action is to
    // wait; with it, the refusal offers a setpoint step, as the baseline's does for a loop in Auto (B3.4 step 2).
    for (offer, text, next_action) in [
        (false, "P1.TemperatureC is still changing by 0.6 degC/min in Auto. Wait for the loop to settle.", NextAction::Wait),
        (
            true,
            "P1.TemperatureC is still changing by 0.6 degC/min in Auto. Wait for the loop to settle, or identify with a setpoint \
             step.",
            NextAction::ChooseMethod,
        ),
    ] {
        let mut rest = tracker(offer);
        let steps = feed(&mut rest, 0, 2_999, |seconds| 34.2 + 0.01 * seconds, |_| 75.25);
        assert!(steps.iter().all(Result::is_ok), "within the budget the wait goes on");
        let refusal = rest
            .feed(300.0, 34.2 + 0.01 * 300.0, 75.25)
            .expect_err("after the budget the moving loop is refused");
        assert_eq!(whole_text(&refusal), text);
        assert_eq!(refusal.cause, Cause::Settings);
        assert_eq!(refusal.next_action, next_action);
    }
}

#[test]
fn a_reading_that_is_not_finite_or_not_later_is_left_out() {
    // A stale or broken reading neither ends the wait early nor restarts it: the loop at rest still rests at 10 s.
    let mut rest = tracker(false);
    let _ = feed(&mut rest, 0, 50, |_| 34.993, |_| 74.965);
    assert!(matches!(
        rest.feed(5.1, f64::NAN, 74.965),
        Ok(RestStep::Waiting { .. })
    ));
    assert!(matches!(
        rest.feed(4.0, 30.0, 10.0),
        Ok(RestStep::Waiting { .. })
    ));
    let steps = feed(&mut rest, 51, 99, |_| 34.993, |_| 74.965);
    assert!(steps
        .iter()
        .all(|step| matches!(step, Ok(RestStep::Waiting { .. }))));
    assert_eq!(rest.feed(10.0, 34.993, 74.965), Ok(RestStep::Rested));
}
