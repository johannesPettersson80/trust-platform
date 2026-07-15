#[derive(Clone)]
struct CallParameterContext {
    name: SmolStr,
    type_id: Option<TypeId>,
}

fn lower_call_parameter_contexts(
    target: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Vec<CallParameterContext>, CompileError> {
    let (semantic_db, semantic_file_id) = match (ctx.semantic_db, ctx.semantic_file_id) {
        (Some(db), Some(file_id)) => (db, file_id),
        _ => return Ok(Vec::new()),
    };
    let analysis = semantic_db.analyze(semantic_file_id);
    let symbols = analysis.symbols.as_ref();
    let Some(target_id) = call_target_symbol_id(target, ctx, symbols) else {
        return Ok(Vec::new());
    };
    let parameter_ids = callable_parameter_ids(symbols, target_id);
    let mut contexts = Vec::with_capacity(parameter_ids.len());
    for parameter_id in parameter_ids {
        let Some(parameter) = symbols.get(parameter_id) else {
            continue;
        };
        if !matches!(parameter.kind, SymbolKind::Parameter { .. }) {
            continue;
        }
        let type_id = lower_concrete_call_parameter_type(parameter.type_id, ctx, symbols)?;
        contexts.push(CallParameterContext {
            name: parameter.name.clone(),
            type_id,
        });
    }
    Ok(contexts)
}

fn call_target_symbol_id(
    target: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
    symbols: &SymbolTable,
) -> Option<SymbolId> {
    match target.kind() {
        SyntaxKind::NameRef => {
            let scope = expression_scope_context(symbols, target);
            resolve_name_symbol_in_scope(
                symbols,
                scope.scope_id,
                scope.current_pou_symbol,
                scope.this_type,
                node_text(target).as_str(),
            )
        }
        SyntaxKind::FieldExpr => {
            if let Some(qualified) = qualified_type_operand_name(target) {
                let parts = qualified.split('.').map(SmolStr::new).collect::<Vec<_>>();
                if let Some(symbol_id) = symbols.resolve_qualified(&parts) {
                    return Some(symbol_id);
                }
            }
            let expressions = direct_expr_children(target);
            let receiver = expressions.first()?;
            let (_, receiver_type) = hir_expression_type(receiver, ctx)?;
            let field = target
                .children()
                .find(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::Literal))?;
            symbols.resolve_member_symbol_in_type(receiver_type, node_text(&field).as_str())
        }
        _ => None,
    }
}

fn callable_parameter_ids(symbols: &SymbolTable, target_id: SymbolId) -> Vec<SymbolId> {
    let Some(target) = symbols.get(target_id) else {
        return Vec::new();
    };
    match &target.kind {
        SymbolKind::Function { parameters, .. } | SymbolKind::Method { parameters, .. } => {
            parameters.clone()
        }
        SymbolKind::Variable { .. } | SymbolKind::Parameter { .. } => {
            let Some(owner_id) = class_owner_from_type(symbols, target.type_id, false) else {
                return Vec::new();
            };
            parameters_from_owner_scope(symbols, owner_id)
        }
        SymbolKind::FunctionBlock => parameters_from_owner_scope(symbols, target_id),
        _ => Vec::new(),
    }
}

fn parameters_from_owner_scope(symbols: &SymbolTable, owner_id: SymbolId) -> Vec<SymbolId> {
    let Some(scope_id) = symbols.scope_for_owner(owner_id) else {
        return Vec::new();
    };
    let Some(scope) = symbols.get_scope(scope_id) else {
        return Vec::new();
    };
    let mut parameters = scope
        .symbol_ids()
        .copied()
        .filter(|symbol_id| {
            symbols
                .get(*symbol_id)
                .is_some_and(|symbol| matches!(symbol.kind, SymbolKind::Parameter { .. }))
        })
        .collect::<Vec<_>>();
    parameters.sort_by_key(|symbol_id| {
        symbols
            .get(*symbol_id)
            .map(|symbol| symbol.range.start())
            .unwrap_or_default()
    });
    parameters
}

fn lower_concrete_call_parameter_type(
    hir_type_id: TypeId,
    ctx: &mut LoweringContext<'_>,
    symbols: &SymbolTable,
) -> Result<Option<TypeId>, CompileError> {
    if hir_type_id == TypeId::UNKNOWN {
        return Ok(None);
    }
    let Some(hir_type) = symbols.type_by_id(hir_type_id) else {
        return Ok(None);
    };
    if matches!(
        hir_type,
        Type::Unknown
            | Type::Any
            | Type::AnyDerived
            | Type::AnyElementary
            | Type::AnyMagnitude
            | Type::AnyInt
            | Type::AnyUnsigned
            | Type::AnySigned
            | Type::AnyReal
            | Type::AnyNum
            | Type::AnyDuration
            | Type::AnyBit
            | Type::AnyChars
            | Type::AnyString
            | Type::AnyChar
            | Type::AnyDate
    ) {
        return Ok(None);
    }
    import_hir_type_to_runtime(ctx.registry, symbols, hir_type_id).map(Some)
}
