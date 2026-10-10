//! Composition services used by the shared reference and execution paths.

use crate::memory::{MemoryLocation, VariableStorage};
use crate::value::{RefSegment, Value};

/// Storage access and assignment type information supplied by a runtime composition.
/// Legacy hosts obtain type information from HIR; source-free execution uses the
/// prepared artifact. Reference traversal itself has one shared implementation.
pub trait ReferenceContext {
    /// Normalize an assignment using the composition's declared type policy.
    fn normalize_assignment(
        &self,
        module: &super::module::VmModule,
        ty: u32,
        value: Value,
    ) -> Result<Value, crate::error::RuntimeError> {
        super::type_policy::normalize_vm_value_for_type(module, ty, value)
    }
    /// Charge allocations needed to update a selected field/element before mutation.
    fn before_path_write(
        &self,
        _root: &Value,
        _path: &[RefSegment],
    ) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }
    /// Journal an actual native copy-back destination immediately before mutation.
    fn before_output_write(
        &mut self,
        _frame: &super::VmFrame,
        _reference: crate::value::ValueRefView<'_>,
    ) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }
    /// Admit the work and allocation needed to materialize this value.
    fn before_value_clone(&self, _value: &Value) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }
    /// Check mutability, visibility and escaping references before a write.
    fn check_reference_write(
        &self,
        reference: crate::value::ValueRefView<'_>,
        value: &Value,
    ) -> Result<(), crate::error::RuntimeError>;
    /// Check lifetime and lexical visibility without allocating an address.
    fn check_reference_read(
        &self,
        reference: crate::value::ValueRefView<'_>,
    ) -> Result<(), crate::error::RuntimeError>;
    /// Resolve the active initializer's private result slot and borrowed path.
    fn initializer_reference<'a>(
        &self,
        initializer_id: u32,
        path: &'a [RefSegment],
    ) -> Result<crate::value::ValueRefView<'a>, super::VmTrap>;
    /// Map an artifact owner to the physical invocation instance.
    fn resolve_instance_owner(
        &self,
        encoded: u32,
        frame: &super::VmFrame,
    ) -> Result<crate::memory::InstanceId, super::VmTrap> {
        Ok(if frame.instance_owner == Some(encoded) {
            frame
                .runtime_instance
                .unwrap_or(crate::memory::InstanceId(encoded))
        } else {
            crate::memory::InstanceId(encoded)
        })
    }
    /// Borrow the composition's shared storage.
    fn storage(&self) -> &VariableStorage;
    /// Access internal storage after the shared policy checks.
    fn storage_mut(&mut self) -> &mut VariableStorage;
    /// Commit a value already normalized and lifetime-checked by the shared store path.
    /// Preserve operational failures; false means only an invalid storage target.
    fn write_typed_storage(
        &mut self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        value: Value,
    ) -> Result<bool, crate::error::RuntimeError>;
    /// Write an already checked and normalized value into a local path.
    fn write_typed_local(&self, root: &mut Value, path: &[RefSegment], value: Value) -> bool;
}

/// Legacy local-reference encoding, retained only at the hosted compatibility boundary.
pub const VM_LOCAL_SENTINEL_FRAME_ID: u32 = u32::MAX;

/// Host extensions around the single bytecode dispatcher.
pub trait ExecutionContext: super::call::context::CallContext {
    /// Initialize one invocation using the existing shared work allowance.
    fn initialize_frame(
        &mut self,
        module: &super::module::VmModule,
        frame: &mut super::VmFrame,
        depth: u32,
    ) -> Result<(), crate::error::RuntimeError>;
    /// Maximum admitted logical call depth, including the enclosing invocation.
    fn call_depth_limit(&self) -> usize {
        super::VM_MAX_CALL_DEPTH
    }
    /// Charge continuation storage before fallible allocation and relocation.
    fn charge_call_storage(
        &self,
        _bytes: usize,
        _work: usize,
    ) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }
    /// Return the single allowance shared by instructions and nested helpers.
    fn execution_budget(&self) -> &super::budget::ExecutionBudget;
    /// Sample the physical deadline at an entry or successful completion.
    fn check_execution_deadline(&self) -> Result<(), crate::error::RuntimeError> {
        if self.deadline_exceeded() {
            Err(super::VmTrap::DeadlineExceeded.into_runtime_error())
        } else {
            Ok(())
        }
    }
    /// Start a root allowance or join a nested one, then check its deadline.
    fn begin_execution(
        &self,
        entry: super::budget::ExecutionEntry,
        limit: usize,
    ) -> Result<(), crate::error::RuntimeError> {
        if entry == super::budget::ExecutionEntry::Root {
            self.execution_budget().reset(limit);
        }
        self.check_execution_deadline()
    }
    /// Consume shared fuel and poll the deadline only at its bounded stride.
    fn charge_execution_work(&self, units: usize) -> Result<(), crate::error::RuntimeError> {
        if self.execution_budget().charge(units)? {
            self.check_execution_deadline()?;
        }
        Ok(())
    }
    /// Invalidate an invocation and release owned temporaries even on budget failure.
    /// Return the cleanup charge failure only after all identities are retired.
    fn retire_frame(&mut self, frame: &super::VmFrame) -> Result<(), crate::error::RuntimeError>;
    /// Reject values whose references would escape the returning invocation.
    fn check_frame_return(
        &self,
        frame: &super::VmFrame,
        value: &Value,
    ) -> Result<(), crate::error::RuntimeError>;
    /// Execute a typed construction opcode through the same dispatcher.
    #[allow(clippy::too_many_arguments)]
    fn execute_initialization_opcode(
        &mut self,
        module: &super::module::VmModule,
        frame: &mut super::VmFrame,
        opcode: u8,
        operand: u32,
        stack: &mut super::OperandStack,
        depth: u32,
    ) -> Result<(), crate::error::RuntimeError>;
    /// Sample the composition's physical deadline, independent of PLC time.
    fn deadline_exceeded(&self) -> bool;
    /// Notify optional debug/profiling hooks at this instruction boundary.
    fn on_statement(
        &mut self,
        module: &super::module::VmModule,
        pou_id: u32,
        pc: usize,
        depth: u32,
    );
    /// Measure a runtime value under the same resource allowance.
    fn sizeof_value(&self, value: &Value) -> Result<u64, crate::error::RuntimeError>;
    /// Acquire cleared scratch buffers for a dispatch entry.
    fn take_execution_buffers(&mut self) -> super::dispatch::ExecutionBuffers {
        super::dispatch::ExecutionBuffers::default()
    }
    /// Return cleared scratch buffers after entry cleanup.
    fn recycle_execution_buffers(&mut self, _buffers: super::dispatch::ExecutionBuffers) {}
}
