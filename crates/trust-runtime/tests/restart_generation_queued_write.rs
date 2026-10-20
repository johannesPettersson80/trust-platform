//! Spec 34 §4.3.1 and spec 11 §6.9.2: a staged write or force that names an instance by its
//! identity keeps the installation generation the identity was resolved in. When a restart or
//! an online change installed another generation, its boundary rejects it with `write_rejected`,
//! the PLC keeps running, and the instance that now has that identity is unchanged.
#![cfg(feature = "full")]
use trust_runtime::{
    debug::{DebugControl, ForcedVarTarget, RuntimeEvent},
    error::RuntimeError,
    harness::CompileSession,
    memory::InstanceId,
    value::Value,
    RestartMode, Runtime,
};

const V1: &str = "VAR_GLOBAL setpoint : DINT := DINT#0; END_VAR \
    PROGRAM Main VAR count : DINT := DINT#0; END_VAR count := count + DINT#1; END_PROGRAM";
const V2: &str = "VAR_GLOBAL setpoint : DINT := DINT#0; END_VAR \
    PROGRAM Main VAR count : DINT := DINT#0; END_VAR count := count + DINT#10; END_PROGRAM";

fn main_instance(runtime: &Runtime) -> InstanceId {
    let Some(Value::Instance(main)) = runtime.storage().get_global("Main") else {
        panic!("program instance")
    };
    *main
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
    let mut runtime = CompileSession::from_source(V1).build_runtime().unwrap();
    let debug = runtime.enable_debug();
    runtime.execute_cycle().unwrap();
    let main = main_instance(&runtime);
    let resolved_in = runtime.storage().generation();

    let changed = CompileSession::from_source(V2)
        .build_bytecode_bytes()
        .unwrap();
    runtime.apply_online_change_bytes(&changed, None).unwrap();
    assert_ne!(runtime.storage().generation(), resolved_in);
    // The replacement gives its program the same identity; only the generation tells the
    // stale request apart.
    assert_eq!(main_instance(&runtime), main);
    assert_eq!(debug.generation(), runtime.storage().generation());

    // Accepted after the commit's clearing boundary, but resolved before it.
    debug.enqueue_instance_write(resolved_in, main, "count", Value::DInt(99));
    let _ = rejections(&debug);
    runtime.execute_cycle().unwrap();

    assert!(!runtime.faulted());
    // The change kept `count` 1 (specification 11 §6.7.1), and the replacement's scan added 10.
    assert_eq!(
        runtime.storage().get_instance_var(main, "count"),
        Some(&Value::DInt(11)),
        "the replacement's own scan ran; the stale write changed nothing"
    );
    assert_eq!(
        rejections(&debug),
        vec![(
            format!("{}.count", main.0),
            RuntimeError::ReferenceLifetime.to_string()
        )]
    );
}

#[test]
fn an_instance_force_resolved_before_a_restart_is_rejected_and_removed() {
    let mut runtime = CompileSession::from_source(V1).build_runtime().unwrap();
    let debug = runtime.enable_debug();
    runtime.execute_cycle().unwrap();
    let main = main_instance(&runtime);
    let resolved_in = runtime.storage().generation();

    runtime.restart(RestartMode::Warm).unwrap();
    assert_ne!(runtime.storage().generation(), resolved_in);
    debug.force_instance(resolved_in, main, "count", Value::DInt(99));
    let target = ForcedVarTarget::Instance(resolved_in, main, "count".into());
    assert_eq!(debug.forced_var_value(&target), Some(Value::DInt(99)));
    let _ = rejections(&debug);
    runtime.execute_cycle().unwrap();

    assert!(!runtime.faulted());
    assert_eq!(
        runtime.storage().get_instance_var(main, "count"),
        Some(&Value::DInt(1))
    );
    assert_eq!(
        rejections(&debug),
        vec![(
            format!("{}.count", main.0),
            RuntimeError::ReferenceLifetime.to_string()
        )]
    );
    assert_eq!(
        debug.forced_var_value(&target),
        None,
        "the rejected force is removed, so it is not applied again"
    );
    runtime.execute_cycle().unwrap();
    assert_eq!(
        runtime.storage().get_instance_var(main, "count"),
        Some(&Value::DInt(2))
    );
    assert!(rejections(&debug).is_empty());
}

#[test]
fn requests_of_the_installed_generation_still_apply_after_a_change() {
    let mut runtime = CompileSession::from_source(V1).build_runtime().unwrap();
    let debug = runtime.enable_debug();
    runtime.execute_cycle().unwrap();
    runtime.restart(RestartMode::Cold).unwrap();
    let changed = CompileSession::from_source(V2)
        .build_bytecode_bytes()
        .unwrap();
    runtime.apply_online_change_bytes(&changed, None).unwrap();
    let main = main_instance(&runtime);
    // A bare identity, as a control client sends it, belongs to the generation installed
    // when the request is accepted.
    assert_eq!(debug.generation(), runtime.storage().generation());
    debug.enqueue_instance_write(debug.generation(), main, "count", Value::DInt(40));
    debug.enqueue_global_write("setpoint", Value::DInt(7));
    let _ = rejections(&debug);
    runtime.execute_cycle().unwrap();

    assert_eq!(
        runtime.storage().get_instance_var(main, "count"),
        Some(&Value::DInt(50))
    );
    assert_eq!(
        runtime.storage().get_global("setpoint"),
        Some(&Value::DInt(7))
    );
    assert!(rejections(&debug).is_empty());
    assert!(!runtime.faulted());
}
