use super::calls::NameResolveOutcome;
use super::calls::ResolvedSymbol;
use super::*;
use crate::symbols::Symbol;

impl<'a> TypeChecker<'a> {
    fn symbol_is_constant(&self, symbol: &Symbol) -> bool {
        symbol.is_constant
            || matches!(
                symbol.kind,
                SymbolKind::Constant | SymbolKind::EnumValue { .. }
            )
    }

    pub(super) fn is_valid_lvalue(&self, node: &SyntaxNode) -> bool {
        if node.kind() == SyntaxKind::ParenExpr {
            if let Some(inner) = node.children().next() {
                return self.is_valid_lvalue(&inner);
            }
        }
        match node.kind() {
            SyntaxKind::NameRef | SyntaxKind::DerefExpr => true,
            SyntaxKind::ThisExpr | SyntaxKind::SuperExpr => false,
            SyntaxKind::FieldExpr | SyntaxKind::IndexExpr => node
                .children()
                .next()
                .map(|base| {
                    matches!(base.kind(), SyntaxKind::ThisExpr | SyntaxKind::SuperExpr)
                        || self.is_valid_lvalue(&base)
                })
                .unwrap_or(false),
            _ => false,
        }
    }

    pub(super) fn is_constant_target(&mut self, node: &SyntaxNode) -> bool {
        self.is_constant_target_with_resolved(node, None)
    }

    pub(super) fn is_constant_target_with_resolved(
        &mut self,
        node: &SyntaxNode,
        resolved: Option<&ResolvedSymbol>,
    ) -> bool {
        if node.kind() == SyntaxKind::ParenExpr {
            if let Some(inner) = node.children().next() {
                return self.is_constant_target_with_resolved(&inner, None);
            }
        }

        if node.kind() == SyntaxKind::FieldExpr {
            if let Some(resolved) = resolved {
                if !resolved.accessible {
                    return false;
                }
                if let Some(symbol) = self.symbols.get(resolved.id) {
                    return self.symbol_is_constant(symbol);
                }
            }
            return node
                .children()
                .next()
                .map(|base| self.is_constant_target_with_resolved(&base, None))
                .unwrap_or(false);
        }

        if node.kind() == SyntaxKind::IndexExpr {
            return node
                .children()
                .next()
                .map(|base| self.is_constant_target_with_resolved(&base, None))
                .unwrap_or(false);
        }

        if node.kind() == SyntaxKind::DerefExpr {
            return false;
        }

        if node.kind() == SyntaxKind::NameRef {
            if let Some(resolved) = resolved {
                if !resolved.accessible {
                    return false;
                }
                if let Some(symbol) = self.symbols.get(resolved.id) {
                    return self.symbol_is_constant(symbol);
                }
            }

            if let Some(name) = self.resolve_ref().get_name_from_ref(node) {
                if let NameResolveOutcome::Resolved(resolved) = self
                    .resolve()
                    .resolve_name_in_context_outcome(&name, node.text_range())
                {
                    if !resolved.accessible {
                        return false;
                    }
                    if let Some(symbol) = self.symbols.get(resolved.id) {
                        return self.symbol_is_constant(symbol);
                    }
                }
            }
        }

        false
    }

