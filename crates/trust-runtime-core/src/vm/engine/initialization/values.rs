//! Bind typed value operations to the active action context and shared limits.
use crate::error::RuntimeError;
use crate::memory::layout::ByteRange;
use crate::value::{DateTimeProfile, Value};
use crate::vm::construction::values::{
    NodeTarget, Target, ValueConstructionContext, ValueOperation,
};
use crate::vm::{
    engine::{EngineState, EngineStore},
    module::invalid_bytecode,
    VmTrap,
};
use core::mem::size_of;

// Three operation bits per admitted type; two types share a byte. The ordered
// stack remains the authority for depth and cleanup, while this mask avoids
// rescanning every enclosing aggregate for each scalar child.
fn operation_bit(type_id: u32, operation: ValueOperation) -> (usize, u8) {
    let bit = match operation {
        ValueOperation::Default => 1,
        ValueOperation::Coerce => 2,
        ValueOperation::Apply => 4,
    };
    ((type_id / 2) as usize, bit << ((type_id & 1) * 4))
}

impl EngineState<'_> {
    pub(in crate::vm::engine) fn reserve_type_operations(&mut self) -> Result<(), RuntimeError> {
        let bytes = self.prepared.vm.types.entries.len().div_ceil(2);
        self.charge_allocation_bytes(bytes)?;
        let operations = &mut self.data.construction.type_operations;
        operations
            .try_reserve_exact(bytes)
            .map_err(|_| RuntimeError::Overflow)?;
        operations.resize(bytes, 0);
        Ok(())
    }
}

