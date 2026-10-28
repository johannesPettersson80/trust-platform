//! The declarations of the standard function blocks, which the program library implements
//! (specification 44 §6.8).

mod registry;

pub use registry::standard_function_blocks;
pub use trust_runtime_core::stdlib::fbs::{builtin_kind, builtin_kind_uppercase, BuiltinFbKind};
