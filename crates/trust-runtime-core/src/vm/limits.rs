//! Fixed VM execution limits.

/// Maximum operand values held by one VM invocation.
pub const VM_MAX_OPERAND_STACK: usize = 16 * 1024;
/// Maximum active VM call frames.
pub const VM_MAX_CALL_DEPTH: usize = 1_024;
/// Maximum original bytecode instructions executed by one top-level invocation.
pub const VM_MAX_EXECUTED_INSTRUCTIONS: usize = 1_000_000;
