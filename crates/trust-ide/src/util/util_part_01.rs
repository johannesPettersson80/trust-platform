use smol_str::SmolStr;
use std::sync::Arc;
use text_size::{TextRange, TextSize};

use rustc_hash::FxHashSet;
use trust_hir::db::{FileId, SemanticDatabase};
use trust_hir::symbols::{ScopeId, Symbol, SymbolKind, SymbolTable};
use trust_hir::{Database, SourceDatabase, SymbolId, Type, TypeId};
use trust_syntax::parser::parse;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};
use trust_syntax::{lex, TokenKind};

use crate::var_decl::find_var_decl_for_range;

/// Returns whether the symbol represents read-only storage within the entity.
pub(crate) fn symbol_is_constant(symbol: &Symbol) -> bool {
    symbol.is_constant || matches!(symbol.kind, SymbolKind::Constant)
}

/// Finds the enclosing POU (Program Organization Unit) node for a given position.
pub fn find_enclosing_pou(root: &SyntaxNode, offset: TextSize) -> Option<SyntaxNode> {
    let token = root.token_at_offset(offset).right_biased()?;
    token
        .parent_ancestors()
        .find(|node| is_pou_kind(node.kind()))
}

/// Checks if a syntax kind is a POU.
pub fn is_pou_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Program
            | SyntaxKind::Function
            | SyntaxKind::FunctionBlock
            | SyntaxKind::Class
            | SyntaxKind::Method
            | SyntaxKind::Property
            | SyntaxKind::Interface
    )
}

/// Checks if a symbol kind represents a POU.
pub fn is_pou_symbol_kind(kind: &SymbolKind) -> bool {
    matches!(
        kind,
        SymbolKind::Program
            | SymbolKind::Function { .. }
            | SymbolKind::FunctionBlock
            | SymbolKind::Class
            | SymbolKind::Method { .. }
            | SymbolKind::Property { .. }
            | SymbolKind::Interface
    )
}

/// Checks if a symbol kind represents a type declaration.
pub(crate) fn is_type_symbol_kind(kind: &SymbolKind) -> bool {
    matches!(
        kind,
        SymbolKind::Type | SymbolKind::FunctionBlock | SymbolKind::Class | SymbolKind::Interface
    )
}

/// Checks if a symbol kind is a member of a type (field, method, property, etc.).
pub(crate) fn is_member_symbol_kind(kind: &SymbolKind) -> bool {
    matches!(
        kind,
        SymbolKind::Variable { .. }
            | SymbolKind::Constant
            | SymbolKind::Method { .. }
            | SymbolKind::Property { .. }
            | SymbolKind::Function { .. }
    )
}

/// Gets the scope ID for a POU node.
pub fn scope_for_pou(symbols: &SymbolTable, pou_node: &SyntaxNode) -> ScopeId {
    // Get the POU name
    let pou_name = pou_node
        .children()
        .find(|n| n.kind() == SyntaxKind::Name)
        .and_then(|n| {
            n.descendants_with_tokens()
                .filter_map(|e| e.into_token())
                .find(|t| t.kind() == SyntaxKind::Ident)
        })
        .map(|t| t.text().to_string());

    let Some(name) = pou_name else {
        return ScopeId::GLOBAL;
    };

    // Find the symbol for this POU
    let pou_symbol = symbols
        .iter()
        .find(|sym| sym.name.eq_ignore_ascii_case(&name) && is_pou_symbol_kind(&sym.kind));

    let Some(pou_sym) = pou_symbol else {
        return ScopeId::GLOBAL;
    };

    // Find the scope owned by this symbol
    for i in 0..symbols.scope_count() {
        let scope_id = ScopeId(i as u32);
        if let Some(scope) = symbols.get_scope(scope_id) {
            if scope.owner == Some(pou_sym.id) {
                return scope_id;
            }
        }
    }

    ScopeId::GLOBAL
}

