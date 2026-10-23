use alloc::vec::Vec;

use smol_str::SmolStr;

use super::{unpack_native_call_payload, VmNativeArgValue};
use crate::memory::{InstanceId, MemoryLocation};
use crate::value::{Value, ValueRef};
use crate::vm::module::VmNativeArgSpec;
use crate::vm::stack::OperandStack;
use crate::vm::VmTrap;

fn spec(name: &str, is_target: bool) -> VmNativeArgSpec {
    VmNativeArgSpec {
        name: Some(SmolStr::new(name)),
        is_target,
    }
}

fn reference(offset: usize) -> ValueRef {
    ValueRef {
        location: MemoryLocation::Global,
        offset,
        path: Vec::new(),
    }
}

fn expr(arg: &super::VmNativeArg) -> &Value {
    let VmNativeArgValue::Expr(value) = &arg.value else {
        panic!("expected expression payload");
    };
    value
}

#[test]
fn drains_receiver_and_arguments_in_spec_order_without_growing_stack_storage() {
    let mut stack = OperandStack::default();
    stack.push(Value::Instance(InstanceId(4))).unwrap();
    stack.push(Value::DInt(17)).unwrap();
    stack.push(Value::Reference(Some(reference(9)))).unwrap();
    let capacity = stack.storage_capacity();

    let (receiver, args) =
        unpack_native_call_payload(&mut stack, &[spec("INPUT", false), spec("OUTPUT", true)], 1)
            .unwrap();

    assert_eq!(receiver, Some(Value::Instance(InstanceId(4))));
    assert_eq!(args.len(), 2);
    assert_eq!(args[0].name.as_deref(), Some("INPUT"));
    assert_eq!(expr(&args[0]), &Value::DInt(17));
    assert_eq!(args[1].name.as_deref(), Some("OUTPUT"));
    assert!(matches!(
        &args[1].value,
        VmNativeArgValue::Target(target) if target == &reference(9)
    ));
    assert_eq!(stack.storage_capacity(), capacity);
    assert!(matches!(stack.pop(), Err(VmTrap::StackUnderflow)));
}

#[test]
fn receiver_counts_above_one_keep_first_arguments_and_discard_surplus_payloads() {
    let mut stack = OperandStack::default();
    for value in [1, 2, 3, 4] {
        stack.push(Value::DInt(value)).unwrap();
    }

    let (receiver, args) = unpack_native_call_payload(
        &mut stack,
        &[spec("FIRST", false), spec("SECOND", false)],
        2,
    )
    .unwrap();

    assert_eq!(receiver, None);
    assert_eq!(expr(&args[0]), &Value::DInt(1));
    assert_eq!(expr(&args[1]), &Value::DInt(2));
    assert!(matches!(stack.pop(), Err(VmTrap::StackUnderflow)));
}

#[test]
fn underflow_consumes_only_available_active_frame_values() {
    let mut stack = OperandStack::default();
    stack.push(Value::DInt(99)).unwrap();
    let (base, previous) = stack.enter_call();
    stack.push(Value::DInt(1)).unwrap();
    stack.push(Value::DInt(2)).unwrap();

    let error = unpack_native_call_payload(
        &mut stack,
        &[spec("A", false), spec("B", false), spec("C", false)],
        0,
    )
    .unwrap_err();

    assert!(matches!(error, VmTrap::StackUnderflow));
    assert!(matches!(stack.pop(), Err(VmTrap::StackUnderflow)));
    stack.leave_call(base, previous);
    assert_eq!(stack.pop().unwrap(), Value::DInt(99));
}

#[test]
fn malformed_target_reports_first_bad_spec_and_drains_the_full_payload() {
    let mut stack = OperandStack::default();
    stack.push(Value::DInt(99)).unwrap();
    let (base, previous) = stack.enter_call();
    stack.push(Value::Instance(InstanceId(7))).unwrap();
    stack.push(Value::DInt(10)).unwrap();
    stack.push(Value::DInt(20)).unwrap();

    let error =
        unpack_native_call_payload(&mut stack, &[spec("FIRST", true), spec("SECOND", true)], 1)
            .unwrap_err();

    assert!(matches!(
        error,
        VmTrap::InvalidNativeCall(message)
            if message.as_str() == "target argument 'FIRST' requires reference payload"
    ));
    assert!(matches!(stack.pop(), Err(VmTrap::StackUnderflow)));
    stack.leave_call(base, previous);
    assert_eq!(stack.pop().unwrap(), Value::DInt(99));
}
