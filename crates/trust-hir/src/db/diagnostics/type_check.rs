use super::super::*;
use super::context::{action_context, is_top_level_stmt_list, pou_context, property_type_for_node};

pub(in crate::db) type ExpressionTypeMap = FxHashMap<(u32, u32), TypeId>;

pub(in crate::db) fn type_check_file(
    symbols: &mut SymbolTable,
    root: &SyntaxNode,
    diagnostics: &mut DiagnosticBuilder,
) -> ExpressionTypeMap {
    let mut expression_types = ExpressionTypeMap::default();

    // Find all POUs and type-check their bodies
    for node in root.descendants() {
        match node.kind() {
            SyntaxKind::Program
            | SyntaxKind::Function
            | SyntaxKind::FunctionBlock
            | SyntaxKind::Method => {
                type_check_pou_with_expression_types(
                    symbols,
                    &node,
                    diagnostics,
                    Some(&mut expression_types),
                );
            }
            SyntaxKind::Action => {
                type_check_action_with_expression_types(
                    symbols,
                    &node,
                    diagnostics,
                    Some(&mut expression_types),
                );
            }
            SyntaxKind::Property => {
                type_check_property_with_expression_types(
                    symbols,
                    &node,
                    diagnostics,
                    Some(&mut expression_types),
                );
            }
            _ => {}
        }
    }
    expression_types
}

/// Type checks a single POU (Program, Function, FunctionBlock, or Method).
fn type_check_pou_with_expression_types(
    symbols: &mut SymbolTable,
    node: &SyntaxNode,
    diagnostics: &mut DiagnosticBuilder,
    expression_types: Option<&mut ExpressionTypeMap>,
) {
    let context = pou_context(symbols, node);

    if node.kind() == SyntaxKind::Method {
        if let Some(symbol_id) = context.symbol_id {
            if let Some(symbol) = symbols.get(symbol_id) {
                if let Some(parent_id) = symbol.parent {
                    if let Some(parent) = symbols.get(parent_id) {
                        if matches!(&parent.kind, SymbolKind::Interface) {
                            return;
                        }
                    }
                }
            }
        }
    }

    // Create type checker
    let mut checker = TypeChecker::new(symbols, diagnostics, context.scope_id);
    checker.set_return_type(context.return_type);
    checker.set_receiver_types(context.this_type, context.super_type);
    checker.set_current_pou(context.symbol_id);

    // Find and check all statements in the POU body
    for stmt_list in node
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::StmtList)
    {
        if !is_top_level_stmt_list(&stmt_list, node) {
            continue;
        }
        checker.stmt().check_statement_list_with_labels(&stmt_list);
    }

    checker.finish_return_checks(node);

    if let Some(expression_types) = expression_types {
        expression_types.extend(checker.take_expression_types());
    }
}

fn type_check_property_with_expression_types(
    symbols: &mut SymbolTable,
    node: &SyntaxNode,
    diagnostics: &mut DiagnosticBuilder,
    mut expression_types: Option<&mut ExpressionTypeMap>,
) {
    let context = pou_context(symbols, node);
    let prop_type = property_type_for_node(symbols, node);

    for stmt_list in node
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::StmtList)
    {
        if !is_top_level_stmt_list(&stmt_list, node) {
            continue;
        }

        let is_get = matches!(
            stmt_list.parent().map(|parent| parent.kind()),
            Some(SyntaxKind::PropertyGet)
        );
        let return_type = match stmt_list.parent().map(|parent| parent.kind()) {
            Some(SyntaxKind::PropertyGet) => prop_type,
            Some(SyntaxKind::PropertySet) => None,
            _ => context.return_type,
        };

        let mut checker = TypeChecker::new(symbols, diagnostics, context.scope_id);
        checker.set_return_type(return_type);
        checker.set_receiver_types(context.this_type, context.super_type);
        if is_get {
            checker.set_current_pou(context.symbol_id);
        }
        checker.stmt().check_statement_list_with_labels(&stmt_list);

        if let Some(expression_types) = expression_types.as_mut() {
            expression_types.extend(checker.take_expression_types());
        }
    }
}

fn type_check_action_with_expression_types(
    symbols: &mut SymbolTable,
    node: &SyntaxNode,
    diagnostics: &mut DiagnosticBuilder,
    expression_types: Option<&mut ExpressionTypeMap>,
) {
    let context = action_context(symbols, node);
    let mut checker = TypeChecker::new(symbols, diagnostics, context.scope_id);
    checker.set_return_type(None);
    checker.set_receiver_types(context.this_type, context.super_type);
    checker.set_current_pou(context.symbol_id);

    if let Some(stmt_list) = node.children().find(|n| n.kind() == SyntaxKind::StmtList) {
        checker.stmt().check_statement_list_with_labels(&stmt_list);
    }

    if let Some(expression_types) = expression_types {
        expression_types.extend(checker.take_expression_types());
    }
}
