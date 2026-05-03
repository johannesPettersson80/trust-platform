fn binary_op_from_node(node: &SyntaxNode) -> Result<BinaryOp, CompileError> {
    for element in node.children_with_tokens() {
        let token = match element.into_token() {
            Some(token) => token,
            None => continue,
        };
        match token.kind() {
            SyntaxKind::Plus => return Ok(BinaryOp::Add),
            SyntaxKind::Minus => return Ok(BinaryOp::Sub),
            SyntaxKind::Star => return Ok(BinaryOp::Mul),
            SyntaxKind::Slash => return Ok(BinaryOp::Div),
            SyntaxKind::Power => return Ok(BinaryOp::Pow),
            SyntaxKind::KwMod => return Ok(BinaryOp::Mod),
            SyntaxKind::KwAnd | SyntaxKind::Ampersand => return Ok(BinaryOp::And),
            SyntaxKind::KwOr => return Ok(BinaryOp::Or),
            SyntaxKind::KwXor => return Ok(BinaryOp::Xor),
            SyntaxKind::Eq => return Ok(BinaryOp::Eq),
            SyntaxKind::Neq => return Ok(BinaryOp::Ne),
            SyntaxKind::Lt => return Ok(BinaryOp::Lt),
            SyntaxKind::LtEq => return Ok(BinaryOp::Le),
            SyntaxKind::Gt => return Ok(BinaryOp::Gt),
            SyntaxKind::GtEq => return Ok(BinaryOp::Ge),
            _ => {}
        }
    }
    Err(CompileError::new("unsupported binary operator"))
}

fn unary_op_from_node(node: &SyntaxNode) -> Result<UnaryOp, CompileError> {
    for element in node.children_with_tokens() {
        let token = match element.into_token() {
            Some(token) => token,
            None => continue,
        };
        match token.kind() {
            SyntaxKind::Plus => return Ok(UnaryOp::Pos),
            SyntaxKind::Minus => return Ok(UnaryOp::Neg),
            SyntaxKind::KwNot => return Ok(UnaryOp::Not),
            _ => {}
        }
    }
    Err(CompileError::new("unsupported unary operator"))
}
