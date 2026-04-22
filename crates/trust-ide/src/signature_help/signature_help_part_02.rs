fn find_token_at_position(root: &SyntaxNode, position: TextSize) -> Option<SyntaxToken> {
    let token = root.token_at_offset(position);
    token
        .clone()
        .right_biased()
        .or_else(|| token.left_biased())
        .or_else(|| root.last_token())
}

fn collect_call_args(arg_list: &SyntaxNode) -> Vec<ArgInfo> {
    let mut args = Vec::new();
    for arg in arg_list.children().filter(|n| n.kind() == SyntaxKind::Arg) {
        let name = arg
            .children()
            .find(|child| child.kind() == SyntaxKind::Name)
            .and_then(|child| name_from_name_node(&child));
        args.push(ArgInfo {
            name,
            range: arg.text_range(),
        });
    }
    args
}

fn active_arg_index(args: &[ArgInfo], arg_list: &SyntaxNode, position: TextSize) -> usize {
    if let Some((index, _)) = args
        .iter()
        .enumerate()
        .find(|(_, arg)| arg.range.contains(position))
    {
        return index;
    }

    let mut comma_count = 0usize;
    for token in arg_list
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
    {
        if token.kind() == SyntaxKind::Comma && token.text_range().end() <= position {
            comma_count += 1;
        }
    }
    comma_count
}

fn active_param_index(args: &[ArgInfo], arg_index: usize, params: &[ParamData]) -> usize {
    if args.is_empty() {
        return 0;
    }
    if arg_index >= args.len() {
        return arg_index;
    }
    let arg = &args[arg_index];
    if let Some(name) = &arg.name {
        if let Some(index) = params
            .iter()
            .position(|param| param.name.eq_ignore_ascii_case(name.as_str()))
        {
            return index;
        }
    }
    arg_index
}

fn arg_types_for_args(db: &Database, file_id: FileId, args: &[ArgInfo]) -> Vec<Option<TypeId>> {
    args.iter()
        .map(|arg| arg_type_at_range(db, file_id, arg.range))
        .collect()
}

fn arg_type_at_range(db: &Database, file_id: FileId, range: TextRange) -> Option<TypeId> {
    let offset = range.start();
    let expr_id = db.expr_id_at_offset(file_id, offset.into())?;
    Some(db.type_of(file_id, expr_id))
}

fn apply_arg_types(signature: &SignatureInfo, arg_types: &[Option<TypeId>]) -> SignatureInfo {
    let mut updated = signature.clone();
    for (idx, param) in updated.params.iter_mut().enumerate() {
        let Some(Some(arg_type)) = arg_types.get(idx) else {
            continue;
        };
        if is_generic_type(param.type_id) {
            param.type_id = *arg_type;
        }
    }

    if let Some(return_type) = updated.return_type {
        if is_generic_type(return_type) {
            if let Some(Some(arg_type)) = arg_types.first() {
                updated.return_type = Some(*arg_type);
            }
        }
    } else if let Some(Some(arg_type)) = arg_types.first() {
        updated.return_type = Some(*arg_type);
    }

    updated
}

fn is_generic_type(type_id: TypeId) -> bool {
    matches!(
        type_id,
        TypeId::ANY
            | TypeId::ANY_DERIVED
            | TypeId::ANY_ELEMENTARY
            | TypeId::ANY_MAGNITUDE
            | TypeId::ANY_INT
            | TypeId::ANY_UNSIGNED
            | TypeId::ANY_SIGNED
            | TypeId::ANY_REAL
            | TypeId::ANY_NUM
            | TypeId::ANY_DURATION
            | TypeId::ANY_BIT
            | TypeId::ANY_CHARS
            | TypeId::ANY_STRING
            | TypeId::ANY_CHAR
            | TypeId::ANY_DATE
    )
}

fn strip_execution_params(signature: &SignatureInfo) -> SignatureInfo {
    let mut filtered = signature.clone();
    filtered
        .params
        .retain(|param| !is_execution_param_name(param.name.as_str()));
    filtered
}

fn is_execution_param_name(name: &str) -> bool {
    name.eq_ignore_ascii_case("EN") || name.eq_ignore_ascii_case("ENO")
}

