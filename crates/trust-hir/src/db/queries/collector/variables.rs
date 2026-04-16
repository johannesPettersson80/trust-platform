use super::*;
use crate::db::diagnostics::is_expression_kind;

impl SymbolCollector<'_> {
    pub(super) fn extract_var_decl_info(
        &mut self,
        node: &SyntaxNode,
    ) -> (Vec<(SmolStr, TextRange)>, TypeId, Option<SmolStr>) {
        let mut names = Vec::new();
        let mut type_id = TypeId::UNKNOWN;

        for child in node.children() {
            match child.kind() {
                SyntaxKind::Name => {
                    if let Some((name, range)) = name_from_node(&child) {
                        names.push((name, range));
                    }
                }
                SyntaxKind::TypeRef => {
                    type_id = self.resolve_type_from_ref(&child);
                }
                _ => {}
            }
        }

        let direct_address = var_decl_direct_address(node);

        (names, type_id, direct_address)
    }

    pub(super) fn collect_var_block(&mut self, node: &SyntaxNode) {
        let qualifier = var_qualifier_from_block(node);
        let is_constant = var_block_is_constant(node);
        let visibility = match qualifier {
            VarQualifier::Input | VarQualifier::Output => Visibility::Public,
            _ => self.visibility_for_var_block(node),
        };
        let use_global_scope = qualifier == VarQualifier::Global && self.in_configuration_scope();
        let previous_scope = self.table.current_scope();
        if use_global_scope {
            self.table.set_current_scope(ScopeId::GLOBAL);
        }
        for child in node.children() {
            if child.kind() == SyntaxKind::VarDecl {
                self.collect_var_decl(&child, qualifier, is_constant, visibility);
            }
        }
        if use_global_scope {
            self.table.set_current_scope(previous_scope);
        }
    }

    pub(super) fn collect_var_decl(
        &mut self,
        node: &SyntaxNode,
        qualifier: VarQualifier,
        is_constant: bool,
        visibility: Visibility,
    ) {
        let mut names = Vec::new();
        let mut type_ref = None;
        for child in node.children() {
            match child.kind() {
                SyntaxKind::Name => names.push(child),
                SyntaxKind::TypeRef => {
                    type_ref = Some(child);
                    break;
                }
                _ => {}
            }
        }

        let type_id = if let Some(type_ref) = type_ref.as_ref() {
            self.resolve_type_from_ref(type_ref)
        } else {
            TypeId::UNKNOWN
        };

        if let Some(expr) = node.children().find(|n| is_expression_kind(n.kind())) {
            self.check_string_initializer(type_id, &expr);
        }

        let direct_address = var_decl_direct_address(node);

        for name_node in names {
            if let Some((name, range)) = name_from_node(&name_node) {
                if is_constant
                    && matches!(
                        self.table
                            .type_by_id(self.table.resolve_alias_type(type_id)),
                        Some(Type::FunctionBlock { .. })
                    )
                {
                    self.diagnostics.error(
                        DiagnosticCode::InvalidOperation,
                        range,
                        "function block instances shall not be declared in CONSTANT sections",
                    );
                }

                let kind = match qualifier {
                    VarQualifier::Input => SymbolKind::Parameter {
                        direction: ParamDirection::In,
                    },
                    VarQualifier::Output => SymbolKind::Parameter {
                        direction: ParamDirection::Out,
                    },
                    VarQualifier::InOut => SymbolKind::Parameter {
                        direction: ParamDirection::InOut,
                    },
                    VarQualifier::Temp => SymbolKind::Variable { qualifier },
                    _ if is_constant => SymbolKind::Constant,
                    _ => SymbolKind::Variable { qualifier },
                };
                let mut symbol = Symbol::new(SymbolId::UNKNOWN, name, kind, type_id, range);
                symbol.is_constant = is_constant;
                symbol.direct_address = direct_address.clone();
                symbol.parent = self.current_parent();
                symbol.visibility = visibility;
                self.declare_symbol(symbol);
            }
        }
    }

    pub(super) fn check_string_initializer(&mut self, type_id: TypeId, expr: &SyntaxNode) {
        let Some(literal) = string_literal_info(expr) else {
            return;
        };
        let resolved = self.table.resolve_alias_type(type_id);
        match self.table.type_by_id(resolved) {
            Some(Type::String {
                max_len: Some(max_len),
            }) if !literal.is_wide && literal.len > *max_len => {
                let type_name = self
                    .table
                    .type_name(resolved)
                    .unwrap_or_else(|| "STRING".into());
                self.diagnostics.error(
                    DiagnosticCode::OutOfRange,
                    expr.text_range(),
                    format!(
                        "STRING literal length {} exceeds {} capacity",
                        literal.len, type_name
                    ),
                );
            }
            Some(Type::WString {
                max_len: Some(max_len),
            }) if literal.is_wide && literal.len > *max_len => {
                let type_name = self
                    .table
                    .type_name(resolved)
                    .unwrap_or_else(|| "WSTRING".into());
                self.diagnostics.error(
                    DiagnosticCode::OutOfRange,
                    expr.text_range(),
                    format!(
                        "WSTRING literal length {} exceeds {} capacity",
                        literal.len, type_name
                    ),
                );
            }
            _ => {}
        }
    }

    pub(super) fn collect_var_access_block(&mut self, node: &SyntaxNode) {
        let use_global_scope = self.in_configuration_scope();
        let previous_scope = self.table.current_scope();
        if use_global_scope {
            self.table.set_current_scope(ScopeId::GLOBAL);
        }
        for access_decl in node
            .children()
            .filter(|n| n.kind() == SyntaxKind::AccessDecl)
        {
            let Some((name, range)) = access_decl
                .children()
                .find(|n| n.kind() == SyntaxKind::Name)
                .and_then(|n| name_from_node(&n))
            else {
                continue;
            };
            let type_id = access_decl
                .children()
                .find(|n| n.kind() == SyntaxKind::TypeRef)
                .map(|n| self.resolve_type_from_ref(&n))
                .unwrap_or(TypeId::UNKNOWN);

            let mode = access_decl_mode(&access_decl);
            let kind = match mode {
                AccessMode::ReadOnly => SymbolKind::Constant,
                AccessMode::ReadWrite => SymbolKind::Variable {
                    qualifier: VarQualifier::Access,
                },
            };

            let mut symbol = Symbol::new(SymbolId::UNKNOWN, name, kind, type_id, range);
            symbol.parent = self.current_parent();
            self.declare_symbol(symbol);
        }
        if use_global_scope {
            self.table.set_current_scope(previous_scope);
        }
    }

    pub(super) fn collect_var_config_block(&mut self, _node: &SyntaxNode) {}

    fn in_configuration_scope(&self) -> bool {
        self.table
            .get_scope(self.table.current_scope())
            .is_some_and(|scope| {
                matches!(scope.kind, ScopeKind::Configuration | ScopeKind::Resource)
            })
    }
}
