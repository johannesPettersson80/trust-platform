use super::*;
use crate::memory::ExecutionFrameOwner;
use alloc::{vec, vec::Vec};

#[test]
fn statement_call_stack_projects_owned_suspended_ancestors_in_activation_order() {
    let mut storage = VariableStorage::new();
    let reserved_only = storage.reserve_execution_frame().unwrap();

    let parent = storage.reserve_execution_frame().unwrap();
    storage.activate_execution_frame(parent).unwrap();
    storage
        .set_execution_frame_owner(parent, 10, Some(crate::memory::InstanceId(4)))
        .unwrap();
    let mut parent_locals = vec![crate::value::Value::Int(17)];
    storage
        .suspend_execution_frame(parent, &mut parent_locals)
        .unwrap();

    let initializer = storage.reserve_execution_frame().unwrap();
    storage.activate_execution_frame(initializer).unwrap();
    let mut initializer_locals = vec![crate::value::Value::Int(23)];
    storage
        .suspend_execution_frame(initializer, &mut initializer_locals)
        .unwrap();

    let child = storage.reserve_execution_frame().unwrap();
    storage.activate_execution_frame(child).unwrap();
    storage
        .set_execution_frame_owner(child, 11, Some(crate::memory::InstanceId(7)))
        .unwrap();

    let frame = super::super::VmFrame {
        activation: Some(child),
        pou_id: Some(11),
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: 0,
        locals: vec![crate::value::Value::Int(31)],
        parameter_values_present: Vec::new(),
        runtime_instance: Some(crate::memory::InstanceId(7)),
        instance_owner: Some(11),
    };
    let observer = StatementFrame::from_frame(&frame).with_call_stack(&storage);
    let projected = observer
        .call_stack()
        .map(|entry| {
            (
                entry.activation(),
                entry.pou_id(),
                entry.runtime_instance(),
                entry.locals().first().cloned(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        projected,
        vec![
            (
                Some(parent),
                Some(10),
                Some(crate::memory::InstanceId(4)),
                Some(crate::value::Value::Int(17)),
            ),
            (
                Some(child),
                Some(11),
                Some(crate::memory::InstanceId(7)),
                Some(crate::value::Value::Int(31)),
            ),
        ]
    );
    assert!(storage.execution_frame_is_live(reserved_only));
}

#[test]
fn statement_call_stack_excludes_ownerless_and_unsuspended_entries() {
    let mut storage = VariableStorage::new();
    let initializer = storage.reserve_execution_frame().unwrap();
    storage.activate_execution_frame(initializer).unwrap();
    let mut initializer_locals = vec![crate::value::Value::Int(23)];
    storage
        .suspend_execution_frame(initializer, &mut initializer_locals)
        .unwrap();
    assert_eq!(
        storage.execution_frame_owner(initializer),
        None::<ExecutionFrameOwner>
    );

    let child = storage.reserve_execution_frame().unwrap();
    storage.activate_execution_frame(child).unwrap();
    storage.set_execution_frame_owner(child, 11, None).unwrap();
    let frame = super::super::VmFrame {
        activation: Some(child),
        pou_id: Some(11),
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: 0,
        locals: vec![crate::value::Value::Int(31)],
        parameter_values_present: Vec::new(),
        runtime_instance: None,
        instance_owner: Some(11),
    };
    let observer = StatementFrame::from_frame(&frame).with_call_stack(&storage);
    let projected = observer.call_stack().collect::<Vec<_>>();
    assert_eq!(projected.len(), 1);
    assert_eq!(projected[0].activation(), Some(child));
}
