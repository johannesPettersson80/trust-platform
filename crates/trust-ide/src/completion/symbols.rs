fn symbols_in_scope(
    filter: &SymbolFilter<'_>,
    scope_id: ScopeId,
    stdlib_filter: &StdlibFilter,
) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    let mut seen: FxHashSet<String> = FxHashSet::default();
    for symbol in filter.scope_symbols(scope_id) {
        // CODESYS's `{attribute 'hide'}`: not offered, though code may name it (specification 14 §6.1.2).
        if symbol.modifiers.hidden {
            continue;
        }
        if matches!(symbol.kind, SymbolKind::FunctionBlock)
            && stdlib_docs::is_standard_fb_name(symbol.name.as_str())
            && !stdlib_filter.allows_function_block(symbol.name.as_str())
        {
            continue;
        }
        let mut item = CompletionItem::new(symbol.name.clone(), symbol_completion_kind(symbol));
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

/// The completion kind of a declaration offered by its name.
fn symbol_completion_kind(symbol: &trust_hir::symbols::Symbol) -> CompletionKind {
    if symbol_is_constant(symbol) {
        return CompletionKind::Constant;
    }
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
        SymbolKind::Namespace => CompletionKind::Namespace,
        SymbolKind::Program
        | SymbolKind::ProgramInstance
        | SymbolKind::Action
        | SymbolKind::Parameter { .. }
        | SymbolKind::Configuration
        | SymbolKind::Resource
        | SymbolKind::Task => CompletionKind::Variable,
    }
}

fn type_symbols(filter: &SymbolFilter<'_>) -> Vec<CompletionItem> {
    filter
        .type_symbols()
        .filter(|symbol| !symbol.modifiers.hidden)
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
    if let Some(items) =
        namespace_member_completions(db, file_id, position, root, symbols, scope_id, stdlib_filter)
    {
        return items;
    }
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
    let dot = member_access_dot(root, position)?;

    if let Some(field_expr) = dot
        .parent_ancestors()
        .find(|n| n.kind() == SyntaxKind::FieldExpr)
        && let Some(base_expr) = field_expr.children().next()
        && let Some(base_type) = base_type_from_expr_node(db, file_id, symbols, root, &base_expr)
    {
        return Some(base_type);
    }

    let offset = u32::from(dot.text_range().start());
    let offset = offset.saturating_sub(1);
    let expr_id = db.expr_id_at_offset(file_id, offset)?;
    Some(db.type_of(file_id, expr_id))
}

/// The declarations of the namespace whose name the dot at `position` follows
/// (`Company.Project.|`), from every file of the project, each one that code there may name (IEC
/// 61131-3 Ed.3 §6.9.3, specification 14 §6.1.2). `None` when the dot follows no namespace, or
/// follows a local declaration named like one, which hides it (specification 09 §1.3).
fn namespace_member_completions(
    db: &Database,
    file_id: trust_hir::db::FileId,
    position: TextSize,
    root: &SyntaxNode,
    file_symbols: &SymbolTable,
    scope_id: ScopeId,
    stdlib_filter: &StdlibFilter,
) -> Option<Vec<CompletionItem>> {
    let dot = member_access_dot(root, position)?;
    let base = dot
        .parent_ancestors()
        .find(|node| node.kind() == SyntaxKind::FieldExpr)?
        .children()
        .next()?;
    let path = match base.kind() {
        SyntaxKind::NameRef => vec![name_from_name_ref(&base)?],
        _ => qualified_name_from_field_expr(&base)?,
    };
    // The file's analysis holds every namespace (specification 14 §10.2), so the base is looked up
    // there, and an ordinary member access (`Timer.`) never pays for the merge of every file's
    // declarations below, which only a namespace's members need.
    let names_a_namespace = file_symbols
        .resolve_qualified(&path)
        .and_then(|id| file_symbols.get(id))
        .is_some_and(|symbol| matches!(symbol.kind, SymbolKind::Namespace));
    if !names_a_namespace || file_symbols.hides_global_head(&path[0], scope_id) {
        return None;
    }
    // Every declaration of the project: the file need not name the one being typed yet.
    let symbols = db.file_symbols_with_every_declaration(file_id);
    let namespace = symbols
        .resolve_qualified(&path)
        .and_then(|id| symbols.get(id))
        .filter(|symbol| matches!(symbol.kind, SymbolKind::Namespace))?;
    let here = namespace_path_for_scope(&symbols, scope_id);
    let items = symbols
        .iter()
        .filter(|member| member.parent == Some(namespace.id) && !member.modifiers.hidden)
        .filter(|member| namespace_member_is_reachable(&symbols, member, &here))
        .map(|member| {
            let mut item = CompletionItem::new(member.name.clone(), symbol_completion_kind(member))
                .with_priority(10);
            if let Some(type_name) = TypeId::builtin_name(member.type_id) {
                item = item.with_detail(type_name);
            }
            attach_symbol_docs_simple(item, member, &symbols, stdlib_filter)
        })
        .collect();
    Some(items)
}

