//! Portable runtime program-model helpers.

#![allow(missing_docs)]

pub mod ops;
pub mod util;

pub use ops::{apply_binary, apply_unary, BinaryOp, UnaryOp};
pub use util::{method_static_storage_owner, property_setter_method_name, static_storage_name};
