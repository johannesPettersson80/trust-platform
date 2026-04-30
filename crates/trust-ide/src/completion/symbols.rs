fn symbols_in_scope(
    filter: &SymbolFilter<'_>,
    scope_id: ScopeId,
    stdlib_filter: &StdlibFilter,
) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    let mut seen: FxHashSet<String> = FxHashSet::default();
    for symbol in filter.scope_symbols(scope_id) {
        if matches!(symbol.kind, SymbolKind::FunctionBlock)
            && stdlib_docs::is_standard_fb_name(symbol.name.as_str())
            && !stdlib_filter.allows_function_block(symbol.name.as_str())
        {
            continue;
        }
        let kind = if symbol_is_constant(symbol) {
            CompletionKind::Constant
        } else {
            match symbol.kind {
            SymbolKind::Variable { .. } => CompletionKind::Variable,
            SymbolKind::Constant => CompletionKind::Constant,
            SymbolKind::Function { .. } => CompletionKind::Function,
            SymbolKind::FunctionBlock => CompletionKind::FunctionBlock,
            SymbolKind::Class => CompletionKind::Type,
            SymbolKind::Method { .. } => CompletionKind::Method,
            SymbolKind::Property { .. } => CompletionKind::Property,
            SymbolKind::Interface | SymbolKind::Type => CompletionKind::Type,
            SymbolKind::EnumValue { .. } => CompletionKind::EnumValue,
            SymbolKind::Program
            | SymbolKind::ProgramInstance
            | SymbolKind::Parameter { .. }
            | SymbolKind::Namespace
            | SymbolKind::Configuration
            | SymbolKind::Resource
            | SymbolKind::Task => CompletionKind::Variable,
            }
        };

        let mut item = CompletionItem::new(symbol.name.clone(), kind);
        if let Some(type_name) = TypeId::builtin_name(symbol.type_id) {
            item = item.with_detail(type_name);
        }
        item = attach_symbol_docs(item, symbol, filter, Some(scope_id), stdlib_filter);
        seen.insert(symbol.name.to_ascii_uppercase());
        items.push(item);
    }

    items.extend(using_scope_symbol_completions(
        filter.symbols(),
        scope_id,
        &seen,
        stdlib_filter,
    ));
    items
}

fn type_symbols(filter: &SymbolFilter<'_>) -> Vec<CompletionItem> {
    filter
        .type_symbols()
        .map(|symbol| CompletionItem::new(symbol.name.clone(), CompletionKind::Type))
        .collect()
}

fn member_access_completions(
    db: &Database,
    file_id: trust_hir::db::FileId,
    position: TextSize,
    root: &SyntaxNode,
    symbols: &SymbolTable,
    scope_id: ScopeId,
    stdlib_filter: &StdlibFilter,
) -> Vec<CompletionItem> {
    let Some(base_type) = member_access_base_type(db, file_id, position, root, symbols) else {
        return Vec::new();
    };
    let base_type = symbols.resolve_alias_type(base_type);

    match symbols.type_by_id(base_type) {
        Some(Type::Struct { fields, .. }) => fields
            .iter()
            .map(|field| {
                let mut item = CompletionItem::new(field.name.clone(), CompletionKind::Variable)
                    .with_priority(10);
                if let Some(detail) = type_detail(symbols, field.type_id) {
                    item = item.with_detail(detail);
                }
                item
            })
            .collect(),
        Some(Type::Union { variants, .. }) => variants
            .iter()
            .map(|variant| {
                let mut item = CompletionItem::new(variant.name.clone(), CompletionKind::Variable)
                    .with_priority(10);
                if let Some(detail) = type_detail(symbols, variant.type_id) {
                    item = item.with_detail(detail);
                }
                item
            })
            .collect(),
        Some(Type::Enum { values, .. }) => values
            .iter()
            .map(|(name, _)| {
                CompletionItem::new(name.clone(), CompletionKind::EnumValue).with_priority(10)
            })
            .collect(),
        Some(Type::FunctionBlock { .. } | Type::Class { .. } | Type::Interface { .. }) => {
            member_symbols_for_type(symbols, base_type, scope_id, stdlib_filter)
        }
        _ => Vec::new(),
    }
}

