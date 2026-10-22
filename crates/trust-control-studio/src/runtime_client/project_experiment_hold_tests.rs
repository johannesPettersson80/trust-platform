//! Port of `control-studio-experiment.test.ts` test 3 (the applied hold of a project step test is read from the runtime's
//! parameters) and of the hold's duration rule, which trust-control owns and this module re-exports.

use std::cell::Cell;

use serde_json::json;

use crate::runtime_client::fake_runtime::{answer, hold_loop, tank_loop, FakeRuntime};
use crate::runtime_client::RuntimeClient;

use super::*;

#[test]
fn test_3_the_applied_hold_is_read_from_the_runtime_parameters() {
    let fake = FakeRuntime::start(|request| {
        Some(answer(
            request,
            json!({ "values": { "P1.HoldSeconds": 30.0, "P1.SelectedStepPoints": 10.0 } }),
        ))
    });
    let connection = fake.connection(None);
    let hold = applied_project_hold_seconds(&hold_loop(), || RuntimeClient::connect(&connection))
        .expect("read");
    assert_eq!(hold, Some(30.0));
    assert_eq!(fake.requests_of("control.parameters.status").len(), 1);
}

#[test]
fn a_loop_without_a_hold_parameter_connects_to_nothing() {
    let connected = Cell::new(false);
    let hold = applied_project_hold_seconds(&tank_loop(), || {
        connected.set(true);
        Err(Failure::unreachable("not expected"))
    })
    .expect("no hold");
    assert_eq!(hold, None);
    assert!(
        !connected.get(),
        "a loop without a hold parameter must not connect"
    );
}

#[test]
fn a_runtime_that_does_not_report_the_hold_is_refused_by_name() {
    let fake = FakeRuntime::start(|request| {
        Some(answer(
            request,
            json!({ "values": { "P1.SelectedStepPoints": 10.0 } }),
        ))
    });
    let connection = fake.connection(None);
    let failure =
        applied_project_hold_seconds(&hold_loop(), || RuntimeClient::connect(&connection))
            .expect_err("the hold is not reported");
    assert!(failure.message.contains("P1.HoldSeconds"), "{failure:?}");
}

#[test]
fn the_hold_duration_is_the_baseline_the_whole_hold_and_one_restoration_scan() {
    // No baseline: a 30 s hold at a 10 ms period is 3000 scans, plus the restoration scan (trust-control's rule).
    assert_eq!(project_step_recording_ms(&tank_loop(), 30.0), 30_010.0);
}

#[test]
fn a_request_without_a_connection_cannot_read_a_hold_and_asks_for_the_runtime() {
    let refused = applied_project_hold_over(&hold_loop(), None).expect_err("no connection");
    assert_eq!(refused, runtime_required());
    assert_eq!(
        applied_project_hold_over(&tank_loop(), None).expect("a loop without a hold reads nothing"),
        None
    );
    let fake = FakeRuntime::start(|request| {
        Some(answer(
            request,
            json!({ "values": { "P1.HoldSeconds": 35.0 } }),
        ))
    });
    let connection = fake.connection(None);
    assert_eq!(
        applied_project_hold_over(&hold_loop(), Some(&connection)).expect("read"),
        Some(35.0)
    );
}
