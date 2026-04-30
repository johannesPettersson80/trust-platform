use super::calls::{NameLookupResult, NameResolveOutcome, ResolvedSymbol};
use super::*;
use crate::semantic::{QualifiedName, SemanticOutcome, SemanticRole};

impl<'a, 'b> ResolveChecker<'a, 'b> {
    pub(super) fn resolve_lvalue_root(&mut self, node: &SyntaxNode) -> Option<ResolvedSymbol> {
        let root = self.checker.resolve_ref().lvalue_root_name_ref(node)?;
        let name = self.checker.resolve_ref().get_name_from_ref(&root)?;
        match self.resolve_name_in_context_outcome(&name, root.text_range()) {
            NameResolveOutcome::Resolved(resolved) => Some(resolved),
            NameResolveOutcome::Ambiguous | NameResolveOutcome::NotFound => None,
        }
    }
}

impl<'a, 'b> ResolveCheckerRef<'a, 'b> {
    pub(super) fn get_name_from_ref(&self, node: &SyntaxNode) -> Option<SmolStr> {
        for token in node
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
        {
            if matches!(
                token.kind(),
                SyntaxKind::Ident
                    | SyntaxKind::KwEn
                    | SyntaxKind::KwEno
                    | SyntaxKind::KwGet
                    | SyntaxKind::KwSet
                    | SyntaxKind::KwRef
                    | SyntaxKind::KwNew
                    | SyntaxKind::KwNewDunder
                    | SyntaxKind::KwDeleteDunder
            ) {
                return Some(SmolStr::new(token.text()));
            }
        }
        None
    }

    pub(super) fn lvalue_root_name_ref(&self, node: &SyntaxNode) -> Option<SyntaxNode> {
        match node.kind() {
            SyntaxKind::NameRef => Some(node.clone()),
            SyntaxKind::ParenExpr => node
                .children()
                .next()
                .and_then(|inner| self.lvalue_root_name_ref(&inner)),
            SyntaxKind::FieldExpr | SyntaxKind::IndexExpr => node
                .children()
                .next()
                .and_then(|base| self.lvalue_root_name_ref(&base)),
            SyntaxKind::DerefExpr => None,
            _ => None,
        }
    }

    pub(in crate::type_check) fn resolve_simple_symbol(
        &self,
        node: &SyntaxNode,
    ) -> Option<SymbolId> {
        if node.kind() == SyntaxKind::ParenExpr {
            if let Some(inner) = node.children().next() {
                return self.resolve_simple_symbol(&inner);
            }
        }

        if node.kind() != SyntaxKind::NameRef {
            return None;
        }

        let name = self.get_name_from_ref(node)?;
        self.checker
            .symbols
            .resolve(&name, self.checker.current_scope)
    }

    pub(super) fn resolve_type_from_expr_outcome(
        &self,
        node: &SyntaxNode,
    ) -> SemanticOutcome<TypeId> {
        match node.kind() {
            SyntaxKind::NameRef => {
                let Some(name) = self.get_name_from_ref(node) else {
                    return SemanticOutcome::Unknown {
                        name: None,
                        range: Some(node.text_range()),
                    };
                };
                self.resolve_type_by_name_outcome(name.as_str(), Some(node.text_range()))
            }
            SyntaxKind::FieldExpr => {
                let Some(parts) = self.qualified_name_from_field_expr(node) else {
                    return SemanticOutcome::Unknown {
                        name: None,
                        range: Some(node.text_range()),
                    };
                };
                let Some(symbol_id) = self.checker.symbols.resolve_qualified(&parts) else {
                    return SemanticOutcome::Unknown {
                        name: QualifiedName::new(parts),
                        range: Some(node.text_range()),
                    };
                };
                let Some(symbol) = self.checker.symbols.get(symbol_id) else {
                    return SemanticOutcome::InvariantViolation {
                        message: SmolStr::new("resolved type symbol is missing from table"),
                        range: Some(node.text_range()),
                    };
                };
                if symbol.is_type() {
                    SemanticOutcome::Resolved(symbol.type_id)
                } else {
                    SemanticOutcome::WrongKind {
                        symbol_id,
                        expected: SemanticRole::Type,
                        actual: semantic_role_for_symbol_kind(&symbol.kind),
                        range: Some(node.text_range()),
                    }
                }
            }
            SyntaxKind::ParenExpr => node
                .children()
                .next()
                .map(|child| self.resolve_type_from_expr_outcome(&child))
                .unwrap_or(SemanticOutcome::Unknown {
                    name: None,
                    range: Some(node.text_range()),
                }),
            _ => SemanticOutcome::Unknown {
                name: None,
                range: Some(node.text_range()),
            },
        }
    }

