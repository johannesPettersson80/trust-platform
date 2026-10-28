#![allow(clippy::float_cmp)]

// No TypeScript test pins the confirmation window: these assertions state specification 35, A7, and the formula of
// `confirmationWindow.ts`.

use serde_json::json;

use super::*;

#[allow(clippy::needless_pass_by_value)] // A test fixture: it takes the JSON literals its callers write in place.
fn status(tasks: Value) -> Value {
    json!({ "tasks": tasks })
}

#[test]
fn without_a_status_the_window_is_ten_seconds_and_the_status_is_read_every_25_ms() {
    let timing = confirmation_timing(None, "MainTask");
    assert_eq!(timing.window_ms, 10_000.0);
    assert_eq!(timing.poll_ms, 25.0);
}

#[test]
fn a_fast_task_keeps_the_ten_second_floor() {
    // 100 ms: three executions plus the 5 s connection-loss timeout are 5.3 s, below the floor (A7).
    let timing = confirmation_timing(
        Some(&status(json!([{"name": "MainTask", "interval_ms": 100}]))),
        "MainTask",
    );
    assert_eq!(timing.window_ms, 10_000.0);
    assert_eq!(timing.poll_ms, 25.0);
}

#[test]
fn a_slow_task_waits_three_executions_plus_the_connection_loss_timeout() {
    // 2 s: 3 × 2 s + 5 s = 11 s, above the floor (A7). The status is read every quarter execution, 500 ms.
    let timing = confirmation_timing(
        Some(&status(json!([{"name": "MainTask", "interval_ms": 2000}]))),
        "MainTask",
    );
    assert_eq!(timing.window_ms, 11_000.0);
    assert_eq!(timing.poll_ms, 500.0);
}

#[test]
fn the_status_is_read_every_quarter_execution_within_25_ms_and_one_second() {
    // 400 ms: a quarter execution is 100 ms.
    let quarter = confirmation_timing(
        Some(&status(json!([{"name": "MainTask", "interval_ms": 400}]))),
        "MainTask",
    );
    assert_eq!(quarter.poll_ms, 100.0);
    // 8 s: a quarter execution is 2 s, read at most every second.
    let slowest = confirmation_timing(
        Some(&status(json!([{"name": "MainTask", "interval_ms": 8000}]))),
        "MainTask",
    );
    assert_eq!(slowest.poll_ms, 1_000.0);
    assert_eq!(slowest.window_ms, 29_000.0);
}

#[test]
fn only_the_named_task_counts_and_an_unusable_interval_is_zero() {
    let other_only = status(json!([{"name": "Other", "interval_ms": 2000}]));
    assert_eq!(task_interval_ms(Some(&other_only), "MainTask"), 0.0);
    let text = status(json!([{"name": "MainTask", "interval_ms": "fast"}]));
    assert_eq!(task_interval_ms(Some(&text), "MainTask"), 0.0);
    let negative = status(json!([{"name": "MainTask", "interval_ms": -5}]));
    assert_eq!(task_interval_ms(Some(&negative), "MainTask"), 0.0);
    let zero = status(json!([{"name": "MainTask", "interval_ms": 0}]));
    assert_eq!(task_interval_ms(Some(&zero), "MainTask"), 0.0);
    let no_tasks = json!({ "runtime": "running" });
    assert_eq!(task_interval_ms(Some(&no_tasks), "MainTask"), 0.0);
    assert_eq!(task_interval_ms(None, "MainTask"), 0.0);
}

#[test]
fn the_first_task_of_the_name_decides_and_a_malformed_entry_is_skipped() {
    let tasks = status(json!([
        "not a task",
        {"name": "MainTask", "interval_ms": 200},
        {"name": "MainTask", "interval_ms": 9000}
    ]));
    assert_eq!(task_interval_ms(Some(&tasks), "MainTask"), 200.0);
}

#[test]
fn the_window_is_the_floor_or_three_executions_plus_the_connection_loss_timeout() {
    assert_eq!(confirmation_window_ms(0.0), 10_000.0);
    assert_eq!(confirmation_window_ms(100.0), 10_000.0);
    assert_eq!(confirmation_window_ms(2_000.0), 11_000.0);
}