/// Whether code in the namespace `here` may name `member`, a declaration of a namespace, as the
/// analysis decides: nothing below an `INTERNAL` namespace, the member itself or one that contains
/// it, from outside that namespace (specification 03 §4).
fn namespace_member_is_reachable(
    symbols: &SymbolTable,
    member: &trust_hir::symbols::Symbol,
    here: &[SmolStr],
) -> bool {
    let within = |path: &[SmolStr]| {
        here.len() >= path.len()
            && here
                .iter()
                .zip(path)
                .all(|(part, name)| part.eq_ignore_ascii_case(name))
    };
    let mut current = Some(member.id);
    while let Some(id) = current {
        let Some(symbol) = symbols.get(id) else {
            break;
        };
        if matches!(symbol.kind, SymbolKind::Namespace) && symbol.visibility == Visibility::Internal
        {
            let mut path = namespace_path_for_symbol(symbols, symbol);
            path.push(symbol.name.clone());
            if !within(&path) {
                return false;
            }
        }
        current = symbol.parent;
    }
    true
}

fn base_type_from_expr_node(
    db: &Database,
    file_id: trust_hir::db::FileId,
    symbols: &SymbolTable,
    root: &SyntaxNode,
    node: &SyntaxNode,
) -> Option<TypeId> {
    // A declaration named through its namespace (`GVL_IO.gPump`, IEC 61131-3 Ed.3 §6.9.3), unless
    // a local declaration named like the namespace hides it (specification 09 §1.3).
    if node.kind() == SyntaxKind::FieldExpr
        && let Some(path) = qualified_name_from_field_expr(node)
        && !symbols.hides_global_head(
            &path[0],
            scope_at_position(symbols, root, node.text_range().start()),
        )
        && let Some(symbol) = symbols
            .resolve_qualified(&path)
            .and_then(|id| symbols.get(id))
            .filter(|symbol| !matches!(symbol.kind, SymbolKind::Namespace))
    {
        return Some(symbol.type_id);
    }
    if node.kind() == SyntaxKind::NameRef {
        let ident = node
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| t.kind() == SyntaxKind::Ident)?;
        let scope_id = scope_at_position(symbols, root, node.text_range().start());
        if let Some(symbol_id) = symbols.resolve(ident.text(), scope_id)
            && let Some(symbol) = symbols.get(symbol_id)
        {
            return Some(symbol.type_id);
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
    let current_method = current_method_for_scope(symbols, scope_id);
    let current_namespace = namespace_path_for_scope(symbols, scope_id);

    filter
        .members_in_hierarchy(owner_id, |symbol| is_member_symbol_kind(&symbol.kind))
        .into_iter()
        .filter(|symbol| {
            !symbol.modifiers.hidden
                && is_member_visible(symbols, symbol, owner_id, current_owner, &current_namespace)
                && !is_body_only_in(symbol, current_method)
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
        if let Some(owner_id) = scope.owner
            && let Some(symbol) = symbols.get(owner_id)
        {
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
        current = scope.parent;
    }
    None
}

/// The method whose body holds `scope_id`: the innermost POU around it, when that is a method.
fn current_method_for_scope(
    symbols: &SymbolTable,
    scope_id: ScopeId,
) -> Option<&trust_hir::symbols::Symbol> {
    let mut current = Some(scope_id);
    while let Some(scope_id) = current {
        let scope = symbols.get_scope(scope_id)?;
        if let Some(symbol) = scope.owner.and_then(|owner_id| symbols.get(owner_id)) {
            match symbol.kind {
                SymbolKind::Method { .. } => return Some(symbol),
                SymbolKind::Program
                | SymbolKind::Function { .. }
                | SymbolKind::FunctionBlock
                | SymbolKind::Class
                | SymbolKind::Property { .. }
                | SymbolKind::Action => return None,
                _ => {}
            }
        }
        current = scope.parent;
    }
    None
}

/// Whether `member` is one of a block's body-only members, an in-out or a `VAR_TEMP`, which a
/// method of the same block may not name: the analysis rejects it there (specification 14 §6.1.2).
fn is_body_only_in(
    member: &trust_hir::symbols::Symbol,
    method: Option<&trust_hir::symbols::Symbol>,
) -> bool {
    method.is_some_and(|method| method.parent == member.parent)
        && matches!(
            member.kind,
            SymbolKind::Parameter {
                direction: ParamDirection::InOut
            } | SymbolKind::Variable {
                qualifier: trust_hir::symbols::VarQualifier::Temp
            }
        )
}

fn namespace_path_for_scope(symbols: &SymbolTable, scope_id: ScopeId) -> Vec<SmolStr> {
    let mut current = Some(scope_id);
    while let Some(scope_id) = current {
        let Some(scope) = symbols.get_scope(scope_id) else {
            break;
        };
        if let Some(owner_id) = scope.owner
            && let Some(symbol) = symbols.get(owner_id)
        {
            return namespace_path_for_symbol(symbols, symbol);
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

/// Whether `derived_id` is `base_id` or extends it, through any of its bases.
fn is_same_or_derived(symbols: &SymbolTable, derived_id: SymbolId, base_id: SymbolId) -> bool {
    let mut visited: FxHashSet<SymbolId> = FxHashSet::default();
    let mut pending = vec![derived_id];
    while let Some(symbol_id) = pending.pop() {
        if symbol_id == base_id {
            return true;
        }
        if visited.insert(symbol_id) {
            pending.extend(symbols.base_symbols(symbol_id));
        }
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
            SymbolKind::Variable { .. } | SymbolKind::Parameter { .. } => CompletionKind::Variable,
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
    } else if stdlib_filter.allows_function_block(symbol.name.as_str())
        && let Some(std_doc) = stdlib_docs::standard_fb_doc(symbol.name.as_str())
    {
        docs.push(std_doc.to_string());
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
    } else if stdlib_filter.allows_function_block(symbol.name.as_str())
        && let Some(std_doc) = stdlib_docs::standard_fb_doc(symbol.name.as_str())
    {
        docs.push(std_doc.to_string());
    }
    if let Some(namespace) = namespace_string_for_symbol(filter.symbols(), symbol) {
        docs.push(format!("Namespace: {namespace}"));
    }
    if let Some(scope_id) = scope_id
        && let Some(using_path) =
            using_path_for_symbol(filter.symbols(), scope_id, symbol.name.as_str(), symbol.id)
    {
        let path = join_namespace_path(&using_path);
        docs.push(format!("USING {path}"));
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
                if matches!(symbol.kind, SymbolKind::Namespace) || symbol.modifiers.hidden {
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

                let mut item =
                    CompletionItem::new(symbol.name.clone(), symbol_completion_kind(symbol));
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
