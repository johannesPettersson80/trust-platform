//! Conservative construction/snapshot charges; physical peaks are measured at board bring-up.
use super::*;

impl EngineState<'_> {
    pub(in crate::vm::engine) fn charge_constructed_value(&mut self) -> Result<(), RuntimeError> {
        self.charge_constructed_values(1)
    }
    pub(in crate::vm::engine) fn charge_constructed_values(
        &mut self,
        units: usize,
    ) -> Result<(), RuntimeError> {
        let count = self
            .resources
            .constructed_values
            .checked_add(units)
            .ok_or_else(|| super::super::VmTrap::BudgetExceeded.into_runtime_error())?;
        if count > self.prepared.limits.max_construction_values {
            return Err(super::super::VmTrap::BudgetExceeded.into_runtime_error());
        }
        self.resources.constructed_values = count;
        Ok(())
    }

    pub(in crate::vm::engine) fn charge_allocation_bytes(
        &self,
        bytes: usize,
    ) -> Result<(), RuntimeError> {
        let total = self
            .resources
            .construction_bytes
            .get()
            .checked_add(bytes)
            .ok_or_else(|| super::super::VmTrap::BudgetExceeded.into_runtime_error())?;
        if total > self.prepared.limits.max_construction_bytes {
            return Err(super::super::VmTrap::BudgetExceeded.into_runtime_error());
        }
        self.resources.construction_bytes.set(total);
        Ok(())
    }

    pub(in crate::vm::engine) fn value_clone_charge(
        &self,
        value: &Value,
        depth: usize,
    ) -> Result<usize, RuntimeError> {
        self.charge_work_units(1)?;
        if depth >= self.prepared.limits.max_call_depth.min(128) {
            return Err(super::super::VmTrap::CallStackOverflow.into_runtime_error());
        }
        let mut bytes = core::mem::size_of::<Value>();
        match value {
            Value::String(_) | Value::Struct(_) => {} // shared/inline payload: clone does not copy its backing allocation
            Value::WString(value) => {
                bytes = bytes
                    .checked_add(value.len())
                    .ok_or(RuntimeError::Overflow)?
            }
            Value::Array(array) => {
                bytes = bytes
                    .checked_add(
                        array
                            .dimensions()
                            .len()
                            .checked_mul(core::mem::size_of::<(i64, i64)>())
                            .ok_or(RuntimeError::Overflow)?,
                    )
                    .ok_or(RuntimeError::Overflow)?;
                for value in array.elements() {
                    bytes = bytes
                        .checked_add(self.value_clone_charge(value, depth + 1)?)
                        .ok_or(RuntimeError::Overflow)?;
                }
            }
            Value::Enum(_) => {
                bytes = bytes
                    .checked_add(core::mem::size_of::<crate::value::EnumValue>())
                    .ok_or(RuntimeError::Overflow)?
            }
            Value::Reference(Some(reference)) => {
                bytes = bytes
                    .checked_add(
                        reference
                            .path
                            .len()
                            .checked_mul(core::mem::size_of::<crate::value::RefSegment>())
                            .ok_or(RuntimeError::Overflow)?,
                    )
                    .ok_or(RuntimeError::Overflow)?;
                for segment in &reference.path {
                    let extra = match segment {
                        crate::value::RefSegment::Field(_) => 0,
                        crate::value::RefSegment::Index(indices) => indices
                            .len()
                            .checked_mul(core::mem::size_of::<i64>())
                            .ok_or(RuntimeError::Overflow)?,
                    };
                    bytes = bytes.checked_add(extra).ok_or(RuntimeError::Overflow)?;
                }
            }
            _ => {}
        }
        Ok(bytes)
    }
}
