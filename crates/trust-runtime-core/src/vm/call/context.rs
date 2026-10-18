//! Native call composition: executable semantics stay in the shared VM.
use crate::vm::{
    context::ReferenceContext, dispatch::VmPouStackResult, frames::VmFrame, module::VmModule,
};
use crate::{
    error::RuntimeError,
    memory::InstanceId,
    stdlib::StandardLibrary,
    value::{DateTimeProfile, DateTimeValue, Duration, Value, ValueRef},
};
use alloc::vec::Vec;
use smol_str::SmolStr;

use super::bindings::{VmFbOutBinding, VmNativeArg, VmOutBinding, VmWriteTarget};

/// Direction of one admitted native function-block parameter.
/// Name and direction used by native function-block argument binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinParamDirection {
    /// Read-only input parameter.
    In,
    /// Output copied back after execution.
    Out,
    /// Parameter bound for both input and output.
    InOut,
}
/// Name and direction used by native function-block argument binding.
#[derive(Debug, Clone)]
pub struct BuiltinParam {
    /// Source-visible field or parameter name.
    pub name: SmolStr,
    /// Input/output transfer direction.
    pub direction: BuiltinParamDirection,
}
/// Input and private phase field used for edge qualification.
#[derive(Debug, Clone)]
pub struct VmEdgeInput {
    /// Source-visible field or parameter name.
    pub name: SmolStr,
    /// Private phase-state field name.
    pub phase_name: SmolStr,
    /// True for rising-edge qualification, false for falling edges.
    pub rising: bool,
}

/// Composition services around the shared native and user-call implementation.
pub trait CallContext: ReferenceContext {
    /// Return result locals to a prepared optimized POU after its caller has
    /// completed copyback. Until this call, that POU's prepared frame payload is
    /// unavailable and recursive re-entry must use the shared stack path. Stack
    /// results and legacy contexts may discard them.
    fn recycle_optimized_result_locals(
        &mut self,
        _pou_id: u32,
        _locals: Vec<Value>,
    ) -> Result<(), RuntimeError> {
        Ok(())
    }

    /// Take the local slots of a call frame being bound, `count` NULL values. A composition
    /// that reserved frame payloads at construction hands one out, so a scan allocates no
    /// local vector (spec 34 MEM-09; census C5). The matching return is
    /// [`CallContext::recycle_optimized_result_locals`].
    fn take_optimized_call_locals(&mut self, count: usize) -> Vec<Value> {
        alloc::vec![Value::Null; count]
    }

    /// Take the evaluated argument list of a call being bound, empty (spec 34 MEM-09; census
    /// C1). A composition that reserved call scratch at construction hands one out, so a scan
    /// allocates no argument vector. The matching return is [`CallContext::recycle_call_args`].
    fn take_call_args(&mut self) -> Vec<VmNativeArg> {
        Vec::new()
    }

    /// Return the argument list of [`CallContext::take_call_args`], emptied.
    fn recycle_call_args(&mut self, _args: Vec<VmNativeArg>) {}

    /// Take `count` false parameter flags — the built-in consumption bits or the user presence
    /// bits (spec 34 MEM-09; census C3, C5). A composition that reserved call scratch hands one
    /// out; the default allocates it, as a binding without the reservation does today.
    fn take_call_flags(&mut self, count: usize) -> Vec<bool> {
        alloc::vec![false; count]
    }

    /// Return the flag vector of [`CallContext::take_call_flags`], emptied.
    fn recycle_call_flags(&mut self, _flags: Vec<bool>) {}

    /// Take the output-binding list of a user function or method call, empty (spec 34 MEM-09;
    /// census C5). A composition that reserved call scratch hands one out. The matching return
    /// is [`CallContext::recycle_call_function_outputs`].
    fn take_call_function_outputs(&mut self) -> Vec<VmOutBinding> {
        Vec::new()
    }

    /// Return the output bindings of [`CallContext::take_call_function_outputs`], emptied.
    fn recycle_call_function_outputs(&mut self, _outputs: Vec<VmOutBinding>) {}

    /// Take the output-binding list of a function-block call, empty (spec 34 MEM-09; census C3,
    /// C4). A composition that reserved call scratch hands one out. The matching return is
    /// [`CallContext::recycle_call_fb_outputs`].
    fn take_call_fb_outputs(&mut self) -> Vec<VmFbOutBinding> {
        Vec::new()
    }

    /// Return the output bindings of [`CallContext::take_call_fb_outputs`], emptied.
    fn recycle_call_fb_outputs(&mut self, _outputs: Vec<VmFbOutBinding>) {}