    pub(super) fn resolve_type_by_name(&self, name: &str) -> Option<TypeId> {
        match self.resolve_type_by_name_outcome(name, None) {
            SemanticOutcome::Resolved(type_id) => Some(type_id),
            _ => None,
        }
    }

    pub(super) fn resolve_type_by_name_outcome(
        &self,
        name: &str,
        range: Option<TextRange>,
    ) -> SemanticOutcome<TypeId> {
        if let Some(id) = TypeId::from_builtin_name(name) {
            return SemanticOutcome::Resolved(id);
        }
        if name.contains('.') {
            let parts: Vec<SmolStr> = name.split('.').map(SmolStr::new).collect();
            let Some(symbol_id) = self.checker.symbols.resolve_qualified(&parts) else {
                return SemanticOutcome::Unknown {
                    name: QualifiedName::new(parts),
                    range,
                };
            };
            let Some(symbol) = self.checker.symbols.get(symbol_id) else {
                return SemanticOutcome::InvariantViolation {
                    message: SmolStr::new("resolved type symbol is missing from table"),
                    range,
                };
            };
            if symbol.is_type() {
                return SemanticOutcome::Resolved(symbol.type_id);
            }
            return SemanticOutcome::WrongKind {
                symbol_id,
                expected: SemanticRole::Type,
                actual: semantic_role_for_symbol_kind(&symbol.kind),
                range,
            };
        }

        match self.lookup_name_symbol(name) {
            NameLookupResult::Found(symbol_id) => {
                let Some(symbol) = self.checker.symbols.get(symbol_id) else {
                    return SemanticOutcome::InvariantViolation {
                        message: SmolStr::new("resolved type symbol is missing from table"),
                        range,
                    };
                };
                if symbol.is_type() {
                    return SemanticOutcome::Resolved(symbol.type_id);
                }
                return SemanticOutcome::WrongKind {
                    symbol_id,
                    expected: SemanticRole::Type,
                    actual: semantic_role_for_symbol_kind(&symbol.kind),
                    range,
                };
            }
            NameLookupResult::Ambiguous => {
                return SemanticOutcome::Ambiguous {
                    name: QualifiedName::new(vec![SmolStr::new(name)])
                        .expect("single-part qualified name"),
                    range,
                };
            }
            NameLookupResult::NotFound => {}
        }

        if let Some(type_id) = self.checker.symbols.lookup_registered_type_name(name) {
            return SemanticOutcome::Resolved(type_id);
        }

        SemanticOutcome::Unknown {
            name: QualifiedName::new(vec![SmolStr::new(name)]),
            range,
        }
    }
}

fn semantic_role_for_symbol_kind(kind: &SymbolKind) -> SemanticRole {
    match kind {
        SymbolKind::Namespace => SemanticRole::Namespace,
        SymbolKind::Function { .. } | SymbolKind::Method { .. } => SemanticRole::Callable,
        SymbolKind::Type
        | SymbolKind::FunctionBlock
        | SymbolKind::Class
        | SymbolKind::Interface => SemanticRole::Type,
        SymbolKind::Program
        | SymbolKind::Configuration
        | SymbolKind::Resource
        | SymbolKind::Task
        | SymbolKind::ProgramInstance => SemanticRole::ScopeOwner,
        _ => SemanticRole::Value,
    }
}
