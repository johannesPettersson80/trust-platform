use super::*;
use crate::semantic::LEGACY_UNKNOWN_TYPE_ID;

impl SymbolCollector<'_> {
    pub(super) fn push_namespace_path(
        &mut self,
        parts: &[(SmolStr, TextRange)],
        visibility: Visibility,
    ) -> Vec<NamespaceEntry> {
        let mut entries = Vec::new();
        for (name, range) in parts {
            let current_scope = self.table.current_scope();
            let existing_local = self
                .table
                .get_scope(current_scope)
                .and_then(|scope| scope.lookup_local(name.as_str()));

            if let Some(existing_id) = existing_local {
                if let Some(existing_symbol) = self.table.get(existing_id) {
                    if matches!(existing_symbol.kind, SymbolKind::Namespace) {
                        let previous_scope = self.table.current_scope();
                        let scope_id =
                            self.table.scope_for_owner(existing_id).unwrap_or_else(|| {
                                self.table
                                    .push_scope(ScopeKind::Namespace, Some(existing_id))
                            });
                        self.table.set_current_scope(scope_id);
                        self.parent_stack.push(existing_id);
                        entries.push(NamespaceEntry::Reused { previous_scope });
                        continue;
                    }
                }
            }

            let mut symbol = Symbol::new(
                SymbolId::UNKNOWN,
                name.clone(),
                SymbolKind::Namespace,
                TypeId::VOID,
                *range,
            );
            symbol.parent = self.current_parent();
            symbol.visibility = visibility;
            let id = self.declare_symbol(symbol);
            self.table.push_scope(ScopeKind::Namespace, Some(id));
            self.parent_stack.push(id);
            entries.push(NamespaceEntry::Pushed);
        }
        entries
    }

    pub(super) fn pop_namespace_path(&mut self, entries: Vec<NamespaceEntry>) {
        for entry in entries.into_iter().rev() {
            self.parent_stack.pop();
            match entry {
                NamespaceEntry::Pushed => self.table.pop_scope(),
                NamespaceEntry::Reused { previous_scope } => {
                    self.table.set_current_scope(previous_scope)
                }
            }
        }
    }

    pub(super) fn visit_node(&mut self, node: &SyntaxNode) {
        match node.kind() {
            SyntaxKind::Program => self.collect_pou(node, SymbolKind::Program, TypeId::VOID),
            SyntaxKind::Configuration => self.collect_configuration(node),
            SyntaxKind::Resource => self.collect_resource(node),
            SyntaxKind::TaskConfig => self.collect_task_config(node),
            SyntaxKind::ProgramConfig => self.collect_program_config(node),
            SyntaxKind::Function => {
                let return_type = self
                    .return_type_from_node(node)
                    .unwrap_or(LEGACY_UNKNOWN_TYPE_ID);
                self.collect_pou(
                    node,
                    SymbolKind::Function {
                        return_type,
                        parameters: Vec::new(),
                    },
                    return_type,
                );
            }
            SyntaxKind::FunctionBlock => {
                self.collect_pou(node, SymbolKind::FunctionBlock, TypeId::VOID);
            }
            SyntaxKind::Class => {
                self.collect_pou(node, SymbolKind::Class, TypeId::VOID);
            }
            SyntaxKind::Namespace => {
                self.collect_namespace(node);
            }
            SyntaxKind::Method => {
                let return_type = self.return_type_from_node(node);
                self.collect_pou(
                    node,
                    SymbolKind::Method {
                        return_type,
                        parameters: Vec::new(),
                    },
                    return_type.unwrap_or(TypeId::VOID),
                );
            }
            SyntaxKind::Property => {
                let prop_type = self
                    .return_type_from_node(node)
                    .unwrap_or(LEGACY_UNKNOWN_TYPE_ID);
                let (has_get, has_set) = self.property_accessors(node);
                self.collect_pou(
                    node,
                    SymbolKind::Property {
                        prop_type,
                        has_get,
                        has_set,
                    },
                    prop_type,
                );
            }
            SyntaxKind::Interface => {
                self.collect_pou(node, SymbolKind::Interface, TypeId::VOID);
            }
            SyntaxKind::TypeDecl => {
                self.collect_type_symbols(node);
            }
            SyntaxKind::VarBlock => {
                self.collect_var_block(node);
            }
            SyntaxKind::VarAccessBlock => {
                self.collect_var_access_block(node);
            }
            SyntaxKind::VarConfigBlock => {
                self.collect_var_config_block(node);
            }
            SyntaxKind::UsingDirective => {
                self.collect_using_directive(node);
            }
            _ => {
                for child in node.children() {
                    self.visit_node(&child);
                }
            }
        }
    }

    fn collect_using_directive(&mut self, node: &SyntaxNode) {
        for child in node.children() {
            if !matches!(child.kind(), SyntaxKind::QualifiedName | SyntaxKind::Name) {
                continue;
            }
            let Some((parts, range)) = qualified_name_parts(&child) else {
                continue;
            };
            let path: Vec<SmolStr> = parts.into_iter().map(|(name, _)| name).collect();
            if !path.is_empty() {
                self.table.add_using_directive(path, range);
            }
        }
    }

    fn collect_pou(&mut self, node: &SyntaxNode, kind: SymbolKind, type_id: TypeId) {
        let mut owner_parts: Option<Vec<SmolStr>> = None;
        let name_and_range = if matches!(kind, SymbolKind::Method { .. }) {
            qualified_name_parts(node).and_then(|(parts, _)| {
                if parts.len() > 1 {
                    let (name, range) = parts.last().cloned()?;
                    let owner: Vec<SmolStr> = parts[..parts.len() - 1]
                        .iter()
                        .map(|(part, _)| part.clone())
                        .collect();
                    owner_parts = Some(owner);
                    Some((name, range))
                } else {
                    name_from_node(node)
                }
            })
        } else {
            name_from_node(node)
        };

        let Some((name, range)) = name_and_range else {
            for child in node.children() {
                self.visit_node(&child);
            }
            return;
        };

        let qualified_name = self.qualify_current_name(&name);

        let type_to_register = match &kind {
            SymbolKind::FunctionBlock => Some(Type::FunctionBlock {
                name: qualified_name.clone(),
            }),
            SymbolKind::Class => Some(Type::Class {
                name: qualified_name.clone(),
            }),
            SymbolKind::Interface => Some(Type::Interface {
                name: qualified_name.clone(),
            }),
            _ => None,
        };

        // Determine scope kind based on symbol kind
        let scope_kind = match &kind {
            SymbolKind::Program => ScopeKind::Program,
            SymbolKind::Function { .. } => ScopeKind::Function,
            SymbolKind::FunctionBlock => ScopeKind::FunctionBlock,
            SymbolKind::Class => ScopeKind::Class,
            SymbolKind::Method { .. } => ScopeKind::Method,
            SymbolKind::Property { .. } => ScopeKind::Property,
            SymbolKind::Interface => ScopeKind::Namespace, // Interface uses namespace-like scope
            _ => ScopeKind::Block,
        };

        let mut symbol_type_id = type_id;
        if let Some(ty) = type_to_register {
            symbol_type_id = self.table.register_type(qualified_name.clone(), ty);
        }

        let mut symbol = Symbol::new(SymbolId::UNKNOWN, name.clone(), kind, symbol_type_id, range);
        symbol.visibility = self.visibility_for_pou(node, &symbol.kind);
        symbol.modifiers = modifiers_from_node(node);
        symbol.parent = self.current_parent();
        if let Some(owner_parts) = owner_parts {
            let owner_id = self
                .table
                .resolve_qualified(&owner_parts)
                .or_else(|| {
                    let mut qualified = self.current_namespace_path();
                    qualified.extend(owner_parts.clone());
                    self.table.resolve_qualified(&qualified)
                })
                .and_then(|id| {
                    self.table
                        .get(id)
                        .filter(|sym| {
                            matches!(sym.kind, SymbolKind::Class | SymbolKind::FunctionBlock)
                        })
                        .map(|_| id)
                });
            if let Some(owner_id) = owner_id {
                symbol.parent = Some(owner_id);
            }
        }
        let id = self.declare_symbol(symbol);

        self.register_extends_clause(node, id);
        self.register_implements_clause(node, id);

        // Push scope and track parent
        self.table.push_scope(scope_kind, Some(id));
        self.parent_stack.push(id);

        for child in node.children() {
            self.visit_node(&child);
        }
        self.attach_parameters(id);

        // Pop scope and parent
        self.parent_stack.pop();
        self.table.pop_scope();
    }

    fn collect_configuration(&mut self, node: &SyntaxNode) {
        let Some((name, range)) = name_from_node(node) else {
            for child in node.children() {
                self.visit_node(&child);
            }
            return;
        };

        let mut symbol = Symbol::new(
            SymbolId::UNKNOWN,
            name.clone(),
            SymbolKind::Configuration,
            TypeId::VOID,
            range,
        );
        symbol.parent = self.current_parent();
        let id = self.declare_symbol(symbol);

        self.table.push_scope(ScopeKind::Configuration, Some(id));
        self.parent_stack.push(id);

        for child in node.children() {
            self.visit_node(&child);
        }

        self.parent_stack.pop();
        self.table.pop_scope();
    }

    fn collect_resource(&mut self, node: &SyntaxNode) {
        let Some((name, range)) = name_from_node(node) else {
            for child in node.children() {
                self.visit_node(&child);
            }
            return;
        };

        let mut symbol = Symbol::new(
            SymbolId::UNKNOWN,
            name.clone(),
            SymbolKind::Resource,
            TypeId::VOID,
            range,
        );
        symbol.parent = self.current_parent();
        let id = self.declare_symbol(symbol);

        self.table.push_scope(ScopeKind::Resource, Some(id));
        self.parent_stack.push(id);

        for child in node.children() {
            self.visit_node(&child);
        }

        self.parent_stack.pop();
        self.table.pop_scope();
    }

    fn collect_task_config(&mut self, node: &SyntaxNode) {
        let Some((name, range)) = name_from_node(node) else {
            return;
        };
        let mut symbol = Symbol::new(
            SymbolId::UNKNOWN,
            name.clone(),
            SymbolKind::Task,
            TypeId::VOID,
            range,
        );
        symbol.parent = self.current_parent();
        self.declare_symbol(symbol);
    }

    fn collect_program_config(&mut self, node: &SyntaxNode) {
        let Some((instance, range)) = name_from_node(node) else {
            return;
        };
        let mut program_type_id = TypeId::UNKNOWN;
        if let Some((_, type_parts)) = program_config_instance_and_type(node) {
            let symbol_id = if type_parts.len() == 1 {
                self.table.lookup(type_parts[0].as_str())
            } else {
                self.table.resolve_qualified(&type_parts)
            };
            match symbol_id.and_then(|id| self.table.get(id)) {
                Some(symbol) if matches!(symbol.kind, SymbolKind::Program) => {
                    program_type_id = symbol.type_id;
                }
                Some(symbol) => {
                    self.diagnostics.error(
                        DiagnosticCode::InvalidOperation,
                        node.text_range(),
                        format!("PROGRAM instance type '{}' is not a PROGRAM", symbol.name),
                    );
                }
                None => {}
            }
        }

        let mut symbol = Symbol::new(
            SymbolId::UNKNOWN,
            instance.clone(),
            SymbolKind::ProgramInstance,
            program_type_id,
            range,
        );
        symbol.parent = self.current_parent();
        self.declare_symbol(symbol);
    }

    fn collect_namespace(&mut self, node: &SyntaxNode) {
        let Some((parts, _range)) = qualified_name_parts(node) else {
            return;
        };
        let entries = self.push_namespace_path(&parts, visibility_from_node(node));

        for child in node.children() {
            self.visit_node(&child);
        }

        self.pop_namespace_path(entries);
    }

    fn default_member_visibility(&self) -> Visibility {
        let Some(parent_id) = self.current_parent() else {
            return Visibility::Public;
        };
        let Some(parent) = self.table.get(parent_id) else {
            return Visibility::Public;
        };
        match parent.kind {
            SymbolKind::Class | SymbolKind::FunctionBlock => Visibility::Protected,
            SymbolKind::Interface => Visibility::Public,
            _ => Visibility::Public,
        }
    }

    fn visibility_for_pou(&self, node: &SyntaxNode, kind: &SymbolKind) -> Visibility {
        if let Some(explicit) = explicit_visibility_from_node(node) {
            return explicit;
        }
        match kind {
            SymbolKind::Method { .. } | SymbolKind::Property { .. } => {
                self.default_member_visibility()
            }
            _ => Visibility::Public,
        }
    }

    pub(super) fn visibility_for_var_block(&self, node: &SyntaxNode) -> Visibility {
        if let Some(explicit) = explicit_visibility_from_node(node) {
            return explicit;
        }
        self.default_member_visibility()
    }

    fn attach_parameters(&mut self, owner: SymbolId) {
        let mut params: Vec<SymbolId> = self
            .table
            .iter()
            .filter(|sym| {
                sym.parent == Some(owner) && matches!(sym.kind, SymbolKind::Parameter { .. })
            })
            .map(|sym| sym.id)
            .collect();
        params.sort_by_key(|id| id.0);

        if let Some(symbol) = self.table.get_mut(owner) {
            match &mut symbol.kind {
                SymbolKind::Function { parameters, .. } | SymbolKind::Method { parameters, .. } => {
                    *parameters = params;
                }
                _ => {}
            }
        }
    }

    fn register_extends_clause(&mut self, node: &SyntaxNode, owner: SymbolId) {
        if !matches!(
            node.kind(),
            SyntaxKind::FunctionBlock | SyntaxKind::Class | SyntaxKind::Interface
        ) {
            return;
        }

        let Some(clause) = node
            .children()
            .find(|child| child.kind() == SyntaxKind::ExtendsClause)
        else {
            return;
        };

        let Some((parts, range)) = qualified_name_parts(&clause) else {
            return;
        };

        let names: Vec<SmolStr> = parts.iter().map(|(name, _)| name.clone()).collect();
        let name = qualified_name_string(&names);
        self.table.set_extends(owner, name.clone());

        let type_id = self.resolve_type_path_at(&names, Some(range));
        if type_id == TypeId::UNKNOWN {
            self.pending_types.push(PendingType {
                name,
                range,
                scope_id: self.table.current_scope(),
            });
        }
    }

    fn register_implements_clause(&mut self, node: &SyntaxNode, owner: SymbolId) {
        if !matches!(node.kind(), SyntaxKind::FunctionBlock | SyntaxKind::Class) {
            return;
        }

        let Some(clause) = node
            .children()
            .find(|child| child.kind() == SyntaxKind::ImplementsClause)
        else {
            return;
        };

        let mut interfaces = Vec::new();
        for (parts, range) in implements_clause_names(&clause) {
            let type_id = self.resolve_type_path_at(&parts, Some(range));
            let name = if type_id == TypeId::UNKNOWN {
                let qualified = qualified_name_string(&parts);
                self.pending_types.push(PendingType {
                    name: qualified.clone(),
                    range,
                    scope_id: self.table.current_scope(),
                });
                qualified
            } else {
                self.table
                    .type_name(type_id)
                    .unwrap_or_else(|| qualified_name_string(&parts))
            };
            interfaces.push(name);
        }

        if !interfaces.is_empty() {
            self.table.set_implements(owner, interfaces);
        }
    }

    pub(super) fn declare_symbol(&mut self, symbol: Symbol) -> SymbolId {
        let name = symbol.name.clone();
        let range = symbol.range;

        let upper_name = name.to_ascii_uppercase();
        let allow_reserved = (matches!(symbol.kind, SymbolKind::Parameter { .. })
            && matches!(upper_name.as_str(), "EN" | "ENO"))
            || (matches!(symbol.kind, SymbolKind::Method { .. })
                && matches!(upper_name.as_str(), "GET" | "SET"));
        self.validate_identifier(&name, range, allow_reserved);

        // Check for duplicate in current scope
        if let Some(existing) = self.table.resolve_current(&name) {
            // Only report duplicate if it's in the same scope (not from parent)
            let current_scope = self.table.current_scope();
            if let Some(scope) = self.table.get_scope(current_scope) {
                if scope.lookup_local(&name).is_some() {
                    let mut diag = Diagnostic::error(
                        DiagnosticCode::DuplicateDeclaration,
                        range,
                        format!("duplicate declaration of '{}'", name),
                    );
                    if let Some(existing_symbol) = self.table.get(existing) {
                        diag = diag.with_related(existing_symbol.range, "previously declared here");
                    }
                    self.diagnostics.add(diag);
                }
            }
        }

        self.table.add_symbol(symbol)
    }

    pub(super) fn validate_identifier(
        &mut self,
        name: &str,
        range: TextRange,
        allow_reserved: bool,
    ) {
        if !allow_reserved && is_reserved_keyword(name) {
            self.diagnostics.error(
                DiagnosticCode::InvalidIdentifier,
                range,
                format!("reserved keyword '{}' cannot be used as identifier", name),
            );
            return;
        }

        if !is_valid_identifier(name) {
            self.diagnostics.error(
                DiagnosticCode::InvalidIdentifier,
                range,
                format!("invalid identifier '{}'", name),
            );
        }
    }

    pub(super) fn current_parent(&self) -> Option<SymbolId> {
        self.parent_stack.last().copied()
    }

    pub(super) fn current_namespace_path(&self) -> Vec<SmolStr> {
        if let Some(namespace) = &self.namespace_override {
            return namespace.clone();
        }
        let mut parts = Vec::new();
        for symbol_id in &self.parent_stack {
            let Some(symbol) = self.table.get(*symbol_id) else {
                continue;
            };
            if matches!(symbol.kind, SymbolKind::Namespace) {
                parts.push(symbol.name.clone());
            }
        }
        parts
    }

    pub(super) fn qualify_current_name(&self, name: &SmolStr) -> SmolStr {
        let parts = self.current_namespace_path();
        qualify_name(&parts, name)
    }
}

#[derive(Clone, Copy)]
pub(super) enum NamespaceEntry {
    Pushed,
    Reused { previous_scope: ScopeId },
}
