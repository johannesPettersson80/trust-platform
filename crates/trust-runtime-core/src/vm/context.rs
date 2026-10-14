//! Composition services used by the shared reference and execution paths.

use crate::memory::{MemoryLocation, VariableStorage};
use crate::value::{RefSegment, Value};

/// Storage access and assignment type information supplied by a runtime composition.
/// Legacy hosts obtain type information from HIR; source-free execution uses the
/// prepared artifact. Reference traversal itself has one shared implementation.
pub trait ReferenceContext {
    /// Prepared candidate references for a dynamic target, in wire order.
    /// None retains full discovery for legacy or temporary modules. An empty
    /// slice means the prepared module has no candidate for this storage slot.
    fn dynamic_reference_candidates(
        &self,
        _module: &super::module::VmModule,
        _reference: crate::value::ValueRefView<'_>,
    ) -> Result<Option<&[u32]>, crate::error::RuntimeError> {
        Ok(None)
    }

    /// Charge text work, one unit per UTF-8 byte, to the current execution allowance
    /// (spec 34 TIME-08, TEXT). Compositions without a work allowance charge nothing.
    fn charge_text_work(&self, _units: usize) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }

    /// Charge a STRING or WSTRING result of a native call as a new value: one work
    /// unit plus its allocation of `size_of::<Value>()` and `bytes` (spec 34 TIME-08,
    /// TEXT). Compositions without a work allowance charge nothing.
    fn charge_text_result(&self, _bytes: usize) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }

    /// Admit an owned reference path before an address instruction copies it.
    fn before_reference_path_clone(
        &self,
        _reference: crate::value::ValueRefView<'_>,
    ) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }

    /// Admit backing storage and copy work before extending an owned reference.
    fn before_reference_path_growth(
        &self,
        _bytes: usize,
        _work: usize,
    ) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }

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
        _input: &Value,
    ) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }
    /// Final operational check after detached staging and before installed mutation.
    fn before_write_commit(&self) -> Result<(), crate::error::RuntimeError> {
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
    #[cfg_attr(all(feature = "std", feature = "test-support"), track_caller)]
    fn before_value_clone(&self, _value: &Value) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }
    /// Admit a private call-transaction snapshot: a VAR_IN_OUT copy-in. IEC passes
    /// IN_OUT by reference, so a work-charging composition admits only its
    /// allocation (spec 34 TIME-08). Other compositions treat it as a copy.
    fn before_transaction_snapshot(&self, value: &Value) -> Result<(), crate::error::RuntimeError> {
        self.before_value_clone(value)
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
    /// Checked local assignment; legacy compositions retain their boolean compatibility hook.
    fn write_typed_local_checked(
        &self,
        root: &mut Value,
        path: &[RefSegment],
        value: Value,
    ) -> Result<bool, crate::error::RuntimeError> {
        Ok(self.write_typed_local(root, path, value))
    }
}

/// Read-only invocation data borrowed for one instruction observer callback.
/// It is neither an executable frame nor a mutation capability; its local slice
/// cannot outlive the callback's borrow of the currently executing invocation.
#[derive(Clone, Copy, Debug)]
pub struct StatementFrame<'a> {
    activation: Option<crate::memory::FrameId>,
    pou_id: Option<u32>,
    runtime_instance: Option<crate::memory::InstanceId>,
    locals: &'a [crate::value::Value],
    storage: Option<&'a VariableStorage>,
}

impl<'a> StatementFrame<'a> {
    pub(crate) fn from_frame(frame: &'a super::VmFrame) -> Self {
        Self {
            activation: frame.activation,
            pou_id: frame.pou_id,
            runtime_instance: frame.runtime_instance,
            locals: &frame.locals,
            storage: None,
        }
    }

