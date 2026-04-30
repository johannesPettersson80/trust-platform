use super::super::*;
use super::*;
use crate::diagnostics::Diagnostic;
use crate::Symbol;

#[derive(Debug, Clone, Copy)]
pub(in crate::type_check) enum NameLookupResult {
    Found(SymbolId),
    Ambiguous,
    NotFound,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::type_check) enum NameResolveOutcome {
    Resolved(ResolvedSymbol),
    Ambiguous,
    NotFound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemberAccessStatus {
    Allowed,
    Private,
    Protected,
    Internal,
}

impl<'a, 'b> ResolveCheckerRef<'a, 'b> {
    fn owner_symbol_from_type(&self, type_id: TypeId, allow_interface: bool) -> Option<SymbolId> {
        let base_type = self.checker.resolve_alias_type(type_id);
        let name = match self.checker.symbols.type_by_id(base_type)? {
            Type::FunctionBlock { name } | Type::Class { name } => name,
            Type::Interface { name } if allow_interface => name,
            _ => return None,
        };

        self.checker
            .symbols
            .resolve_global_or_qualified_name(name.as_str())
    }

    fn member_owner_from_type(&self, type_id: TypeId) -> Option<SymbolId> {
        self.owner_symbol_from_type(type_id, true)
    }

    fn class_owner_from_type(&self, type_id: TypeId) -> Option<SymbolId> {
        self.owner_symbol_from_type(type_id, false)
    }

    pub(in crate::type_check) fn resolve_call_target(
        &self,
        symbol_id: SymbolId,
    ) -> Option<CallTargetInfo> {
        let symbol = self.checker.symbols.get(symbol_id)?;
        let symbol_kind = symbol.kind.clone();
        let symbol_type = symbol.type_id;

        match &symbol_kind {
            SymbolKind::Function { return_type, .. } => Some(CallTargetInfo {
                return_type: *return_type,
                param_owner: symbol_id,
                kind: symbol_kind,
            }),
            SymbolKind::Method { return_type, .. } => Some(CallTargetInfo {
                return_type: return_type.unwrap_or(TypeId::VOID),
                param_owner: symbol_id,
                kind: symbol_kind,
            }),
            SymbolKind::FunctionBlock => Some(CallTargetInfo {
                return_type: symbol_type,
                param_owner: symbol_id,
                kind: symbol_kind,
            }),
            SymbolKind::Property { .. } => None,
            _ => self
                .resolve_function_block_instance(symbol_type)
                .map(|(fb_id, fb_kind)| CallTargetInfo {
                    return_type: symbol_type,
                    param_owner: fb_id,
                    kind: fb_kind,
                }),
        }
    }

    pub(in crate::type_check) fn resolve_call_target_from_type(
        &self,
        type_id: TypeId,
    ) -> Option<CallTargetInfo> {
        self.resolve_function_block_instance(type_id)
            .map(|(fb_id, fb_kind)| CallTargetInfo {
                return_type: type_id,
                param_owner: fb_id,
                kind: fb_kind,
            })
    }

    pub(in crate::type_check) fn resolve_function_block_instance(
        &self,
        type_id: TypeId,
    ) -> Option<(SymbolId, SymbolKind)> {
        let base_type = self.checker.resolve_alias_type(type_id);
        let Type::FunctionBlock { name } = self.checker.symbols.type_by_id(base_type)? else {
            return None;
        };

        let fb_symbol_id = self
            .checker
            .symbols
            .resolve_global_or_qualified_name(name.as_str())?;
        let fb_kind = self.checker.symbols.get(fb_symbol_id)?.kind.clone();
        Some((fb_symbol_id, fb_kind))
    }

    pub(in crate::type_check) fn resolve_member_symbol_in_hierarchy(
        &self,
        root_id: SymbolId,
        field_name: &str,
    ) -> Option<SymbolId> {
        self.checker
            .symbols
            .resolve_member_symbol_in_hierarchy(root_id, field_name)
    }

    pub(in crate::type_check) fn resolve_namespace_qualified_symbol(
        &self,
        node: &SyntaxNode,
    ) -> Option<SymbolId> {
        let parts = self.qualified_name_from_field_expr(node)?;
        self.checker.symbols.resolve_qualified(&parts)
    }

    fn resolve_using_in_scope(&self, scope_id: ScopeId, name: &str) -> UsingResolution {
        let Some(scope) = self.checker.symbols.get_scope(scope_id) else {
            return UsingResolution::None;
        };
        self.checker.symbols.resolve_using_in_scope(scope, name)
    }

    pub(in crate::type_check) fn lookup_name_symbol(&self, name: &str) -> NameLookupResult {
        let mut scope_id = Some(self.checker.current_scope);
        let mut after_class_scope = None;
        let mut class_scope_id = None;

        while let Some(sid) = scope_id {
            let (parent, kind) = match self.checker.symbols.get_scope(sid) {
                Some(scope) => {
                    if let Some(symbol_id) = scope.lookup_local(name) {
                        return NameLookupResult::Found(symbol_id);
                    }
                    (scope.parent, scope.kind)
                }
                None => break,
            };

            if matches!(kind, ScopeKind::Class | ScopeKind::FunctionBlock) {
                after_class_scope = parent;
                class_scope_id = Some(sid);
                break;
            }

            match self.resolve_using_in_scope(sid, name) {
                UsingResolution::Single(symbol_id) => {
                    return NameLookupResult::Found(symbol_id);
                }
                UsingResolution::Ambiguous => return NameLookupResult::Ambiguous,
                UsingResolution::None => {}
            }

            scope_id = parent;
        }

        if let Some(owner_id) = self.current_class_owner() {
            if let Some(member_id) = self.resolve_member_symbol_in_hierarchy(owner_id, name) {
                return NameLookupResult::Found(member_id);
            }
        }

        if let Some(class_scope_id) = class_scope_id {
            match self.resolve_using_in_scope(class_scope_id, name) {
                UsingResolution::Single(symbol_id) => {
                    return NameLookupResult::Found(symbol_id);
                }
                UsingResolution::Ambiguous => return NameLookupResult::Ambiguous,
                UsingResolution::None => {}
            }
        }

        let mut scope_id = after_class_scope;
        while let Some(sid) = scope_id {
            let parent = match self.checker.symbols.get_scope(sid) {
                Some(scope) => {
                    if let Some(symbol_id) = scope.lookup_local(name) {
                        return NameLookupResult::Found(symbol_id);
                    }
                    scope.parent
                }
                None => break,
            };

            match self.resolve_using_in_scope(sid, name) {
                UsingResolution::Single(symbol_id) => {
                    return NameLookupResult::Found(symbol_id);
                }
                UsingResolution::Ambiguous => return NameLookupResult::Ambiguous,
                UsingResolution::None => {}
            }

            scope_id = parent;
        }

        NameLookupResult::NotFound
    }

    pub(in crate::type_check) fn resolve_member_symbol_in_type(
        &self,
        type_id: TypeId,
        field_name: &str,
    ) -> Option<SymbolId> {
        let owner_id = self.member_owner_from_type(type_id)?;
        self.resolve_member_symbol_in_hierarchy(owner_id, field_name)
    }

    pub(in crate::type_check) fn qualified_name_from_field_expr(
        &self,
        node: &SyntaxNode,
    ) -> Option<Vec<SmolStr>> {
        if node.kind() != SyntaxKind::FieldExpr {
            return None;
        }
        let mut parts: Vec<SmolStr> = Vec::new();
        let mut current = node.clone();
        loop {
            let mut children = current.children();
            let base = children.next()?;
            let member = children.next()?;
            let member_name = self.get_name_from_ref(&member)?;
            parts.push(member_name);
            match base.kind() {
                SyntaxKind::FieldExpr => {
                    current = base;
                }
                SyntaxKind::NameRef => {
                    let base_name = self.get_name_from_ref(&base)?;
                    parts.push(base_name);
                    break;
                }
                _ => return None,
            }
        }
        parts.reverse();
        Some(parts)
    }
}

impl<'a, 'b> ResolveChecker<'a, 'b> {
    pub(in crate::type_check) fn resolve_member_symbol_in_type(
        &mut self,
        type_id: TypeId,
        field_name: &str,
        range: TextRange,
    ) -> Option<ResolvedSymbol> {
        let member_id = self
            .checker
            .resolve_ref()
            .resolve_member_symbol_in_type(type_id, field_name)?;
        let accessible = self.check_member_access(member_id, range);
        Some(ResolvedSymbol {
            id: member_id,
            accessible,
        })
    }

    pub(in crate::type_check) fn resolve_name_in_context_outcome(
        &mut self,
        name: &str,
        range: TextRange,
    ) -> NameResolveOutcome {
        match self.checker.resolve_ref().lookup_name_symbol(name) {
            NameLookupResult::Found(symbol_id) => {
                let accessible = self.check_member_access(symbol_id, range);
                NameResolveOutcome::Resolved(ResolvedSymbol {
                    id: symbol_id,
                    accessible,
                })
            }
            NameLookupResult::Ambiguous => {
                self.checker.diagnostics.error(
                    DiagnosticCode::CannotResolve,
                    range,
                    format!("ambiguous reference to '{}'; qualify the name", name),
                );
                NameResolveOutcome::Ambiguous
            }
            NameLookupResult::NotFound => NameResolveOutcome::NotFound,
        }
    }

    pub(in crate::type_check) fn check_member_access(
        &mut self,
        member_id: SymbolId,
        range: TextRange,
    ) -> bool {
        let Some(member) = self.checker.symbols.get(member_id) else {
            return true;
        };
        let owner_id = self.checker.resolve_ref().member_owner(member_id);
        match self
            .checker
            .resolve_ref()
            .access_status(member.visibility, owner_id)
        {
            MemberAccessStatus::Allowed => true,
            MemberAccessStatus::Private => {
                let mut diagnostic = Diagnostic::error(
                    DiagnosticCode::InvalidOperation,
                    range,
                    format!("cannot access PRIVATE member '{}'", member.name),
                );
                if let Some(hint) =
                    self.access_hint_message(MemberAccessStatus::Private, member, owner_id)
                {
                    diagnostic = diagnostic.with_related(range, hint);
                }
                self.checker.diagnostics.add(diagnostic);
                false
            }
            MemberAccessStatus::Protected => {
                let mut diagnostic = Diagnostic::error(
                    DiagnosticCode::InvalidOperation,
                    range,
                    format!("cannot access PROTECTED member '{}'", member.name),
                );
                if let Some(hint) =
                    self.access_hint_message(MemberAccessStatus::Protected, member, owner_id)
                {
                    diagnostic = diagnostic.with_related(range, hint);
                }
                self.checker.diagnostics.add(diagnostic);
                false
            }
            MemberAccessStatus::Internal => {
                let mut diagnostic = Diagnostic::error(
                    DiagnosticCode::InvalidOperation,
                    range,
                    format!("cannot access INTERNAL member '{}'", member.name),
                );
                if let Some(hint) =
                    self.access_hint_message(MemberAccessStatus::Internal, member, owner_id)
                {
                    diagnostic = diagnostic.with_related(range, hint);
                }
                self.checker.diagnostics.add(diagnostic);
                false
            }
        }
    }

    fn access_hint_message(
        &self,
        status: MemberAccessStatus,
        member: &Symbol,
        owner_id: Option<SymbolId>,
    ) -> Option<String> {
        let owner_name = owner_id
            .and_then(|owner| self.checker.symbols.get(owner))
            .map(|symbol| symbol.name.as_str().to_string());
        match status {
            MemberAccessStatus::Allowed => None,
            MemberAccessStatus::Private => {
                let owner = owner_name.unwrap_or_else(|| "the declaring class".to_string());
                Some(format!(
                    "Hint: PRIVATE members are accessible only inside {owner}. Consider moving the call or changing '{}' to PROTECTED/PUBLIC.",
                    member.name
                ))
            }
            MemberAccessStatus::Protected => {
                let owner = owner_name.unwrap_or_else(|| "the declaring class".to_string());
                Some(format!(
                    "Hint: PROTECTED members are accessible in {owner} and derived classes. Consider calling through a derived method or changing '{}' to PUBLIC.",
                    member.name
                ))
            }
            MemberAccessStatus::Internal => {
                let namespace = owner_id
                    .map(|owner| self.checker.resolve_ref().namespace_path_for_symbol(owner))
                    .filter(|parts| !parts.is_empty())
                    .map(|parts| {
                        parts
                            .iter()
                            .map(|part| part.as_str())
                            .collect::<Vec<_>>()
                            .join(".")
                    });
                let namespace = namespace.unwrap_or_else(|| "the current namespace".to_string());
                Some(format!(
                    "Hint: INTERNAL members are accessible only within {namespace}. Consider moving the caller or changing '{}' to PUBLIC.",
                    member.name
                ))
            }
        }
    }
}

impl<'a, 'b> ResolveCheckerRef<'a, 'b> {
    pub(in crate::type_check) fn member_owner(&self, member_id: SymbolId) -> Option<SymbolId> {
        let symbol = self.checker.symbols.get(member_id)?;
        let parent_id = symbol.parent?;
        let parent = self.checker.symbols.get(parent_id)?;
        match parent.kind {
            SymbolKind::Class | SymbolKind::FunctionBlock => Some(parent_id),
            _ => None,
        }
    }

    pub(in crate::type_check) fn current_class_owner(&self) -> Option<SymbolId> {
        if let Some(pou_id) = self.checker.current_pou_symbol {
            if let Some(symbol) = self.checker.symbols.get(pou_id) {
                match symbol.kind {
                    SymbolKind::Class | SymbolKind::FunctionBlock => return Some(pou_id),
                    SymbolKind::Method { .. } | SymbolKind::Property { .. } => {
                        return symbol.parent;
                    }
                    _ => {}
                }
            }
        }

        let this_type = self.checker.this_type?;
        self.class_owner_from_type(this_type)
    }

    fn access_status(
        &self,
        visibility: Visibility,
        owner_id: Option<SymbolId>,
    ) -> MemberAccessStatus {
        let current_owner = self.current_class_owner();
        match visibility {
            Visibility::Public => MemberAccessStatus::Allowed,
            Visibility::Private => {
                let allowed = owner_id.is_some_and(|owner| current_owner == Some(owner));
                if allowed {
                    MemberAccessStatus::Allowed
                } else {
                    MemberAccessStatus::Private
                }
            }
            Visibility::Protected => {
                let allowed = owner_id.is_some_and(|owner| {
                    current_owner.is_some_and(|current| self.is_same_or_derived(current, owner))
                });
                if allowed {
                    MemberAccessStatus::Allowed
                } else {
                    MemberAccessStatus::Protected
                }
            }
            Visibility::Internal => {
                let allowed = owner_id.is_some_and(|owner| {
                    self.current_namespace_path() == self.namespace_path_for_symbol(owner)
                });
                if allowed {
                    MemberAccessStatus::Allowed
                } else {
                    MemberAccessStatus::Internal
                }
            }
        }
    }

    pub(in crate::type_check) fn is_same_or_derived(
        &self,
        derived_id: SymbolId,
        base_id: SymbolId,
    ) -> bool {
        if derived_id == base_id {
            return true;
        }

        let mut visited = FxHashSet::default();
        let mut current = self
            .checker
            .symbols
            .extends_name(derived_id)
            .and_then(|name| {
                self.checker
                    .symbols
                    .resolve_oop_reference_for_owner(derived_id, name.as_str())
            });

        while let Some(symbol_id) = current {
            if !visited.insert(symbol_id) {
                break;
            }
            if symbol_id == base_id {
                return true;
            }
            current = self
                .checker
                .symbols
                .extends_name(symbol_id)
                .and_then(|name| {
                    self.checker
                        .symbols
                        .resolve_oop_reference_for_owner(symbol_id, name.as_str())
                });
        }

        false
    }

    pub(in crate::type_check) fn namespace_path_for_symbol(
        &self,
        symbol_id: SymbolId,
    ) -> Vec<SmolStr> {
        let mut parts = Vec::new();
        let mut current = self
            .checker
            .symbols
            .get(symbol_id)
            .and_then(|sym| sym.parent);
        while let Some(parent_id) = current {
            let Some(parent) = self.checker.symbols.get(parent_id) else {
                break;
            };
            if matches!(parent.kind, SymbolKind::Namespace) {
                parts.push(parent.name.clone());
            }
            current = parent.parent;
        }
        parts.reverse();
        parts
    }

    pub(in crate::type_check) fn current_namespace_path(&self) -> Vec<SmolStr> {
        let mut parts = Vec::new();
        let mut current = Some(self.checker.current_scope);
        while let Some(scope_id) = current {
            let Some(scope) = self.checker.symbols.get_scope(scope_id) else {
                break;
            };
            if matches!(scope.kind, ScopeKind::Namespace) {
                if let Some(owner) = scope.owner {
                    if let Some(symbol) = self.checker.symbols.get(owner) {
                        parts.push(symbol.name.clone());
                    }
                }
            }
            current = scope.parent;
        }
        parts.reverse();
        parts
    }

    pub(in crate::type_check) fn resolve_member_in_type(
        &self,
        type_id: TypeId,
        field_name: &str,
    ) -> Option<TypeId> {
        let base_type = self.checker.resolve_alias_type(type_id);
        self.resolve_member_in_base_type(base_type, field_name)
    }

    fn resolve_member_in_base_type(&self, base_type: TypeId, field_name: &str) -> Option<TypeId> {
        match self.checker.symbols.type_by_id(base_type)? {
            Type::Struct { fields, .. } => fields
                .iter()
                .find(|field| field.name.eq_ignore_ascii_case(field_name))
                .map(|field| field.type_id),
            Type::Union { variants, .. } => variants
                .iter()
                .find(|variant| variant.name.eq_ignore_ascii_case(field_name))
                .map(|variant| variant.type_id),
            _ => None,
        }
    }
}
