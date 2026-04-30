pub(crate) fn resolve_target_at_position(
    db: &Database,
    file_id: FileId,
    position: TextSize,
) -> Option<ResolvedTarget> {
    let context = IdeContext::new(db, file_id);
    context.resolve_target_at_position(position)
}

/// Returns the resolved symbol name at the given position, if any.
pub fn symbol_name_at_position(
    db: &Database,
    file_id: FileId,
    position: TextSize,
) -> Option<SmolStr> {
    let target = resolve_target_at_position(db, file_id, position)?;
    let ResolvedTarget::Symbol(symbol_id) = target else {
        return None;
    };
    let symbols = db.file_symbols_with_project(file_id);
    symbols.get(symbol_id).map(|symbol| symbol.name.clone())
}

pub(crate) fn resolve_target_at_position_with_context(
    db: &Database,
    file_id: FileId,
    position: TextSize,
    source: &str,
    root: &SyntaxNode,
    symbols: &SymbolTable,
) -> Option<ResolvedTarget> {
    let (name, range) = ident_at_offset(source, position)?;
    let anchor = range.start();
    let scope_id = scope_at_position(symbols, root, anchor);

    if let Some(symbol) = symbols.iter().find(|sym| {
        if sym.range != range {
            return false;
        }
        match sym.origin {
            Some(origin) => origin.file_id == file_id,
            None => true,
        }
    }) {
        if let Some(field_target) = field_target_for_symbol_declaration(symbols, symbol) {
            return Some(ResolvedTarget::Field(field_target));
        }
        return Some(ResolvedTarget::Symbol(canonical_symbol_id(
            symbols,
            file_id,
            root,
            symbol.id,
        )));
    }

    let token_candidates = [
        root.token_at_offset(position).right_biased(),
        root.token_at_offset(anchor).right_biased(),
        root.token_at_offset(anchor).left_biased(),
    ];
    for token in token_candidates.into_iter().flatten() {
        let Some(name_node) = name_node_at_token(&token) else {
            continue;
        };
        if name_node.kind() == SyntaxKind::Name {
            if let Some(field_target) = resolve_field_decl_target(symbols, &name_node, name) {
                return Some(ResolvedTarget::Field(field_target));
            }

            if let Some(target) = resolve_field_target(db, file_id, symbols, &name_node, name) {
                return Some(target);
            }

            if let Some(field_expr) = name_node
                .parent()
                .filter(|parent| parent.kind() == SyntaxKind::FieldExpr)
            {
                if let Some(parts) = qualified_name_from_field_expr(&field_expr) {
                    if let Some(symbol_id) = symbols.resolve_qualified(&parts) {
                        return Some(ResolvedTarget::Symbol(canonical_symbol_id(
                            symbols,
                            file_id,
                            root,
                            symbol_id,
                        )));
                    }
                }
            }

            if is_type_name_node(&name_node) {
                if let Some(parts) = qualified_name_parts_from_node(&name_node) {
                    if let Some(symbol_id) = resolve_type_symbol(symbols, &parts, scope_id) {
                        return Some(ResolvedTarget::Symbol(canonical_symbol_id(
                            symbols,
                            file_id,
                            root,
                            symbol_id,
                        )));
                    }
                } else if let Some(symbol_id) =
                    resolve_type_symbol(symbols, &[SmolStr::new(name)], scope_id)
                {
                    return Some(ResolvedTarget::Symbol(canonical_symbol_id(
                        symbols,
                        file_id,
                        root,
                        symbol_id,
                    )));
                }
            }
        } else if name_node.kind() == SyntaxKind::NameRef {
            if let Some(field_target) = resolve_field_decl_target(symbols, &name_node, name) {
                return Some(ResolvedTarget::Field(field_target));
            }

            if let Some(target) = resolve_field_target(db, file_id, symbols, &name_node, name) {
                return Some(target);
            }

            if let Some(field_expr) = name_node
                .parent()
                .filter(|parent| parent.kind() == SyntaxKind::FieldExpr)
            {
                if let Some(parts) = qualified_name_from_field_expr(&field_expr) {
                    if let Some(symbol_id) = symbols.resolve_qualified(&parts) {
                        return Some(ResolvedTarget::Symbol(canonical_symbol_id(
                            symbols,
                            file_id,
                            root,
                            symbol_id,
                        )));
                    }
                }
            }
        }
    }

    if let Some(symbol_id) = symbols.resolve(name, scope_id) {
        return Some(ResolvedTarget::Symbol(canonical_symbol_id(
            symbols,
            file_id,
            root,
            symbol_id,
        )));
    }

    // Some type-usage contexts (for example enum qualified literals like
    // `E_State#Value`) do not classify as a TypeRef node; fall back to type lookup.
    if let Some(symbol_id) = resolve_type_symbol(symbols, &[SmolStr::new(name)], scope_id) {
        return Some(ResolvedTarget::Symbol(canonical_symbol_id(
            symbols,
            file_id,
            root,
            symbol_id,
        )));
    }

    if let Some(symbol_id) = symbols
        .iter()
        .find(|symbol| symbol.is_type() && symbol.name.eq_ignore_ascii_case(name))
        .map(|symbol| symbol.id)
    {
        return Some(ResolvedTarget::Symbol(canonical_symbol_id(
            symbols,
            file_id,
            root,
            symbol_id,
        )));
    }

    None
}

