use super::super::*;
use crate::error::RuntimeError;
use crate::memory::{FrameId, Generation, InstanceId};
use crate::value::Value;

fn address(text: &str) -> IoAddress {
    IoAddress::parse(text).expect("test address")
}

#[test]
fn pending_global_write_replaces_same_target_in_place() {
    let control = DebugControl::new();
    control
        .try_enqueue_global_write("a", Value::LInt(1))
        .expect("the write fits the bound");
    control
        .try_enqueue_retain_write("b", Value::LInt(2))
        .expect("the write fits the bound");
    control
        .try_enqueue_global_write("a", Value::LInt(3))
        .expect("the write fits the bound");

    let writes = control.drain_var_writes();
    assert_eq!(writes.len(), 2);
    assert!(matches!(
        &writes[0].target,
        PendingVarTarget::Global(name) if name == "a"
    ));
    assert_eq!(writes[0].value, Value::LInt(3));
    assert!(matches!(
        &writes[1].target,
        PendingVarTarget::Retain(name) if name == "b"
    ));
    assert_eq!(writes[1].value, Value::LInt(2));
}

#[test]
fn pending_write_identity_includes_target_kind() {
    let control = DebugControl::new();
    control
        .try_enqueue_global_write("same", Value::LInt(1))
        .expect("the write fits the bound");
    control
        .try_enqueue_retain_write("same", Value::LInt(2))
        .expect("the write fits the bound");
    control
        .try_enqueue_instance_write(Generation::FIRST, InstanceId(7), "same", Value::LInt(3))
        .expect("the write fits the bound");
    control
        .try_enqueue_local_write(FrameId(7), "same", Value::LInt(4))
        .expect("the write fits the bound");

    let writes = control.drain_var_writes();
    assert_eq!(writes.len(), 4);
    assert!(matches!(
        &writes[0].target,
        PendingVarTarget::Global(name) if name == "same"
    ));
    assert!(matches!(
        &writes[1].target,
        PendingVarTarget::Retain(name) if name == "same"
    ));
    assert!(matches!(
        &writes[2].target,
        PendingVarTarget::Instance(_, InstanceId(7), name) if name == "same"
    ));
    assert!(matches!(
        &writes[3].target,
        PendingVarTarget::Local(FrameId(7), name) if name == "same"
    ));
}

#[test]
fn pending_instance_write_identity_includes_instance_id() {
    let control = DebugControl::new();
    control
        .try_enqueue_instance_write(Generation::FIRST, InstanceId(1), "value", Value::LInt(1))
        .expect("the write fits the bound");
    control
        .try_enqueue_instance_write(Generation::FIRST, InstanceId(2), "value", Value::LInt(2))
        .expect("the write fits the bound");
    control
        .try_enqueue_instance_write(Generation::FIRST, InstanceId(1), "value", Value::LInt(3))
        .expect("the write fits the bound");

    let writes = control.drain_var_writes();
    assert_eq!(writes.len(), 2);
    assert!(matches!(
        &writes[0].target,
        PendingVarTarget::Instance(_, InstanceId(1), name) if name == "value"
    ));
    assert_eq!(writes[0].value, Value::LInt(3));
    assert!(matches!(
        &writes[1].target,
        PendingVarTarget::Instance(_, InstanceId(2), name) if name == "value"
    ));
}

#[test]
fn pending_local_write_identity_includes_frame_id() {
    let control = DebugControl::new();
    control
        .try_enqueue_local_write(FrameId(1), "value", Value::LInt(1))
        .expect("the write fits the bound");
    control
        .try_enqueue_local_write(FrameId(2), "value", Value::LInt(2))
        .expect("the write fits the bound");
    control
        .try_enqueue_local_write(FrameId(1), "value", Value::LInt(3))
        .expect("the write fits the bound");

    let writes = control.drain_var_writes();
    assert_eq!(writes.len(), 2);
    assert!(matches!(
        &writes[0].target,
        PendingVarTarget::Local(FrameId(1), name) if name == "value"
    ));
    assert_eq!(writes[0].value, Value::LInt(3));
    assert!(matches!(
        &writes[1].target,
        PendingVarTarget::Local(FrameId(2), name) if name == "value"
    ));
}

