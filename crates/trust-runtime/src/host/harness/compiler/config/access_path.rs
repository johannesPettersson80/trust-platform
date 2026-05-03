fn config_init_address(node: &SyntaxNode) -> Result<Option<IoAddress>, CompileError> {
    let mut seen_at = false;
    for element in node.children_with_tokens() {
        let token = match element.into_token() {
            Some(token) => token,
            None => continue,
        };
        match token.kind() {
            SyntaxKind::KwAt => seen_at = true,
            SyntaxKind::DirectAddress if seen_at => {
                let address = IoAddress::parse(token.text())
                    .map_err(|err| CompileError::new(err.to_string()))?;
                return Ok(Some(address));
            }
            _ if !token.kind().is_trivia() => seen_at = false,
            _ => {}
        }
    }
    Ok(None)
}

fn parse_access_path(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<AccessPath, CompileError> {
    let mut parts = Vec::new();
    let mut index_nodes: Vec<SyntaxNode> = Vec::new();
    let mut in_index = false;
    let mut saw_root = false;

    for element in node.children_with_tokens() {
        if let Some(token) = element.as_token() {
            match token.kind() {
                SyntaxKind::LBracket => {
                    in_index = true;
                    index_nodes.clear();
                    continue;
                }
                SyntaxKind::RBracket => {
                    in_index = false;
                    if index_nodes.is_empty() {
                        return Err(CompileError::new("empty array index in access path"));
                    }
                    let mut indices = Vec::new();
                    for expr in &index_nodes {
                        let value = const_int_from_node(expr, ctx)?;
                        indices.push(value);
                    }
                    parts.push(AccessPart::Index(indices));
                    index_nodes.clear();
                    continue;
                }
                SyntaxKind::DirectAddress if !saw_root => {
                    let text = SmolStr::new(token.text());
                    let address = IoAddress::parse(text.as_ref())
                        .map_err(|err| CompileError::new(err.to_string()))?;
                    return Ok(AccessPath::Direct { address, text });
                }
                SyntaxKind::DirectAddress => {
                    let text = token.text();
                    if let Some(partial) = crate::value::parse_partial_access(text) {
                        parts.push(AccessPart::Partial(partial));
                    } else {
                        return Err(CompileError::new(
                            "unexpected direct address in access path",
                        ));
                    }
                }
                SyntaxKind::IntLiteral => {
                    if let Some(partial) = crate::value::parse_partial_access(token.text()) {
                        parts.push(AccessPart::Partial(partial));
                    }
                }
                _ => {}
            }
            continue;
        }
        if let Some(child) = element.as_node() {
            if in_index {
                if is_expression_kind(child.kind()) {
                    index_nodes.push(child.clone());
                }
                continue;
            }
            if child.kind() == SyntaxKind::Name {
                let name = SmolStr::new(node_text(child));
                parts.push(AccessPart::Name(name));
                saw_root = true;
            } else if is_expression_kind(child.kind()) {
                index_nodes.push(child.clone());
            }
        }
    }

    if in_index {
        return Err(CompileError::new("unterminated array index in access path"));
    }
    if parts.is_empty() {
        return Err(CompileError::new("empty access path"));
    }
    Ok(AccessPath::Parts(parts))
}
