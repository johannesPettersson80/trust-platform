//! The rest of a loop found in Auto before a Manual method's mode change (specification 35, B3.4 step 1). Every expected value
//! follows from the rule of the steady baseline (B3.4 step 2): with a 200 s maximum duration and readings 0.1 s of process time
//! apart, the window is the longer of twenty readings (2 s) and 5 % of the maximum (10 s), so 10 s; the budget is the longest of
//! five minutes, a fifth of the maximum (40 s) and two windows (20 s), so 300 s. A signal is steady when its trend moves it across
//! the window by no more than four times its noise (from successive differences) or 0.001 % of its range. A PID group (C7) is the
//! two-member group of `mode_transfer_group_tests`, whose signals all run from 0 to 100 and whose task runs every 100 ms, so its
//! window and budget are the same.

use serde_json::json;

use super::{
    baseline_window_seconds, median_spacing, retained_start, trailing_window, RestStep,
    RestTracker, TimedValue, Watched, RATE_WINDOW_SAMPLES,
};
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::mode_transfer::group_tests::pid_group_loop;
use crate::identification::mode_transfer::mode_binding;
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
        vec![Watched::of_signal(&water()), Watched::of_signal(&heater())],
        200_000,
        100.0,
        None,
        offer_setpoint_step,
    )
}

/// The tracker of the two-member PID group before its mode transfer, for a 200 s maximum with no declared settling time.
fn group_tracker() -> RestTracker {
    let loop_definition = pid_group_loop();
    let binding = mode_binding(&loop_definition).expect("the group's mode binding");
    RestTracker::for_transfer(&loop_definition, &binding, 200_000, None, false)
        .expect("the group's rest")
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
            tracker.feed(seconds, &[water(seconds), heater(seconds)])
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
    assert_eq!(rest.feed(10.0, &[34.993, 74.965]), Ok(RestStep::Rested));
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
            .feed(300.0, &[34.2 + 0.01 * 300.0, 75.25])
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
        rest.feed(5.1, &[f64::NAN, 74.965]),
        Ok(RestStep::Waiting { .. })
    ));
    assert!(matches!(
        rest.feed(4.0, &[30.0, 10.0]),
        Ok(RestStep::Waiting { .. })
    ));
    let steps = feed(&mut rest, 51, 99, |_| 34.993, |_| 74.965);
    assert!(steps
        .iter()
        .all(|step| matches!(step, Ok(RestStep::Waiting { .. }))));
    assert_eq!(rest.feed(10.0, &[34.993, 74.965]), Ok(RestStep::Rested));
}

/// Feeds the group readings 0 to 100 (0 to 10 s), each the four values that `values` gives at its process time, and returns each
/// step's answer.
fn feed_group(
    tracker: &mut RestTracker,
    values: impl Fn(f64) -> [f64; 4],
) -> Vec<Result<RestStep, Failure>> {
    (0..=100)
        .map(|index| {
            let seconds = at(index);
            tracker.feed(seconds, &values(seconds))
        })
        .collect()
}

#[test]
fn a_pid_group_watches_every_members_measurement_and_every_held_output() {
    // Codex's review of #147 (31 October 2026): the rest watched the loop's own pair alone, while the group's transfer holds every
    // member's output. Catches a rest that leaves out a member: the measurements come first, the loop's own leading each pair.
    let rest = group_tracker();
    assert_eq!(
        rest.symbols().collect::<Vec<_>>(),
        [
            "P1.ProcessValue",
            "P1.SecondaryProcessValue",
            "P1.ManipulatedValue",
            "P1.SecondaryManipulatedValue"
        ]
    );
}

#[test]
fn a_second_member_still_moving_its_output_holds_the_wait_while_the_loops_own_pair_is_flat() {
    // The second member's output falls 0.02 % per second, 0.2 % across the 10 s window, beyond four times its noise of 0.0014 %:
    // 1.2 %/min. Every other signal is flat, so a rest judged on the loop's own pair would end here.
    let mut rest = group_tracker();
    let steps = feed_group(&mut rest, |seconds| {
        [40.0, 60.0, 30.0, 50.0 - 0.02 * seconds]
    });
    assert!(matches!(steps.last(), Some(Ok(RestStep::Waiting { .. }))));
    assert_eq!(
        progress_texts(&steps).last().map(String::as_str),
        Some("Waiting for the loop to settle in Auto (P1.SecondaryManipulatedValue changing 1.2 %/min)…")
    );
}

