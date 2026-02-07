//! Refactoring helpers for Structured Text.
//!
//! This module provides cross-file refactor primitives that go beyond rename.

mod operations;
mod utilities;

pub(crate) use operations::namespace_full_path;
pub use operations::{
    convert_function_block_to_function, convert_function_to_function_block, extract_method,
    extract_pou, extract_property, generate_interface_stubs, inline_symbol, move_namespace_path,
    parse_namespace_path, ExtractResult, ExtractTargetKind, InlineResult, InlineTargetKind,
};
