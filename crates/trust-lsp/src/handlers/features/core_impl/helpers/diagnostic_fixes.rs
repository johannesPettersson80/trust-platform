//! Responsibility: focused helper module in the core LSP feature layer.
use super::*;

pub(in super::super) fn unused_symbol_removal_range(
    source: &str,
    root: &SyntaxNode,
    symbol_range: TextRange,
) -> Option<TextRange> {
    let var_decl = find_var_decl_for_range(root, symbol_range)?;
    let names: Vec<SyntaxToken> = var_decl
        .children()
        .filter(|node| node.kind() == SyntaxKind::Name)
        .filter_map(|node| {
            node.descendants_with_tokens()
                .filter_map(|element| element.into_token())
                .find(|token| token.kind() == SyntaxKind::Ident)
        })
        .collect();
    if names.is_empty() {
        return None;
    }
    let index = names
        .iter()
        .position(|token| token.text_range() == symbol_range)?;

    if names.len() == 1 {
        return Some(extend_range_to_line_end(source, var_decl.text_range()));
    }

    if index + 1 < names.len() {
        let end = names[index + 1].text_range().start();
        return Some(TextRange::new(symbol_range.start(), end));
    }

    let tokens: Vec<SyntaxToken> = var_decl
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .collect();
    let target_index = tokens
        .iter()
        .position(|token| token.text_range() == symbol_range)?;
    let comma = tokens[..target_index]
        .iter()
        .rev()
        .find(|token| token.kind() == SyntaxKind::Comma)?;

    let mut end = var_decl.text_range().end();
    for token in tokens.iter().skip(target_index + 1) {
        if token.kind().is_trivia() {
            end = token.text_range().end();
            continue;
        }
        end = token.text_range().start();
        break;
    }

    Some(TextRange::new(comma.text_range().start(), end))
}

pub(in super::super) fn missing_else_text_edit(
    source: &str,
    root: &SyntaxNode,
    range: Range,
) -> Option<TextEdit> {
    let start = position_to_offset(source, range.start)?;
    let end = position_to_offset(source, range.end)?;
    let diag_range = TextRange::new(TextSize::from(start), TextSize::from(end));
    let case_stmt = find_case_stmt_for_range(root, diag_range)?;

    if case_stmt
        .children()
        .any(|child| child.kind() == SyntaxKind::ElseBranch)
    {
        return None;
    }

    let end_case_token = case_stmt
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == SyntaxKind::KwEndCase)?;
    let end_case_offset = usize::from(end_case_token.text_range().start());
    let line_start = line_start_offset(source, end_case_offset);

    let indent = case_stmt
        .children()
        .find(|child| child.kind() == SyntaxKind::CaseBranch)
        .map(|branch| indent_at_offset(source, usize::from(branch.text_range().start())))
        .unwrap_or_else(|| indent_at_offset(source, end_case_offset));

    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let insert_text = format!("{indent}ELSE{newline}");
    let insert_pos = offset_to_position(source, line_start as u32);

    Some(TextEdit {
        range: Range {
            start: insert_pos,
            end: insert_pos,
        },
        new_text: insert_text,
    })
}