fn member_access_base_type(
    db: &Database,
    file_id: trust_hir::db::FileId,
    position: TextSize,
    root: &SyntaxNode,
    symbols: &SymbolTable,
) -> Option<TypeId> {
    let token = find_token_at_position(root, position)?;
    let dot = if token.kind() == SyntaxKind::Dot {
        Some(token)
    } else {
        previous_non_trivia_token(&token).filter(|t| t.kind() == SyntaxKind::Dot)
    }?;

    if let Some(field_expr) = dot
        .parent_ancestors()
        .find(|n| n.kind() == SyntaxKind::FieldExpr)
    {
        if let Some(base_expr) = field_expr.children().next() {
            if let Some(base_type) =
                base_type_from_expr_node(db, file_id, symbols, root, &base_expr)
            {
                return Some(base_type);
            }
        }
    }

    let offset = u32::from(dot.text_range().start());
    let offset = offset.saturating_sub(1);
    let expr_id = db.expr_id_at_offset(file_id, offset)?;
    Some(db.type_of(file_id, expr_id))
}

fn base_type_from_expr_node(
    db: &Database,
    file_id: trust_hir::db::FileId,
    symbols: &SymbolTable,
    root: &SyntaxNode,
    node: &SyntaxNode,
) -> Option<TypeId> {
    if node.kind() == SyntaxKind::NameRef {
        let ident = node
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| t.kind() == SyntaxKind::Ident)?;
        let scope_id = scope_at_position(symbols, root, node.text_range().start());
        if let Some(symbol_id) = symbols.resolve(ident.text(), scope_id) {
            if let Some(symbol) = symbols.get(symbol_id) {
                return Some(symbol.type_id);
            }
        }
    }

    let offset = u32::from(node.text_range().start());
    let expr_id = db.expr_id_at_offset(file_id, offset)?;
    Some(db.type_of(file_id, expr_id))
}

fn member_symbols_for_type(
    symbols: &SymbolTable,
    type_id: TypeId,
    scope_id: ScopeId,
    stdlib_filter: &StdlibFilter,
) -> Vec<CompletionItem> {
    let filter = SymbolFilter::new(symbols);
    let Some(owner_id) = filter.owner_for_type(type_id) else {
        return Vec::new();
    };
    let current_owner = current_owner_for_scope(symbols, scope_id);
    let current_namespace = namespace_path_for_scope(symbols, scope_id);

    filter
        .members_in_hierarchy(owner_id, |symbol| is_member_symbol_kind(&symbol.kind))
        .into_iter()
        .filter(|symbol| {
            is_member_visible(symbols, symbol, owner_id, current_owner, &current_namespace)
        })
        .filter_map(|symbol| completion_item_for_symbol(symbol, symbols, stdlib_filter))
        .collect()
}

fn current_owner_for_scope(symbols: &SymbolTable, scope_id: ScopeId) -> Option<SymbolId> {
    let mut current = Some(scope_id);
    while let Some(scope_id) = current {
        let Some(scope) = symbols.get_scope(scope_id) else {
            break;
        };
        if let Some(owner_id) = scope.owner {
            if let Some(symbol) = symbols.get(owner_id) {
                match symbol.kind {
                    SymbolKind::Class | SymbolKind::FunctionBlock => return Some(owner_id),
                    SymbolKind::Method { .. } | SymbolKind::Property { .. } => {
                        if let Some(parent) = symbol.parent {
                            return Some(parent);
                        }
                        return Some(owner_id);
                    }
                    _ => {}
                }
            }
        }
        current = scope.parent;
    }
    None
}

fn namespace_path_for_scope(symbols: &SymbolTable, scope_id: ScopeId) -> Vec<SmolStr> {
    let mut current = Some(scope_id);
    while let Some(scope_id) = current {
        let Some(scope) = symbols.get_scope(scope_id) else {
            break;
        };
        if let Some(owner_id) = scope.owner {
            if let Some(symbol) = symbols.get(owner_id) {
                return namespace_path_for_symbol(symbols, symbol);
            }
        }
        current = scope.parent;
    }
    Vec::new()
}

fn is_member_visible(
    symbols: &SymbolTable,
    member: &trust_hir::symbols::Symbol,
    owner_id: SymbolId,
    current_owner: Option<SymbolId>,
    current_namespace: &[SmolStr],
) -> bool {
    match member.visibility {
        Visibility::Public => true,
        Visibility::Private => current_owner == Some(owner_id),
        Visibility::Protected => {
            current_owner.is_some_and(|current| is_same_or_derived(symbols, current, owner_id))
        }
        Visibility::Internal => {
            let owner_namespace = symbols
                .get(owner_id)
                .map(|symbol| namespace_path_for_symbol(symbols, symbol))
                .unwrap_or_default();
            owner_namespace == current_namespace
        }
    }
}