fn callee_name_offset(call_expr: &SyntaxNode) -> Option<TextSize> {
    let arg_list = call_expr
        .children()
        .find(|child| child.kind() == SyntaxKind::ArgList)?;
    let limit = arg_list.text_range().start();
    call_expr
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| token.kind() == SyntaxKind::Ident && token.text_range().end() <= limit)
        .last()
        .map(|token| token.text_range().start())
}

fn callee_name_text(call_expr: &SyntaxNode) -> Option<SmolStr> {
    let arg_list = call_expr
        .children()
        .find(|child| child.kind() == SyntaxKind::ArgList)?;
    let limit = arg_list.text_range().start();
    call_expr
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| token.kind() == SyntaxKind::Ident && token.text_range().end() <= limit)
        .last()
        .map(|token| SmolStr::new(token.text()))
}

fn signature_from_symbol(symbols: &SymbolTable, symbol: &Symbol) -> Option<SignatureInfo> {
    let params = callable_params(symbols, symbol);
    if params.is_empty() && !symbol.is_callable() {
        return None;
    }

    let return_type = match symbol.kind {
        SymbolKind::Function { return_type, .. } => Some(return_type),
        SymbolKind::Method { return_type, .. } => return_type,
        _ => None,
    };

    Some(SignatureInfo {
        name: symbol.name.clone(),
        params,
        return_type,
    })
}

fn signature_from_type(symbols: &SymbolTable, type_id: TypeId) -> Option<SignatureInfo> {
    let symbol = symbols.iter().find(|sym| {
        sym.type_id == type_id
            && matches!(
                sym.kind,
                SymbolKind::FunctionBlock | SymbolKind::Class | SymbolKind::Interface
            )
    })?;

    let params = callable_params(symbols, symbol);
    Some(SignatureInfo {
        name: symbol.name.clone(),
        params,
        return_type: None,
    })
}

fn callable_params(symbols: &SymbolTable, symbol: &Symbol) -> Vec<ParamData> {
    let mut ids: Vec<_> = match &symbol.kind {
        SymbolKind::Function { parameters, .. } | SymbolKind::Method { parameters, .. } => {
            parameters.clone()
        }
        _ => Vec::new(),
    };

    if ids.is_empty() {
        ids = symbols
            .iter()
            .filter(|sym| {
                sym.parent == Some(symbol.id) && matches!(sym.kind, SymbolKind::Parameter { .. })
            })
            .map(|sym| sym.id)
            .collect();
    }

    ids.sort_by_key(|id| id.0);
    ids.into_iter()
        .filter_map(|id| {
            let sym = symbols.get(id)?;
            match sym.kind {
                SymbolKind::Parameter { direction } => Some(ParamData {
                    name: sym.name.clone(),
                    type_id: sym.type_id,
                    direction,
                    is_constant: sym.is_constant,
                }),
                _ => None,
            }
        })
        .collect()
}

fn format_signature_label(symbols: &SymbolTable, signature: &SignatureInfo) -> String {
    let params = signature
        .params
        .iter()
        .map(|param| format_param_label(symbols, param))
        .collect::<Vec<_>>()
        .join(", ");

    let mut label = format!("{}({})", signature.name, params);
    if let Some(return_type) = signature.return_type {
        let ret_name = format_type_name(symbols, return_type);
        label.push_str(&format!(" : {}", ret_name));
    }
    label
}

fn format_param_label(symbols: &SymbolTable, param: &ParamData) -> String {
    let type_name = format_type_name(symbols, param.type_id);
    let mut label = format!("{}: {}", param.name, type_name);
    let suffix = match param.direction {
        ParamDirection::In if param.is_constant => Some("IN CONSTANT"),
        ParamDirection::In => None,
        ParamDirection::Out if param.is_constant => Some("OUT CONSTANT"),
        ParamDirection::Out => Some("OUT"),
        ParamDirection::InOut if param.is_constant => Some("IN_OUT CONSTANT"),
        ParamDirection::InOut => Some("IN_OUT"),
    };
    if let Some(dir) = suffix {
        label.push_str(&format!(" ({})", dir));
    }
    label
}

fn format_type_name(symbols: &SymbolTable, type_id: TypeId) -> String {
    if let Some(name) = symbols.type_name(type_id) {
        return name.to_string();
    }
    type_id
        .builtin_name()
        .map(|name| name.to_string())
        .unwrap_or_else(|| "?".to_string())
}
