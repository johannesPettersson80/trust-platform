//! Responsibility: focused helper module in the core LSP feature layer.
use super::*;

pub(in super::super) fn convert_call_style_text_edit(
    state: &ServerState,
    doc: &crate::state::Document,
    root: &SyntaxNode,
    diagnostic: &Diagnostic,
) -> Option<Vec<(String, TextEdit)>> {
    let message = diagnostic.message.as_str();
    let ordering_error = message.contains("positional arguments must precede formal arguments");
    if !message.contains("formal calls cannot mix positional arguments")
        && !message.contains("formal call arguments must be named")
        && !ordering_error
    {
        return None;
    }

    let start = position_to_offset(&doc.content, diagnostic.range.start)?;
    let range = TextRange::new(TextSize::from(start), TextSize::from(start));
    let call_expr = find_enclosing_node_of_kind(root, range, SyntaxKind::CallExpr)?;
    let arg_list = call_expr
        .children()
        .find(|child| child.kind() == SyntaxKind::ArgList)?;

    let args = parse_call_args(&arg_list, &doc.content);
    if args.is_empty() {
        return None;
    }

    let params = state.with_database(|db| {
        call_signature_info(db, doc.file_id, TextSize::from(start)).map(|info| info.params)
    })?;
    let params = params
        .into_iter()
        .filter(|param| !is_execution_param(param.name.as_str()))
        .collect::<Vec<_>>();

    let mut edits = Vec::new();
    if ordering_error {
        if let Some(text) = build_positional_first_call(&args, &params) {
            edits.push((
                "Reorder to positional-first call".to_string(),
                replace_arg_list_edit(&doc.content, &arg_list, text),
            ));
        }
    }
    if let Some(text) = build_formal_call(&args, &params) {
        edits.push((
            "Convert to formal call".to_string(),
            replace_arg_list_edit(&doc.content, &arg_list, text),
        ));
    }
    if let Some(text) = build_positional_call(&args, &params) {
        edits.push((
            "Convert to positional call".to_string(),
            replace_arg_list_edit(&doc.content, &arg_list, text),
        ));
    }
    (!edits.is_empty()).then_some(edits)
}

pub(in super::super) fn namespace_disambiguation_actions(
    state: &ServerState,
    doc: &crate::state::Document,
    root: &SyntaxNode,
    diagnostic: &Diagnostic,
) -> Vec<CodeActionOrCommand> {
    if !diagnostic.message.contains("ambiguous reference to") {
        return Vec::new();
    }
    let name = extract_quoted_name(&diagnostic.message).or_else(|| {
        let start = position_to_offset(&doc.content, diagnostic.range.start)?;
        trust_ide::util::ident_at_offset(&doc.content, TextSize::from(start))
            .map(|(name, _)| name.to_string())
    });
    let Some(name) = name else {
        return Vec::new();
    };
    let Some(start) = position_to_offset(&doc.content, diagnostic.range.start) else {
        return Vec::new();
    };
    let scope_id = state.with_database(|db| {
        let symbols = db.file_symbols_with_project(doc.file_id);
        scope_at_position(&symbols, root, TextSize::from(start))
    });

    let candidates = state.with_database(|db| {
        let symbols = db.file_symbols_with_project(doc.file_id);
        collect_using_candidates(&symbols, scope_id, &name)
    });
    if candidates.is_empty() {
        return Vec::new();
    }

    let mut actions = Vec::new();
    for parts in candidates {
        let qualified = join_namespace_path(&parts);
        let edit = TextEdit {
            range: diagnostic.range,
            new_text: qualified.clone(),
        };
        let mut changes: std::collections::HashMap<Url, Vec<TextEdit>> =
            std::collections::HashMap::new();
        changes.insert(doc.uri.clone(), vec![edit]);

        let action = CodeAction {
            title: format!("Qualify with {qualified}"),
            kind: Some(CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![diagnostic.clone()]),
            edit: Some(WorkspaceEdit {
                changes: Some(changes),
                document_changes: None,
                change_annotations: None,
            }),
            is_preferred: Some(true),
            ..Default::default()
        };
        actions.push(CodeActionOrCommand::CodeAction(action));
    }

    actions
}

