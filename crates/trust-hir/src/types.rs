//! Type system for IEC 61131-3 Structured Text.
//!
//! This module defines all types in the ST type system, including elementary
//! types, compound types, and user-defined types.

mod builtins;
mod compat;
mod defs;
mod registry;

pub use defs::{ArrayDimensionExt, StructField, Type, TypeId, UnionVariant};
pub use registry::TypeRegistry;
