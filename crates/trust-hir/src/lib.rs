//! `trust-hir` - High-level IR and semantic analysis for IEC 61131-3 Structured Text.
//!
//! This crate provides semantic analysis built on top of `trust-syntax`:
//!
//! - **Name Resolution**: Resolving identifiers to their declarations
//! - **Type Checking**: Validating type correctness
//! - **Symbol Table**: Queryable database of all declarations
//! - **Diagnostics**: Semantic error reporting
//!
//! # Architecture
//!
//! The crate uses `salsa` for incremental computation.
//! Source/parse/file-symbol queries and semantic query families
//! (`analyze`, diagnostics, and `type_of`) are Salsa-backed.
//!
//! # Example
//!
//! ```ignore
//! use trust_hir::{Database, SourceDatabase};
//!
//! let mut db = Database::default();
//! db.set_source_text(file_id, source.to_string());
//!
//! let symbols = db.file_symbols(file_id);
//! let diagnostics = db.diagnostics(file_id);
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

pub mod db;
pub mod diagnostics;
pub mod ident;
/// Project-wide source registry and database helpers.
pub mod project;
pub mod symbols;
pub mod type_check;
pub mod types;

pub use db::{Database, SourceDatabase};
pub use diagnostics::{Diagnostic, DiagnosticSeverity};
pub use ident::{is_reserved_keyword, is_valid_identifier};
pub use project::{Project, SourceKey, SourceRegistry};
pub use symbols::{Symbol, SymbolId, SymbolKind};
pub use type_check::TypeChecker;
pub use types::{Type, TypeId};
