//! Bind typed value operations to the active action context and shared limits.
use crate::bytecode::TypeTable;
use crate::error::RuntimeError;
use crate::value::{DateTimeProfile, Value};
use crate::vm::construction::values::{ValueConstructionContext, ValueOperation};
use crate::vm::{engine::EngineState, module::invalid_bytecode, VmTrap};
use core::mem::size_of;
use smol_str::SmolStr;

impl ValueConstructionContext for EngineState<'_> {
    fn types(&self) -> &TypeTable {
        &self.prepared.vm.types
    }
    fn strings(&self) -> &[SmolStr] {
        &self.prepared.vm.strings
    }
    fn profile(&self) -> DateTimeProfile {
        self.profile
    }

    fn type_recipe(&self, type_id: u32) -> Option<u32> {
        let context = self.construction.initializers.last()?.context;
        self.prepared.recipe(context, type_id, None)
    }

    fn member_recipe(&self, type_id: u32, member: u32) -> Option<u32> {
        let context = self.construction.initializers.last()?.context;
        self.prepared.recipe(context, type_id, Some(member))
    }

    fn evaluate_recipe(&mut self, recipe_id: u32) -> Result<Value, RuntimeError> {
        let active = self.construction.initializers.last().ok_or_else(|| {
            invalid_bytecode(smol_str::SmolStr::new_static(
                "default recipe has no active action context",
            ))
        })?;
        let instance = active.instance;
        let depth = active
            .depth
            .checked_add(1)
            .ok_or_else(|| VmTrap::CallStackOverflow.into_runtime_error())?;
        // The evaluator receives the same lexical frame by ownership transfer.
        // Never clone locals or retain a borrow of the context across dispatch.
        let mut lexical = self.construction.frames.pop();
        let result = self.evaluate_initializer(recipe_id, lexical.as_mut(), instance, depth);
        if let Some(frame) = lexical {
            self.construction.frames.push(frame);
        }
        result
    }

    fn construct_instance(&mut self, pou_id: u32, intrinsic: bool) -> Result<Value, RuntimeError> {
        self.construct_typed_instance(pou_id, intrinsic)
            .map(Value::Instance)
    }

    fn charge_value(&mut self) -> Result<(), RuntimeError> {
        self.charge_constructed_value()
    }

    fn charge_work(&mut self, units: usize) -> Result<(), RuntimeError> {
        self.charge_work_units(units)
    }

    fn charge_allocation(&mut self, bytes: usize) -> Result<(), RuntimeError> {
        self.charge_allocation_bytes(bytes)
    }

    fn check_value(&mut self, type_id: u32, value: &Value) -> Result<(), RuntimeError> {
        self.check_typed_value(type_id, value)?;
        // A freshly constructed instance may remain owned by the current staged
        // result. The later commit boundary checks/reparents its final lifetime.
        let destination = self
            .construction
            .initializers
            .last()
            .map(|active| active.result)
            .or_else(|| {
                self.construction
                    .frames
                    .last()
                    .and_then(|frame| frame.activation)
            })
            .or_else(|| self.lifetimes.live_activations.last().copied());
        self.check_value_lifetime(value, destination)
    }

    // Entry policy and reservation finish before recursive type construction;
    // their temporary error/allocation state does not belong to its live frame.
    #[inline(never)]
    fn enter_type(&mut self, type_id: u32, operation: ValueOperation) -> Result<(), RuntimeError> {
        self.charge_work_units(1)?;
        let callback_depth = self
            .construction
            .initializers
            .last()
            .map_or(self.lifetimes.live_activations.len(), |active| {
                active.depth as usize
            });
        let depth = callback_depth
            .checked_add(self.construction.types.len())
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| VmTrap::CallStackOverflow.into_runtime_error())?;
        if depth > self.prepared.limits.max_call_depth {
            return Err(VmTrap::CallStackOverflow.into_runtime_error());
        }
        self.charge_work_units(self.construction.types.len())?;
        if self.construction.types.contains(&(type_id, operation)) {
            return Err(invalid_bytecode(smol_str::SmolStr::new_static(
                "cyclic typed default construction",
            )));
        }
        self.charge_allocation(size_of::<(u32, ValueOperation)>())?;
        self.construction
            .types
            .try_reserve_exact(1)
            .map_err(|_| VmTrap::BudgetExceeded.into_runtime_error())?;
        self.construction.types.push((type_id, operation));
        Ok(())
    }

    fn leave_type(&mut self) {
        self.construction.types.pop();
    }
}