impl ValueConstructionContext for EngineState<'_> {
    fn bind_structure_layout_preserving(
        &mut self,
        type_id: u32,
        value: crate::value::StructValue,
        source: &crate::value::StructValue,
    ) -> Result<crate::value::StructValue, RuntimeError> {
        let layout = self.prepared.vm.physical_layout(type_id);
        if source.physical_layout().is_some() && layout.is_none() {
            return Err(RuntimeError::TypeMismatch);
        }
        if let Some(layout) = &layout {
            self.charge_allocation_bytes(layout.byte_len()?)?;
            self.charge_allocation_bytes(layout.initialization_staging_allocation(&value)?)?;
            self.charge_work_units(layout.construction_work()?)?;
        }
        value.with_physical_layout_preserving(layout, source)
    }
    fn bind_structure_layout(
        &mut self,
        type_id: u32,
        value: crate::value::StructValue,
    ) -> Result<crate::value::StructValue, RuntimeError> {
        let layout = self.prepared.vm.physical_layout(type_id);
        if let Some(layout) = &layout
            && !value.has_physical_layout(Some(layout))
        {
            self.charge_allocation_bytes(layout.byte_len()?)?;
            self.charge_allocation_bytes(layout.initialization_staging_allocation(&value)?)?;
            self.charge_work_units(layout.construction_work()?)?;
        }
        value.with_physical_layout(layout)
    }
    fn initialize_structure_layout(
        &mut self,
        type_id: u32,
        value: crate::value::StructValue,
    ) -> Result<crate::value::StructValue, RuntimeError> {
        let layout = self.prepared.vm.physical_layout(type_id);
        if let Some(layout) = &layout
            && !value.has_physical_layout(Some(layout))
        {
            self.charge_allocation_bytes(layout.byte_len()?)?;
            self.charge_allocation_bytes(layout.initialization_staging_allocation(&value)?)?;
            self.charge_work_units(layout.construction_work()?)?;
        }
        value.initialize_physical_layout(layout)
    }
    fn strings(&self) -> &crate::vm::Names {
        &self.prepared.vm.strings
    }
    fn profile(&self) -> DateTimeProfile {
        self.data.profile
    }

    fn type_recipe(&self, type_id: u32) -> Option<u32> {
        let context = self.data.construction.initializers.last()?.context;
        self.prepared.recipe(context, type_id, None)
    }

    fn member_recipe(&self, type_id: u32, member: u32) -> Option<u32> {
        let context = self.data.construction.initializers.last()?.context;
        self.prepared.recipe(context, type_id, Some(member))
    }

    fn evaluate_recipe(&mut self, recipe_id: u32) -> Result<Value, RuntimeError> {
        let active = self.data.construction.initializers.last().ok_or_else(|| {
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
        let mut lexical = self.data.construction.frames.pop();
        let result = self.evaluate_initializer(recipe_id, lexical.as_mut(), instance, depth);
        if let Some(frame) = lexical {
            self.data.construction.frames.push(frame);
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

    #[cfg_attr(all(feature = "std", feature = "test-support"), track_caller)]
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
            .data
            .construction
            .initializers
            .last()
            .map(|active| active.result)
            .or_else(|| {
                self.data
                    .construction
                    .frames
                    .last()
                    .and_then(|frame| frame.activation)
            })
            .or_else(|| self.data.storage.last_execution_activation());
        self.check_value_lifetime(value, destination)
    }

    // Entry policy and reservation finish before recursive type construction;
    // their temporary error/allocation state does not belong to its live frame.
    #[inline(never)]
    fn enter_type(&mut self, type_id: u32, operation: ValueOperation) -> Result<(), RuntimeError> {
        // Aggregate helpers are not VM invocations. Recipe/POU entry checks
        // max_call_depth separately; this stack bounds structural recursion.
        if self.data.construction.types.len() > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        if type_id as usize >= self.prepared.vm.types.entries.len() {
            return Err(RuntimeError::TypeMismatch);
        }
        let (index, bit) = operation_bit(type_id, operation);
        let active = self
            .data
            .construction
            .type_operations
            .get(index)
            .ok_or_else(|| RuntimeError::InvalidExecutionState)?;
        if active & bit != 0 {
            return Err(invalid_bytecode(smol_str::SmolStr::new_static(
                "cyclic typed default construction",
            )));
        }
        self.charge_allocation(size_of::<(u32, ValueOperation)>())?;
        self.data
            .construction
            .types
            .try_reserve_exact(1)
            .map_err(|_| VmTrap::BudgetExceeded.into_runtime_error())?;
        self.data.construction.types.push((type_id, operation));
        self.data.construction.type_operations[index] |= bit;
        Ok(())
    }

    fn leave_type(&mut self) {
        if let Some((type_id, operation)) = self.data.construction.types.pop() {
            let (index, bit) = operation_bit(type_id, operation);
            // Every successful push first checks this same immutable-size mask.
            self.data.construction.type_operations[index] &= !bit;
        }
    }

    fn recycle_array(&mut self, array: alloc::boxed::Box<crate::value::ArrayValue>) {
        self.data.resources.raw_shells.keep_array(array);
    }

    fn recycle_struct(&mut self, structure: alloc::sync::Arc<crate::value::StructValue>) {
        self.data.resources.raw_shells.keep_structure(structure);
    }

    fn write_in_place(
        &mut self,
        at: ByteRange,
        type_id: u32,
        value: &Value,
    ) -> Result<(), RuntimeError> {
        let EngineStore::Cells(arm) = &mut self.data.store else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        arm.write_in_place(at, type_id, value, self.prepared)
    }

    fn write_enum_in_place(
        &mut self,
        at: ByteRange,
        type_id: u32,
        numeric: i64,
    ) -> Result<(), RuntimeError> {
        let EngineStore::Cells(arm) = &mut self.data.store else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        arm.write_enum_in_place(at, type_id, numeric, self.prepared)
    }

    fn element_in_place(
        &self,
        at: ByteRange,
        type_id: u32,
        index: usize,
    ) -> Result<ByteRange, RuntimeError> {
        self.element_range(at, type_id, index)
    }

    fn member_in_place(
        &self,
        at: ByteRange,
        type_id: u32,
        ordinal: usize,
    ) -> Result<ByteRange, RuntimeError> {
        self.member_range(at, type_id, ordinal)
    }

    fn check_in_place(&mut self, at: ByteRange, type_id: u32) -> Result<(), RuntimeError> {
        self.check_constructed_place(at, type_id)
    }

    fn lies_as_backing(&self, type_id: u32) -> bool {
        self.prepared.vm.physical_layout_ref(type_id).is_some()
    }

    fn node_child(
        &self,
        node: NodeTarget,
        type_id: u32,
        ordinal: usize,
    ) -> Result<Target, RuntimeError> {
        self.construction_node_child(node, type_id, ordinal)
    }

    fn write_node(
        &mut self,
        node: NodeTarget,
        _type_id: u32,
        value: &Value,
    ) -> Result<(), RuntimeError> {
        let layout = self
            .prepared
            .vm
            .physical_layout_ref(node.root_type)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let bits = layout.initial_leaf_bits(node.anchor, value)?;
        self.write_construction_leaf(node, bits)
    }

    fn write_enum_node(
        &mut self,
        node: NodeTarget,
        _type_id: u32,
        numeric: i64,
    ) -> Result<(), RuntimeError> {
        // An enumeration's bits are its value, as its value's leaf is written.
        self.write_construction_leaf(node, numeric as u64)
    }

    fn finish_backing(&mut self, node: NodeTarget, type_id: u32) -> Result<(), RuntimeError> {
        self.finish_construction_backing(node, type_id)
    }

    fn check_node(&mut self, node: NodeTarget, type_id: u32) -> Result<(), RuntimeError> {
        self.check_constructed_node(node, type_id)
    }
}

#[cfg(test)]
#[path = "type_operation_tests.rs"]
mod type_operation_tests;
