fn declaration_kind(node: &SyntaxNode) -> Option<ApiItemKind> {
    match node.kind() {
        SyntaxKind::Program => {
            let first = first_non_trivia_token(node)?;
            if first == SyntaxKind::KwTestProgram {
                Some(ApiItemKind::TestProgram)
            } else {
                Some(ApiItemKind::Program)
            }
        }
        SyntaxKind::Function => Some(ApiItemKind::Function),
        SyntaxKind::FunctionBlock => {
            let first = first_non_trivia_token(node)?;
            if first == SyntaxKind::KwTestFunctionBlock {
                Some(ApiItemKind::TestFunctionBlock)
            } else {
                Some(ApiItemKind::FunctionBlock)
            }
        }
        SyntaxKind::Class => Some(ApiItemKind::Class),
        SyntaxKind::Interface => Some(ApiItemKind::Interface),
        SyntaxKind::Method => Some(ApiItemKind::Method),
        SyntaxKind::Property => Some(ApiItemKind::Property),
        _ => None,
    }
}

fn first_non_trivia_token(node: &SyntaxNode) -> Option<SyntaxKind> {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
        .map(|token| token.kind())
}

fn first_non_trivia_token_start(node: &SyntaxNode) -> Option<usize> {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
        .map(|token| usize::from(token.text_range().start()))
}

fn declaration_name(node: &SyntaxNode) -> Option<SmolStr> {
    node.children()
        .find(|child| child.kind() == SyntaxKind::Name)
        .map(|name| {
            let text = name.text().to_string();
            SmolStr::new(text.trim())
        })
}

fn qualified_name(node: &SyntaxNode, name: &SmolStr) -> SmolStr {
    let mut parts = vec![name.to_string()];
    for ancestor in node.ancestors().skip(1) {
        let include = matches!(
            ancestor.kind(),
            SyntaxKind::Namespace
                | SyntaxKind::Class
                | SyntaxKind::FunctionBlock
                | SyntaxKind::Interface
        );
        if !include {
            continue;
        }
        if let Some(ancestor_name) = declaration_name(&ancestor) {
            parts.push(ancestor_name.to_string());
        }
    }
    parts.reverse();
    SmolStr::new(parts.join("."))
}

fn declaration_has_return(node: &SyntaxNode, kind: ApiItemKind) -> bool {
    match kind {
        ApiItemKind::Function => true,
        ApiItemKind::Method | ApiItemKind::Property => node
            .children()
            .any(|child| child.kind() == SyntaxKind::TypeRef),
        _ => false,
    }
}

fn declared_param_names(node: &SyntaxNode) -> Vec<SmolStr> {
    let mut names = Vec::new();
    for block in node
        .children()
        .filter(|child| child.kind() == SyntaxKind::VarBlock)
    {
        if !is_parameter_var_block(&block) {
            continue;
        }
        for decl in block
            .children()
            .filter(|child| child.kind() == SyntaxKind::VarDecl)
        {
            for name in decl
                .children()
                .filter(|child| child.kind() == SyntaxKind::Name)
            {
                let text = name.text().to_string();
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    names.push(SmolStr::new(trimmed));
                }
            }
        }
    }
    names
}

fn is_parameter_var_block(block: &SyntaxNode) -> bool {
    let Some(token) = block
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
    else {
        return false;
    };
    matches!(
        token.kind(),
        SyntaxKind::KwVarInput | SyntaxKind::KwVarOutput | SyntaxKind::KwVarInOut
    )
}

fn leading_comment_block(
    source: &str,
    tokens: &[Token],
    declaration_start: usize,
) -> Option<CommentBlock> {
    let token_pos =
        tokens.partition_point(|token| usize::from(token.range.start()) < declaration_start);
    if token_pos == 0 {
        return None;
    }

    let mut idx = token_pos;
    let mut collected = Vec::new();
    let mut seen_comment = false;
    while idx > 0 {
        idx -= 1;
        let token = tokens[idx];
        match token.kind {
            TokenKind::Whitespace => {
                let ws = token_text(source, token);
                let newlines = ws.bytes().filter(|byte| *byte == b'\n').count();
                if newlines > 1 {
                    break;
                }
            }
            TokenKind::LineComment | TokenKind::BlockComment => {
                seen_comment = true;
                collected.push(token);
            }
            _ => break,
        }
    }

    if !seen_comment {
        return None;
    }

    collected.reverse();
    let mut lines = Vec::new();
    for token in &collected {
        let raw = token_text(source, *token);
        lines.extend(normalize_comment_lines(token.kind, raw));
    }

    while matches!(lines.first(), Some(line) if line.trim().is_empty()) {
        lines.remove(0);
    }
    while matches!(lines.last(), Some(line) if line.trim().is_empty()) {
        lines.pop();
    }
    if lines.is_empty() {
        return None;
    }

    let start_line = line_for_offset(source, usize::from(collected[0].range.start()));
    Some(CommentBlock { lines, start_line })
}

fn normalize_comment_lines(kind: TokenKind, raw: &str) -> Vec<String> {
    match kind {
        TokenKind::LineComment => vec![raw
            .strip_prefix("//")
            .unwrap_or(raw)
            .trim_start()
            .trim_end()
            .to_string()],
        TokenKind::BlockComment => {
            let mut body = raw.trim_end();
            if let Some(stripped) = body
                .strip_prefix("(*")
                .and_then(|text| text.strip_suffix("*)"))
            {
                body = stripped;
            } else if let Some(stripped) = body
                .strip_prefix("/*")
                .and_then(|text| text.strip_suffix("*/"))
            {
                body = stripped;
            }
            body.lines()
                .map(|line| {
                    let trimmed = line.trim_start();
                    let without_star = trimmed.strip_prefix('*').map_or(trimmed, str::trim_start);
                    without_star.trim_end().to_string()
                })
                .collect()
        }
        _ => Vec::new(),
    }
}
