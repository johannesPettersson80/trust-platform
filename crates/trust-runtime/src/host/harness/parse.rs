use smol_str::SmolStr;

use crate::program_model::{Expr, LValue};
use crate::value::DateTimeProfile;
use trust_hir::types::TypeRegistry;
use trust_syntax::parser;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::util::{direct_expr_children, first_expr_child, node_text};
use super::{CompileError, LoweringContext};

/// Parse and lower a debug/watch expression with side-effect restrictions.
pub fn parse_debug_expression(
    expression: &str,
    registry: &mut TypeRegistry,
    profile: DateTimeProfile,
    using: &[SmolStr],
) -> Result<Expr, CompileError> {
    let expression = expression.trim();
    let expression = expression.strip_suffix(';').unwrap_or(expression).trim();
    if expression.is_empty() {
        return Err(CompileError::new("empty watch expression"));
    }

    let wrapped = format!("PROGRAM __WATCH\n__watch := {expression};\nEND_PROGRAM");
    let parse = parser::parse(&wrapped);
    if !parse.ok() {
        let message = parse
            .errors()
            .iter()
            .map(|err| err.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        return Err(CompileError::new(message));
    }

    let syntax = parse.syntax();
    let assign = syntax
        .descendants()
        .find(|node| node.kind() == SyntaxKind::AssignStmt)
        .ok_or_else(|| CompileError::new("missing watch expression"))?;
    let exprs = direct_expr_children(&assign);
    if exprs.len() != 2 {
        return Err(CompileError::new("invalid watch expression"));
    }
    let expr = &exprs[1];
    if expression_has_side_effects(expr) {
        return Err(CompileError::new(
            "watch expressions must be side-effect free (only pure standard functions are allowed)",
        ));
    }

    let mut statement_locations = Vec::new();
    let mut ctx = LoweringContext {
        registry,
        profile,
        using: using.to_vec(),
        file_id: 0,
        semantic_db: None,
        semantic_file_id: None,
        statement_locations: &mut statement_locations,
        compile_time_consts: Default::default(),
    };
    super::lower_expr(expr, &mut ctx)
}

/// Parse and lower a debug assignment target with side-effect restrictions.
pub fn parse_debug_lvalue(
    expression: &str,
    registry: &mut TypeRegistry,
    profile: DateTimeProfile,
    using: &[SmolStr],
) -> Result<LValue, CompileError> {
    let expression = expression.trim();
    let expression = expression.strip_suffix(';').unwrap_or(expression).trim();
    if expression.is_empty() {
        return Err(CompileError::new("empty assignment target"));
    }

    let wrapped = format!("PROGRAM __WATCH\n{expression} := 0;\nEND_PROGRAM");
    let parse = parser::parse(&wrapped);
    if !parse.ok() {
        let message = parse
            .errors()
            .iter()
            .map(|err| err.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        return Err(CompileError::new(message));
    }

    let syntax = parse.syntax();
    let assign = syntax
        .descendants()
        .find(|node| node.kind() == SyntaxKind::AssignStmt)
        .ok_or_else(|| CompileError::new("missing assignment target"))?;
    let exprs = direct_expr_children(&assign);
    if exprs.len() != 2 {
        return Err(CompileError::new("invalid assignment target"));
    }
    let target = &exprs[0];
    if expression_has_side_effects(target) {
        return Err(CompileError::new(
            "assignment target must be side-effect free",
        ));
    }

    let mut statement_locations = Vec::new();
    let mut ctx = LoweringContext {
        registry,
        profile,
        using: using.to_vec(),
        file_id: 0,
        semantic_db: None,
        semantic_file_id: None,
        statement_locations: &mut statement_locations,
        compile_time_consts: Default::default(),
    };
    super::lower::lower_lvalue(target, &mut ctx)
}

fn expression_has_side_effects(node: &SyntaxNode) -> bool {
    for call in node
        .descendants()
        .filter(|child| child.kind() == SyntaxKind::CallExpr)
    {
        let Some(target) = first_expr_child(&call) else {
            return true;
        };
        let Some(name) = call_target_name_node(&target) else {
            return true;
        };
        if !is_allowed_watch_call(&name) {
            return true;
        }
    }
    false
}

fn call_target_name_node(node: &SyntaxNode) -> Option<String> {
    match node.kind() {
        SyntaxKind::NameRef => Some(node_text(node)),
        SyntaxKind::FieldExpr => {
            let exprs = direct_expr_children(node);
            let target = exprs.first()?;
            let prefix = call_target_name_node(target)?;
            let field = node
                .children()
                .find(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::Literal))?;
            let field_name = node_text(&field);
            Some(format!("{prefix}.{field_name}"))
        }
        _ => None,
    }
}

fn is_allowed_watch_call(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    is_pure_stdlib_name(&upper)
        || crate::stdlib::conversions::is_conversion_name(&upper)
        || crate::stdlib::time::is_split_name(&upper)
}

fn is_pure_stdlib_name(name: &str) -> bool {
    matches!(
        name,
        "ABS"
            | "MIN"
            | "MAX"
            | "LIMIT"
            | "SEL"
            | "MUX"
            | "SQRT"
            | "LN"
            | "LOG"
            | "EXP"
            | "SIN"
            | "COS"
            | "TAN"
            | "ASIN"
            | "ACOS"
            | "ATAN"
            | "ATAN2"
            | "LEN"
    )
}
