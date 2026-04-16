use super::const_utils::*;
use super::*;

impl SymbolCollector<'_> {
    pub(super) fn evaluate_constants(&mut self) {
        let keys: Vec<_> = self.const_exprs.keys().cloned().collect();
        let mut guard = FxHashSet::default();
        for (scope, name) in keys {
            let _ = self.resolve_const_value_for_scope(name.as_str(), &scope, &mut guard);
        }
    }

    pub(super) fn eval_int_expr_in_scope(
        &mut self,
        node: &SyntaxNode,
        scopes: &[Option<SmolStr>],
    ) -> Option<i64> {
        let mut guard = FxHashSet::default();
        self.eval_int_expr(node, scopes, &mut guard)
    }

    pub(super) fn eval_int_expr(
        &mut self,
        node: &SyntaxNode,
        scopes: &[Option<SmolStr>],
        guard: &mut FxHashSet<(Option<SmolStr>, SmolStr)>,
    ) -> Option<i64> {
        match node.kind() {
            SyntaxKind::Literal => parse_int_literal_from_node(node),
            SyntaxKind::NameRef => {
                let name = first_ident_token(node)?.text().to_string();
                self.resolve_const_value(&name, scopes, guard)
                    .or_else(|| self.table.enum_value_by_name(&name))
            }
            SyntaxKind::ParenExpr => node
                .children()
                .next()
                .and_then(|child| self.eval_int_expr(&child, scopes, guard)),
            SyntaxKind::UnaryExpr => {
                let op = unary_op_from_node(node)?;
                let expr = node.children().next()?;
                let value = self.eval_int_expr(&expr, scopes, guard)?;
                match op {
                    IntUnaryOp::Plus => Some(value),
                    IntUnaryOp::Minus => value.checked_neg(),
                }
            }
            SyntaxKind::BinaryExpr => {
                let children: Vec<_> = node.children().collect();
                if children.len() < 2 {
                    return None;
                }
                let lhs = self.eval_int_expr(&children[0], scopes, guard)?;
                let rhs = self.eval_int_expr(&children[children.len() - 1], scopes, guard)?;
                match binary_op_from_node(node)? {
                    IntBinaryOp::Add => lhs.checked_add(rhs),
                    IntBinaryOp::Sub => lhs.checked_sub(rhs),
                    IntBinaryOp::Mul => lhs.checked_mul(rhs),
                    IntBinaryOp::Div => {
                        if rhs == 0 {
                            None
                        } else {
                            lhs.checked_div(rhs)
                        }
                    }
                    IntBinaryOp::Mod => {
                        if rhs == 0 {
                            None
                        } else {
                            lhs.checked_rem(rhs)
                        }
                    }
                    IntBinaryOp::Power => {
                        if rhs < 0 {
                            None
                        } else {
                            lhs.checked_pow(rhs as u32)
                        }
                    }
                }
            }
            _ => None,
        }
    }

    pub(super) fn resolve_const_value(
        &mut self,
        name: &str,
        scopes: &[Option<SmolStr>],
        guard: &mut FxHashSet<(Option<SmolStr>, SmolStr)>,
    ) -> Option<i64> {
        for scope in scopes {
            if let Some(value) = self.resolve_const_value_for_scope(name, scope, guard) {
                return Some(value);
            }
        }
        None
    }

    pub(super) fn resolve_const_value_for_scope(
        &mut self,
        name: &str,
        scope: &Option<SmolStr>,
        guard: &mut FxHashSet<(Option<SmolStr>, SmolStr)>,
    ) -> Option<i64> {
        let key = const_key(scope, name);
        if let Some(value) = self.const_values.get(&key) {
            return Some(*value);
        }
        let expr = self.const_exprs.get(&key).cloned()?;
        if !guard.insert(key.clone()) {
            return None;
        }
        let scopes = scope_chain_for_node(&expr);
        let value = self.eval_int_expr(&expr, &scopes, guard);
        guard.remove(&key);
        if let Some(value) = value {
            self.const_values.insert(key, value);
        }
        value
    }
}
