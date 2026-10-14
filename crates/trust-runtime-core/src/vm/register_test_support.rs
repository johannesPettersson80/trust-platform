//! Explicit fixtures for the historical register algorithm and malformed-body tests.
//!
//! Available only under `test-support`, selected by development dependencies.
//! These helpers can construct unadmitted programs and do not create a controller
//! admission token. Production consumers use `vm::register` observations and the
//! shared prepared-controller API instead.

pub use super::helpers::opcode_operand_len;
pub use super::register_ir::*;

/// Recycle frame locals returned by a register fixture after call copyback.
pub fn recycle_result_locals(
    buffers: &mut RegisterExecutionBuffers,
    locals: alloc::vec::Vec<crate::value::Value>,
) -> Result<(), crate::error::RuntimeError> {
    buffers.recycle_result_locals(locals)
}

/// Report whether a fixture's reserved frame payload is available for entry.
pub fn can_enter_with_frame_payload(buffers: &RegisterExecutionBuffers) -> bool {
    buffers.can_enter_with_frame_payload()
}