pub(in super::super) fn missing_var_text_edit(
    state: &ServerState,
    doc: &crate::state::Document,
    root: &SyntaxNode,
    diagnostic: &Diagnostic,
) -> Option<TextEdit> {
    let name = extract_quoted_name(&diagnostic.message)?;
    let start = position_to_offset(&doc.content, diagnostic.range.start)?;
    let text_range = TextRange::new(TextSize::from(start), TextSize::from(start));
    let pou = trust_ide::util::find_enclosing_pou(root, text_range.start())?;

    let type_name =
        infer_missing_var_type(state, doc, root, text_range).unwrap_or_else(|| "INT".to_string());
    let newline = newline_for_source(&doc.content);

    if let Some(var_block) = find_var_block(&pou) {
        let end_var_token = var_block
            .descendants_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| token.kind() == SyntaxKind::KwEndVar)?;
        let insert_offset = line_start_offset(
            &doc.content,
            usize::from(end_var_token.text_range().start()),
        );
        let decl_indent = indent_for_var_block(&doc.content, &var_block, &end_var_token)
            .unwrap_or_else(|| {
                let base = indent_at_offset(&doc.content, insert_offset);
                format!("{base}{}", infer_indent_unit(&doc.content))
            });
        let insert_text = format!("{decl_indent}{name} : {type_name};{newline}");
        let insert_pos = offset_to_position(&doc.content, insert_offset as u32);
        return Some(TextEdit {
            range: Range {
                start: insert_pos,
                end: insert_pos,
            },
            new_text: insert_text,
        });
    }

    let header_indent = indent_at_offset(&doc.content, usize::from(pou.text_range().start()));
    let indent_unit = infer_indent_unit(&doc.content);
    let body_indent = format!("{header_indent}{indent_unit}");
    let insert_offset = line_end_offset(&doc.content, usize::from(pou.text_range().start()));
    let insert_text = format!(
        "{newline}{header_indent}VAR{newline}{body_indent}{name} : {type_name};{newline}{header_indent}END_VAR{newline}"
    );
    let insert_pos = offset_to_position(&doc.content, insert_offset as u32);
    Some(TextEdit {
        range: Range {
            start: insert_pos,
            end: insert_pos,
        },
        new_text: insert_text,
    })
}

pub(in super::super) fn find_var_block(pou: &SyntaxNode) -> Option<SyntaxNode> {
    pou.descendants()
        .filter(|node| node.kind() == SyntaxKind::VarBlock)
        .find(|block| {
            block
                .children_with_tokens()
                .filter_map(|element| element.into_token())
                .any(|token| token.kind() == SyntaxKind::KwVar)
        })
}

pub(in super::super) fn indent_for_var_block(
    source: &str,
    block: &SyntaxNode,
    end_var_token: &SyntaxToken,
) -> Option<String> {
    let mut decl_indent = None;
    for decl in block
        .children()
        .filter(|node| node.kind() == SyntaxKind::VarDecl)
    {
        let indent = indent_at_offset(source, usize::from(decl.text_range().start()));
        if !indent.is_empty() {
            decl_indent = Some(indent);
            break;
        }
    }
    if decl_indent.is_some() {
        return decl_indent;
    }
    let base = indent_at_offset(source, usize::from(end_var_token.text_range().start()));
    Some(format!("{}{}", base, infer_indent_unit(source)))
}

pub(in super::super) fn infer_missing_var_type(
    state: &ServerState,
    doc: &crate::state::Document,
    root: &SyntaxNode,
    range: TextRange,
) -> Option<String> {
    let token = root.token_at_offset(range.start()).right_biased()?;
    let name_node = token.parent().and_then(|parent| {
        parent
            .ancestors()
            .find(|node| matches!(node.kind(), SyntaxKind::NameRef | SyntaxKind::Name))
    })?;

    if name_node
        .ancestors()
        .any(|node| node.kind() == SyntaxKind::Condition)
    {
        return Some("BOOL".to_string());
    }

    if let Some(assign_stmt) = name_node
        .ancestors()
        .find(|node| node.kind() == SyntaxKind::AssignStmt)
    {
        let mut children = assign_stmt.children();
        let lhs = children.next();
        if let Some(lhs) = lhs
            && lhs.text_range().contains(name_node.text_range().start())
            && let Some(expr_node) = assign_stmt
                .children()
                .filter(|node| is_expression_kind(node.kind()))
                .last()
        {
            let expr_offset = u32::from(expr_node.text_range().start());
            let type_id = state.with_database(|db| {
                let expr_id = db.expr_id_at_offset(doc.file_id, expr_offset)?;
                Some(db.type_of(doc.file_id, expr_id))
            })?;
            return type_name_for_type_id(state, doc, type_id);
        }
    }

    None
}

pub(in super::super) fn type_name_for_type_id(
    state: &ServerState,
    doc: &crate::state::Document,
    type_id: TypeId,
) -> Option<String> {
    state.with_database(|db| {
        let symbols = db.file_symbols_with_project(doc.file_id);
        symbols
            .type_name(type_id)
            .map(|name| name.to_string())
            .or_else(|| type_id.builtin_name().map(|name| name.to_string()))
    })
}

