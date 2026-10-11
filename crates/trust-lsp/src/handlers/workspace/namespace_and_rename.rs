fn is_st_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "st" | "pou"))
        .unwrap_or(false)
}

fn is_config_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| CONFIG_FILES.iter().any(|candidate| candidate == &name))
        .unwrap_or(false)
}

fn find_namespace_symbol(symbols: &SymbolTable, stem: &str) -> Option<SymbolId> {
    let mut candidate = None;
    for symbol in symbols.iter() {
        if symbol.origin.is_some() {
            continue;
        }
        if !matches!(symbol.kind, SymbolKind::Namespace) {
            continue;
        }
        if symbol.range.is_empty() {
            continue;
        }
        if !symbol.name.eq_ignore_ascii_case(stem) {
            continue;
        }
        if candidate.is_some() {
            return None;
        }
        candidate = Some(symbol.id);
    }
    candidate
}

fn namespace_full_path(symbols: &SymbolTable, symbol_id: SymbolId) -> Option<Vec<SmolStr>> {
    let mut parts = Vec::new();
    let mut current = Some(symbol_id);
    while let Some(id) = current {
        let symbol = symbols.get(id)?;
        if matches!(symbol.kind, SymbolKind::Namespace) {
            parts.push(symbol.name.clone());
        }
        current = symbol.parent;
    }
    parts.reverse();
    (!parts.is_empty()).then_some(parts)
}

fn using_directive_edits(
    db: &trust_hir::Database,
    documents: &[crate::state::Document],
    namespace_path: &[SmolStr],
    new_name: &str,
) -> HashMap<Url, Vec<TextEdit>> {
    let mut changes = HashMap::new();
    if namespace_path.is_empty() {
        return changes;
    }

    for doc in documents {
        let symbols = db.file_symbols(doc.file_id);
        let mut edits = Vec::new();
        for scope in symbols.scopes() {
            for using in &scope.using_directives {
                if !path_eq_ignore_ascii_case(&using.path, namespace_path) {
                    continue;
                }
                let mut updated = using.path.clone();
                if let Some(last) = updated.last_mut() {
                    *last = SmolStr::new(new_name);
                }
                let new_text = join_namespace_path(&updated);
                edits.push(TextEdit {
                    range: tower_lsp::lsp_types::Range {
                        start: lsp_utils::offset_to_position(
                            &doc.content,
                            using.range.start().into(),
                        ),
                        end: lsp_utils::offset_to_position(&doc.content, using.range.end().into()),
                    },
                    new_text,
                });
            }
        }
        if !edits.is_empty() {
            changes.insert(doc.uri.clone(), edits);
        }
    }
    changes
}

fn path_eq_ignore_ascii_case(a: &[SmolStr], b: &[SmolStr]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .all(|(left, right)| left.eq_ignore_ascii_case(right.as_str()))
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

fn has_conflict(symbols: &SymbolTable, symbol_id: SymbolId, new_name: &str) -> bool {
    let declaring_scope = find_declaring_scope(symbols, symbol_id);
    if let Some(scope) = symbols.get_scope(declaring_scope)
        && let Some(existing_id) = scope.lookup_local(new_name)
    {
        return existing_id != symbol_id;
    }
    false
}

fn find_declaring_scope(symbols: &SymbolTable, symbol_id: SymbolId) -> ScopeId {
    for i in 0..symbols.scope_count() {
        let scope_id = ScopeId(i as u32);
        if let Some(scope) = symbols.get_scope(scope_id)
            && scope.symbols.values().any(|&id| id == symbol_id)
        {
            return scope_id;
        }
    }
    ScopeId::GLOBAL
}
