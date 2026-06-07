/// Computes completions at the given position.
pub fn complete(
    db: &Database,
    file_id: trust_hir::db::FileId,
    position: TextSize,
) -> Vec<CompletionItem> {
    complete_with_filter(db, file_id, position, &StdlibFilter::allow_all())
}

/// Computes completions with stdlib filtering.
pub fn complete_with_filter(
    db: &Database,
    file_id: trust_hir::db::FileId,
    position: TextSize,
    stdlib_filter: &StdlibFilter,
) -> Vec<CompletionItem> {
    let mut items = Vec::new();

    if let Some(openot_items) = openot_attribute_completions(db, file_id, position) {
        return dedupe_items(openot_items);
    }

    let context = IdeContext::new(db, file_id);
    let root = &context.root;
    let symbols = &context.symbols;
    let filter = SymbolFilter::new(symbols);
    let detect = detect_context(root, position);
    let typed_literal_context = typed_literal_completion_context(&context, position);
    let scope_id = context.scope_at_position(position);

    match detect {
        CompletionContext::TopLevel => {
            items.extend(keyword_snippets());
        }
        CompletionContext::Statement => {
            items.extend(keyword_snippets());
            items.extend(symbols_in_scope(&filter, scope_id, stdlib_filter));
            items.extend(standard_function_completions(stdlib_filter));
            items.extend(typed_literal_completions_with_context(
                typed_literal_context.as_ref(),
            ));
        }
        CompletionContext::MemberAccess => {
            items.extend(member_access_completions(
                db,
                file_id,
                position,
                root,
                symbols,
                scope_id,
                stdlib_filter,
            ));
        }
        CompletionContext::TypeAnnotation => {
            items.extend(type_keywords());
            items.extend(type_symbols(&filter));
        }
        CompletionContext::VarBlock => {
            items.extend(keyword_snippets());
            items.extend(var_block_keywords());
        }
        CompletionContext::Argument => {
            items.extend(parameter_name_completions(db, file_id, position, symbols));
            items.extend(expression_keywords());
            items.extend(symbols_in_scope(&filter, scope_id, stdlib_filter));
            items.extend(standard_function_completions(stdlib_filter));
            items.extend(typed_literal_completions_with_context(
                typed_literal_context.as_ref(),
            ));
        }
        _ => {
            // General: include keywords and symbols
            items.extend(keyword_snippets());
            items.extend(expression_keywords());
            items.extend(symbols_in_scope(&filter, scope_id, stdlib_filter));
            items.extend(standard_function_completions(stdlib_filter));
            items.extend(typed_literal_completions_with_context(
                typed_literal_context.as_ref(),
            ));
        }
    }

    // Sort by priority
    items.sort_by_key(|item| item.sort_priority);
    items = dedupe_items(items);
    items
}

fn dedupe_items(items: Vec<CompletionItem>) -> Vec<CompletionItem> {
    let mut seen: FxHashSet<String> = FxHashSet::default();
    let mut deduped = Vec::new();
    for item in items {
        let key = item.label.to_ascii_uppercase();
        if seen.insert(key) {
            deduped.push(item);
        }
    }
    deduped
}

fn parameter_name_completions(
    db: &Database,
    file_id: trust_hir::db::FileId,
    position: TextSize,
    symbols: &SymbolTable,
) -> Vec<CompletionItem> {
    let Some(context) = call_signature_context(db, file_id, position) else {
        return Vec::new();
    };

    let mut items = Vec::new();
    for param in context.signature.params {
        let key = SmolStr::new(param.name.to_ascii_uppercase());
        if context.used_params.contains(&key) {
            continue;
        }

        let op = match param.direction {
            ParamDirection::Out => "=>",
            ParamDirection::In | ParamDirection::InOut => ":=",
        };

        let type_name = type_detail(symbols, param.type_id)
            .map(|name| name.to_string())
            .or_else(|| param.type_id.builtin_name().map(|name| name.to_string()))
            .unwrap_or_else(|| "?".to_string());
        let direction = match param.direction {
            ParamDirection::In => "IN",
            ParamDirection::Out => "OUT",
            ParamDirection::InOut => "IN_OUT",
        };
        let detail = format!("{direction} : {type_name}");

        let mut item =
            CompletionItem::new(param.name.clone(), CompletionKind::Variable).with_priority(5);
        item.detail = Some(SmolStr::new(detail));
        item.insert_text = Some(SmolStr::new(format!("{} {} $0", param.name, op)));
        items.push(item);
    }

    items
}
