//! Type system for IEC 61131-3 Structured Text.
//!
//! This module defines all types in the ST type system, including elementary
//! types, compound types, and user-defined types.

mod builtins;
mod compat;
mod defs;
mod registry;

pub use defs::{
    ArrayDimensionExt, InitializerCatalog, InitializerId, InitializerRecord, StructField, Type,
    TypeId, UnionVariant,
};
pub use registry::TypeRegistry;

/// User-visible `SIZEOF` result for `POINTER TO` / `REF_TO` storage operands.
///
/// This follows platform pointer width (`usize`) rather than truST's internal
/// runtime handle layout so migrations match common ST/CODESYS expectations.
pub const POINTER_REFERENCE_HANDLE_SIZE_BYTES: u64 = std::mem::size_of::<usize>() as u64;