#[test]
fn variable_forces_preserve_distinct_target_kinds_and_order() {
    let control = DebugControl::new();
    control
        .try_force_global("same", Value::LInt(1))
        .expect("the force fits the bound");
    control
        .try_force_retain("same", Value::LInt(2))
        .expect("the force fits the bound");
    control
        .try_force_instance(Generation::FIRST, InstanceId(7), "same", Value::LInt(3))
        .expect("the force fits the bound");

    let forced = control.forced_snapshot().vars;
    assert_eq!(forced.len(), 3);
    assert!(matches!(
        &forced[0].target,
        ForcedVarTarget::Global(name) if name == "same"
    ));
    assert!(matches!(
        &forced[1].target,
        ForcedVarTarget::Retain(name) if name == "same"
    ));
    assert!(matches!(
        &forced[2].target,
        ForcedVarTarget::Instance(_, InstanceId(7), name) if name == "same"
    ));
}

#[test]
fn variable_force_replacement_keeps_original_position() {
    let control = DebugControl::new();
    control
        .try_force_global("a", Value::LInt(1))
        .expect("the force fits the bound");
    control
        .try_force_retain("b", Value::LInt(2))
        .expect("the force fits the bound");
    control
        .try_force_global("a", Value::LInt(3))
        .expect("the force fits the bound");

    let forced = control.forced_snapshot().vars;
    assert_eq!(forced.len(), 2);
    assert_eq!(forced[0].value, Value::LInt(3));
    assert_eq!(forced[1].value, Value::LInt(2));
}

#[test]
fn instance_force_identity_includes_instance_id() {
    let control = DebugControl::new();
    control
        .try_force_instance(Generation::FIRST, InstanceId(1), "value", Value::LInt(1))
        .expect("the force fits the bound");
    control
        .try_force_instance(Generation::FIRST, InstanceId(2), "value", Value::LInt(2))
        .expect("the force fits the bound");
    control
        .try_force_instance(Generation::FIRST, InstanceId(1), "value", Value::LInt(3))
        .expect("the force fits the bound");

    let forced = control.forced_snapshot().vars;
    assert_eq!(forced.len(), 2);
    assert!(matches!(
        &forced[0].target,
        ForcedVarTarget::Instance(_, InstanceId(1), name) if name == "value"
    ));
    assert_eq!(forced[0].value, Value::LInt(3));
    assert!(matches!(
        &forced[1].target,
        ForcedVarTarget::Instance(_, InstanceId(2), name) if name == "value"
    ));
}

#[test]
fn releases_are_exact_across_variable_target_kinds() {
    let control = DebugControl::new();
    control
        .try_force_global("same", Value::LInt(1))
        .expect("the force fits the bound");
    control
        .try_force_retain("same", Value::LInt(2))
        .expect("the force fits the bound");
    control
        .try_force_instance(Generation::FIRST, InstanceId(7), "same", Value::LInt(3))
        .expect("the force fits the bound");
    control
        .try_force_instance(Generation::FIRST, InstanceId(8), "same", Value::LInt(4))
        .expect("the force fits the bound");

    control.release_global("same");
    control.release_instance(InstanceId(7), "same");
    let forced = control.forced_snapshot().vars;
    assert_eq!(forced.len(), 2);
    assert!(matches!(
        &forced[0].target,
        ForcedVarTarget::Retain(name) if name == "same"
    ));
    assert!(matches!(
        &forced[1].target,
        ForcedVarTarget::Instance(_, InstanceId(8), name) if name == "same"
    ));
}

#[test]
fn variable_release_is_idempotent_for_absent_target() {
    let control = DebugControl::new();
    control
        .try_force_global("a", Value::LInt(1))
        .expect("the force fits the bound");

    control.release_global("missing");
    control.release_retain("missing");
    control.release_instance(InstanceId(1), "missing");
    let forced = control.forced_snapshot().vars;
    assert_eq!(forced.len(), 1);
    assert_eq!(forced[0].value, Value::LInt(1));
}

