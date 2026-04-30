use super::literals::{
    int_binary_op_from_node, int_unary_op_from_node, parse_int_literal_from_node, IntBinaryOp,
    IntUnaryOp,
};
use super::*;
use crate::symbols::EnumValueResolution;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConstEvalError {
    NotConstant,
    UndefinedName(SmolStr),
    DivideByZero,
    IntegerOverflow,
    NegativeExponent,
    CyclicDependency(SmolStr),
    AmbiguousName(SmolStr),
}

impl<'a> TypeChecker<'a> {
    pub(super) fn eval_const_int_expr_or_report(&mut self, node: &SyntaxNode) -> Option<i64> {
        match self.try_eval_const_int_expr(node) {
            Ok(value) => Some(value),
            Err(ConstEvalError::NotConstant) => None,
            Err(ConstEvalError::UndefinedName(name))
                if self
                    .symbols
                    .resolve(name.as_str(), self.current_scope)
                    .is_some() =>
            {
                None
            }
            Err(err) => {
                self.report_const_int_eval_error(err, node.text_range(), None);
                None
            }
        }
    }

    pub(super) fn require_const_int_expr(
        &mut self,
        node: &SyntaxNode,
        not_constant_message: &'static str,
    ) -> Option<i64> {
        match self.try_eval_const_int_expr(node) {
            Ok(value) => Some(value),
            Err(ConstEvalError::UndefinedName(name))
                if self
                    .symbols
                    .resolve(name.as_str(), self.current_scope)
                    .is_some() =>
            {
                self.diagnostics.error(
                    DiagnosticCode::InvalidOperation,
                    node.text_range(),
                    not_constant_message,
                );
                None
            }
            Err(err) => {
                self.report_const_int_eval_error(
                    err,
                    node.text_range(),
                    Some(not_constant_message),
                );
                None
            }
        }
    }

    fn report_const_int_eval_error(
        &mut self,
        err: ConstEvalError,
        range: TextRange,
        not_constant_message: Option<&'static str>,
    ) {
        match err {
            ConstEvalError::CyclicDependency(name) => self.diagnostics.error(
                DiagnosticCode::CyclicDependency,
                range,
                format!("cyclic constant reference involving '{name}'"),
            ),
            ConstEvalError::DivideByZero => self.diagnostics.error(
                DiagnosticCode::InvalidOperation,
                range,
                "constant expression divides by zero",
            ),
            ConstEvalError::IntegerOverflow => self.diagnostics.error(
                DiagnosticCode::InvalidOperation,
                range,
                "constant expression overflows",
            ),
            ConstEvalError::NegativeExponent => self.diagnostics.error(
                DiagnosticCode::InvalidOperation,
                range,
                "integer exponent must be non-negative",
            ),
            ConstEvalError::UndefinedName(name) => self.diagnostics.error(
                DiagnosticCode::UndefinedVariable,
                range,
                format!("undefined constant '{name}'"),
            ),
            ConstEvalError::AmbiguousName(name) => self.diagnostics.error(
                DiagnosticCode::CannotResolve,
                range,
                format!("ambiguous enum value '{name}'"),
            ),
            ConstEvalError::NotConstant => {
                if let Some(message) = not_constant_message {
                    self.diagnostics
                        .error(DiagnosticCode::InvalidOperation, range, message);
                }
            }
        }
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
                let result = match self.symbols.resolve_enum_value_by_name(name.as_str()) {
                    EnumValueResolution::Resolved(value) => Ok(value),
                    EnumValueResolution::NotFound => {
                        Err(ConstEvalError::UndefinedName(name.clone()))
                    }
                    EnumValueResolution::Ambiguous => {
                        Err(ConstEvalError::AmbiguousName(name.clone()))
                    }
                };
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

            if const_named_scope_kind(scope.kind) {
                if let Some(scope_name) = self.const_scope_name_for_scope(scope_id) {
                    if scopes.last() != Some(&Some(scope_name.clone())) {
                        scopes.push(Some(scope_name));
                    }
                }
            }

            current = scope.parent;
        }

        scopes.push(None);
        scopes
    }

    fn const_scope_name_for_scope(&self, scope_id: ScopeId) -> Option<SmolStr> {
        let mut parts = Vec::new();
        let mut current = Some(scope_id);

        while let Some(scope_id) = current {
            let scope = self.symbols.get_scope(scope_id)?;
            if const_named_scope_kind(scope.kind) {
                if let Some(owner) = scope.owner {
                    if let Some(symbol) = self.symbols.get(owner) {
                        parts.push(symbol.name.clone());
                    }
                }
            }
            current = scope.parent;
        }

        parts.reverse();
        (!parts.is_empty()).then(|| qualified_scope_name(&parts))
    }
}

fn const_named_scope_kind(kind: ScopeKind) -> bool {
    matches!(
        kind,
        ScopeKind::Program
            | ScopeKind::Function
            | ScopeKind::FunctionBlock
            | ScopeKind::Class
            | ScopeKind::Method
            | ScopeKind::Property
            | ScopeKind::Namespace
            | ScopeKind::Configuration
            | ScopeKind::Resource
    )
}

fn qualified_scope_name(parts: &[SmolStr]) -> SmolStr {
    let mut buf = String::new();
    for (idx, part) in parts.iter().enumerate() {
        if idx > 0 {
            buf.push('.');
        }
        buf.push_str(part.as_str());
    }
    SmolStr::new(buf)
}