    pub(crate) fn with_call_stack<'b>(self, storage: &'b VariableStorage) -> StatementFrame<'b>
    where
        'a: 'b,
    {
        StatementFrame {
            storage: Some(storage),
            ..self
        }
    }
    /// Current invocation identity. Legacy fixtures and initializer bodies
    /// without a lexical invocation may have no activation identity.
    pub fn activation(self) -> Option<crate::memory::FrameId> {
        self.activation
    }
    /// Artifact identity of the active POU, absent for ownerless initializer code.
    pub fn pou_id(self) -> Option<u32> {
        self.pou_id
    }
    /// Physical instance receiving the invocation, if it has one.
    pub fn runtime_instance(self) -> Option<crate::memory::InstanceId> {
        self.runtime_instance
    }
    /// Current local values, including bound parameters and the return slot.
    pub fn locals(self) -> &'a [crate::value::Value] {
        self.locals
    }

    /// Visit live suspended ancestors followed by the currently executing frame.
    /// Values and identities are borrowed from the owner; no snapshot allocation
    /// or mutable frame capability is introduced into the execution path.
    /// Each returned frame has a single-frame view rather than a recursive stack.
    pub fn call_stack(self) -> impl Iterator<Item = StatementFrame<'a>> {
        self.storage
            .into_iter()
            .flat_map(VariableStorage::suspended_execution_activations)
            .filter_map(move |(activation, owner, locals)| {
                if Some(activation) == self.activation {
                    return None;
                }
                Some(StatementFrame {
                    activation: Some(activation),
                    pou_id: Some(owner.pou_id),
                    runtime_instance: owner.runtime_instance,
                    locals,
                    storage: None,
                })
            })
            .chain(core::iter::once(StatementFrame {
                storage: None,
                ..self
            }))
    }
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
    /// Admit continuation storage bytes before fallible allocation and relocation.
    /// The work argument is frame-stack bookkeeping (TIME-08) and is not charged.
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
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _timing =
            super::diagnostic_timing::begin(super::diagnostic_timing::Phase::DeadlineCheck);
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
    #[cfg_attr(all(feature = "std", feature = "test-support"), track_caller)]
    fn charge_execution_work(&self, units: usize) -> Result<(), crate::error::RuntimeError> {
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _timing = super::diagnostic_timing::begin(super::diagnostic_timing::Phase::WorkCharge);

        #[cfg(not(all(feature = "std", feature = "test-support")))]
        {
            if self.execution_budget().charge(units)? {
                self.check_execution_deadline()?;
            }
            Ok(())
        }

        #[cfg(all(feature = "std", feature = "test-support"))]
        {
            let caller = if super::diagnostic_timing::callsite_accounting_log_enabled() {
                Some(core::panic::Location::caller())
            } else {
                None
            };
            let result = match self.execution_budget().charge(units) {
                Err(error) => Err(error),
                Ok(true) => self.check_execution_deadline(),
                Ok(false) => Ok(()),
            };
            if let Some(caller) = caller {
                let (consumed, outcome) = match &result {
                    Ok(()) => (units, super::diagnostic_timing::WorkChargeOutcome::Charged),
                    Err(error) => {
                        let outcome = super::diagnostic_timing::work_outcome(error);
                        let consumed = if matches!(
                            error,
                            crate::error::RuntimeError::ExecutionLimit {
                                kind: crate::error::ExecutionLimitKind::Work,
                                ..
                            }
                        ) {
                            0
                        } else {
                            units
                        };
                        (consumed, outcome)
                    }
                };
                super::diagnostic_timing::record_work_charge(caller, units, consumed, outcome);
            }
            result
        }
    }
    /// Invalidate an invocation and release owned temporaries even when ownership checks fail.
    /// Return that failure only after all identities are retired.
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
    /// Notify an observer with the active invocation's read-only local view.
    /// Existing adapters retain their old hook through this default delegation.
    fn on_statement_with_frame(
        &mut self,
        module: &super::module::VmModule,
        pou_id: u32,
        pc: usize,
        depth: u32,
        _frame: StatementFrame<'_>,
    ) {
        self.on_statement(module, pou_id, pc, depth);
    }

    /// Invoke the observer without hiding a composition invariant failure.
    /// The dispatcher owns cleanup if this callback fails.
    fn on_statement_with_frame_checked(
        &mut self,
        module: &super::module::VmModule,
        pou_id: u32,
        pc: usize,
        depth: u32,
        frame: StatementFrame<'_>,
    ) -> Result<(), crate::error::RuntimeError> {
        self.on_statement_with_frame(module, pou_id, pc, depth, frame);
        Ok(())
    }
    /// Measure a runtime value under the same resource allowance.
    fn sizeof_value(&self, value: &Value) -> Result<u64, crate::error::RuntimeError>;
    /// Whether this context owns the iterative register activation pump.
    #[cfg(feature = "register-ir")]
    #[doc(hidden)]
    fn has_register_activation_pump(&self) -> bool {
        false
    }
    /// Start a prepared register activation without recursively executing it.
    /// The default keeps older `try_execute_optimized` adapters available.
    #[cfg(feature = "register-ir")]
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    fn start_register_activation(
        &mut self,
        _module: &super::module::VmModule,
        _pou_id: u32,
        _entry_instance: Option<crate::memory::InstanceId>,
        _initial_locals: Option<&[Value]>,
        _owned_initial_locals: Option<&mut Option<alloc::vec::Vec<Value>>>,
        _parameter_values_present: Option<&[bool]>,
        _capture_return: bool,
        _depth: u32,
        _entry: super::budget::ExecutionEntry,
        _capture_result_locals: bool,
    ) -> Result<Option<super::RegisterActivation>, crate::error::RuntimeError> {
        Ok(None)
    }
    /// Advance one prepared register activation until it calls or returns.
    #[cfg(feature = "register-ir")]
    #[doc(hidden)]
    fn step_register_activation(
        &mut self,
        _module: &super::module::VmModule,
        _activation: &mut super::RegisterActivation,
    ) -> Result<super::RegisterActivationStep, crate::error::RuntimeError> {
        Err(crate::error::RuntimeError::InvalidExecutionState)
    }
    /// Recycle a completed activation's reserved register scratch.
    #[cfg(feature = "register-ir")]
    #[doc(hidden)]
    fn recycle_register_activation(
        &mut self,
        _activation: super::RegisterActivation,
    ) -> Result<(), crate::error::RuntimeError> {
        Err(crate::error::RuntimeError::InvalidExecutionState)
    }
    /// Clean and recycle an activation interrupted by a child fault.
    #[cfg(feature = "register-ir")]
    #[doc(hidden)]
    fn abort_register_activation(
        &mut self,
        _activation: super::RegisterActivation,
    ) -> Result<(), crate::error::RuntimeError> {
        Err(crate::error::RuntimeError::InvalidExecutionState)
    }
    /// Acquire cleared scratch buffers for a dispatch entry.
    fn take_execution_buffers(&mut self) -> super::dispatch::ExecutionBuffers {
        super::dispatch::ExecutionBuffers::default()
    }
    /// Return cleared scratch buffers after entry cleanup.
    fn recycle_execution_buffers(&mut self, _buffers: super::dispatch::ExecutionBuffers) {}
}

#[cfg(test)]
#[path = "context/tests.rs"]
mod tests;