pub(in super::super) fn missing_type_text_edit(
    doc: &crate::state::Document,
    root: &SyntaxNode,
    diagnostic: &Diagnostic,
) -> Option<TextEdit> {
    let name = extract_quoted_name(&diagnostic.message)?;
    let newline = newline_for_source(&doc.content);

    let insert_offset = if let Some(last_type) = root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::TypeDecl)
        .max_by_key(|node| node.text_range().end())
    {
        line_end_offset(&doc.content, usize::from(last_type.text_range().end()))
    } else if let Some(first_pou) = root.descendants().find(|node| is_pou_kind(node.kind())) {
        line_start_offset(&doc.content, usize::from(first_pou.text_range().start()))
    } else {
        0
    };

    let insert_text = format!("{newline}TYPE {name} : INT;{newline}END_TYPE{newline}");
    let insert_pos = offset_to_position(&doc.content, insert_offset as u32);
    Some(TextEdit {
        range: Range {
            start: insert_pos,
            end: insert_pos,
        },
        new_text: insert_text,
    })
}

pub(in super::super) fn missing_end_text_edit(
    doc: &crate::state::Document,
    root: &SyntaxNode,
    diagnostic: &Diagnostic,
) -> Option<TextEdit> {
    let expected = expected_end_keyword(&diagnostic.message)?;
    let start = position_to_offset(&doc.content, diagnostic.range.start)?;
    let diag_range = TextRange::new(TextSize::from(start), TextSize::from(start));

    let indent = if let Some(kind) = node_kind_for_end_keyword(expected) {
        if let Some(node) = find_enclosing_node_of_kind(root, diag_range, kind) {
            indent_at_offset(&doc.content, usize::from(node.text_range().start()))
        } else {
            indent_at_offset(&doc.content, start as usize)
        }
    } else {
        indent_at_offset(&doc.content, start as usize)
    };

    let newline = newline_for_source(&doc.content);
    let insert_text = format!("{indent}{expected}{newline}");
    let insert_offset = line_start_offset(&doc.content, start as usize);
    let insert_pos = offset_to_position(&doc.content, insert_offset as u32);
    Some(TextEdit {
        range: Range {
            start: insert_pos,
            end: insert_pos,
        },
        new_text: insert_text,
    })
}

pub(in super::super) fn missing_return_text_edit(
    state: &ServerState,
    doc: &crate::state::Document,
    root: &SyntaxNode,
    diagnostic: &Diagnostic,
) -> Option<TextEdit> {
    let start = position_to_offset(&doc.content, diagnostic.range.start)?;
    let diag_range = TextRange::new(TextSize::from(start), TextSize::from(start));
    let func_node = find_enclosing_node_of_kind(root, diag_range, SyntaxKind::Function)?;
    let end_token = func_node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == SyntaxKind::KwEndFunction)?;

    let name_node = func_node
        .children()
        .find(|node| node.kind() == SyntaxKind::Name)?;
    let func_name = name_node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == SyntaxKind::Ident)?
        .text()
        .to_string();

    let return_type = state.with_database(|db| {
        let symbols = db.file_symbols_with_project(doc.file_id);
        let symbol_id = symbols.resolve(&func_name, trust_hir::symbols::ScopeId::GLOBAL)?;
        let symbol = symbols.get(symbol_id)?;
        match symbol.kind {
            trust_hir::symbols::SymbolKind::Function { return_type, .. } => Some(return_type),
            _ => None,
        }
    })?;

    let default_value = default_literal_for_type(state, doc, return_type).unwrap_or("0".into());
    let end_offset = usize::from(end_token.text_range().start());
    let insert_offset = line_start_offset(&doc.content, end_offset);
    let base_indent = indent_at_offset(&doc.content, end_offset);
    let indent = format!("{base_indent}{}", infer_indent_unit(&doc.content));
    let newline = newline_for_source(&doc.content);
    let insert_text = format!("{indent}RETURN {default_value};{newline}");
    let insert_pos = offset_to_position(&doc.content, insert_offset as u32);
    Some(TextEdit {
        range: Range {
            start: insert_pos,
            end: insert_pos,
        },
        new_text: insert_text,
    })
}

