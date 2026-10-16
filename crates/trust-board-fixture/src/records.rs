//! Bounded, allocation-free `B1,` trace records and borrowed runtime observations.
//! The record grammar and field order are the M2E oracle; every board emits them.
use crate::{
    board::{DigitalIo, Memory},
    stack,
};
use core::fmt::Write;
use trust_runtime_core::{
    memory::{InstanceId, VariableStorage},
    value::Value,
};

/// Report a failure through one cold path; callers retain their safe-off and stop order.
#[cold]
#[inline(never)]
pub fn failure<K: Write>(console: &mut K, phase: &str, code: &str) {
    let _ = writeln!(console, "B1,FAIL,{phase},{code}\r");
}

pub fn field<'a>(
    storage: &'a VariableStorage,
    owner: InstanceId,
    name: &str,
) -> Result<&'a Value, &'static str> {
    storage.get_instance_var(owner, name).ok_or("missing-field")
}

pub fn instance(value: Option<&Value>) -> Result<InstanceId, &'static str> {
    match value {
        Some(Value::Instance(id)) => Ok(*id),
        _ => Err("missing-instance"),
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
