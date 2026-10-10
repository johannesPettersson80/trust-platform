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
    #[cfg(feature = "hir")]
    RegisterReadClone,
    /// A register value was moved.
    #[cfg(feature = "hir")]
    RegisterReadMove,
    /// A referenced value was materialized.
    ReadReference,
    /// An expression value was cloned for argument binding.
    BindExpression,
    /// A call result was cloned for copy-back.
    CopyOutput,
}