fn is_same_or_derived(symbols: &SymbolTable, derived_id: SymbolId, base_id: SymbolId) -> bool {
    if derived_id == base_id {
        return true;
    }
    let mut visited: FxHashSet<SymbolId> = FxHashSet::default();
    let mut current = symbols.extends_reference(derived_id).and_then(|reference| {
        let name = reference.name().display();
        symbols.resolve_oop_reference_for_owner(derived_id, name.as_str())
    });
    while let Some(symbol_id) = current {
        if !visited.insert(symbol_id) {
            break;
        }
        if symbol_id == base_id {
            return true;
        }
        current = symbols.extends_reference(symbol_id).and_then(|reference| {
            let name = reference.name().display();
            symbols.resolve_oop_reference_for_owner(symbol_id, name.as_str())
        });
    }
    false
}

fn completion_item_for_symbol(
    symbol: &trust_hir::symbols::Symbol,
    symbols: &SymbolTable,
    stdlib_filter: &StdlibFilter,
) -> Option<CompletionItem> {
    if matches!(symbol.kind, SymbolKind::FunctionBlock)
        && stdlib_docs::is_standard_fb_name(symbol.name.as_str())
        && !stdlib_filter.allows_function_block(symbol.name.as_str())
    {
        return None;
    }
    let kind = if symbol_is_constant(symbol) {
        CompletionKind::Constant
    } else {
        match symbol.kind {
        SymbolKind::Variable { .. } => CompletionKind::Variable,
        SymbolKind::Function { .. } => CompletionKind::Function,
        SymbolKind::Method { .. } => CompletionKind::Method,
        SymbolKind::Property { .. } => CompletionKind::Property,
        _ => return None,
        }
    };
    let mut item = CompletionItem::new(symbol.name.clone(), kind).with_priority(10);
    if let Some(detail) = type_detail(symbols, symbol.type_id) {
        item = item.with_detail(detail);
    }
    item = attach_symbol_docs_simple(item, symbol, symbols, stdlib_filter);
    Some(item)
}

fn attach_symbol_docs_simple(
    mut item: CompletionItem,
    symbol: &trust_hir::symbols::Symbol,
    symbols: &SymbolTable,
    stdlib_filter: &StdlibFilter,
) -> CompletionItem {
    let mut docs = Vec::new();
    if let Some(existing) = &item.documentation {
        docs.push(existing.to_string());
    }
    if let Some(doc) = &symbol.doc {
        docs.push(doc.to_string());
    } else if stdlib_filter.allows_function_block(symbol.name.as_str()) {
        if let Some(std_doc) = stdlib_docs::standard_fb_doc(symbol.name.as_str()) {
            docs.push(std_doc.to_string());
        }
    }
    if let Some(namespace) = namespace_string_for_symbol(symbols, symbol) {
        docs.push(format!("Namespace: {namespace}"));
    }
    if let Some(visibility) = visibility_label(symbol.visibility) {
        docs.push(format!("Visibility: {visibility}"));
    }
    if let Some(mods) = modifiers_label(symbol.modifiers) {
        docs.push(format!("Modifiers: {mods}"));
    }
    if !docs.is_empty() {
        item.documentation = Some(SmolStr::new(docs.join("\n\n")));
    }
    item
}

fn attach_symbol_docs(
    mut item: CompletionItem,
    symbol: &trust_hir::symbols::Symbol,
    filter: &SymbolFilter<'_>,
    scope_id: Option<ScopeId>,
    stdlib_filter: &StdlibFilter,
) -> CompletionItem {
    let mut docs = Vec::new();
    if let Some(doc) = &symbol.doc {
        docs.push(doc.to_string());
    } else if stdlib_filter.allows_function_block(symbol.name.as_str()) {
        if let Some(std_doc) = stdlib_docs::standard_fb_doc(symbol.name.as_str()) {
            docs.push(std_doc.to_string());
        }
    }
    if let Some(namespace) = namespace_string_for_symbol(filter.symbols(), symbol) {
        docs.push(format!("Namespace: {namespace}"));
    }
    if let Some(scope_id) = scope_id {
        if let Some(using_path) =
            using_path_for_symbol(filter.symbols(), scope_id, symbol.name.as_str(), symbol.id)
        {
            let path = join_namespace_path(&using_path);
            docs.push(format!("USING {path}"));
        }
    }
    if let Some(visibility) = visibility_label(symbol.visibility) {
        docs.push(format!("Visibility: {visibility}"));
    }
    if let Some(mods) = modifiers_label(symbol.modifiers) {
        docs.push(format!("Modifiers: {mods}"));
    }
    if !docs.is_empty() {
        item.documentation = Some(SmolStr::new(docs.join("\n\n")));
    }
    item
}

