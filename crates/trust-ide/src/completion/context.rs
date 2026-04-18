/// Detects the completion context at a given position.
fn detect_context(root: &SyntaxNode, position: TextSize) -> CompletionContext {
    // Try to find the token at or just before the position
    let token = find_token_at_position(root, position);

    let Some(token) = token else {
        return CompletionContext::General;
    };

    // Check for trigger characters by looking at the previous non-trivia token
    if let Some(prev) = previous_non_trivia_token(&token) {
        match prev.kind() {
            // Dot triggers member access completion
            SyntaxKind::Dot => return CompletionContext::MemberAccess,
            // Colon triggers type annotation completion
            SyntaxKind::Colon => return CompletionContext::TypeAnnotation,
            // Comma in argument list
            SyntaxKind::Comma if is_in_argument_list(&prev) => {
                return CompletionContext::Argument;
            }
            // Opening paren might be function call
            SyntaxKind::LParen if is_in_call_expr(&prev) => {
                return CompletionContext::Argument;
            }
            _ => {}
        }
    }

    // Walk up ancestors to determine context
    for ancestor in token.parent_ancestors() {
        match ancestor.kind() {
            // Inside a type reference
            SyntaxKind::TypeRef => return CompletionContext::TypeAnnotation,

            // Inside extends/implements clause
            SyntaxKind::ExtendsClause | SyntaxKind::ImplementsClause => {
                return CompletionContext::TypeAnnotation;
            }

            // Inside a VAR block (but not in a type ref)
            SyntaxKind::VarBlock => {
                if is_recovered_statement_position(&ancestor, position) {
                    return CompletionContext::Statement;
                }
                return CompletionContext::VarBlock;
            }

            // Inside a VAR declaration (for type context)
            SyntaxKind::VarDecl => {
                if is_recovered_statement_position(&ancestor, position) {
                    return CompletionContext::Statement;
                }
                // Check if we're after the colon (type context)
                if has_colon_before_position(&ancestor, position) {
                    return CompletionContext::TypeAnnotation;
                }
                return CompletionContext::VarBlock;
            }

            // Inside an argument list
            SyntaxKind::ArgList => return CompletionContext::Argument,

            // Inside a statement list
            SyntaxKind::StmtList => return CompletionContext::Statement,

            // Inside a POU - we're in statement context
            SyntaxKind::Program
            | SyntaxKind::Function
            | SyntaxKind::FunctionBlock
            | SyntaxKind::Method
                if is_past_var_blocks(&ancestor, position) =>
            {
                return CompletionContext::Statement;
            }

            // At the source file level
            SyntaxKind::SourceFile if !is_inside_pou(&ancestor, position) => {
                return CompletionContext::TopLevel;
            }

            _ => {}
        }
    }

    CompletionContext::General
}

fn is_recovered_statement_position(node: &SyntaxNode, position: TextSize) -> bool {
    node.ancestors().any(|ancestor| {
        matches!(
            ancestor.kind(),
            SyntaxKind::Program
                | SyntaxKind::Function
                | SyntaxKind::FunctionBlock
                | SyntaxKind::Method
        ) && is_past_var_blocks(&ancestor, position)
    })
}

/// Finds the token at or just before a position.
fn find_token_at_position(root: &SyntaxNode, position: TextSize) -> Option<SyntaxToken> {
    // Try to get token at position, prefer right-biased
    if let Some(token) = root.token_at_offset(position).right_biased() {
        // If we're at the start of a token, return it
        if token.text_range().start() == position {
            return Some(token);
        }
        // If we're inside or at the end, return the previous token if position is at start
        return Some(token);
    }

    // If position is beyond file end, get the last token
    root.last_token()
}

/// Gets the previous non-trivia (non-whitespace, non-comment) token.
fn previous_non_trivia_token(token: &SyntaxToken) -> Option<SyntaxToken> {
    let mut prev = token.prev_token();
    while let Some(t) = prev {
        if !is_trivia(t.kind()) {
            return Some(t);
        }
        prev = t.prev_token();
    }
    None
}

/// Checks if a syntax kind is trivia (whitespace or comment).
fn is_trivia(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Whitespace | SyntaxKind::LineComment | SyntaxKind::BlockComment
    )
}

/// Checks if the token is inside an argument list.
fn is_in_argument_list(token: &SyntaxToken) -> bool {
    token
        .parent_ancestors()
        .any(|n| n.kind() == SyntaxKind::ArgList)
}

/// Checks if the token is inside a call expression.
fn is_in_call_expr(token: &SyntaxToken) -> bool {
    token
        .parent_ancestors()
        .any(|n| n.kind() == SyntaxKind::CallExpr)
}

/// Checks if there's a colon before the position in the given node.
fn has_colon_before_position(node: &SyntaxNode, position: TextSize) -> bool {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .any(|t| t.kind() == SyntaxKind::Colon && t.text_range().end() <= position)
}

/// Checks if the position is past all VAR blocks in a POU.
fn is_past_var_blocks(pou: &SyntaxNode, position: TextSize) -> bool {
    // Find the last VAR block
    let last_var_block = pou
        .children()
        .filter(|n| n.kind() == SyntaxKind::VarBlock)
        .last();

    if let Some(var_block) = last_var_block {
        position > var_block.text_range().end()
    } else {
        // No VAR blocks, we're in statement context after the POU header
        // Check if we're past the POU name
        let pou_name_end = pou
            .children()
            .find(|n| n.kind() == SyntaxKind::Name)
            .map(|n| n.text_range().end())
            .unwrap_or(pou.text_range().start());
        position > pou_name_end
    }
}

/// Checks if the position is inside a POU.
fn is_inside_pou(source_file: &SyntaxNode, position: TextSize) -> bool {
    for child in source_file.children() {
        let is_pou = matches!(
            child.kind(),
            SyntaxKind::Program
                | SyntaxKind::Function
                | SyntaxKind::FunctionBlock
                | SyntaxKind::Method
                | SyntaxKind::Interface
        );
        if is_pou && child.text_range().contains(position) {
            return true;
        }
    }
    false
}