    /// Take the preflighted output-target list of a user function-block call, empty (spec 34
    /// MEM-09; census C4). A composition that reserved call scratch hands one out. The matching
    /// return is [`CallContext::recycle_call_prepared_targets`].
    fn take_call_prepared_targets(&mut self) -> Vec<Option<(VmWriteTarget, Option<u32>)>> {
        Vec::new()
    }

    /// Return the prepared targets of [`CallContext::take_call_prepared_targets`], emptied.
    fn recycle_call_prepared_targets(
        &mut self,
        _targets: Vec<Option<(VmWriteTarget, Option<u32>)>>,
    ) {
    }

    /// Take the preflighted output-value list of a call's copy-back group, empty (spec 34
    /// MEM-09; census C7). A composition that reserved call scratch hands one out. The matching
    /// return is [`CallContext::recycle_call_prepared_outputs`].
    fn take_call_prepared_outputs(&mut self) -> Vec<(VmWriteTarget, Value)> {
        Vec::new()
    }

    /// Return the prepared outputs of [`CallContext::take_call_prepared_outputs`], emptied.
    fn recycle_call_prepared_outputs(&mut self, _outputs: Vec<(VmWriteTarget, Value)>) {}

    /// Take the evaluated argument values of a standard-library call, empty (spec 34 MEM-09;
    /// census C9). A composition that reserved call scratch hands one out, so a scan allocates
    /// no standard-library value vector. The matching return is
    /// [`CallContext::recycle_call_stdlib_values`].
    fn take_call_stdlib_values(&mut self) -> Vec<Value> {
        Vec::new()
    }

    /// Return the standard-library values of [`CallContext::take_call_stdlib_values`], emptied.
    fn recycle_call_stdlib_values(&mut self, _values: Vec<Value>) {}

    /// Take the POU's edge-input plan into a reusable list (spec 34 MEM-09; census A5). A
    /// composition that keeps the prepared index reserves the list at construction; the
    /// default allocates it, as a composition without a prepared module does today.
    fn take_edge_inputs_for_pou(
        &mut self,
        module: &VmModule,
        pou_id: u32,
        owner: &str,
    ) -> Vec<VmEdgeInput> {
        self.edge_inputs_for_pou(module, pou_id, owner)
    }

    /// Return the edge-input list of [`CallContext::take_edge_inputs_for_pou`] after the
    /// call applied its transitions.
    fn recycle_edge_inputs(&mut self, _inputs: Vec<VmEdgeInput>) {}

    /// Take the reusable original-input list for edge restoration (spec 34 MEM-09;
    /// census A5). A composition that reserved it at construction hands one out.
    fn take_edge_restores(&mut self) -> Vec<(usize, Value)> {
        Vec::new()
    }

    /// Return the restore list of [`CallContext::take_edge_restores`] once the edges are
    /// restored. Each entry is a member ordinal and its original value (C1-9d2a, R2).
    fn recycle_edge_restores(&mut self, _restores: Vec<(usize, Value)>) {}