#[test]
fn io_force_replacement_keeps_original_position() {
    let control = DebugControl::new();
    control
        .try_force_io(address("%QW0"), Value::LInt(1))
        .expect("the force fits the bound");
    control
        .try_force_io(address("%QW2"), Value::LInt(2))
        .expect("the force fits the bound");
    control
        .try_force_io(address("%QW0"), Value::LInt(3))
        .expect("the force fits the bound");

    let forced = control.forced_snapshot().io;
    assert_eq!(forced.len(), 2);
    assert_eq!(forced[0], (address("%QW0"), Value::LInt(3)));
    assert_eq!(forced[1], (address("%QW2"), Value::LInt(2)));
}

#[test]
fn io_release_is_exact_and_idempotent() {
    let control = DebugControl::new();
    control
        .try_force_io(address("%QW0"), Value::LInt(1))
        .expect("the force fits the bound");
    control
        .try_force_io(address("%QW2"), Value::LInt(2))
        .expect("the force fits the bound");

    control.release_io(&address("%QW0"));
    control.release_io(&address("%QW0"));
    control.release_io(&address("%QW4"));
    assert_eq!(
        control.forced_snapshot().io,
        vec![(address("%QW2"), Value::LInt(2))]
    );
}

#[test]
fn forced_snapshot_is_an_owned_copy() {
    let control = DebugControl::new();
    control
        .try_force_global("a", Value::LInt(1))
        .expect("the force fits the bound");
    control
        .try_force_io(address("%QW0"), Value::LInt(2))
        .expect("the force fits the bound");

    let mut first = control.forced_snapshot();
    first.vars[0].value = Value::LInt(99);
    first.io.clear();

    let second = control.forced_snapshot();
    assert_eq!(second.vars[0].value, Value::LInt(1));
    assert_eq!(second.io, vec![(address("%QW0"), Value::LInt(2))]);
}

#[test]
fn runtime_mutation_clear_removes_every_queue_and_force() {
    let control = DebugControl::new();
    control
        .try_enqueue_io_write(address("%QW0"), Value::LInt(1))
        .expect("the write fits the bound");
    control
        .try_enqueue_global_write("global", Value::LInt(2))
        .expect("the write fits the bound");
    control
        .try_enqueue_retain_write("retain", Value::LInt(3))
        .expect("the write fits the bound");
    control
        .try_enqueue_instance_write(Generation::FIRST, InstanceId(1), "instance", Value::LInt(4))
        .expect("the write fits the bound");
    control
        .try_enqueue_local_write(FrameId(1), "local", Value::LInt(5))
        .expect("the write fits the bound");
    control
        .try_force_global("global", Value::LInt(7))
        .expect("the force fits the bound");
    control
        .try_force_io(address("%QW2"), Value::LInt(8))
        .expect("the force fits the bound");

    control.clear_runtime_mutations();

    assert!(control.drain_var_writes().is_empty());
    let forced = control.forced_snapshot();
    assert!(forced.vars.is_empty());
    assert!(forced.io.is_empty());
}

#[test]
fn clearing_runtime_mutations_preserves_unrelated_debug_state() {
    let control = DebugControl::new();
    control.compiled_debugger().pause();
    control.push_runtime_event(RuntimeEvent::fault(
        &RuntimeError::SimulationFault("fault".into()),
        crate::value::Duration::from_millis(1),
    ));
    control
        .try_enqueue_global_write("a", Value::LInt(1))
        .expect("the write fits the bound");
    control
        .try_force_global("a", Value::LInt(2))
        .expect("the force fits the bound");

    control.clear_runtime_mutations();

    assert_eq!(
        control.hold_label(),
        Some("pause"),
        "the debugger's hold stays"
    );
    assert_eq!(control.drain_runtime_events().len(), 1);
}
