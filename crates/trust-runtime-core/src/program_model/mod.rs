//! Portable runtime program-model helpers.

#![allow(missing_docs)]

#[cfg(feature = "hir")]
pub mod expr;
#[cfg(feature = "hir")]
pub mod initializers;
pub mod ops;
pub mod util;

#[cfg(feature = "hir")]
pub use expr::{ArgValue, CallArg, Expr, LValue, SizeOfTarget};
#[cfg(feature = "hir")]
pub use initializers::InitializerCatalog;
pub use ops::{apply_binary, apply_unary, BinaryOp, UnaryOp};
pub use util::{method_static_storage_owner, property_setter_method_name, static_storage_name};