    /// Apply a native output group under the composition's copy-back policy.
    /// Legacy hosts preserve their existing writes; source-free state stages them.
    fn with_output_transaction<T>(
        &mut self,
        frame: &mut VmFrame,
        action: impl FnOnce(&mut Self, &mut VmFrame) -> Result<T, crate::vm::VmTrap>,
    ) -> Result<T, crate::vm::VmTrap>
    where
        Self: Sized,
    {
        action(self, frame)
    }
    /// Return the admitted date/time interpretation profile.
    fn profile(&self) -> &DateTimeProfile;
    /// Return the current logical PLC sample time.
    fn current_time(&self) -> Duration;
    /// Sample an explicitly admitted platform wall clock.
    fn current_dt(&mut self) -> Result<DateTimeValue, RuntimeError>;
    /// Borrow the prepared native function registry.
    fn stdlib(&self) -> &StandardLibrary;
    /// Obtain the declared native parameter binding plan.
    fn builtin_params(&self, key: &str) -> Option<Vec<BuiltinParam>>;
    /// Obtain the edge-input plan for this physical POU owner.
    fn edge_inputs(&self, owner: &str) -> Vec<VmEdgeInput>;
    /// Obtain a POU's edge-input plan when its admitted module and identity are
    /// available. Older contexts retain their owner-name lookup behavior.
    fn edge_inputs_for_pou(
        &self,
        _module: &VmModule,
        _pou_id: u32,
        owner: &str,
    ) -> Vec<VmEdgeInput> {
        self.edge_inputs(owner)
    }
    /// The prepared plan of the `CALL_NATIVE` site at `code_offset`, when the
    /// composition keeps plans (spec 34 MEM-09). Older contexts resolve every call at
    /// the call site.
    fn call_plan(
        &self,
        _code_offset: u32,
    ) -> Option<alloc::sync::Arc<crate::vm::prepared::CallPlan>> {
        None
    }
    /// Move caller locals into addressable suspended storage before a nested call.
    fn suspend_frame(&mut self, frame: &mut VmFrame) -> Result<(), RuntimeError>;
    /// Restore those same caller locals after the nested call.
    fn resume_frame(&mut self, frame: &mut VmFrame) -> Result<(), RuntimeError>;
    /// Record an optional hosted call-profile event.
    fn record_call_op(&mut self, kind: RegisterCallOpKind);
    /// Record an optional hosted value-profile event.
    fn record_value_op(&mut self, kind: RegisterValueOpKind);
    /// Check actual target/value lifetimes before a native argument or result write.
    fn check_native_write(
        &self,
        caller_frame: Option<&VmFrame>,
        reference: &ValueRef,
        value: &Value,
    ) -> Result<(), RuntimeError>;
    /// Admit native state mutation, including hidden function-block slots.
    fn check_builtin_call(&self, instance_id: InstanceId) -> Result<(), RuntimeError>;
    /// Run a built-in function block against the composition's own storage (spec 34
    /// MEM-09; census C3, C7). The composition owns the storage, so it supplies the store
    /// here, and the shared call path needs no concrete `VariableStorage`.
    fn execute_builtin_fb(
        &mut self,
        now: Duration,
        instance_id: InstanceId,
        kind: crate::stdlib::fbs::BuiltinFbKind,
    ) -> Result<(), RuntimeError>;
    /// An optional prepared hosted optimization; None selects the common stack loop.
    #[allow(clippy::too_many_arguments)]
    fn try_execute_optimized(
        &mut self,
        _module: &VmModule,
        _pou_id: u32,
        _instance: Option<InstanceId>,
        _initial_locals: Option<&[Value]>,
        _capture_return: bool,
        _depth: u32,
        _entry: crate::vm::budget::ExecutionEntry,
    ) -> Result<Option<VmPouStackResult>, RuntimeError> {
        Ok(None)
    }

    /// Preserve explicit argument presence, including a supplied NULL, in an
    /// optional optimized callee. Older adapters safely retain stack fallback.
    #[allow(clippy::too_many_arguments)]
    fn try_execute_optimized_with_parameter_presence(
        &mut self,
        module: &VmModule,
        pou_id: u32,
        instance: Option<InstanceId>,
        initial_locals: Option<&[Value]>,
        parameter_values_present: Option<&[bool]>,
        capture_return: bool,
        depth: u32,
        entry: crate::vm::budget::ExecutionEntry,
    ) -> Result<Option<VmPouStackResult>, RuntimeError> {
        if parameter_values_present.is_some() {
            return Ok(None);
        }
        self.try_execute_optimized(
            module,
            pou_id,
            instance,
            initial_locals,
            capture_return,
            depth,
            entry,
        )
    }

    /// Try the optimized entry with invocation-owned locals available to transfer.
    ///
    /// The default preserves compatibility by using the borrowed entry hook and
    /// leaves `initial_locals` untouched, including when the optimized path
    /// declines the call.
    #[allow(clippy::too_many_arguments)]
    fn try_execute_optimized_with_parameter_presence_owned(
        &mut self,
        module: &VmModule,
        pou_id: u32,
        instance: Option<InstanceId>,
        initial_locals: &mut Option<Vec<Value>>,
        parameter_values_present: Option<&[bool]>,
        capture_return: bool,
        depth: u32,
        entry: crate::vm::budget::ExecutionEntry,
    ) -> Result<Option<VmPouStackResult>, RuntimeError> {
        self.try_execute_optimized_with_parameter_presence(
            module,
            pou_id,
            instance,
            initial_locals.as_deref(),
            parameter_values_present,
            capture_return,
            depth,
            entry,
        )
    }
}

/// Call events reported to optional hosted profiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterCallOpKind {
    /// A call frame was entered.
    FramePush,
    /// A call frame was removed.
    FramePop,
    /// A function-block invocation was entered.
    FunctionBlockCallEntry,
    /// A call argument was bound.
    ParameterBinding,
    /// An output value was committed.
    OutputCopyBack,
}

/// Value materialization events reported to optional hosted profiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterValueOpKind {
    /// A constant value was materialized.
    LoadConstant,
    /// A register value was cloned.
    #[cfg(feature = "register-ir")]
    RegisterReadClone,
    /// A register value was moved.
    #[cfg(feature = "register-ir")]
    RegisterReadMove,
    /// A referenced value was materialized.
    ReadReference,
    /// An expression value was cloned for argument binding.
    BindExpression,
    /// A call result was cloned for copy-back.
    CopyOutput,
}
