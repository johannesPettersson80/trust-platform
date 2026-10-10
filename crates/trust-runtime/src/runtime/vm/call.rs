//! Hosted facade for shared native call execution and binding.
#[cfg(test)]
use trust_runtime_core::vm::hosted::call::bindings::{
    bind_builtin_function_block_arguments, bind_vm_call_arguments,
    bind_vm_function_block_arguments, normalize_output_copyback_value, read_vm_reference,
    read_vm_target_value, resolve_named_arg_index, unpack_native_call_payload, write_output_int,
    VmFbFieldBinding, VmFbOutSource, VmNativeArg, VmNativeArgValue, VmWriteTarget,
};
#[cfg(test)]
use trust_runtime_core::vm::hosted::call::stdlib::{
    bind_conversion_value, bind_stdlib_named_values, bind_stdlib_positional_values,
    dispatch_native_stdlib_call,
};
pub(super) use trust_runtime_core::vm::hosted::call::{execute_native_call, push_call_frame};
pub(super) use trust_runtime_core::vm::hosted::context::VM_LOCAL_SENTINEL_FRAME_ID;
#[cfg(test)]
use trust_runtime_core::vm::hosted::symbols::{
    preparse_native_symbol_spec, resolve_native_symbol_specs,
};
#[cfg(test)]
use trust_runtime_core::vm::VmTrap;
#[cfg(test)]
mod tests;
