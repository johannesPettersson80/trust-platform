//! Semantic tokens for Structured Text.
//!
//! This module provides semantic token computation for rich syntax highlighting.

use text_size::{TextRange, TextSize};

use trust_hir::db::{FileId, SemanticDatabase};
use trust_hir::symbols::{SymbolKind, SymbolTable};
use trust_hir::{Database, SourceDatabase};
use trust_syntax::parser::parse;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};
use trust_syntax::{lex, TokenKind};

use crate::util::{
    resolve_target_at_position_with_context, scope_at_position, symbol_is_constant, ResolvedTarget,
    SymbolFilter,
};

/// Semantic token types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticTokenType {
    /// A keyword.
    Keyword,
    /// A type name.
    Type,
    /// A variable.
    Variable,
    /// A property.
    Property,
    /// A method.
    Method,
    /// A function.
    Function,
    /// A parameter.
    Parameter,
    /// A number literal.
    Number,
    /// A string literal.
    String,
    /// A comment.
    Comment,
    /// An operator.
    Operator,
    /// An enum member.
    EnumMember,
    /// A namespace.
    Namespace,
}

/// Semantic token modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SemanticTokenModifiers {
    /// This is a declaration.
    pub declaration: bool,
    /// This is a definition.
    pub definition: bool,
    /// This is readonly (constant).
    pub readonly: bool,
    /// This is static.
    pub is_static: bool,
    /// This is a modification (write).
    pub modification: bool,
}

/// A semantic token.
#[derive(Debug, Clone)]
pub struct SemanticToken {
    /// The range of the token.
    pub range: TextRange,
    /// The token type.
    pub token_type: SemanticTokenType,
    /// The token modifiers.
    pub modifiers: SemanticTokenModifiers,
}

impl SemanticToken {
    /// Creates a new semantic token.
    pub fn new(range: TextRange, token_type: SemanticTokenType) -> Self {
        Self {
            range,
            token_type,
            modifiers: SemanticTokenModifiers::default(),
        }
    }

    /// Adds the declaration modifier.
    #[must_use]
    pub fn declaration(mut self) -> Self {
        self.modifiers.declaration = true;
        self
    }

    /// Adds the readonly modifier.
    #[must_use]
    pub fn readonly(mut self) -> Self {
        self.modifiers.readonly = true;
        self
    }
}

/// Maps a `SymbolKind` to a `SemanticTokenType`.
fn symbol_kind_to_token_type(kind: &SymbolKind) -> SemanticTokenType {
    match kind {
        SymbolKind::Program => SemanticTokenType::Namespace,
        SymbolKind::Configuration => SemanticTokenType::Namespace,
        SymbolKind::Resource => SemanticTokenType::Namespace,
        SymbolKind::Task => SemanticTokenType::Function,
        SymbolKind::ProgramInstance => SemanticTokenType::Variable,
        SymbolKind::Namespace => SemanticTokenType::Namespace,
        SymbolKind::Function { .. } => SemanticTokenType::Function,
        SymbolKind::FunctionBlock => SemanticTokenType::Type,
        SymbolKind::Class => SemanticTokenType::Type,
        SymbolKind::Method { .. } => SemanticTokenType::Method,
        SymbolKind::Property { .. } => SemanticTokenType::Property,
        SymbolKind::Interface => SemanticTokenType::Type,
        SymbolKind::Variable { .. } => SemanticTokenType::Variable,
        SymbolKind::Constant => SemanticTokenType::Variable, // with readonly modifier
        SymbolKind::Type => SemanticTokenType::Type,
        SymbolKind::EnumValue { .. } => SemanticTokenType::EnumMember,
        SymbolKind::Parameter { .. } => SemanticTokenType::Parameter,
    }
}