pub(in super::super) fn namespace_move_action(
    doc: &crate::state::Document,
    root: &SyntaxNode,
    params: &CodeActionParams,
) -> Option<CodeActionOrCommand> {
    if !allows_refactor_action(&params.context.only) {
        return None;
    }
    let start = position_to_offset(&doc.content, params.range.start)?;
    let end = position_to_offset(&doc.content, params.range.end).unwrap_or(start);
    let range = TextRange::new(TextSize::from(start), TextSize::from(end));

    let namespace_node = find_enclosing_node_of_kind(root, range, SyntaxKind::Namespace)?;
    let name_node = namespace_node
        .children()
        .find(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::QualifiedName))?;
    let name_range = name_node.text_range();
    if !name_range.contains(range.start()) || !name_range.contains(range.end()) {
        return None;
    }

    let title = "Move namespace (rename path)".to_string();
    let command = Command {
        title: title.clone(),
        command: "editor.action.rename".to_string(),
        arguments: None,
    };
    let action = CodeAction {
        title,
        kind: Some(CodeActionKind::REFACTOR_REWRITE),
        command: Some(command),
        ..Default::default()
    };
    Some(CodeActionOrCommand::CodeAction(action))
}

pub(in super::super) fn interface_stub_action(
    state: &ServerState,
    doc: &crate::state::Document,
    params: &CodeActionParams,
) -> Option<CodeActionOrCommand> {
    if !allows_refactor_action(&params.context.only) {
        return None;
    }
    let offset = position_to_offset(&doc.content, params.range.start)?;
    let result = state.with_database(|db| {
        trust_ide::generate_interface_stubs(db, doc.file_id, TextSize::from(offset))
    })?;
    let changes = rename_result_to_changes(state, result)?;

    let action = CodeAction {
        title: "Generate interface stubs".to_string(),
        kind: Some(CodeActionKind::REFACTOR),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }),
        ..Default::default()
    };
    Some(CodeActionOrCommand::CodeAction(action))
}

pub(in super::super) fn inline_symbol_action(
    state: &ServerState,
    doc: &crate::state::Document,
    params: &CodeActionParams,
) -> Option<CodeActionOrCommand> {
    if !allows_refactor_action(&params.context.only) {
        return None;
    }
    let offset = position_to_offset(&doc.content, params.range.start)?;
    let result = state
        .with_database(|db| trust_ide::inline_symbol(db, doc.file_id, TextSize::from(offset)))?;
    let changes = rename_result_to_changes(state, result.edits)?;

    let title = match result.kind {
        InlineTargetKind::Constant => "Inline constant".to_string(),
        InlineTargetKind::Variable => "Inline variable".to_string(),
    };
    let action = CodeAction {
        title,
        kind: Some(CodeActionKind::REFACTOR_INLINE),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }),
        ..Default::default()
    };
    Some(CodeActionOrCommand::CodeAction(action))
}

pub(in super::super) fn extract_actions(
    state: &ServerState,
    doc: &crate::state::Document,
    params: &CodeActionParams,
) -> Vec<CodeActionOrCommand> {
    if !allows_refactor_action(&params.context.only) {
        return Vec::new();
    }
    let start = match position_to_offset(&doc.content, params.range.start) {
        Some(offset) => offset,
        None => return Vec::new(),
    };
    let end = position_to_offset(&doc.content, params.range.end).unwrap_or(start);
    if start == end {
        return Vec::new();
    }
    let range = TextRange::new(TextSize::from(start), TextSize::from(end));

    let mut actions = Vec::new();

    if let Some(result) = state.with_database(|db| extract_method(db, doc.file_id, range)) {
        if let Some(changes) = rename_result_to_changes(state, result.edits) {
            let action = CodeAction {
                title: "Extract method".to_string(),
                kind: Some(CodeActionKind::REFACTOR_EXTRACT),
                edit: Some(WorkspaceEdit {
                    changes: Some(changes),
                    document_changes: None,
                    change_annotations: None,
                }),
                ..Default::default()
            };
            actions.push(CodeActionOrCommand::CodeAction(action));
        }
    }

    if let Some(result) = state.with_database(|db| extract_property(db, doc.file_id, range)) {
        if let Some(changes) = rename_result_to_changes(state, result.edits) {
            let action = CodeAction {
                title: "Extract property".to_string(),
                kind: Some(CodeActionKind::REFACTOR_EXTRACT),
                edit: Some(WorkspaceEdit {
                    changes: Some(changes),
                    document_changes: None,
                    change_annotations: None,
                }),
                ..Default::default()
            };
            actions.push(CodeActionOrCommand::CodeAction(action));
        }
    }

    if let Some(result) = state.with_database(|db| extract_pou(db, doc.file_id, range)) {
        if let Some(changes) = rename_result_to_changes(state, result.edits) {
            let action = CodeAction {
                title: "Extract function".to_string(),
                kind: Some(CodeActionKind::REFACTOR_EXTRACT),
                edit: Some(WorkspaceEdit {
                    changes: Some(changes),
                    document_changes: None,
                    change_annotations: None,
                }),
                ..Default::default()
            };
            actions.push(CodeActionOrCommand::CodeAction(action));
        }
    }

    actions
}

