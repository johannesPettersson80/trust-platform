#[cfg(test)]
use crate::bytecode::BytecodeModule;

#[cfg(test)]
use crate::bytecode::{
    PouKind, RefEntry, RefLocation, RefTable, SectionData, SectionId, StringTable, VarMeta,
};
use crate::error::RuntimeError;
#[cfg(test)]
use crate::memory::IoArea;
use crate::task::ProgramDef;
use crate::value::ValueRef;
#[cfg(test)]
use crate::value::{RefPath, RefSegment as ValueRefSegment};
use smol_str::SmolStr;

mod budget;
mod call;
mod call_context;
mod context;
#[cfg(test)]
mod debug_map;
mod dispatch;
mod dispatch_refs;
mod dispatch_sizeof;
mod edge;
mod errors;
mod frames;
#[cfg(test)]
mod limits;
mod local_init;
mod register_ir;
mod stack;
#[cfg(test)]
mod type_policy;

// VM module ownership notes (Phase B):
// - dispatch: instruction pointer loop + opcode routing + debug-hook emission.
// - dispatch_ops: arithmetic/logic execution helpers + operand/jump decoding.
// - dispatch_refs: ref/deref chain execution and storage bridge helpers.
// - dispatch_sizeof: TYPE_TABLE driven SIZEOF evaluation helpers.
// - const_pool: VM CONST_POOL decode + primitive literal materialization.
// - stack: operand stack invariants and overflow/underflow enforcement.
// - frames/call: call-stack and frame lifecycle.
// - errors: VM trap taxonomy and stable RuntimeError mapping.
// - debug_map: symbol/source lookup tables for external name/debug APIs.
// - register_ir: Phase A scaffold for stack-bytecode -> register-IR lowering + verifier.

use super::core::Runtime;

pub(super) use local_init::VmLocalInitPlanCacheState;
pub(super) use register_ir::{
    RegisterLoweringCacheState, RegisterProfileState, RegisterTier1SpecializedExecutorState,
};
pub(super) use trust_runtime_core::vm::hosted::{materialize_borrowed_value, opcode_operand_len};

#[cfg(test)]
pub(super) const DEFAULT_INSTRUCTION_BUDGET: usize =
    trust_runtime_core::vm::VM_MAX_EXECUTED_INSTRUCTIONS;

pub(super) fn execute_program(
    runtime: &mut Runtime,
    program: &ProgramDef,
) -> Result<(), RuntimeError> {
    dispatch::execute_program(runtime, program)
}

pub(super) fn execute_program_by_name(
    runtime: &mut Runtime,
    program_name: &SmolStr,
) -> Result<(), RuntimeError> {
    dispatch::execute_program_by_name(runtime, program_name)
}

pub(super) fn execute_function_block_ref(
    runtime: &mut Runtime,
    reference: &ValueRef,
) -> Result<(), RuntimeError> {
    dispatch::execute_function_block_ref(runtime, reference)
}

pub(super) use trust_runtime_core::vm::hosted::module::invalid_bytecode;
#[cfg(test)]
pub(super) use trust_runtime_core::vm::hosted::module::VmRef;
#[cfg(test)]
use trust_runtime_core::vm::hosted::module::{
    build_ref_type_map, decode_vm_ref, infer_primary_instance_owner,
};
#[cfg(test)]
pub(super) use trust_runtime_core::vm::hosted::module::{
    VmNativeArgSpec, VmNativeSymbolSpec, VmParamMeta, VmPouEntry,
};

pub(super) use trust_runtime_core::vm::hosted::module::VmModule;

#[cfg(test)]
fn materialize_test_module(module: &BytecodeModule) -> Result<VmModule, RuntimeError> {
    VmModule::from_validated(&module.validated().map_err(RuntimeError::from)?)
}

#[cfg(test)]
mod tests;
