use super::literals::{
    int_binary_op_from_node, int_unary_op_from_node, parse_int_literal_from_node, IntBinaryOp,
    IntUnaryOp,
};
use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConstEvalError {
    NotConstant,
    UndefinedName(SmolStr),
    DivideByZero,
    IntegerOverflow,
    NegativeExponent,
    CyclicDependency(SmolStr),
}

impl<'a> TypeChecker<'a> {
    pub(super) fn eval_const_int_expr(&self, node: &SyntaxNode) -> Option<i64> {
        self.try_eval_const_int_expr(node).ok()
    }

    pub(super) fn try_eval_const_int_expr(&self, node: &SyntaxNode) -> Result<i64, ConstEvalError> {
        let mut guard = FxHashSet::default();
        self.try_eval_const_int_expr_inner(node, &mut guard)
    }

    fn try_eval_const_int_expr_inner(
        &self,
        node: &SyntaxNode,
        guard: &mut FxHashSet<SmolStr>,
    ) -> Result<i64, ConstEvalError> {
        match node.kind() {
            SyntaxKind::Literal => {
                if let Some(value) = parse_int_literal_from_node(node) {
                    return Ok(value);
                }
                self.enum_value_from_typed_literal(node)
                    .ok_or(ConstEvalError::NotConstant)
            }
            SyntaxKind::NameRef => {
                let name = self
                    .resolve_ref()
                    .get_name_from_ref(node)
                    .ok_or(ConstEvalError::NotConstant)?;
                let key = SmolStr::new(name.to_ascii_uppercase());
                if !guard.insert(key.clone()) {
                    return Err(ConstEvalError::CyclicDependency(key));
                }
                for scope in self.const_scope_chain() {
                    if let Some(value) = self.symbols.const_value(&scope, name.as_str()) {
                        guard.remove(&key);
                        return Ok(value);
                    }
                }
                let result = self
                    .symbols
                    .enum_value_by_name(name.as_str())
                    .ok_or_else(|| ConstEvalError::UndefinedName(name.clone()));
                guard.remove(&key);
                result
            }
            SyntaxKind::ParenExpr => node
                .children()
                .next()
                .ok_or(ConstEvalError::NotConstant)
                .and_then(|child| self.try_eval_const_int_expr_inner(&child, guard)),
            SyntaxKind::UnaryExpr => {
                let op = int_unary_op_from_node(node).ok_or(ConstEvalError::NotConstant)?;
                let expr = node.children().next().ok_or(ConstEvalError::NotConstant)?;
                let value = self.try_eval_const_int_expr_inner(&expr, guard)?;
                match op {
                    IntUnaryOp::Plus => Ok(value),
                    IntUnaryOp::Minus => value.checked_neg().ok_or(ConstEvalError::IntegerOverflow),
                }
            }
            SyntaxKind::BinaryExpr => {
                let children: Vec<_> = node.children().collect();
                if children.len() < 2 {
                    return Err(ConstEvalError::NotConstant);
                }
                let lhs = self.try_eval_const_int_expr_inner(&children[0], guard)?;
                let rhs =
                    self.try_eval_const_int_expr_inner(&children[children.len() - 1], guard)?;
                match int_binary_op_from_node(node).ok_or(ConstEvalError::NotConstant)? {
                    IntBinaryOp::Add => lhs.checked_add(rhs).ok_or(ConstEvalError::IntegerOverflow),
                    IntBinaryOp::Sub => lhs.checked_sub(rhs).ok_or(ConstEvalError::IntegerOverflow),
                    IntBinaryOp::Mul => lhs.checked_mul(rhs).ok_or(ConstEvalError::IntegerOverflow),
                    IntBinaryOp::Div => {
                        if rhs == 0 {
                            Err(ConstEvalError::DivideByZero)
                        } else {
                            lhs.checked_div(rhs).ok_or(ConstEvalError::IntegerOverflow)
                        }
                    }
                    IntBinaryOp::Mod => {
                        if rhs == 0 {
                            Err(ConstEvalError::DivideByZero)
                        } else {
                            lhs.checked_rem(rhs).ok_or(ConstEvalError::IntegerOverflow)
                        }
                    }
                    IntBinaryOp::Power => {
                        if rhs < 0 {
                            Err(ConstEvalError::NegativeExponent)
                        } else {
                            lhs.checked_pow(rhs as u32)
                                .ok_or(ConstEvalError::IntegerOverflow)
                        }
                    }
                }
            }
            _ => Err(ConstEvalError::NotConstant),
        }
    }

    fn enum_value_from_typed_literal(&self, node: &SyntaxNode) -> Option<i64> {
        let mut type_name = None;
        for token in node
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
        {
            if token.kind() != SyntaxKind::TypedLiteralPrefix {
                continue;
            }
            type_name = Some(token.text().trim_end_matches('#').to_string());
            break;
        }
        let type_name = type_name?;
        let type_id = self
            .resolve_ref()
            .resolve_type_by_name(type_name.as_str())?;
        let resolved = self.resolve_alias_type(type_id);
        let Type::Enum { values, .. } = self.symbols.type_by_id(resolved)? else {
            return None;
        };

        let text = node.text().to_string();
        let (_, value_text) = text.split_once('#')?;
        let value_text = value_text.trim();
        if value_text.is_empty() {
            return None;
        }
        let value_name = value_text
            .split('.')
            .next_back()
            .map(str::trim)
            .unwrap_or(value_text);

        values
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(value_name))
            .map(|(_, value)| *value)
    }

    pub(super) fn const_scope_chain(&self) -> Vec<Option<SmolStr>> {
        let mut scopes = Vec::new();
        let mut current = Some(self.current_scope);

        while let Some(scope_id) = current {
            let Some(scope) = self.symbols.get_scope(scope_id) else {
                break;
            };

            if matches!(
                scope.kind,
                ScopeKind::Program
                    | ScopeKind::Function
                    | ScopeKind::FunctionBlock
                    | ScopeKind::Class
                    | ScopeKind::Method
                    | ScopeKind::Property
                    | ScopeKind::Namespace
                    | ScopeKind::Configuration
                    | ScopeKind::Resource
            ) {
                if let Some(owner) = scope.owner {
                    if let Some(symbol) = self.symbols.get(owner) {
                        scopes.push(Some(symbol.name.clone()));
                    }
                }
            }

            current = scope.parent;
        }

        scopes.push(None);
        scopes
    }
}