/// Finds the scope ID at a given position.
pub fn scope_at_position(symbols: &SymbolTable, root: &SyntaxNode, offset: TextSize) -> ScopeId {
    if let Some(pou_node) = find_enclosing_pou(root, offset) {
        scope_for_pou(symbols, &pou_node)
    } else if let Some(scope_id) = scope_for_namespace(symbols, root, offset) {
        scope_id
    } else {
        ScopeId::GLOBAL
    }
}

/// Finds the identifier at a given offset in the source text.
pub fn ident_at_offset(source: &str, offset: TextSize) -> Option<(&str, TextRange)> {
    let offset = u32::from(offset) as usize;
    let tokens = lex(source);
    if let Some(hit) = ident_match_at_offset(source, &tokens, offset) {
        return Some(hit);
    }

    const MAX_LOOKBACK: usize = 4;
    let bytes = source.as_bytes();
    let mut fallback = offset.min(bytes.len());
    for _ in 0..MAX_LOOKBACK {
        if fallback == 0 {
            break;
        }
        fallback -= 1;
        let byte = bytes[fallback];
        if byte.is_ascii_whitespace() || byte.is_ascii_punctuation() {
            continue;
        }
        if let Some(hit) = ident_match_at_offset(source, &tokens, fallback) {
            return Some(hit);
        }
        break;
    }
    None
}

fn ident_match_at_offset<'a>(
    source: &'a str,
    tokens: &[trust_syntax::Token],
    offset: usize,
) -> Option<(&'a str, TextRange)> {
    for token in tokens {
        if let Some(hit) = ident_match_at(source, token.kind, token.range, offset) {
            return Some(hit);
        }
    }
    None
}

fn ident_match_at(
    source: &str,
    kind: TokenKind,
    range: TextRange,
    offset: usize,
) -> Option<(&str, TextRange)> {
    let start = usize::from(range.start());
    let end = usize::from(range.end());
    if start > offset || offset >= end {
        return None;
    }

    match kind {
        TokenKind::Ident => Some((&source[start..end], range)),
        // `E_State#Running` is lexed as a typed-literal prefix token (`E_State#`)
        // plus an identifier value; we treat the prefix name as a navigable symbol.
        TokenKind::TypedLiteralPrefix if end > start + 1 => {
            let name_end = end - 1;
            let name_range = TextRange::new(
                TextSize::from(start as u32),
                TextSize::from(name_end as u32),
            );
            if offset < name_end {
                Some((&source[start..name_end], name_range))
            } else {
                None
            }
        }
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FieldTarget {
    pub(crate) type_id: TypeId,
    pub(crate) name: SmolStr,
    pub(crate) type_name: Option<SmolStr>,
}

#[derive(Debug, Clone)]
pub(crate) enum ResolvedTarget {
    Symbol(SymbolId),
    Field(FieldTarget),
}

pub(crate) struct IdeContext<'a> {
    pub(crate) db: &'a Database,
    pub(crate) file_id: FileId,
    pub(crate) source: Arc<String>,
    pub(crate) root: SyntaxNode,
    pub(crate) symbols: Arc<SymbolTable>,
}

impl<'a> IdeContext<'a> {
    pub(crate) fn new(db: &'a Database, file_id: FileId) -> Self {
        let source = db.source_text(file_id);
        let parsed = parse(&source);
        let root = parsed.syntax();
        let symbols = db.file_symbols_with_project(file_id);
        Self {
            db,
            file_id,
            source,
            root,
            symbols,
        }
    }

    pub(crate) fn resolve_target_at_position(&self, position: TextSize) -> Option<ResolvedTarget> {
        resolve_target_at_position_with_context(
            self.db,
            self.file_id,
            position,
            &self.source,
            &self.root,
            &self.symbols,
        )
    }

    pub(crate) fn scope_at_position(&self, position: TextSize) -> ScopeId {
        scope_at_position(&self.symbols, &self.root, position)
    }
}
