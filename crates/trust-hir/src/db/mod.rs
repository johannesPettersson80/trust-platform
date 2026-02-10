//! Semantic database and query traits.
//!
//! This module uses salsa-backed incremental computation for source text,
//! parse, and semantic query families (`file_symbols`, `analyze`,
//! diagnostics, `type_of`).

use rustc_hash::{FxHashMap, FxHashSet};
use smol_str::SmolStr;
use std::sync::Arc;
use text_size::{TextRange, TextSize};

use crate::diagnostics::{Diagnostic, DiagnosticBuilder, DiagnosticCode};
use crate::ident::{is_reserved_keyword, is_valid_identifier};
use crate::symbols::{
    ParamDirection, ScopeId, ScopeKind, Symbol, SymbolId, SymbolKind, SymbolModifiers,
    SymbolOrigin, SymbolTable, VarQualifier, Visibility,
};
use crate::type_check::{string_literal_info, TypeChecker};
use crate::types::{Type, TypeId};
use trust_syntax::parser::parse;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

mod diagnostics;
mod queries;
mod symbol_import;

pub use queries::{Database, FileId, SalsaEventSnapshot, SemanticDatabase, SourceDatabase};