fn canonical_symbol_id(
    symbols: &SymbolTable,
    local_file_id: FileId,
    root: &SyntaxNode,
    symbol_id: SymbolId,
) -> SymbolId {
    let Some(symbol) = symbols.get(symbol_id) else {
        return symbol_id;
    };
    if let Some(origin) = symbol.origin {
        if origin.file_id != local_file_id {
            if let Some(candidate_id) = canonical_origin_candidate(symbols, symbol_id, origin) {
                return candidate_id;
            }
        }
    }
    if symbol_declares_external(root, symbol.range) {
        if let Some(candidate_id) = canonical_external_target(symbols, local_file_id, symbol_id) {
            return candidate_id;
        }
    }
    symbol_id
}

fn canonical_origin_candidate(
    symbols: &SymbolTable,
    symbol_id: SymbolId,
    origin: trust_hir::symbols::SymbolOrigin,
) -> Option<SymbolId> {
    symbols
        .iter()
        .find(|candidate| {
            candidate.id != symbol_id
                && candidate.origin == Some(origin)
                && !matches!(
                    candidate.kind,
                    SymbolKind::Variable {
                        qualifier: trust_hir::symbols::VarQualifier::External,
                    }
                )
        })
        .map(|candidate| candidate.id)
}

fn canonical_external_target(
    symbols: &SymbolTable,
    local_file_id: FileId,
    symbol_id: SymbolId,
) -> Option<SymbolId> {
    let symbol = symbols.get(symbol_id)?;
    symbols
        .iter()
        .find(|candidate| {
            candidate.id != symbol_id
                && candidate.name.eq_ignore_ascii_case(&symbol.name)
                && candidate.origin.map(|origin| origin.file_id) != Some(local_file_id)
                && !matches!(
                    candidate.kind,
                    SymbolKind::Variable {
                        qualifier: trust_hir::symbols::VarQualifier::External,
                    }
                )
                && std::mem::discriminant(&candidate.kind) == std::mem::discriminant(&symbol.kind)
        })
        .map(|candidate| candidate.id)
}

fn symbol_declares_external(root: &SyntaxNode, symbol_range: TextRange) -> bool {
    find_var_decl_for_range(root, symbol_range)
        .and_then(|var_decl| {
            var_decl
                .ancestors()
                .find(|node| node.kind() == SyntaxKind::VarBlock)
        })
        .map(|block| {
            block
                .descendants_with_tokens()
                .filter_map(|element| element.into_token())
                .any(|token| token.kind() == SyntaxKind::KwVarExternal)
        })
        .unwrap_or(false)
}

fn field_target_for_symbol_declaration(
    symbols: &SymbolTable,
    symbol: &Symbol,
) -> Option<FieldTarget> {
    if !matches!(
        symbol.kind,
        SymbolKind::Variable { .. } | SymbolKind::Constant
    ) {
        return None;
    }
    let parent_id = symbol.parent?;
    let parent = symbols.get(parent_id)?;
    if !matches!(parent.kind, SymbolKind::Type) {
        return None;
    }
    let type_id = symbols.resolve_alias_type(parent.type_id);
    match symbols.type_by_id(type_id) {
        Some(Type::Struct { .. } | Type::Union { .. }) => Some(FieldTarget {
            type_id,
            name: symbol.name.clone(),
            type_name: Some(parent.name.clone()),
        }),
        _ => None,
    }
}
fn name_node_at_token(token: &SyntaxToken) -> Option<SyntaxNode> {
    token
        .parent_ancestors()
        .find(|n| matches!(n.kind(), SyntaxKind::Name | SyntaxKind::NameRef))
}

pub(crate) fn is_type_name_node(name_node: &SyntaxNode) -> bool {
    name_node.ancestors().skip(1).any(|n| {
        matches!(
            n.kind(),
            SyntaxKind::TypeRef | SyntaxKind::ExtendsClause | SyntaxKind::ImplementsClause
        )
    })
}

pub(crate) fn resolve_type_symbol(
    symbols: &SymbolTable,
    parts: &[SmolStr],
    scope_id: ScopeId,
) -> Option<SymbolId> {
    if parts.is_empty() {
        return None;
    }
    if parts.len() > 1 {
        let symbol_id = symbols.resolve_qualified(parts)?;
        return symbols
            .get(symbol_id)
            .filter(|sym| sym.is_type())
            .map(|sym| sym.id);
    }
    if let Some(symbol_id) = symbols.resolve(parts[0].as_str(), scope_id) {
        if let Some(symbol) = symbols.get(symbol_id) {
            if symbol.is_type() {
                return Some(symbol_id);
            }
        }
    }
    let type_id = symbols.lookup_registered_type_name(parts[0].as_str())?;
    symbols
        .iter()
        .find(|sym| sym.is_type() && sym.type_id == type_id)
        .map(|sym| sym.id)
}

pub(crate) fn resolve_type_symbol_at_node(
    symbols: &SymbolTable,
    root: &SyntaxNode,
    name_node: &SyntaxNode,
) -> Option<SymbolId> {
    let parts = qualified_name_parts_from_node(name_node)?;
    if parts.is_empty() {
        return None;
    }
    let scope_id = scope_at_position(symbols, root, name_node.text_range().start());
    resolve_type_symbol(symbols, &parts, scope_id)
}
