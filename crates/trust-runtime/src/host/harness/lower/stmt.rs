use crate::debug::SourceLocation;
use crate::program_model::{property_setter_method_name, ArgValue, CallArg, CaseLabel, Expr, Stmt};
use crate::value::Value;
use trust_hir::TypeId;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::super::util::{direct_expr_children, first_expr_child, is_statement_kind, node_text};
use super::super::{CompileError, LoweringContext};
use super::expr::{
    const_value_from_node, enum_literal_value, field_expr_property_accessor_name, lower_expr,
    lower_expr_with_context, lower_expression_type, lower_lvalue, resolve_initializer_enum_variant,
    PropertyAccessor,
};

pub(in crate::harness) fn lower_stmt_list(
    program: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Vec<Stmt>, CompileError> {
    let mut stmts = Vec::new();
    let stmt_nodes: Vec<SyntaxNode> = if let Some(stmt_list) = program
        .children()
        .find(|child| child.kind() == SyntaxKind::StmtList)
    {
        stmt_list.children().collect()
    } else {
        program.children().collect()
    };

    for stmt_node in stmt_nodes {
        if !is_statement_kind(stmt_node.kind()) {
            continue;
        }
        if let Some(stmt) = lower_stmt(&stmt_node, ctx)? {
            stmts.push(stmt);
        }
    }
    Ok(stmts)
}

fn stmt_location(node: &SyntaxNode, ctx: &mut LoweringContext<'_>) -> Option<SourceLocation> {
    let range = node.text_range();
    let start = node
        .descendants_with_tokens()
        .find_map(|element| match element.into_token() {
            Some(token) if !token.kind().is_trivia() => Some(token.text_range().start()),
            _ => None,
        })
        .unwrap_or(range.start());
    let location = SourceLocation::new(ctx.file_id, start.into(), range.end().into());
    ctx.statement_locations.push(location);
    Some(location)
}

fn lower_stmt(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Option<Stmt>, CompileError> {
    match node.kind() {
        SyntaxKind::AssignStmt => lower_assign(node, ctx).map(Some),
        SyntaxKind::ExprStmt => {
            let expr = first_expr_child(node)
                .ok_or_else(|| CompileError::new("missing expression statement"))?;
            Ok(Some(Stmt::Expr {
                expr: lower_expr(&expr, ctx)?,
                location: stmt_location(node, ctx),
            }))
        }
        SyntaxKind::IfStmt => lower_if(node, ctx).map(Some),
        SyntaxKind::CaseStmt => lower_case(node, ctx).map(Some),
        SyntaxKind::ForStmt => lower_for(node, ctx).map(Some),
        SyntaxKind::WhileStmt => lower_while(node, ctx).map(Some),
        SyntaxKind::RepeatStmt => lower_repeat(node, ctx).map(Some),
        SyntaxKind::ReturnStmt => lower_return(node, ctx).map(Some),
        SyntaxKind::ExitStmt => Ok(Some(Stmt::Exit {
            location: stmt_location(node, ctx),
        })),
        SyntaxKind::ContinueStmt => Ok(Some(Stmt::Continue {
            location: stmt_location(node, ctx),
        })),
        SyntaxKind::EmptyStmt => Ok(None),
        SyntaxKind::LabelStmt => lower_label_stmt(node, ctx).map(Some),
        SyntaxKind::JmpStmt => lower_jmp_stmt(node, ctx).map(Some),
        _ => Err(CompileError::new("unsupported statement")),
    }
}

fn lower_assign(node: &SyntaxNode, ctx: &mut LoweringContext<'_>) -> Result<Stmt, CompileError> {
    let exprs = direct_expr_children(node);
    if exprs.len() != 2 {
        return Err(CompileError::new("invalid assignment"));
    }
    let target_type = lower_expression_type(&exprs[0], ctx)?;
    let property_setter = field_expr_property_accessor_name(&exprs[0], ctx, PropertyAccessor::Set)?;
    let target = lower_lvalue(&exprs[0], ctx)?;
    let value = lower_expr_with_context(&exprs[1], ctx, target_type)?;
    let value = match target_type {
        Some(type_id) => resolve_initializer_enum_variant(&exprs[1], value, type_id, ctx)?,
        None => value,
    };
    let location = stmt_location(node, ctx);
    if let Some(property_name) = property_setter {
        let field_parts = direct_expr_children(&exprs[0]);
        let receiver = field_parts
            .first()
            .ok_or_else(|| CompileError::new("invalid property assignment"))?;
        return Ok(Stmt::Expr {
            expr: Expr::Call {
                target: Box::new(Expr::Field {
                    target: Box::new(lower_expr(receiver, ctx)?),
                    field: property_setter_method_name(&property_name),
                }),
                args: vec![CallArg {
                    name: None,
                    value: ArgValue::Expr(value),
                }],
            },
            location,
        });
    }
    if assignment_is_attempt(node) {
        Ok(Stmt::AssignAttempt {
            target,
            value,
            location,
        })
    } else {
        Ok(Stmt::Assign {
            target,
            value,
            location,
        })
    }
}

fn assignment_is_attempt(node: &SyntaxNode) -> bool {
    node.children_with_tokens()
        .filter_map(|child| child.into_token())
        .any(|token| token.kind() == SyntaxKind::RefAssign)
}

fn lower_if(node: &SyntaxNode, ctx: &mut LoweringContext<'_>) -> Result<Stmt, CompileError> {
    let condition =
        first_expr_child(node).ok_or_else(|| CompileError::new("missing IF condition"))?;
    let condition = lower_expr(&condition, ctx)?;

    let mut then_block = Vec::new();
    let mut else_if = Vec::new();
    let mut else_block = Vec::new();
    let mut seen_branch = false;

    for child in node.children() {
        match child.kind() {
            SyntaxKind::ElsifBranch => {
                seen_branch = true;
                else_if.push(lower_elsif(&child, ctx)?);
            }
            SyntaxKind::ElseBranch => {
                seen_branch = true;
                else_block = lower_else_block(&child, ctx)?;
            }
            _ if is_statement_kind(child.kind()) && !seen_branch => {
                if let Some(stmt) = lower_stmt(&child, ctx)? {
                    then_block.push(stmt);
                }
            }
            _ => {}
        }
    }

    Ok(Stmt::If {
        condition,
        then_block,
        else_if,
        else_block,
        location: stmt_location(node, ctx),
    })
}

fn lower_elsif(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<(Expr, Vec<Stmt>), CompileError> {
    let condition =
        first_expr_child(node).ok_or_else(|| CompileError::new("missing ELSIF condition"))?;
    let condition = lower_expr(&condition, ctx)?;
    let mut stmts = Vec::new();
    for child in node.children() {
        if !is_statement_kind(child.kind()) {
            continue;
        }
        if let Some(stmt) = lower_stmt(&child, ctx)? {
            stmts.push(stmt);
        }
    }
    Ok((condition, stmts))
}

fn lower_else_block(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Vec<Stmt>, CompileError> {
    let mut stmts = Vec::new();
    for child in node.children() {
        if !is_statement_kind(child.kind()) {
            continue;
        }
        if let Some(stmt) = lower_stmt(&child, ctx)? {
            stmts.push(stmt);
        }
    }
    Ok(stmts)
}

fn lower_case(node: &SyntaxNode, ctx: &mut LoweringContext<'_>) -> Result<Stmt, CompileError> {
    let selector_node =
        first_expr_child(node).ok_or_else(|| CompileError::new("missing CASE selector"))?;
    let selector_type = lower_expression_type(&selector_node, ctx)?;
    let selector = lower_expr(&selector_node, ctx)?;

    let mut branches = Vec::new();
    let mut else_block = Vec::new();

    for child in node.children() {
        match child.kind() {
            SyntaxKind::CaseBranch => {
                branches.push(lower_case_branch(&child, selector_type, ctx)?);
            }
            SyntaxKind::ElseBranch => {
                else_block = lower_else_block(&child, ctx)?;
            }
            _ => {}
        }
    }

    Ok(Stmt::Case {
        selector,
        branches,
        else_block,
        location: stmt_location(node, ctx),
    })
}

fn lower_case_branch(
    node: &SyntaxNode,
    selector_type: Option<TypeId>,
    ctx: &mut LoweringContext<'_>,
) -> Result<(Vec<CaseLabel>, Vec<Stmt>), CompileError> {
    let mut labels = Vec::new();
    let mut stmts = Vec::new();

    for child in node.children() {
        match child.kind() {
            SyntaxKind::CaseLabel => labels.extend(lower_case_label(&child, selector_type, ctx)?),
            _ if is_statement_kind(child.kind()) => {
                if let Some(stmt) = lower_stmt(&child, ctx)? {
                    stmts.push(stmt);
                }
            }
            _ => {}
        }
    }

    Ok((labels, stmts))
}

fn lower_case_label(
    node: &SyntaxNode,
    selector_type: Option<TypeId>,
    ctx: &mut LoweringContext<'_>,
) -> Result<Vec<CaseLabel>, CompileError> {
    let exprs = if let Some(subrange) = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Subrange)
    {
        direct_expr_children(&subrange)
    } else {
        direct_expr_children(node)
    };
    if exprs.is_empty() {
        return Err(CompileError::new("missing CASE label"));
    }
    if exprs.len() == 1 {
        let value = const_case_label_value(&exprs[0], selector_type, ctx)?;
        return Ok(vec![CaseLabel::Single(value)]);
    }
    if exprs.len() == 2 {
        let lower = const_case_label_int(&exprs[0], selector_type, ctx)?;
        let upper = const_case_label_int(&exprs[1], selector_type, ctx)?;
        return Ok(vec![CaseLabel::Range(lower, upper)]);
    }
    Err(CompileError::new("invalid CASE label"))
}

fn const_case_label_value(
    node: &SyntaxNode,
    selector_type: Option<TypeId>,
    ctx: &mut LoweringContext<'_>,
) -> Result<Value, CompileError> {
    match const_value_from_node(node, ctx) {
        Ok(value) => Ok(value),
        Err(err) => {
            let Some(type_id) = selector_type else {
                return Err(err);
            };
            if node.kind() != SyntaxKind::NameRef {
                return Err(err);
            }
            enum_literal_value(node_text(node).as_str(), type_id, ctx.registry).ok_or(err)
        }
    }
}

fn const_case_label_int(
    node: &SyntaxNode,
    selector_type: Option<TypeId>,
    ctx: &mut LoweringContext<'_>,
) -> Result<i64, CompileError> {
    match const_case_label_value(node, selector_type, ctx)? {
        Value::SInt(v) => Ok(v as i64),
        Value::Int(v) => Ok(v as i64),
        Value::DInt(v) => Ok(v as i64),
        Value::LInt(v) => Ok(v),
        Value::USInt(v) => Ok(v as i64),
        Value::UInt(v) => Ok(v as i64),
        Value::UDInt(v) => Ok(v as i64),
        Value::ULInt(v) => {
            Ok(i64::try_from(v).map_err(|_| CompileError::new("integer constant out of range"))?)
        }
        Value::Byte(v) => Ok(v as i64),
        Value::Word(v) => Ok(v as i64),
        Value::DWord(v) => Ok(v as i64),
        Value::LWord(v) => {
            Ok(i64::try_from(v).map_err(|_| CompileError::new("integer constant out of range"))?)
        }
        Value::Enum(enum_value) => Ok(enum_value.numeric_value()),
        _ => Err(CompileError::new("expected integer constant")),
    }
}

fn lower_for(node: &SyntaxNode, ctx: &mut LoweringContext<'_>) -> Result<Stmt, CompileError> {
    let control = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)
        .ok_or_else(|| CompileError::new("missing FOR control variable"))?;
    let control = node_text(&control).into();

    let exprs = direct_expr_children(node);
    if exprs.len() < 2 {
        return Err(CompileError::new("missing FOR bounds"));
    }
    let start = lower_expr(&exprs[0], ctx)?;
    let end = lower_expr(&exprs[1], ctx)?;
    let step = if exprs.len() >= 3 {
        lower_expr(&exprs[2], ctx)?
    } else {
        Expr::Literal(Value::Int(1))
    };

    let mut body = Vec::new();
    for child in node.children() {
        if !is_statement_kind(child.kind()) {
            continue;
        }
        if let Some(stmt) = lower_stmt(&child, ctx)? {
            body.push(stmt);
        }
    }

    Ok(Stmt::For {
        control,
        start,
        end,
        step,
        body,
        location: stmt_location(node, ctx),
    })
}

fn lower_while(node: &SyntaxNode, ctx: &mut LoweringContext<'_>) -> Result<Stmt, CompileError> {
    let condition =
        first_expr_child(node).ok_or_else(|| CompileError::new("missing WHILE condition"))?;
    let condition = lower_expr(&condition, ctx)?;
    let mut body = Vec::new();
    for child in node.children() {
        if !is_statement_kind(child.kind()) {
            continue;
        }
        if let Some(stmt) = lower_stmt(&child, ctx)? {
            body.push(stmt);
        }
    }
    Ok(Stmt::While {
        condition,
        body,
        location: stmt_location(node, ctx),
    })
}

fn lower_repeat(node: &SyntaxNode, ctx: &mut LoweringContext<'_>) -> Result<Stmt, CompileError> {
    let condition =
        first_expr_child(node).ok_or_else(|| CompileError::new("missing UNTIL condition"))?;
    let condition = lower_expr(&condition, ctx)?;
    let mut body = Vec::new();
    for child in node.children() {
        if !is_statement_kind(child.kind()) {
            continue;
        }
        if let Some(stmt) = lower_stmt(&child, ctx)? {
            body.push(stmt);
        }
    }
    Ok(Stmt::Repeat {
        body,
        until: condition,
        location: stmt_location(node, ctx),
    })
}

fn lower_label_stmt(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Stmt, CompileError> {
    let name = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)
        .ok_or_else(|| CompileError::new("missing label name"))?;
    let name = node_text(&name).into();

    let mut inner_stmt = None;
    for child in node.children() {
        if !is_statement_kind(child.kind()) {
            continue;
        }
        inner_stmt = lower_stmt(&child, ctx)?.map(Box::new);
        break;
    }

    Ok(Stmt::Label {
        name,
        stmt: inner_stmt,
        location: stmt_location(node, ctx),
    })
}

fn lower_jmp_stmt(node: &SyntaxNode, ctx: &mut LoweringContext<'_>) -> Result<Stmt, CompileError> {
    let target = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)
        .ok_or_else(|| CompileError::new("missing JMP target"))?;
    Ok(Stmt::Jmp {
        target: node_text(&target).into(),
        location: stmt_location(node, ctx),
    })
}

fn lower_return(node: &SyntaxNode, ctx: &mut LoweringContext<'_>) -> Result<Stmt, CompileError> {
    let expr = first_expr_child(node)
        .map(|expr| lower_expr(&expr, ctx))
        .transpose()?;
    Ok(Stmt::Return {
        expr,
        location: stmt_location(node, ctx),
    })
}
