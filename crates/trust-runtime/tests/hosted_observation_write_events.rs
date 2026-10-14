//! Rejected management writes are reported with their cycle time and never fault the PLC.
use trust_runtime::{
    debug::RuntimeEvent,
    error::RuntimeError,
    value::{Duration, Value},
    Runtime,
};

const GPIO: &[u8] = include_bytes!("fixtures/portability/f401/gpio.stbc");

/// Spec 34 HOST-09 (lines 503-506): a rejected queued write reports its destination,
/// error and the cycle time at which it was attempted.
#[test]
fn hosted_observation_rejected_queued_write_reports_the_cycle_time() {
    let mut runtime = Runtime::from_bytes(GPIO, Default::default()).unwrap();
    let debug = runtime.enable_debug();
    debug.enqueue_global_write("missing", Value::DInt(7));
    runtime.set_current_time(Duration::from_millis(35));

    runtime
        .execute_cycle()
        .expect("an invalid engineering request is not a PLC fault");
    assert!(!runtime.faulted());
    let rejected: Vec<bool> = debug
        .drain_runtime_events()
        .iter()
        .filter_map(|event| match event {
            RuntimeEvent::WriteRejected {
                target,
                error,
                time,
            } => Some(
                target.as_str() == "missing"
                    && error == &RuntimeError::NullReference.to_string()
                    && *time == Duration::from_millis(35),
            ),
            _ => None,
        })
        .collect();
    assert_eq!(
        rejected,
        vec![true],
        "exactly one rejection, stamped with 35 ms"
    );
}

/// Spec 34 HOST-09 (lines 503-506): a rejected forced write reports its destination,
/// error and cycle time, and the force does not fault the PLC.
#[test]
fn hosted_observation_rejected_forced_write_reports_the_cycle_time() {
    let mut runtime = Runtime::from_bytes(GPIO, Default::default()).unwrap();
    let debug = runtime.enable_debug();
    debug.force_global("missing", Value::DInt(7));
    runtime.set_current_time(Duration::from_millis(10));

    runtime
        .execute_cycle()
        .expect("a rejected force is not a PLC fault");
    assert!(!runtime.faulted());
    assert!(runtime.storage().get_global("missing").is_none());
    let events = debug.drain_runtime_events();
    assert!(
        events.iter().any(|event| matches!(event,
            RuntimeEvent::WriteRejected { target, error, time }
                if target.as_str() == "missing"
                    && error == &RuntimeError::NullReference.to_string()
                    && *time == Duration::from_millis(10))),
        "expected a forced-write rejection stamped with 10 ms, got {events:?}"
    );
}
