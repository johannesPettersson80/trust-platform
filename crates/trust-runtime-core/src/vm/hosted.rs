//! Compatibility boundary for the source-backed hosted runtime and register tiers.
//!
//! Enabled only with `hir`. These hooks let the existing hosted runtime supply
//! storage, clock, debugging and optimized execution to the single shared VM.
//! They do not admit source-free applications: use `PreparedModule` and
//! `RuntimeState` at the parent module for that boundary. Legacy fixture APIs
//! intentionally permit malformed instruction bodies for verifier tests, but
//! reject STBC 2.0 and cannot create a prepared admission token.

/// Hosted execution fuel and cooperative deadline policy.
pub mod budget {
    pub use super::super::budget::*;
}
/// Shared call binding and native dispatch hooks.
pub mod call {
    pub use super::super::call::{
        execute_native_call, push_call_frame, VM_LOCAL_SENTINEL_FRAME_ID,
    };
    /// Hosted parameter binding, reference targets and output copy-back.
    pub mod bindings {
        pub use super::super::super::call::bindings::*;
    }
    /// Services supplied by the hosted call adapter.
    pub mod context {
        pub use super::super::super::call::context::*;
    }
    /// Shared standard-function dispatch and output helpers.
    pub mod stdlib {
        pub use super::super::super::call::stdlib::*;
    }
}
/// Storage, lifecycle and execution services implemented by the host.
pub mod context {
    pub use super::super::context::*;
}
/// Shared stack interpreter entry points and reusable buffers.
pub mod dispatch {
    pub use super::super::dispatch::*;
}
/// Checked shared reference instructions used by hosted optimized tiers.
pub mod dispatch_refs {
    pub use super::super::dispatch_refs::*;
}
/// Decoded debugger source and symbol metadata.
pub mod debug_map {
    pub use super::super::debug_map::*;
}
/// Transactional edge-parameter input handling.
pub mod edge {
    pub use super::super::edge::*;
}
/// Immutable legacy execution metadata and explicit legacy fixture construction.
pub mod module {
    pub use super::super::module::*;
}
/// Legacy materialization bounds shared with register-tier fixtures.
pub mod materialization_limits {
    pub use super::super::materialization_limits::*;
}
/// Native symbol descriptor parsing and binding.
pub mod symbols {
    pub use super::super::symbols::*;
}
/// Shared declared-type assignment and reference compatibility rules.
pub mod type_policy {
    pub use super::super::type_policy::*;
}

pub use super::const_pool::decode_const_pool_entries;
pub use super::dispatch_ops::{apply_jump, execute_binary, execute_unary, read_i32, read_u32};
pub use super::frames::{ensure_global_call_depth, FrameStack, VmFrame};
pub use super::helpers::{materialize_borrowed_value, opcode_operand_len};
pub use super::stack::OperandStack;
pub use super::{
    sizeof_type_from_table, sizeof_type_from_table_with, VmTrap, VM_MAX_CALL_DEPTH,
    VM_MAX_EXECUTED_INSTRUCTIONS, VM_MAX_OPERAND_STACK,
};
