use super::super::core::Runtime;
use crate::memory::{MemoryLocation, VariableStorage};
use crate::value::{RefSegment, Value};
use trust_runtime_core::vm::hosted::context::ReferenceContext;

impl ReferenceContext for Runtime {
    fn check_reference_write(
        &self,
        _reference: crate::value::ValueRefView<'_>,
        _value: &Value,
    ) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }
    fn check_reference_read(
        &self,
        _reference: crate::value::ValueRefView<'_>,
    ) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }
    fn initializer_reference<'a>(
        &self,
        _initializer_id: u32,
        _path: &'a [RefSegment],
    ) -> Result<crate::value::ValueRefView<'a>, trust_runtime_core::vm::VmTrap> {
        Err(trust_runtime_core::vm::VmTrap::UnsupportedRefLocation(
            "initializer-result",
        ))
    }

    fn storage(&self) -> &VariableStorage {
        &self.storage
    }
    fn storage_mut(&mut self) -> &mut VariableStorage {
        &mut self.storage
    }
    fn write_typed_storage(
        &mut self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        value: Value,
    ) -> Result<bool, crate::error::RuntimeError> {
        Ok(self
            .storage
            .write_by_ref_parts_typed(&self.registry, location, offset, path, value))
    }
    fn write_typed_local(&self, root: &mut Value, path: &[RefSegment], value: Value) -> bool {
        crate::value::write_value_path_typed(root, path, value, &self.registry)
    }
}

impl trust_runtime_core::vm::hosted::context::ExecutionContext for Runtime {
    fn initialize_frame(
        &mut self,
        module: &super::VmModule,
        frame: &mut super::frames::VmFrame,
        _depth: u32,
    ) -> Result<(), crate::error::RuntimeError> {
        super::local_init::initialize_declared_locals(self, module, frame)
    }

    fn execution_budget(&self) -> &trust_runtime_core::vm::hosted::budget::ExecutionBudget {
        &self.vm_execution_budget
    }
    fn retire_frame(&mut self, _frame: &super::frames::VmFrame) {}
    fn check_frame_return(
        &self,
        _frame: &super::frames::VmFrame,
        _value: &Value,
    ) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }
    fn execute_initialization_opcode(
        &mut self,
        _module: &super::VmModule,
        _frame: &mut super::frames::VmFrame,
        opcode: u8,
        _operand: u32,
        _stack: &mut super::stack::OperandStack,
        _depth: u32,
    ) -> Result<(), crate::error::RuntimeError> {
        Err(trust_runtime_core::vm::VmTrap::InvalidOpcode(opcode).into_runtime_error())
    }

    fn deadline_exceeded(&self) -> bool {
        super::dispatch::deadline_exceeded(self.effective_execution_deadline())
    }

    fn on_statement(&mut self, module: &super::VmModule, pou_id: u32, pc: usize, depth: u32) {
        use crate::debug::DebugHook;
        let Some(source) = module.debug_map().source_by_pc.get(&(pou_id, pc as u32)) else {
            return;
        };
        let Some(location) =
            self.resolve_vm_debug_location(source.file.as_str(), source.line, source.column)
        else {
            return;
        };
        if let Some(debug) = self.debug.as_mut() {
            debug.refresh_snapshot_from_storage(&self.storage, self.current_time);
            debug.on_statement(Some(&location), depth);
        }
    }

    fn sizeof_value(&self, value: &Value) -> Result<u64, crate::error::RuntimeError> {
        crate::value::size_of_value(&self.registry, value)
            .map_err(super::dispatch_sizeof::sizeof_error_to_runtime)
    }

    fn take_execution_buffers(
        &mut self,
    ) -> trust_runtime_core::vm::hosted::dispatch::ExecutionBuffers {
        super::dispatch::take_shared_buffers()
    }

    fn recycle_execution_buffers(
        &mut self,
        buffers: trust_runtime_core::vm::hosted::dispatch::ExecutionBuffers,
    ) {
        super::dispatch::recycle_shared_buffers(buffers);
    }
}
