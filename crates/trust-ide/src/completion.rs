//! Code completion for Structured Text.
//!
//! This module provides context-aware completion suggestions.

use rustc_hash::FxHashSet;
use smol_str::SmolStr;
use text_size::{TextRange, TextSize};

use trust_hir::db::SemanticDatabase;
use trust_hir::symbols::{ParamDirection, ScopeId, SymbolId, SymbolTable, Visibility};
use trust_hir::{Database, SymbolKind, Type, TypeId};
use trust_syntax::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

use crate::signature_help::call_signature_context;
use crate::stdlib_docs::{self, StdlibFilter};
use crate::util::{
    is_member_symbol_kind, namespace_path_for_symbol, scope_at_position, symbol_is_constant,
    type_detail, using_path_for_symbol, IdeContext, SymbolFilter,
};

include!("completion/types.rs");
include!("completion/context.rs");
include!("completion/engine.rs");
include!("completion/keywords.rs");
include!("completion/symbols.rs");
include!("completion/typed_literals.rs");
include!("completion/tests.rs");
