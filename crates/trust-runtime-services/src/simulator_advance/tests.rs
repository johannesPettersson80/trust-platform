//! `simulator.advance` (specification 11 §6.8.1, "Simulator for machine tests"): its parameters,
//! and the refusal of every runtime on the wall clock (specification 34 STATE-08).

use alloc::string::ToString;

use serde_json::{json, Value};
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value::Duration;

use super::{parse, runs_cycles, Advance};
use crate::tests::runner_ports::Fixture;

fn advance(id: u64, params: &Value) -> Value {
    json!({"id": id, "type": "simulator.advance", "params": params})
}

#[test]
fn an_advance_takes_a_count_of_cycles_or_a_duration() {
    // The request names how far the time moves: `cycles`, a whole number of at least one, or
    // `duration_ns`, a whole number of nanoseconds, never both. Catches a request read with both
    // members, with neither, or with a zero count, which would run nothing and look answered.
    assert_eq!(parse(Some(&json!({"cycles": 3}))), Ok(Advance::Cycles(3)));
    assert_eq!(
        parse(Some(&json!({"duration_ns": 25_000_000}))),
        Ok(Advance::Duration(Duration::from_millis(25)))
    );
    assert_eq!(
        parse(Some(&json!({"cycles": 3, "duration_ns": 1}))),
        Err("give exactly one of cycles and duration_ns".to_string())
    );
    assert_eq!(
        parse(Some(&json!({}))),
        Err("give exactly one of cycles and duration_ns".to_string())
    );
    assert_eq!(
        parse(Some(&json!({"cycles": 0}))),
        Err("cycles must be a whole number of at least 1".to_string())
    );
    assert_eq!(
        parse(Some(&json!({"duration_ns": -1}))),
        Err("duration_ns must be a whole number of nanoseconds".to_string())
    );
    assert!(parse(Some(&json!({"cycles": 1, "time_ns": 0}))).is_err());
    assert!(parse(None).is_err());
}

#[test]
fn a_runtime_on_the_wall_clock_refuses_an_advance() {
    // Every runtime answers `simulator.advance` alike (specification 34 STATE-08): one whose time
    // is the wall clock, a chip's here, refuses it as `clock_not_manual`, and its resource runs on.
    // Catches a platform that does not know the request, which would refuse it as unknown instead.
    let fixture = Fixture::board_probe();
    let mut runner = fixture.runner();
    fixture.pass_at(&mut runner, 0);
    let reply = fixture.call(&mut runner, &advance(2, &json!({"cycles": 1})));
    assert_eq!(reply["ok"], json!(false));
    assert_eq!(reply["error_code"], json!("clock_not_manual"));
    assert_eq!(runner.resource_state(), ResourceState::Running);
}

#[test]
fn an_advance_runs_on_a_resource_that_is_running_or_still_starting() {
    // A simulator's first request may come before its scan thread has left `Boot`; its loop then
    // runs the first cycle once it is up, so the advance waits for it (specification 11 §6.8.1,
    // "Simulator for machine tests"). A paused, stopped or faulted resource runs no cycle. Catches
    // the first advance of a fresh simulator that ran no cycle, as it did in CI on 23 November 2026.
    assert!(runs_cycles(ResourceState::Running));
    assert!(runs_cycles(ResourceState::Boot));
    for state in [
        ResourceState::Ready,
        ResourceState::Paused,
        ResourceState::Faulted,
        ResourceState::Stopped,
    ] {
        assert!(!runs_cycles(state), "{state:?}");
    }
}