fn namespace_string_for_symbol(
    symbols: &SymbolTable,
    symbol: &trust_hir::symbols::Symbol,
) -> Option<String> {
    let parts = namespace_path_for_symbol(symbols, symbol);
    if parts.is_empty() {
        return None;
    }
    Some(join_namespace_path(&parts))
}

fn join_namespace_path(parts: &[SmolStr]) -> String {
    let mut out = String::new();
    for (idx, part) in parts.iter().enumerate() {
        if idx > 0 {
            out.push('.');
        }
        out.push_str(part.as_str());
    }
    out
}

fn visibility_label(visibility: trust_hir::symbols::Visibility) -> Option<&'static str> {
    match visibility {
        trust_hir::symbols::Visibility::Public => None,
        trust_hir::symbols::Visibility::Private => Some("PRIVATE"),
        trust_hir::symbols::Visibility::Protected => Some("PROTECTED"),
        trust_hir::symbols::Visibility::Internal => Some("INTERNAL"),
    }
}

fn modifiers_label(modifiers: trust_hir::symbols::SymbolModifiers) -> Option<String> {
    let mut parts = Vec::new();
    if modifiers.is_final {
        parts.push("FINAL");
    }
    if modifiers.is_abstract {
        parts.push("ABSTRACT");
    }
    if modifiers.is_override {
        parts.push("OVERRIDE");
    }
    (!parts.is_empty()).then_some(parts.join(" "))
}

fn using_scope_symbol_completions(
    symbols: &SymbolTable,
    scope_id: ScopeId,
    seen: &FxHashSet<String>,
    stdlib_filter: &StdlibFilter,
) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    let mut known = seen.clone();
    let mut current = Some(scope_id);
    while let Some(scope_id) = current {
        let Some(scope) = symbols.get_scope(scope_id) else {
            break;
        };
        for using in &scope.using_directives {
            let Some(namespace_id) = symbols.resolve_qualified(&using.path) else {
                continue;
            };
            for symbol in symbols
                .iter()
                .filter(|sym| sym.parent == Some(namespace_id))
            {
                if matches!(symbol.kind, SymbolKind::Namespace) {
                    continue;
                }
                if matches!(symbol.kind, SymbolKind::FunctionBlock)
                    && stdlib_docs::is_standard_fb_name(symbol.name.as_str())
                    && !stdlib_filter.allows_function_block(symbol.name.as_str())
                {
                    continue;
                }
                let key = symbol.name.to_ascii_uppercase();
                if !known.insert(key) {
                    continue;
                }

                let kind = if symbol_is_constant(symbol) {
                    CompletionKind::Constant
                } else {
                    match symbol.kind {
                    SymbolKind::Variable { .. } => CompletionKind::Variable,
                    SymbolKind::Constant => CompletionKind::Constant,
                    SymbolKind::Function { .. } => CompletionKind::Function,
                    SymbolKind::FunctionBlock => CompletionKind::FunctionBlock,
                    SymbolKind::Class => CompletionKind::Type,
                    SymbolKind::Method { .. } => CompletionKind::Method,
                    SymbolKind::Property { .. } => CompletionKind::Property,
                    SymbolKind::Interface | SymbolKind::Type => CompletionKind::Type,
                    SymbolKind::EnumValue { .. } => CompletionKind::EnumValue,
                    SymbolKind::Program
                    | SymbolKind::ProgramInstance
                    | SymbolKind::Parameter { .. }
                    | SymbolKind::Namespace
                    | SymbolKind::Configuration
                    | SymbolKind::Resource
                    | SymbolKind::Task => CompletionKind::Variable,
                    }
                };

                let mut item = CompletionItem::new(symbol.name.clone(), kind);
                if let Some(type_name) = TypeId::builtin_name(symbol.type_id) {
                    item = item.with_detail(type_name);
                }
                let path = join_namespace_path(&using.path);
                item.documentation = Some(SmolStr::new(format!("USING {path}")));
                item = attach_symbol_docs_simple(item, symbol, symbols, stdlib_filter);
                items.push(item);
            }
        }
        current = scope.parent;
    }
    items
}

fn standard_function_completions(stdlib_filter: &StdlibFilter) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    for entry in stdlib_docs::standard_function_entries() {
        if !stdlib_filter.allows_function(entry.name.as_str()) {
            continue;
        }
        let mut item =
            CompletionItem::new(entry.name.clone(), CompletionKind::Function).with_priority(120);
        item.detail = Some(SmolStr::new("standard function"));
        item.documentation = Some(SmolStr::new(entry.doc));
        item.insert_text = Some(SmolStr::new(format!("{}($0)", entry.name)));
        items.push(item);
    }
    items
}
