//! Hover information for Structured Text.
//!
//! This module provides hover functionality to display type information
//! and documentation when hovering over symbols.

use text_size::{TextRange, TextSize};

use smol_str::SmolStr;
use trust_hir::db::SemanticDatabase;
use trust_hir::diagnostics::DiagnosticCode;
use trust_hir::symbols::{ScopeId, SymbolModifiers, SymbolTable, VarQualifier, Visibility};
use trust_hir::types::ArrayDimensionExt;
use trust_hir::{Database, SourceDatabase, Symbol, SymbolKind, Type, TypeId};
use trust_syntax::parser::parse;
use trust_syntax::syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};
use trust_syntax::{lex, TokenKind};

use crate::signature_help::signature_help;
use crate::stdlib_docs::{self, StdlibFilter};
use crate::util::{
    field_type, ident_at_offset, ident_token_in_name, name_range_from_node,
    namespace_path_for_symbol, scope_at_position, symbol_is_constant, using_path_for_symbol,
    IdeContext, ResolvedTarget, SymbolFilter,
};
use crate::var_decl::{var_decl_info_for_name, var_decl_info_for_symbol};

include!("hover/resolution_and_render.rs");
include!("hover/type_and_stdlib.rs");
include!("hover/config_and_tests.rs");
