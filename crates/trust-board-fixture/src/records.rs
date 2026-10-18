//! Bounded, allocation-free `B1,` trace records and scalar runtime observations.
//! The record grammar and field order are the M2E oracle; every board emits them.
use crate::{
    board::{DigitalIo, Memory},
    stack,
};
use core::fmt::Write;
use trust_runtime_core::{memory::InstanceId, value::Value, vm::RuntimeState};

/// Report a failure through one cold path; callers retain their safe-off and stop order.
#[cold]
#[inline(never)]
pub fn failure<K: Write>(console: &mut K, phase: &str, code: &str) {
    let _ = writeln!(console, "B1,FAIL,{phase},{code}\r");
}

/// A declared scalar member of `owner`, read by value. A member that is not a scalar fails the
/// fixture; it is never read as a default.
pub fn field(
    state: &RuntimeState<'_>,
    owner: InstanceId,
    name: &str,
) -> Result<Value, &'static str> {
    match state.read_instance_scalar(owner, name) {
        Ok(Some(value)) => Ok(value),
        Ok(None) => Err("missing-field"),
        Err(_) => Err("field-not-scalar"),
    }
}

/// The instance a resource global holds, such as the program instance `Plant`.
pub fn global_instance(state: &RuntimeState<'_>, name: &str) -> Result<InstanceId, &'static str> {
    match state.read_global_instance(name) {
        Ok(Some(id)) => Ok(id),
        Ok(None) | Err(_) => Err("missing-instance"),
    }
}

/// The instance a declared member of `owner` holds.
pub fn member_instance(
    state: &RuntimeState<'_>,
    owner: InstanceId,
    name: &str,
) -> Result<InstanceId, &'static str> {
    match state.read_instance_member_instance(owner, name) {
        Ok(Some(id)) => Ok(id),
        Ok(None) => Err("missing-field"),
        Err(_) => Err("missing-instance"),
    }
}

pub fn integer(value: &Value) -> Result<i64, &'static str> {
    match value {
        Value::Int(value) => Ok(i64::from(*value)),
        Value::DInt(value) => Ok(i64::from(*value)),
        Value::Time(value) => Ok(value.as_nanos()),
        _ => Err("field-integer-type"),
    }
}

pub fn boolean(value: &Value) -> Result<u8, &'static str> {
    match value {
        Value::Bool(value) => Ok(u8::from(*value)),
        _ => Err("field-boolean-type"),
    }
}

pub fn memory<K: Write, M: Memory>(
    console: &mut K,
    memory: &M,
    phase: &str,
) -> Result<(), &'static str> {
    let value = memory.measure();
    let stack_peak = memory.boot_stack_peak();
    writeln!(
        console,
        "B1,MEM,{phase},{},{},{},{},{},{},{},{}\r",
        value.used,
        value.peak,
        value.allocations,
        value.failures,
        stack_peak,
        stack::STACK_BYTES.saturating_sub(stack_peak),
        core::mem::size_of::<Value>(),
        core::mem::align_of::<Value>()
    )
    .map_err(|_| "uart")?;
    stack::check(stack_peak, stack::STACK_BYTES.saturating_sub(stack_peak))
}

pub fn io<K: Write, I: DigitalIo>(
    console: &mut K,
    io: &I,
    phase: &str,
    command: bool,
) -> Result<(), &'static str> {
    let input = io.sample();
    writeln!(
        console,
        "B1,IO,{phase},{},{},{},{}\r",
        u8::from(input.pin_high),
        u8::from(input.pressed),
        u8::from(command),
        u8::from(io.output_readback())
    )
    .map_err(|_| "uart")
}
