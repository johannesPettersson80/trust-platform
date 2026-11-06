//! Symbol table and symbol definitions.
//!
//! This module provides the symbol table that tracks all declarations
//! in a Structured Text program.

mod builtins;
mod defs;
mod helpers;
mod strings;
mod table;

pub use builtins::{STANDARD_FUNCTION_BLOCKS, STANDARD_FUNCTION_BLOCK_ALIASES};
pub use defs::*;
pub(crate) use table::UniqueSymbolResolution;
pub use table::{EnumValueResolution, SymbolTable};