pub(in super::super) fn default_literal_for_type(
    state: &ServerState,
    doc: &crate::state::Document,
    type_id: TypeId,
) -> Option<String> {
    let resolved = state.with_database(|db| {
        let symbols = db.file_symbols_with_project(doc.file_id);
        symbols.resolve_alias_type(type_id)
    });

    match resolved {
        TypeId::BOOL => Some("FALSE".to_string()),
        TypeId::REAL | TypeId::LREAL => Some("0.0".to_string()),
        TypeId::STRING => Some("''".to_string()),
        TypeId::WSTRING => Some("\"\"".to_string()),
        TypeId::TIME => Some("T#0s".to_string()),
        TypeId::LTIME => Some("LTIME#0s".to_string()),
        TypeId::DATE => Some("DATE#1970-01-01".to_string()),
        TypeId::LDATE => Some("LDATE#1970-01-01".to_string()),
        TypeId::TOD => Some("TOD#00:00:00".to_string()),
        TypeId::LTOD => Some("LTOD#00:00:00".to_string()),
        TypeId::DT => Some("DT#1970-01-01-00:00:00".to_string()),
        TypeId::LDT => Some("LDT#1970-01-01-00:00:00".to_string()),
        _ => None,
    }
}

pub(in super::super) fn implicit_conversion_text_edit(
    doc: &crate::state::Document,
    root: &SyntaxNode,
    diagnostic: &Diagnostic,
) -> Option<TextEdit> {
    let (source, target) = parse_conversion_types(&diagnostic.message)?;
    let func = format!("{}_TO_{}", source, target);
    let start = position_to_offset(&doc.content, diagnostic.range.start)?;
    let end = position_to_offset(&doc.content, diagnostic.range.end)?;
    let diag_range = TextRange::new(TextSize::from(start), TextSize::from(end));
    let expr_range = find_enclosing_node_of_kind(root, diag_range, SyntaxKind::AssignStmt)
        .and_then(|assign| {
            assign
                .children()
                .filter(|node| is_expression_kind(node.kind()))
                .last()
                .map(|expr| expr.text_range())
        })
        .unwrap_or(diag_range);
    let expr_text = text_for_range(&doc.content, expr_range);
    if expr_text.is_empty() {
        return None;
    }
    let new_text = format!("{func}({expr_text})");
    Some(TextEdit {
        range: Range {
            start: offset_to_position(&doc.content, expr_range.start().into()),
            end: offset_to_position(&doc.content, expr_range.end().into()),
        },
        new_text,
    })
}

pub(in super::super) fn parse_conversion_types(message: &str) -> Option<(String, String)> {
    let message = message.trim();
    let start = message.find('\'')?;
    let rest = &message[start + 1..];
    let mid = rest.find('\'')?;
    let source = rest[..mid].to_string();
    let rest = &rest[mid + 1..];
    let start = rest.find('\'')?;
    let rest = &rest[start + 1..];
    let end = rest.find('\'')?;
    let target = rest[..end].to_string();
    Some((source.to_ascii_uppercase(), target.to_ascii_uppercase()))
}

pub(in super::super) fn fix_output_binding_text_edit(
    doc: &crate::state::Document,
    root: &SyntaxNode,
    diagnostic: &Diagnostic,
) -> Option<TextEdit> {
    let message = diagnostic.message.as_str();
    let (expected, replacement) = if message.contains("must use '=>'") {
        (SyntaxKind::Assign, "=>")
    } else if message.contains("use ':='") {
        (SyntaxKind::Arrow, ":=")
    } else {
        return None;
    };

    let start = position_to_offset(&doc.content, diagnostic.range.start)?;
    let range = TextRange::new(TextSize::from(start), TextSize::from(start));
    let arg_node = find_enclosing_node_of_kind(root, range, SyntaxKind::Arg)?;
    let assign_token = arg_node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == expected)?;
    Some(TextEdit {
        range: Range {
            start: offset_to_position(&doc.content, assign_token.text_range().start().into()),
            end: offset_to_position(&doc.content, assign_token.text_range().end().into()),
        },
        new_text: replacement.to_string(),
    })
}
