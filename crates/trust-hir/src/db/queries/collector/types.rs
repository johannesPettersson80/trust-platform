use super::const_utils::*;
use super::*;
use crate::db::diagnostics::{is_expression_kind, resolve_pending_types_with_table};
use crate::types::{ArrayDimensionExt, StructField, UnionVariant};

impl SymbolCollector<'_> {
    pub(super) fn collect_type_symbols(&mut self, node: &SyntaxNode) {
        let mut pending: Option<(SmolStr, TextRange)> = None;
        for child in node.children() {
            match child.kind() {
                SyntaxKind::Name => {
                    pending = name_from_node(&child);
                }
                SyntaxKind::StructDef
                | SyntaxKind::UnionDef
                | SyntaxKind::EnumDef
                | SyntaxKind::ArrayType
                | SyntaxKind::TypeRef => {
                    let Some((type_name, name_range)) = pending.take() else {
                        continue;
                    };
                    self.register_type_symbol(&child, type_name, name_range);
                }
                _ => {}
            }
        }
    }

    pub(super) fn register_type_symbol(
        &mut self,
        type_def: &SyntaxNode,
        type_name: SmolStr,
        name_range: TextRange,
    ) {
        let qualified_name = self.qualify_current_name(&type_name);
        self.register_type_symbol_with_qualified_name(
            type_def,
            type_name,
            qualified_name,
            name_range,
        );
    }

    fn register_type_symbol_with_qualified_name(
        &mut self,
        type_def: &SyntaxNode,
        type_name: SmolStr,
        qualified_name: SmolStr,
        name_range: TextRange,
    ) {
        // Create TYPE symbol first with placeholder type_id, so that nested symbols
        // (like enum values) can have this symbol as their parent
        let mut symbol = Symbol::new(
            SymbolId::UNKNOWN,
            type_name,
            SymbolKind::Type,
            TypeId::UNKNOWN, // Placeholder, will be updated below
            name_range,
        );
        symbol.parent = self.current_parent();
        let type_symbol_id = self.declare_symbol(symbol);

        // Push TYPE symbol onto parent stack so enum values get it as parent
        self.parent_stack.push(type_symbol_id);

        let type_id = match type_def.kind() {
            SyntaxKind::StructDef => self.collect_struct_type(type_def, qualified_name.clone()),
            SyntaxKind::UnionDef => self.collect_union_type(type_def, qualified_name.clone()),
            SyntaxKind::EnumDef => self.collect_enum_type(type_def, qualified_name.clone()),
            SyntaxKind::ArrayType => {
                let target_type = self.collect_array_type(type_def);
                self.table.register_type(
                    qualified_name.clone(),
                    Type::Alias {
                        name: qualified_name.clone(),
                        target: target_type,
                    },
                )
            }
            SyntaxKind::TypeRef => {
                let target_type = self.resolve_type_from_ref(type_def);
                self.table.register_type(
                    qualified_name.clone(),
                    Type::Alias {
                        name: qualified_name.clone(),
                        target: target_type,
                    },
                )
            }
            _ => self.table.register_type(
                qualified_name.clone(),
                Type::Alias {
                    name: qualified_name.clone(),
                    target: TypeId::UNKNOWN,
                },
            ),
        };

        // Pop parent stack
        self.parent_stack.pop();

        // Update the TYPE symbol with the actual type_id
        if let Some(sym) = self.table.get_mut(type_symbol_id) {
            sym.type_id = type_id;
        }
    }

    fn register_imported_carrier_type(
        &mut self,
        qualified_name: &SmolStr,
        kind: ProjectTypeKind,
    ) -> TypeId {
        let carrier_type = match kind {
            ProjectTypeKind::FunctionBlock => Type::FunctionBlock {
                name: qualified_name.clone(),
            },
            ProjectTypeKind::Class => Type::Class {
                name: qualified_name.clone(),
            },
            ProjectTypeKind::Interface => Type::Interface {
                name: qualified_name.clone(),
            },
            ProjectTypeKind::Data => return TypeId::UNKNOWN,
        };
        self.table
            .register_type(qualified_name.clone(), carrier_type)
    }

    fn resolve_project_type_path(&mut self, parts: &[SmolStr]) -> TypeId {
        let Some(provider) = self.project_types else {
            return TypeId::UNKNOWN;
        };

        for candidate in self.project_type_candidates(parts) {
            let Some(entry) = provider.catalog_entry(candidate.as_str()) else {
                continue;
            };
            let imported = match entry.kind {
                ProjectTypeKind::Data => {
                    if self.importing_project_types.contains(candidate.as_str()) {
                        TypeId::UNKNOWN
                    } else {
                        self.importing_project_types.insert(candidate.clone());
                        let imported = provider
                            .load_type_declaration(candidate.as_str(), &entry)
                            .and_then(|decl| self.import_project_data_type(&candidate, &decl));
                        self.importing_project_types.remove(candidate.as_str());
                        imported.unwrap_or(TypeId::UNKNOWN)
                    }
                }
                ProjectTypeKind::FunctionBlock
                | ProjectTypeKind::Class
                | ProjectTypeKind::Interface => {
                    self.register_imported_carrier_type(&candidate, entry.kind)
                }
            };
            if imported == TypeId::UNKNOWN {
                continue;
            }
            return imported;
        }

        TypeId::UNKNOWN
    }

    fn import_project_data_type(
        &mut self,
        qualified_name: &SmolStr,
        decl: &SyntaxNode,
    ) -> Option<TypeId> {
        let type_def = if matches!(
            decl.kind(),
            SyntaxKind::StructDef
                | SyntaxKind::UnionDef
                | SyntaxKind::EnumDef
                | SyntaxKind::ArrayType
                | SyntaxKind::TypeRef
        ) {
            decl.clone()
        } else {
            decl.children().find(|child| {
                matches!(
                    child.kind(),
                    SyntaxKind::StructDef
                        | SyntaxKind::UnionDef
                        | SyntaxKind::EnumDef
                        | SyntaxKind::ArrayType
                        | SyntaxKind::TypeRef
                )
            })?
        };
        let (_, namespace_parts) = split_imported_type_name(qualified_name)?;
        let previous_namespace = self.namespace_override.replace(namespace_parts);
        let type_id = self.register_imported_data_type(&type_def, qualified_name.clone());
        self.namespace_override = previous_namespace;
        (type_id != TypeId::UNKNOWN).then_some(type_id)
    }

    fn register_imported_data_type(
        &mut self,
        type_def: &SyntaxNode,
        qualified_name: SmolStr,
    ) -> TypeId {
        match type_def.kind() {
            SyntaxKind::StructDef => self.collect_struct_type(type_def, qualified_name),
            SyntaxKind::UnionDef => self.collect_union_type(type_def, qualified_name),
            SyntaxKind::EnumDef => self.collect_imported_enum_type(type_def, qualified_name),
            SyntaxKind::ArrayType => {
                let target_type = self.collect_array_type(type_def);
                self.table.register_type(
                    qualified_name.clone(),
                    Type::Alias {
                        name: qualified_name,
                        target: target_type,
                    },
                )
            }
            SyntaxKind::TypeRef => {
                let target_type = self.resolve_type_from_ref(type_def);
                self.table.register_type(
                    qualified_name.clone(),
                    Type::Alias {
                        name: qualified_name,
                        target: target_type,
                    },
                )
            }
            _ => TypeId::UNKNOWN,
        }
    }

    pub(super) fn collect_struct_type(&mut self, node: &SyntaxNode, name: SmolStr) -> TypeId {
        let mut fields = Vec::new();

        for var_decl in node.children().filter(|n| n.kind() == SyntaxKind::VarDecl) {
            let (field_names, field_type, direct_address) = self.extract_var_decl_info(&var_decl);
            for (field_name, range) in field_names {
                self.validate_identifier(&field_name, range, false);
                fields.push(StructField {
                    name: field_name,
                    type_id: field_type,
                    address: direct_address.clone(),
                });
            }
        }

        self.table.register_struct_type(name, fields)
    }

    pub(super) fn collect_union_type(&mut self, node: &SyntaxNode, name: SmolStr) -> TypeId {
        let mut variants = Vec::new();

        for var_decl in node.children().filter(|n| n.kind() == SyntaxKind::VarDecl) {
            let (field_names, field_type, direct_address) = self.extract_var_decl_info(&var_decl);
            for (field_name, range) in field_names {
                self.validate_identifier(&field_name, range, false);
                variants.push(UnionVariant {
                    name: field_name,
                    type_id: field_type,
                    address: direct_address.clone(),
                });
            }
        }

        self.table.register_union_type(name, variants)
    }

    pub(super) fn collect_enum_type(&mut self, node: &SyntaxNode, name: SmolStr) -> TypeId {
        let mut values = Vec::new();
        let mut value_symbols = Vec::new();
        let mut next_value: i64 = 0;
        let mut base_type = TypeId::INT; // Default base type

        // Check for base type specification
        if let Some(type_ref) = node.children().find(|n| n.kind() == SyntaxKind::TypeRef) {
            base_type = self.resolve_type_from_ref(&type_ref);
        }

        // Collect enum values
        for child in node.children() {
            if child.kind() == SyntaxKind::EnumValue {
                if let Some((value_name, range)) = name_from_node(&child) {
                    self.validate_identifier(&value_name, range, false);
                    // Check for explicit value assignment
                    let value = self.extract_enum_value(&child).unwrap_or(next_value);
                    values.push((value_name.clone(), value));
                    value_symbols.push((value_name, value, range));
                    next_value = value + 1;
                }
            } else if child.kind() == SyntaxKind::Name {
                // Simple enum value without EnumValue wrapper
                if let Some((value_name, range)) = name_from_node(&child) {
                    self.validate_identifier(&value_name, range, false);
                    values.push((value_name.clone(), next_value));
                    value_symbols.push((value_name, next_value, range));
                    next_value += 1;
                }
            }
        }

        let type_id = self.table.register_enum_type(name, base_type, values);
        for (value_name, value, range) in value_symbols {
            let mut symbol = Symbol::new(
                SymbolId::UNKNOWN,
                value_name,
                SymbolKind::EnumValue { value },
                type_id,
                range,
            );
            symbol.parent = self.current_parent();
            self.declare_symbol(symbol);
        }

        type_id
    }

    fn collect_imported_enum_type(&mut self, node: &SyntaxNode, name: SmolStr) -> TypeId {
        let mut values = Vec::new();
        let mut next_value: i64 = 0;
        let mut base_type = TypeId::INT;

        if let Some(type_ref) = node.children().find(|n| n.kind() == SyntaxKind::TypeRef) {
            base_type = self.resolve_type_from_ref(&type_ref);
        }

        for child in node.children() {
            if child.kind() == SyntaxKind::EnumValue {
                if let Some((value_name, _range)) = name_from_node(&child) {
                    let value = self.extract_enum_value(&child).unwrap_or(next_value);
                    values.push((value_name, value));
                    next_value = value + 1;
                }
            } else if child.kind() == SyntaxKind::Name {
                if let Some((value_name, _range)) = name_from_node(&child) {
                    values.push((value_name, next_value));
                    next_value += 1;
                }
            }
        }

        self.table.register_enum_type(name, base_type, values)
    }

    pub(super) fn extract_enum_value(&mut self, node: &SyntaxNode) -> Option<i64> {
        let expr = node
            .children()
            .find(|child| is_expression_kind(child.kind()))?;
        let scopes = scope_chain_for_node(node);
        self.eval_int_expr_in_scope(&expr, &scopes)
    }

    pub(super) fn resolve_type_from_ref(&mut self, node: &SyntaxNode) -> TypeId {
        // Handle array types
        if let Some(array_node) = node.children().find(|n| n.kind() == SyntaxKind::ArrayType) {
            return self.collect_array_type(&array_node);
        }

        // Handle pointer types
        if let Some(pointer_node) = node
            .children()
            .find(|n| n.kind() == SyntaxKind::PointerType)
        {
            if let Some(inner_ref) = pointer_node
                .children()
                .find(|n| n.kind() == SyntaxKind::TypeRef)
            {
                let target = self.resolve_type_from_ref(&inner_ref);
                return self.table.register_pointer_type(target);
            }
        }

        // Handle reference types
        if let Some(ref_node) = node
            .children()
            .find(|n| n.kind() == SyntaxKind::ReferenceType)
        {
            if let Some(inner_ref) = ref_node
                .children()
                .find(|n| n.kind() == SyntaxKind::TypeRef)
            {
                let target = self.resolve_type_from_ref(&inner_ref);
                return self.table.register_reference_type(target);
            }
        }

        // Handle string types with length
        if let Some(string_node) = node.children().find(|n| n.kind() == SyntaxKind::StringType) {
            return self.collect_string_type(&string_node);
        }

        let subrange_node = node.children().find(|n| n.kind() == SyntaxKind::Subrange);

        // Handle simple type name
        if let Some((parts, range)) = type_path_from_type_ref(node) {
            let names: Vec<SmolStr> = parts.iter().map(|(name, _)| name.clone()).collect();
            let type_id = self.resolve_type_path(&names);
            if type_id == TypeId::UNKNOWN {
                self.pending_types.push(PendingType {
                    name: qualified_name_string(&names),
                    range,
                    scope_id: self.table.current_scope(),
                });
            }
            if let Some(subrange) = subrange_node {
                return self.collect_subrange_type(type_id, &subrange);
            }
            return type_id;
        }

        TypeId::UNKNOWN
    }

    pub(super) fn collect_array_type(&mut self, node: &SyntaxNode) -> TypeId {
        let mut dimensions = Vec::new();
        let mut element_type = TypeId::UNKNOWN;
        let subranges: Vec<_> = node
            .children()
            .filter(|n| n.kind() == SyntaxKind::Subrange)
            .collect();

        // Collect dimensions from Subrange children
        for subrange in &subranges {
            if let Some((lower, upper)) = self.extract_subrange(subrange) {
                dimensions.push((lower, upper));
            }
        }

        // Get element type from TypeRef child
        if let Some(type_ref) = node.children().find(|n| n.kind() == SyntaxKind::TypeRef) {
            element_type = self.resolve_type_from_ref(&type_ref);
        }

        if dimensions.is_empty() {
            // Single dimension without subrange, assume 0..MAX
            dimensions.push((0, i64::MAX));
        }

        self.validate_array_wildcard_usage(node, &subranges, &dimensions);

        self.table.register_array_type(element_type, dimensions)
    }

    pub(super) fn extract_subrange(&mut self, node: &SyntaxNode) -> Option<(i64, i64)> {
        if subrange_is_exact_wildcard(node) || subrange_contains_star(node) {
            return Some((0, i64::MAX));
        }
        let mut values = Vec::new();
        let scopes = scope_chain_for_node(node);
        for child in node.children().filter(|n| is_expression_kind(n.kind())) {
            let value = self.eval_int_expr_in_scope(&child, &scopes)?;
            values.push(value);
        }
        if values.len() >= 2 {
            Some((values[0], values[1]))
        } else if values.len() == 1 {
            Some((0, values[0]))
        } else {
            None
        }
    }

    fn validate_array_wildcard_usage(
        &mut self,
        node: &SyntaxNode,
        subranges: &[SyntaxNode],
        dimensions: &[(i64, i64)],
    ) {
        let contains_star = subranges.iter().any(subrange_contains_star);
        if !contains_star {
            return;
        }

        let wildcard_dims = subranges
            .iter()
            .filter(|subrange| subrange_is_exact_wildcard(subrange))
            .count();

        let allowed_context = node
            .ancestors()
            .find(|ancestor| ancestor.kind() == SyntaxKind::VarBlock)
            .is_some_and(|block| {
                matches!(
                    var_qualifier_from_block(&block),
                    VarQualifier::Input | VarQualifier::Output | VarQualifier::InOut
                )
            });

        if !allowed_context {
            self.diagnostics.error(
                DiagnosticCode::InvalidOperation,
                node.text_range(),
                "ARRAY[*] array wildcard '*' is only allowed in VAR_INPUT, VAR_OUTPUT, or VAR_IN_OUT declarations",
            );
        }

        let exact_wildcard_dims = dimensions.iter().filter(|dim| dim.is_wildcard()).count();
        if subranges.len() != 1 || wildcard_dims != 1 || exact_wildcard_dims != 1 {
            self.diagnostics.error(
                DiagnosticCode::InvalidOperation,
                node.text_range(),
                "ARRAY[*] wildcard form requires exactly one wildcard dimension",
            );
        }
    }

    pub(super) fn collect_string_type(&mut self, node: &SyntaxNode) -> TypeId {
        // Check if it's STRING or WSTRING
        let is_wstring = node
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .any(|t| t.kind() == SyntaxKind::KwWString);

        // Look for length specification
        if let Some(expr) = node.children().find(|n| is_expression_kind(n.kind())) {
            let scopes = scope_chain_for_node(node);
            if let Some(value) = self.eval_int_expr_in_scope(&expr, &scopes) {
                if value <= 0 {
                    self.diagnostics.error(
                        DiagnosticCode::OutOfRange,
                        expr.text_range(),
                        "string length must be a positive integer",
                    );
                    return if is_wstring {
                        TypeId::WSTRING
                    } else {
                        TypeId::STRING
                    };
                }
                let Ok(len) = u32::try_from(value) else {
                    self.diagnostics.error(
                        DiagnosticCode::OutOfRange,
                        expr.text_range(),
                        "string length is out of range",
                    );
                    return if is_wstring {
                        TypeId::WSTRING
                    } else {
                        TypeId::STRING
                    };
                };
                let name = if is_wstring {
                    format!("WSTRING[{}]", len)
                } else {
                    format!("STRING[{}]", len)
                };
                let ty = if is_wstring {
                    Type::WString { max_len: Some(len) }
                } else {
                    Type::String { max_len: Some(len) }
                };
                return self.table.register_type(name, ty);
            }

            self.diagnostics.error(
                DiagnosticCode::TypeMismatch,
                expr.text_range(),
                "string length must be a constant expression",
            );
        }

        // No length specified, return default STRING or WSTRING
        if is_wstring {
            TypeId::WSTRING
        } else {
            TypeId::STRING
        }
    }

    pub(super) fn collect_subrange_type(&mut self, base_type: TypeId, node: &SyntaxNode) -> TypeId {
        let mut resolved_base = self.table.resolve_alias_type(base_type);
        if let Some(Type::Subrange { base, .. }) = self.table.type_by_id(resolved_base) {
            resolved_base = *base;
        }

        let Some(base) = self.table.type_by_id(resolved_base) else {
            return base_type;
        };

        if !base.is_integer() {
            self.diagnostics.error(
                DiagnosticCode::TypeMismatch,
                node.text_range(),
                "subrange base type must be an integer type",
            );
            return base_type;
        }

        let Some((lower, upper)) = self.extract_subrange_bounds(node) else {
            return base_type;
        };

        self.table
            .register_subrange_type(resolved_base, lower, upper)
    }

    pub(super) fn extract_subrange_bounds(&mut self, node: &SyntaxNode) -> Option<(i64, i64)> {
        let scopes = scope_chain_for_node(node);
        let mut values = Vec::new();
        for child in node.children().filter(|n| is_expression_kind(n.kind())) {
            match self.eval_int_expr_in_scope(&child, &scopes) {
                Some(value) => values.push(value),
                None => {
                    self.diagnostics.error(
                        DiagnosticCode::TypeMismatch,
                        child.text_range(),
                        "subrange bounds must be constant expressions",
                    );
                    return None;
                }
            }
        }

        if values.len() != 2 {
            self.diagnostics.error(
                DiagnosticCode::TypeMismatch,
                node.text_range(),
                "subrange requires lower and upper bounds",
            );
            return None;
        }

        let lower = values[0];
        let upper = values[1];
        if lower > upper {
            self.diagnostics.error(
                DiagnosticCode::OutOfRange,
                node.text_range(),
                "subrange lower bound must not exceed upper bound",
            );
            return None;
        }

        Some((lower, upper))
    }

    pub(super) fn resolve_type_path(&mut self, parts: &[SmolStr]) -> TypeId {
        if parts.is_empty() {
            return TypeId::UNKNOWN;
        }
        if parts.len() == 1 {
            let type_id = self.resolve_type_in_scope(parts[0].as_str(), self.table.current_scope());
            if type_id != TypeId::UNKNOWN {
                return type_id;
            }
            return self.resolve_project_type_path(parts);
        }

        let symbol_id = self.table.resolve_qualified(parts);
        if let Some(symbol) = symbol_id.and_then(|id| self.table.get(id)) {
            if symbol.is_type() {
                return symbol.type_id;
            }
        }
        self.resolve_project_type_path(parts)
    }

    pub(super) fn resolve_type_in_scope(&self, name: &str, scope_id: ScopeId) -> TypeId {
        if let Some(id) = TypeId::from_builtin_name(name) {
            return id;
        }
        if let Some(symbol_id) = self.table.resolve(name, scope_id) {
            if let Some(symbol) = self.table.get(symbol_id) {
                if symbol.is_type() {
                    return symbol.type_id;
                }
            }
        }
        if let Some(id) = self.table.lookup_type(name) {
            return id;
        }
        TypeId::UNKNOWN
    }

    fn project_type_candidates(&self, parts: &[SmolStr]) -> Vec<SmolStr> {
        if parts.is_empty() {
            return Vec::new();
        }
        if parts.len() > 1 {
            return vec![qualified_name_string(parts)];
        }

        let name = parts[0].clone();
        let mut candidates = Vec::new();
        candidates.push(name.clone());

        let current_namespace = self.current_namespace_path();
        for prefix_len in (1..=current_namespace.len()).rev() {
            let qualified = qualify_name(&current_namespace[..prefix_len], &name);
            if !candidates.contains(&qualified) {
                candidates.push(qualified);
            }
        }

        let mut scope_id = Some(self.table.current_scope());
        while let Some(current) = scope_id {
            let Some(scope) = self.table.get_scope(current) else {
                break;
            };
            for using in &scope.using_directives {
                let mut qualified = using.path.clone();
                qualified.push(name.clone());
                let qualified = qualified_name_string(&qualified);
                if !candidates.contains(&qualified) {
                    candidates.push(qualified);
                }
            }
            scope_id = scope.parent;
        }

        candidates
    }

    pub(super) fn resolve_pending_types(&mut self) {
        let pending = std::mem::take(&mut self.pending_types);
        resolve_pending_types_with_table(&self.table, pending, &mut self.diagnostics);
    }

    pub(super) fn register_type_names(&mut self, node: &SyntaxNode, namespace: &[SmolStr]) {
        for child in node.children() {
            if child.kind() == SyntaxKind::Name {
                if let Some((name, _)) = name_from_node(&child) {
                    let qualified = qualify_name(namespace, &name);
                    self.table.register_type(
                        qualified.clone(),
                        Type::Alias {
                            name: qualified,
                            target: TypeId::UNKNOWN,
                        },
                    );
                }
            }
        }
    }

    pub(super) fn return_type_from_node(&mut self, node: &SyntaxNode) -> Option<TypeId> {
        node.children()
            .find(|n| n.kind() == SyntaxKind::TypeRef)
            .map(|type_ref| self.resolve_type_from_ref(&type_ref))
    }

    pub(super) fn property_accessors(&self, node: &SyntaxNode) -> (bool, bool) {
        let mut has_get = false;
        let mut has_set = false;
        for child in node.children() {
            match child.kind() {
                SyntaxKind::PropertyGet => has_get = true,
                SyntaxKind::PropertySet => has_set = true,
                _ => {}
            }
        }
        (has_get, has_set)
    }
}

fn split_imported_type_name(qualified_name: &SmolStr) -> Option<(SmolStr, Vec<SmolStr>)> {
    let parts: Vec<SmolStr> = qualified_name
        .split('.')
        .filter(|part| !part.is_empty())
        .map(SmolStr::new)
        .collect();
    let leaf = parts.last()?.clone();
    let namespace = parts[..parts.len().saturating_sub(1)].to_vec();
    Some((leaf, namespace))
}

fn subrange_is_exact_wildcard(node: &SyntaxNode) -> bool {
    let token_kinds: Vec<_> = node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.kind())
        .collect();
    token_kinds == [SyntaxKind::Star]
}

fn subrange_contains_star(node: &SyntaxNode) -> bool {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .any(|token| token.kind() == SyntaxKind::Star)
}
