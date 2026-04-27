use super::super::*;

pub(in crate::db) fn check_unreachable_statements(
    root: &SyntaxNode,
    diagnostics: &mut DiagnosticBuilder,
) {
    check_unreachable_after_terminators(root, diagnostics);
    check_constant_if_branches(root, diagnostics);
}

fn check_unreachable_after_terminators(root: &SyntaxNode, diagnostics: &mut DiagnosticBuilder) {
    for stmt_list in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::StmtList)
    {
        let mut terminated = false;
        for stmt in stmt_list.children() {
            if terminated {
                diagnostics.warning(
                    DiagnosticCode::UnreachableCode,
                    stmt.text_range(),
                    "unreachable statement",
                );
                continue;
            }
            if is_terminator_stmt(&stmt) {
                terminated = true;
            }
        }
    }
}

fn check_constant_if_branches(root: &SyntaxNode, diagnostics: &mut DiagnosticBuilder) {
    for if_stmt in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::IfStmt)
    {
        let condition = first_expression_child(&if_stmt).and_then(|node| const_bool_expr(&node));
        let (then_stmts, branches) = collect_if_branches(&if_stmt);
        if matches!(condition, Some(false)) {
            mark_unreachable_statements(&then_stmts, diagnostics);
        }

        let mut previous_all_false = matches!(condition, Some(false));
        let mut branch_always_taken = matches!(condition, Some(true));

        for branch in branches {
            if branch_always_taken {
                mark_unreachable_statements(&branch.statements, diagnostics);
                continue;
            }

            if let Some(expr) = branch.condition.as_ref() {
                match const_bool_expr(expr) {
                    Some(false) => {
                        mark_unreachable_statements(&branch.statements, diagnostics);
                    }
                    Some(true) if previous_all_false => {
                        branch_always_taken = true;
                    }
                    Some(true) => {
                        previous_all_false = false;
                    }
                    None => {
                        previous_all_false = false;
                    }
                }
            } else if branch.kind == IfBranchKind::Else && branch_always_taken {
                mark_unreachable_statements(&branch.statements, diagnostics);
            }
        }
    }
}

fn is_terminator_stmt(stmt: &SyntaxNode) -> bool {
    matches!(
        stmt.kind(),
        SyntaxKind::ReturnStmt
            | SyntaxKind::ExitStmt
            | SyntaxKind::ContinueStmt
            | SyntaxKind::JmpStmt
    )
}

fn first_expression_child(node: &SyntaxNode) -> Option<SyntaxNode> {
    node.children()
        .find(|child| is_expression_kind(child.kind()))
}

fn is_expression_kind(kind: SyntaxKind) -> bool {
    kind.is_initializer_expression_node()
}

fn is_statement_kind(kind: SyntaxKind) -> bool {
    kind.is_statement_node()
}

fn const_bool_expr(node: &SyntaxNode) -> Option<bool> {
    match node.kind() {
        SyntaxKind::Literal => parse_bool_literal(node),
        SyntaxKind::ParenExpr => node
            .children()
            .find(|child| is_expression_kind(child.kind()))
            .and_then(|child| const_bool_expr(&child)),
        SyntaxKind::UnaryExpr => {
            let is_not = node
                .children_with_tokens()
                .filter_map(|child| child.into_token())
                .any(|token| token.text().eq_ignore_ascii_case("NOT"));
            if !is_not {
                return None;
            }
            let expr = node
                .children()
                .find(|child| is_expression_kind(child.kind()))?;
            const_bool_expr(&expr).map(|value| !value)
        }
        SyntaxKind::BinaryExpr => {
            let op = bool_binary_op(node)?;
            let mut exprs = node
                .children()
                .filter(|child| is_expression_kind(child.kind()));
            let lhs = exprs.next().and_then(|child| const_bool_expr(&child))?;
            let rhs = exprs
                .last()
                .and_then(|child| const_bool_expr(&child))
                .unwrap_or(lhs);
            Some(match op {
                BoolBinaryOp::And => lhs && rhs,
                BoolBinaryOp::Or => lhs || rhs,
                BoolBinaryOp::Xor => lhs ^ rhs,
            })
        }
        _ => None,
    }
}

fn parse_bool_literal(node: &SyntaxNode) -> Option<bool> {
    let text = node.text().to_string();
    if text.trim().eq_ignore_ascii_case("TRUE") {
        Some(true)
    } else if text.trim().eq_ignore_ascii_case("FALSE") {
        Some(false)
    } else {
        None
    }
}

fn bool_binary_op(node: &SyntaxNode) -> Option<BoolBinaryOp> {
    for token in node
        .children_with_tokens()
        .filter_map(|child| child.into_token())
    {
        let text = token.text();
        if text.eq_ignore_ascii_case("AND") {
            return Some(BoolBinaryOp::And);
        }
        if text.eq_ignore_ascii_case("OR") {
            return Some(BoolBinaryOp::Or);
        }
        if text.eq_ignore_ascii_case("XOR") {
            return Some(BoolBinaryOp::Xor);
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoolBinaryOp {
    And,
    Or,
    Xor,
}

#[derive(Debug)]
struct IfBranch {
    kind: IfBranchKind,
    condition: Option<SyntaxNode>,
    statements: Vec<SyntaxNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IfBranchKind {
    Elsif,
    Else,
}

fn collect_if_branches(if_stmt: &SyntaxNode) -> (Vec<SyntaxNode>, Vec<IfBranch>) {
    let mut then_stmts = Vec::new();
    let mut branches = Vec::new();
    let mut seen_branch = false;

    for child in if_stmt.children() {
        match child.kind() {
            SyntaxKind::ElsifBranch => {
                seen_branch = true;
                branches.push(IfBranch {
                    kind: IfBranchKind::Elsif,
                    condition: first_expression_child(&child),
                    statements: branch_statements(&child),
                });
            }
            SyntaxKind::ElseBranch => {
                seen_branch = true;
                branches.push(IfBranch {
                    kind: IfBranchKind::Else,
                    condition: None,
                    statements: branch_statements(&child),
                });
            }
            _ if is_statement_kind(child.kind()) && !seen_branch => {
                then_stmts.push(child);
            }
            _ => {}
        }
    }

    (then_stmts, branches)
}

fn branch_statements(node: &SyntaxNode) -> Vec<SyntaxNode> {
    node.children()
        .filter(|child| is_statement_kind(child.kind()))
        .collect()
}

fn mark_unreachable_statements(statements: &[SyntaxNode], diagnostics: &mut DiagnosticBuilder) {
    for stmt in statements {
        diagnostics.warning(
            DiagnosticCode::UnreachableCode,
            stmt.text_range(),
            "unreachable statement",
        );
    }
}
