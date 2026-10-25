//! Spec 34 §4.3.1 and spec 11 §6.9.2: a staged write or force that names an instance by its
//! identity keeps the installation generation the identity was resolved in. When a restart or
//! an online change installed another generation, its boundary rejects it with `write_rejected`,
//! the PLC keeps running, and the instance that now has that identity is unchanged. A compiled
//! program's instance identity is the index of the instance's symbol (specification 44 §13.3).
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.
#![cfg(feature = "full")]
use trust_runtime::{
    debug::{DebugControl, ForcedVarTarget, RuntimeEvent},
    error::RuntimeError,
    harness::TestHarness,
    memory::InstanceId,
    value::Value,
    RestartMode,
};

const V1: &str = "FUNCTION_BLOCK Counter VAR_INPUT increment : DINT := DINT#1; END_VAR \
    VAR count : DINT := DINT#0; END_VAR count := count + increment; END_FUNCTION_BLOCK \
    VAR_GLOBAL setpoint : DINT := DINT#0; END_VAR \
    PROGRAM Main VAR counter : Counter; END_VAR counter(increment := DINT#1); END_PROGRAM";
const V2: &str = "FUNCTION_BLOCK Counter VAR_INPUT increment : DINT := DINT#1; END_VAR \
    VAR count : DINT := DINT#0; END_VAR count := count + increment; END_FUNCTION_BLOCK \
    VAR_GLOBAL setpoint : DINT := DINT#0; END_VAR \
    PROGRAM Main VAR counter : Counter; END_VAR counter(increment := DINT#10); END_PROGRAM";

/// The harness of `source` with a debug control attached.
fn debugged(source: &str) -> (TestHarness, DebugControl) {
    let mut harness = TestHarness::from_source(source).unwrap();
    let debug = DebugControl::new();
    harness.runtime_mut().set_debug_control(debug.clone());
    (harness, debug)
}

/// The identity of the program's `counter` instance: its symbol's index.
fn counter_instance(harness: &TestHarness) -> InstanceId {
    InstanceId(
        harness
            .runtime()
            .find_variable("Main.counter")
            .expect("the counter instance"),
    )
}

fn count(harness: &TestHarness) -> Result<Value, RuntimeError> {
    harness.runtime().read_variable("Main.counter.count")
}

/// The `write_rejected` events since the previous call, as (target, error) pairs.
fn rejections(debug: &DebugControl) -> Vec<(String, String)> {
    debug
        .drain_runtime_events()
        .into_iter()
        .filter_map(|event| match event {
            RuntimeEvent::WriteRejected { target, error, .. } => Some((target.to_string(), error)),
            _ => None,
        })
        .collect()
}

#[test]
fn an_instance_write_resolved_before_an_online_change_is_rejected_at_its_boundary() {
    let (mut harness, debug) = debugged(V1);
    harness.runtime_mut().execute_cycle().unwrap();
    let counter = counter_instance(&harness);
    let resolved_in = debug.generation();

    harness.reload_source(V2).unwrap();
    assert_ne!(debug.generation(), resolved_in);
    // The replacement gives the instance the same identity; only the generation tells the
    // stale request apart.
    assert_eq!(counter_instance(&harness), counter);

    // Accepted after the commit's clearing boundary, but resolved before it.
    debug
        .try_enqueue_instance_write(resolved_in, counter, "count", Value::DInt(99))
        .unwrap();
    let _ = rejections(&debug);
    harness.runtime_mut().execute_cycle().unwrap();

    assert!(!harness.runtime().faulted());
    // The change kept `count` 1 (specification 11 §6.7.1), and the replacement's scan added 10.
    assert_eq!(
        count(&harness),
        Ok(Value::DInt(11)),
        "the replacement's own scan ran; the stale write changed nothing"
    );
    assert_eq!(
        rejections(&debug),
        vec![(
            format!("{}.count", counter.0),
            RuntimeError::ReferenceLifetime.to_string()
        )]
    );
}

#[test]
fn an_instance_force_resolved_before_a_restart_is_rejected_and_removed() {
    let (mut harness, debug) = debugged(V1);
    harness.runtime_mut().execute_cycle().unwrap();
    let counter = counter_instance(&harness);
    let resolved_in = debug.generation();

    harness.restart(RestartMode::Warm).unwrap();
    assert_ne!(debug.generation(), resolved_in);
    debug
        .try_force_instance(resolved_in, counter, "count", Value::DInt(99))
        .unwrap();
    let target = ForcedVarTarget::Instance(resolved_in, counter, "count".into());
    assert_eq!(debug.forced_var_value(&target), Some(Value::DInt(99)));
    let _ = rejections(&debug);
    harness.runtime_mut().execute_cycle().unwrap();

    assert!(!harness.runtime().faulted());
    assert_eq!(count(&harness), Ok(Value::DInt(1)));
    assert_eq!(
        rejections(&debug),
        vec![(
            format!("{}.count", counter.0),
            RuntimeError::ReferenceLifetime.to_string()
        )]
    );
    assert_eq!(
        debug.forced_var_value(&target),
        None,
        "the rejected force is removed, so it is not applied again"
    );
    harness.runtime_mut().execute_cycle().unwrap();
    assert_eq!(count(&harness), Ok(Value::DInt(2)));
    assert!(rejections(&debug).is_empty());
}

#[test]
fn requests_of_the_installed_generation_still_apply_after_a_change() {
    let (mut harness, debug) = debugged(V1);
    harness.runtime_mut().execute_cycle().unwrap();
    harness.restart(RestartMode::Cold).unwrap();
    harness.reload_source(V2).unwrap();
    let counter = counter_instance(&harness);
    // A bare identity, as a control client sends it, belongs to the generation installed
    // when the request is accepted.
    debug
        .try_enqueue_instance_write(debug.generation(), counter, "count", Value::DInt(40))
        .unwrap();
    debug
        .try_enqueue_global_write("setpoint", Value::DInt(7))
        .unwrap();
    let _ = rejections(&debug);
    harness.runtime_mut().execute_cycle().unwrap();

    assert_eq!(count(&harness), Ok(Value::DInt(50)));
    assert_eq!(
        harness.runtime().read_variable("setpoint"),
        Ok(Value::DInt(7))
    );
    assert!(rejections(&debug).is_empty());
    assert!(!harness.runtime().faulted());
}