/// Classifies an identifier token based on semantic analysis.
fn classify_identifier(
    db: &Database,
    file_id: FileId,
    source: &str,
    symbols: &SymbolTable,
    root: &SyntaxNode,
    name: &str,
    range: TextRange,
) -> (SemanticTokenType, SemanticTokenModifiers) {
    let mut modifiers = SemanticTokenModifiers::default();
    let offset = range.start();

    let filter = SymbolFilter::new(symbols);

    // Check if this is a declaration site (range matches a symbol's range)
    if let Some(symbol) = filter.symbol_at_range(range) {
        modifiers.declaration = true;
        if symbol_is_constant(symbol) {
            modifiers.readonly = true;
        }
        return (symbol_kind_to_token_type(&symbol.kind), modifiers);
    }

    let is_field_decl = is_struct_field_declaration(root, offset);
    let is_field_member = is_field_expr_member(root, offset);
    if is_field_decl || is_field_member {
        if let Some(target) =
            resolve_target_at_position_with_context(db, file_id, offset, source, root, symbols)
        {
            match target {
                ResolvedTarget::Symbol(symbol_id) => {
                    if let Some(symbol) = symbols.get(symbol_id) {
                        if symbol_is_constant(symbol) {
                            modifiers.readonly = true;
                        }
                        if symbol.range == range {
                            modifiers.declaration = true;
                        }
                        return (symbol_kind_to_token_type(&symbol.kind), modifiers);
                    }
                }
                ResolvedTarget::Field(_) => {
                    if is_field_decl {
                        modifiers.declaration = true;
                    }
                    return (SemanticTokenType::Property, modifiers);
                }
            }
        } else if is_field_decl {
            modifiers.declaration = true;
            return (SemanticTokenType::Property, modifiers);
        }
    }

    // Find the scope at this position and resolve the name
    let scope_id = scope_at_position(symbols, root, offset);
    if let Some(symbol) = filter.resolve_in_scope(name, scope_id) {
        if symbol_is_constant(symbol) {
            modifiers.readonly = true;
        }
        return (symbol_kind_to_token_type(&symbol.kind), modifiers);
    }

    // Fallback: try global lookup
    if let Some(symbol) = filter.lookup_global(name) {
        if symbol_is_constant(symbol) {
            modifiers.readonly = true;
        }
        return (symbol_kind_to_token_type(&symbol.kind), modifiers);
    }

    // If in a type position, classify as Type even if unresolved
    if is_type_position(root, offset) {
        return (SemanticTokenType::Type, modifiers);
    }

    // Unknown identifier - default to variable
    (SemanticTokenType::Variable, modifiers)
}

fn is_struct_field_declaration(root: &SyntaxNode, offset: TextSize) -> bool {
    let Some(token) = root.token_at_offset(offset).right_biased() else {
        return false;
    };
    let Some(name_node) = token
        .parent_ancestors()
        .find(|node| node.kind() == SyntaxKind::Name)
    else {
        return false;
    };
    let Some(var_decl) = name_node.parent() else {
        return false;
    };
    if var_decl.kind() != SyntaxKind::VarDecl {
        return false;
    }
    var_decl.ancestors().any(|ancestor| {
        matches!(
            ancestor.kind(),
            SyntaxKind::StructDef | SyntaxKind::UnionDef
        )
    })
}

fn is_field_expr_member(root: &SyntaxNode, offset: TextSize) -> bool {
    let Some(token) = root.token_at_offset(offset).right_biased() else {
        return false;
    };
    let Some(name_node) = token
        .parent_ancestors()
        .find(|node| matches!(node.kind(), SyntaxKind::Name | SyntaxKind::NameRef))
    else {
        return false;
    };
    let Some(field_expr) = name_node.parent() else {
        return false;
    };
    if field_expr.kind() != SyntaxKind::FieldExpr {
        return false;
    }
    let mut children = field_expr.children();
    let _base = children.next();
    let member = children.next();
    matches!(member, Some(node) if node == name_node)
}

/// Checks if an identifier is in a type position (after colon or in type reference).
fn is_type_position(root: &SyntaxNode, offset: TextSize) -> bool {
    let Some(token) = root.token_at_offset(offset).right_biased() else {
        return false;
    };

    for ancestor in token.parent_ancestors() {
        match ancestor.kind() {
            SyntaxKind::TypeRef => return true,
            SyntaxKind::ExtendsClause | SyntaxKind::ImplementsClause => return true,
            _ => {}
        }
    }
    false
}