pub(in super::super) fn convert_function_action(
    state: &ServerState,
    doc: &crate::state::Document,
    params: &CodeActionParams,
) -> Option<CodeActionOrCommand> {
    if !allows_refactor_action(&params.context.only) {
        return None;
    }
    let offset = position_to_offset(&doc.content, params.range.start)?;
    let result = state.with_database(|db| {
        convert_function_to_function_block(db, doc.file_id, TextSize::from(offset))
    })?;
    let changes = rename_result_to_changes(state, result)?;

    let action = CodeAction {
        title: "Convert FUNCTION to FUNCTION_BLOCK".to_string(),
        kind: Some(CodeActionKind::REFACTOR_REWRITE),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }),
        ..Default::default()
    };
    Some(CodeActionOrCommand::CodeAction(action))
}

pub(in super::super) fn convert_function_block_action(
    state: &ServerState,
    doc: &crate::state::Document,
    params: &CodeActionParams,
) -> Option<CodeActionOrCommand> {
    if !allows_refactor_action(&params.context.only) {
        return None;
    }
    let offset = position_to_offset(&doc.content, params.range.start)?;
    let result = state.with_database(|db| {
        convert_function_block_to_function(db, doc.file_id, TextSize::from(offset))
    })?;
    let changes = rename_result_to_changes(state, result)?;

    let action = CodeAction {
        title: "Convert FUNCTION_BLOCK to FUNCTION".to_string(),
        kind: Some(CodeActionKind::REFACTOR_REWRITE),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }),
        ..Default::default()
    };
    Some(CodeActionOrCommand::CodeAction(action))
}

pub(in super::super) fn allows_refactor_action(only: &Option<Vec<CodeActionKind>>) -> bool {
    let Some(only) = only else {
        return true;
    };
    only.iter().any(|kind| {
        let value = kind.as_str();
        value == CodeActionKind::REFACTOR.as_str()
            || value == CodeActionKind::REFACTOR_REWRITE.as_str()
            || value.starts_with(CodeActionKind::REFACTOR.as_str())
    })
}

pub(in super::super) fn collect_using_candidates(
    symbols: &SymbolTable,
    scope_id: ScopeId,
    name: &str,
) -> Vec<Vec<SmolStr>> {
    let mut candidates = Vec::new();
    let mut current = Some(scope_id);
    while let Some(scope_id) = current {
        let Some(scope) = symbols.get_scope(scope_id) else {
            break;
        };
        if scope.lookup_local(name).is_some() {
            break;
        }
        for using in &scope.using_directives {
            let mut parts = using.path.clone();
            parts.push(SmolStr::new(name));
            if symbols.resolve_qualified(&parts).is_some() {
                candidates.push(parts);
            }
        }
        current = scope.parent;
    }

    let mut seen = FxHashSet::default();
    let mut unique = Vec::new();
    for parts in candidates {
        let key = parts
            .iter()
            .map(|part| part.to_ascii_uppercase())
            .collect::<Vec<_>>()
            .join(".");
        if seen.insert(key) {
            unique.push(parts);
        }
    }
    unique
}

