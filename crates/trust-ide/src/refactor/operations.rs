//! Refactoring helpers for Structured Text.
//!
//! This module provides cross-file refactor primitives that go beyond rename.

use rustc_hash::{FxHashMap, FxHashSet};
use smol_str::SmolStr;
use text_size::{TextRange, TextSize};

use trust_hir::db::{FileId, SemanticDatabase};
use trust_hir::symbols::{SymbolKind, SymbolTable};
use trust_hir::{is_reserved_keyword, is_valid_identifier, Database, SourceDatabase, SymbolId};
use trust_syntax::parser::parse;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

use super::utilities;
use crate::references::{find_references, FindReferencesOptions};
use crate::rename::{RenameResult, TextEdit};
use crate::util::{
    ident_token_in_name, is_type_name_node, name_from_name_node, qualified_name_from_field_expr,
    qualified_name_parts_from_node, resolve_target_at_position,
    resolve_target_at_position_with_context, resolve_type_symbol_at_node, symbol_is_constant,
    ResolvedTarget,
};

/// Result of an inline refactor request.
#[derive(Debug, Clone)]
pub struct InlineResult {
    /// Edits required to inline the target.
    pub edits: RenameResult,
    /// The inline target name.
    pub name: SmolStr,
    /// The inline target kind.
    pub kind: InlineTargetKind,
}

struct InlineExprInfo {
    text: String,
    kind: SyntaxKind,
    is_const_expr: bool,
    is_path_like: bool,
    requires_local_scope: bool,
}

/// The inline target kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineTargetKind {
    /// Inline a variable.
    Variable,
    /// Inline a constant.
    Constant,
}

/// Result of an extract refactor request.
#[derive(Debug, Clone)]
pub struct ExtractResult {
    /// Edits required to perform the extraction.
    pub edits: RenameResult,
    /// The extracted symbol name.
    pub name: SmolStr,
    /// The extracted symbol kind.
    pub kind: ExtractTargetKind,
}

/// The extract target kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractTargetKind {
    /// Extract a METHOD.
    Method,
    /// Extract a PROPERTY (GET-only).
    Property,
    /// Extract a FUNCTION (POU).
    Function,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExtractParamDirection {
    Input,
    InOut,
}

#[derive(Debug, Clone)]
struct ExtractParam {
    name: SmolStr,
    type_name: SmolStr,
    direction: ExtractParamDirection,
    first_pos: TextSize,
}

include!("operations/core_refactors.rs");
include!("operations/namespace_and_extract_support.rs");
include!("operations/extract_and_range_helpers.rs");
include!("operations/convert_callsite_updates.rs");
include!("operations/inline_and_namespace_helpers.rs");
include!("operations/tests.rs");