/// Computes semantic tokens for a file.
pub fn semantic_tokens(db: &Database, file_id: FileId) -> Vec<SemanticToken> {
    let source = db.source_text(file_id);
    let tokens = lex(&source);
    let parsed = parse(&source);
    let root = parsed.syntax();
    let symbols = db.file_symbols(file_id);

    let mut result = Vec::new();

    for token in tokens {
        let semantic_type = match token.kind {
            // Keywords
            TokenKind::KwProgram
            | TokenKind::KwEndProgram
            | TokenKind::KwFunction
            | TokenKind::KwEndFunction
            | TokenKind::KwFunctionBlock
            | TokenKind::KwEndFunctionBlock
            | TokenKind::KwClass
            | TokenKind::KwEndClass
            | TokenKind::KwMethod
            | TokenKind::KwEndMethod
            | TokenKind::KwProperty
            | TokenKind::KwEndProperty
            | TokenKind::KwInterface
            | TokenKind::KwEndInterface
            | TokenKind::KwVar
            | TokenKind::KwEndVar
            | TokenKind::KwVarInput
            | TokenKind::KwVarOutput
            | TokenKind::KwVarInOut
            | TokenKind::KwVarTemp
            | TokenKind::KwVarGlobal
            | TokenKind::KwVarExternal
            | TokenKind::KwVarAccess
            | TokenKind::KwVarConfig
            | TokenKind::KwVarStat
            | TokenKind::KwConstant
            | TokenKind::KwRetain
            | TokenKind::KwNonRetain
            | TokenKind::KwPersistent
            | TokenKind::KwIf
            | TokenKind::KwThen
            | TokenKind::KwElsif
            | TokenKind::KwElse
            | TokenKind::KwEndIf
            | TokenKind::KwCase
            | TokenKind::KwEndCase
            | TokenKind::KwFor
            | TokenKind::KwTo
            | TokenKind::KwBy
            | TokenKind::KwDo
            | TokenKind::KwEndFor
            | TokenKind::KwWhile
            | TokenKind::KwEndWhile
            | TokenKind::KwRepeat
            | TokenKind::KwUntil
            | TokenKind::KwEndRepeat
            | TokenKind::KwReturn
            | TokenKind::KwExit
            | TokenKind::KwContinue
            | TokenKind::KwJmp
            | TokenKind::KwStep
            | TokenKind::KwEndStep
            | TokenKind::KwInitialStep
            | TokenKind::KwTransition
            | TokenKind::KwEndTransition
            | TokenKind::KwFrom
            | TokenKind::KwAnd
            | TokenKind::KwOr
            | TokenKind::KwXor
            | TokenKind::KwNot
            | TokenKind::KwMod
            | TokenKind::KwTrue
            | TokenKind::KwFalse
            | TokenKind::KwConfiguration
            | TokenKind::KwEndConfiguration
            | TokenKind::KwResource
            | TokenKind::KwEndResource
            | TokenKind::KwOn
            | TokenKind::KwReadOnly
            | TokenKind::KwReadWrite
            | TokenKind::KwExtends
            | TokenKind::KwImplements
            | TokenKind::KwThis
            | TokenKind::KwSuper
            | TokenKind::KwPublic
            | TokenKind::KwPrivate
            | TokenKind::KwProtected
            | TokenKind::KwNamespace
            | TokenKind::KwEndNamespace
            | TokenKind::KwUsing
            | TokenKind::KwAction
            | TokenKind::KwEndAction
            | TokenKind::KwGet
            | TokenKind::KwSet
            | TokenKind::KwEndGet
            | TokenKind::KwEndSet
            | TokenKind::KwTask
            | TokenKind::KwWith
            | TokenKind::KwAt
            | TokenKind::KwEn
            | TokenKind::KwEno
            | TokenKind::KwREdge
            | TokenKind::KwFEdge
            | TokenKind::KwAdr
            | TokenKind::KwSizeOf => Some(SemanticTokenType::Keyword),

            // Type keywords
            TokenKind::KwBool
            | TokenKind::KwSInt
            | TokenKind::KwInt
            | TokenKind::KwDInt
            | TokenKind::KwLInt
            | TokenKind::KwUSInt
            | TokenKind::KwUInt
            | TokenKind::KwUDInt
            | TokenKind::KwULInt
            | TokenKind::KwReal
            | TokenKind::KwLReal
            | TokenKind::KwByte
            | TokenKind::KwWord
            | TokenKind::KwDWord
            | TokenKind::KwLWord
            | TokenKind::KwTime
            | TokenKind::KwLTime
            | TokenKind::KwDate
            | TokenKind::KwLDate
            | TokenKind::KwTimeOfDay
            | TokenKind::KwLTimeOfDay
            | TokenKind::KwDateAndTime
            | TokenKind::KwLDateAndTime
            | TokenKind::KwString
            | TokenKind::KwWString
            | TokenKind::KwChar
            | TokenKind::KwWChar
            | TokenKind::KwArray
            | TokenKind::KwOf
            | TokenKind::KwPointer
            | TokenKind::KwRef
            | TokenKind::KwRefTo
            | TokenKind::KwAny
            | TokenKind::KwAnyDerived
            | TokenKind::KwAnyElementary
            | TokenKind::KwAnyMagnitude
            | TokenKind::KwAnyInt
            | TokenKind::KwAnyUnsigned
            | TokenKind::KwAnySigned
            | TokenKind::KwAnyReal
            | TokenKind::KwAnyNum
            | TokenKind::KwAnyDuration
            | TokenKind::KwAnyBit
            | TokenKind::KwAnyChars
            | TokenKind::KwAnyString
            | TokenKind::KwAnyChar
            | TokenKind::KwAnyDate => Some(SemanticTokenType::Type),

            // Literals
            TokenKind::IntLiteral | TokenKind::RealLiteral => Some(SemanticTokenType::Number),
            TokenKind::StringLiteral
            | TokenKind::WideStringLiteral
            | TokenKind::TimeLiteral
            | TokenKind::DateLiteral
            | TokenKind::TimeOfDayLiteral
            | TokenKind::DateAndTimeLiteral => Some(SemanticTokenType::String),

            // Comments
            TokenKind::LineComment | TokenKind::BlockComment => Some(SemanticTokenType::Comment),

            // Operators
            TokenKind::Assign
            | TokenKind::Eq
            | TokenKind::Neq
            | TokenKind::Lt
            | TokenKind::LtEq
            | TokenKind::Gt
            | TokenKind::GtEq
            | TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Star
            | TokenKind::Slash
            | TokenKind::Power
            | TokenKind::Ampersand => Some(SemanticTokenType::Operator),

            // Identifiers - use semantic analysis
            TokenKind::Ident => {
                let name = &source[token.range];
                let (token_type, mods) =
                    classify_identifier(db, file_id, &source, &symbols, &root, name, token.range);
                result.push(SemanticToken {
                    range: token.range,
                    token_type,
                    modifiers: mods,
                });
                continue; // Skip the generic handling below
            }

            _ => None,
        };

        if let Some(token_type) = semantic_type {
            result.push(SemanticToken::new(token.range, token_type));
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_token_creation() {
        let range = TextRange::new(0.into(), 5.into());
        let token = SemanticToken::new(range, SemanticTokenType::Keyword);

        assert_eq!(token.token_type, SemanticTokenType::Keyword);
        assert!(!token.modifiers.declaration);
    }

    #[test]
    fn test_semantic_token_modifiers() {
        let range = TextRange::new(0.into(), 5.into());
        let token = SemanticToken::new(range, SemanticTokenType::Variable)
            .declaration()
            .readonly();

        assert!(token.modifiers.declaration);
        assert!(token.modifiers.readonly);
    }
}