pub(in super::super) fn join_namespace_path(parts: &[SmolStr]) -> String {
    let mut out = String::new();
    for (idx, part) in parts.iter().enumerate() {
        if idx > 0 {
            out.push('.');
        }
        out.push_str(part.as_str());
    }
    out
}

fn parse_call_args(arg_list: &SyntaxNode, source: &str) -> Vec<ParsedArg> {
    let mut args = Vec::new();
    for arg in arg_list
        .children()
        .filter(|node| node.kind() == SyntaxKind::Arg)
    {
        let name = arg
            .children()
            .find(|node| node.kind() == SyntaxKind::Name)
            .and_then(|node| {
                node.descendants_with_tokens()
                    .filter_map(|element| element.into_token())
                    .find(|token| token.kind() == SyntaxKind::Ident)
            })
            .map(|token| token.text().to_string());

        let expr_node = arg
            .children()
            .filter(|node| node.kind() != SyntaxKind::Name)
            .last();
        let expr_text = expr_node
            .map(|node| text_for_range(source, node.text_range()))
            .unwrap_or_default();

        args.push(ParsedArg { name, expr_text });
    }
    args
}

#[derive(Debug, Clone)]
struct ParsedArg {
    name: Option<String>,
    expr_text: String,
}

fn build_formal_call(
    args: &[ParsedArg],
    params: &[trust_ide::CallSignatureParam],
) -> Option<String> {
    if args.len() > params.len() {
        return None;
    }
    let mut out = Vec::new();
    for (idx, arg) in args.iter().enumerate() {
        let param = params.get(idx)?;
        let op = match param.direction {
            ParamDirection::Out => "=>",
            ParamDirection::In | ParamDirection::InOut => ":=",
        };
        out.push(format!("{} {} {}", param.name, op, arg.expr_text));
    }
    Some(format!("({})", out.join(", ")))
}

fn build_positional_call(
    args: &[ParsedArg],
    params: &[trust_ide::CallSignatureParam],
) -> Option<String> {
    let mut positional = args
        .iter()
        .filter(|arg| arg.name.is_none())
        .map(|arg| arg.expr_text.clone())
        .collect::<Vec<_>>();
    let mut positional_iter = positional.drain(..);
    let mut by_name: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for arg in args {
        if let Some(name) = &arg.name {
            by_name.insert(name.to_ascii_uppercase(), arg.expr_text.clone());
        }
    }

    let mut out = Vec::new();
    for param in params {
        let key = param.name.to_ascii_uppercase();
        if let Some(expr) = by_name.remove(&key) {
            out.push(expr);
        } else {
            let expr = positional_iter.next()?;
            out.push(expr);
        }
    }

    Some(format!("({})", out.join(", ")))
}

fn build_positional_first_call(
    args: &[ParsedArg],
    params: &[trust_ide::CallSignatureParam],
) -> Option<String> {
    let mut positional = Vec::new();
    let mut named = Vec::new();
    for arg in args {
        if arg.name.is_some() {
            named.push(arg);
        } else {
            positional.push(arg);
        }
    }
    if positional.is_empty() || named.is_empty() {
        return None;
    }

    let mut out = positional
        .iter()
        .map(|arg| arg.expr_text.clone())
        .collect::<Vec<_>>();

    for arg in named {
        let name = arg.name.as_ref()?;
        let param = params
            .iter()
            .find(|param| param.name.eq_ignore_ascii_case(name))?;
        let op = match param.direction {
            ParamDirection::Out => "=>",
            ParamDirection::In | ParamDirection::InOut => ":=",
        };
        out.push(format!("{name} {op} {}", arg.expr_text));
    }

    Some(format!("({})", out.join(", ")))
}

pub(in super::super) fn replace_arg_list_edit(
    source: &str,
    arg_list: &SyntaxNode,
    new_text: String,
) -> TextEdit {
    TextEdit {
        range: Range {
            start: offset_to_position(source, arg_list.text_range().start().into()),
            end: offset_to_position(source, arg_list.text_range().end().into()),
        },
        new_text,
    }
}