#[test]
fn a_second_members_measurement_still_moving_is_named_before_any_output() {
    // The second member's process value rises 0.01 degC per second (0.6 degC/min) while its output also moves: the measured output
    // is named first, as for a single loop.
    let mut rest = group_tracker();
    let steps = feed_group(&mut rest, |seconds| {
        [40.0, 60.0 + 0.01 * seconds, 30.0, 50.0 - 0.02 * seconds]
    });
    assert_eq!(
        progress_texts(&steps).last().map(String::as_str),
        Some("Waiting for the loop to settle in Auto (P1.SecondaryProcessValue changing 0.6 degC/min)…")
    );
}

#[test]
fn a_pid_group_at_rest_rests_once_one_window_is_full() {
    // Every member flat: the wait goes on until 10 s and ends with the reading at 10 s, as for a single loop.
    let mut rest = group_tracker();
    let steps = feed_group(&mut rest, |_| [40.0, 60.0, 30.0, 50.0]);
    assert!(steps[..100]
        .iter()
        .all(|step| matches!(step, Ok(RestStep::Waiting { .. }))));
    assert_eq!(steps.last(), Some(&Ok(RestStep::Rested)));
}

#[test]
fn a_group_reading_with_a_value_too_few_is_left_out() {
    // A reading of three values for four signals neither ends the wait nor counts towards the window. Catches values paired with
    // the wrong signals.
    let mut rest = group_tracker();
    assert!(matches!(
        rest.feed(0.0, &[40.0, 60.0, 30.0]),
        Ok(RestStep::Waiting { .. })
    ));
    assert!(rest.signals.iter().all(|signal| signal.points.is_empty()));
}

#[test]
fn a_long_wait_keeps_only_the_readings_its_window_can_judge() {
    // Codex's review of #147 (F31): the wait kept every reading until its budget, about 84 MiB on a 24-hour campaign of a
    // 16-member group. Here the window is 10 s, 101 readings, and the spacing is judged on the last 200, so whatever the wait's
    // length a signal keeps those 200 and fewer than 200 older ones, at most 399, instead of the 3,000 of the 300 s budget. The
    // loop still moving is refused at 300 s with the rate of its last window, as before. Catches a history that grows with the
    // wait.
    let mut rest = tracker(false);
    let mut most = 0;
    for index in 0..3_000 {
        let seconds = at(index);
        assert!(rest.feed(seconds, &[34.2 + 0.01 * seconds, 75.25]).is_ok());
        let kept = rest.signals.iter().map(|signal| signal.points.len());
        most = most.max(kept.max().unwrap_or(0));
    }
    assert!(most <= 399, "a signal kept {most} readings");
    let refusal = rest
        .feed(300.0, &[34.2 + 0.01 * 300.0, 75.25])
        .expect_err("after the budget the moving loop is refused");
    assert_eq!(
        whole_text(&refusal),
        "P1.TemperatureC is still changing by 0.6 degC/min in Auto. Wait for the loop to settle."
    );
}

#[test]
fn the_readings_kept_hold_the_window_when_the_readings_slow_down() {
    // 300 readings 0.1 s apart, then 300 readings 2 s apart: once most of the last 200 gaps are 2 s, the window is twenty of them,
    // 40 s, beyond its 10 s cap (a 200 s maximum). After every reading, every reading the window judges is kept. Catches readings
    // forgotten at the cap, which would judge that wider window on its last 10 s alone.
    let mut points = Vec::new();
    let mut seconds = 0.0;
    let mut window = 0.0;
    for index in 0..600 {
        seconds += if index < 300 { 0.1 } else { 2.0 };
        points.push(TimedValue {
            seconds,
            value: 0.0,
        });
        let spacing = &points[points.len().saturating_sub(RATE_WINDOW_SAMPLES)..];
        window = baseline_window_seconds(200.0, median_spacing(spacing, 0.1), None);
        let judged = trailing_window(&points, window).len();
        assert!(
            points.len() - judged >= retained_start(&points, 10.0),
            "reading {index}: the window of {window} s judges readings that are forgotten"
        );
    }
    assert!(
        (window - 40.0).abs() < 1e-9,
        "the window widened to {window} s"
    );
}