    pub(super) fn check_assignable_target_symbol(
        &mut self,
        node: &SyntaxNode,
        resolved: Option<&ResolvedSymbol>,
    ) -> bool {
        if self.is_return_target(node) {
            return true;
        }

        let Some(resolved) = resolved else {
            return true;
        };
        if !resolved.accessible {
            return false;
        }
        let Some(symbol) = self.symbols.get(resolved.id) else {
            return true;
        };

        if symbol.is_constant
            || matches!(
                symbol.kind,
                SymbolKind::Constant | SymbolKind::EnumValue { .. }
            )
        {
            self.diagnostics.error(
                DiagnosticCode::ConstantModification,
                node.text_range(),
                format!("cannot assign to constant '{}'", symbol.name),
            );
            return false;
        }

        match symbol.kind {
            SymbolKind::Variable { .. } => true,
            SymbolKind::Parameter {
                direction: ParamDirection::Out | ParamDirection::InOut,
            } => true,
            SymbolKind::Parameter {
                direction: ParamDirection::In,
            } => {
                self.diagnostics.error(
                    DiagnosticCode::InvalidAssignmentTarget,
                    node.text_range(),
                    format!("cannot assign to input parameter '{}'", symbol.name),
                );
                false
            }
            SymbolKind::Property { has_set, .. } => {
                if has_set {
                    true
                } else {
                    self.diagnostics.error(
                        DiagnosticCode::InvalidAssignmentTarget,
                        node.text_range(),
                        format!("property '{}' has no setter", symbol.name),
                    );
                    false
                }
            }
            SymbolKind::Constant | SymbolKind::EnumValue { .. } => false,
            _ => {
                self.diagnostics.error(
                    DiagnosticCode::InvalidAssignmentTarget,
                    node.text_range(),
                    format!("'{}' is not an assignable target", symbol.name),
                );
                false
            }
        }
    }

    pub(super) fn assignment_target_symbol(&mut self, node: &SyntaxNode) -> Option<ResolvedSymbol> {
        if node.kind() == SyntaxKind::ParenExpr {
            return node
                .children()
                .next()
                .and_then(|inner| self.assignment_target_symbol(&inner));
        }

        match node.kind() {
            SyntaxKind::NameRef => {
                let name = self.resolve_ref().get_name_from_ref(node)?;
                match self
                    .resolve()
                    .resolve_name_in_context_outcome(&name, node.text_range())
                {
                    NameResolveOutcome::Resolved(resolved) => Some(resolved),
                    NameResolveOutcome::Ambiguous | NameResolveOutcome::NotFound => None,
                }
            }
            SyntaxKind::FieldExpr => {
                if let Some(symbol_id) = self.resolve_ref().resolve_namespace_qualified_symbol(node)
                {
                    let accessible = self
                        .resolve()
                        .check_member_access(symbol_id, node.text_range());
                    return Some(ResolvedSymbol {
                        id: symbol_id,
                        accessible,
                    });
                }
                let children: Vec<_> = node.children().collect();
                if children.len() < 2 {
                    return None;
                }
                let base_type = self.expr().check_expression(&children[0]);
                let field_name = self.resolve_ref().get_name_from_ref(&children[1])?;
                self.resolve().resolve_member_symbol_in_type(
                    base_type,
                    field_name.as_str(),
                    children[1].text_range(),
                )
            }
            SyntaxKind::IndexExpr => node
                .children()
                .next()
                .and_then(|base| self.assignment_target_symbol(&base)),
            SyntaxKind::DerefExpr => None,
            _ => None,
        }
    }

    pub(super) fn type_of_assignment_target(
        &mut self,
        node: &SyntaxNode,
        resolved: Option<&ResolvedSymbol>,
    ) -> TypeId {
        if self.is_return_target(node) {
            return self.current_function_return.unwrap_or(TypeId::VOID);
        }
        if let Some(resolved) = resolved {
            if let Some(symbol) = self.symbols.get(resolved.id) {
                if matches!(symbol.kind, SymbolKind::Property { .. }) {
                    return symbol.type_id;
                }
            }
        }
        self.expr().check_expression(node)
    }

    pub(super) fn is_return_target(&self, node: &SyntaxNode) -> bool {
        let Some(current_id) = self.current_pou_symbol else {
            return false;
        };
        if node.kind() == SyntaxKind::ParenExpr {
            if let Some(inner) = node.children().next() {
                return self.is_return_target(&inner);
            }
        }
        if node.kind() == SyntaxKind::NameRef {
            if let Some(name) = self.resolve_ref().get_name_from_ref(node) {
                if self
                    .symbols
                    .get(current_id)
                    .is_some_and(|symbol| symbol.name.eq_ignore_ascii_case(name.as_str()))
                {
                    return true;
                }
                if let Some(symbol_id) = self.symbols.resolve(&name, self.current_scope) {
                    return symbol_id == current_id;
                }
            }
        }
        false
    }
}
