use super::const_utils::*;
use super::*;
use crate::symbols::EnumValueResolution;
use crate::types::POINTER_REFERENCE_HANDLE_SIZE_BYTES;

impl SymbolCollector<'_> {
    pub(super) fn evaluate_constants(&mut self) {
        let keys: Vec<_> = self.const_exprs.keys().cloned().collect();
        let mut guard = FxHashSet::default();
        for (scope, name) in keys {
            if let Err(err) =
                self.try_resolve_const_value_for_scope(name.as_str(), &scope, &mut guard)
            {
                if let Some(expr) = self
                    .const_exprs
                    .get(&const_key(&scope, name.as_str()))
                    .cloned()
                {
                    self.report_const_eval_error(err, expr.text_range());
                }
            }
        }
    }

    pub(super) fn eval_int_expr_in_scope(
        &mut self,
        node: &SyntaxNode,
        scopes: &[Option<SmolStr>],
    ) -> Option<i64> {
        match self.try_eval_optional_int_expr_in_scope(node, scopes) {
            Ok(value) => value,
            Err(err) => {
                self.report_const_eval_error(err, node.text_range());
                None
            }
        }
    }

    pub(super) fn try_eval_optional_int_expr_in_scope(
        &mut self,
        node: &SyntaxNode,
        scopes: &[Option<SmolStr>],
    ) -> Result<Option<i64>, ConstEvalError> {
        let mut guard = FxHashSet::default();
        match self.try_eval_int_expr(node, scopes, &mut guard) {
            Ok(value) => Ok(Some(value)),
            Err(ConstEvalError::NotConstant) => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub(super) fn try_eval_int_expr(
        &mut self,
        node: &SyntaxNode,
        scopes: &[Option<SmolStr>],
        guard: &mut FxHashSet<(Option<SmolStr>, SmolStr)>,
    ) -> Result<i64, ConstEvalError> {
        match node.kind() {
            SyntaxKind::Literal => {
                parse_int_literal_from_node(node).ok_or(ConstEvalError::NotConstant)
            }
            SyntaxKind::SizeOfExpr => self.try_eval_sizeof_int_expr(node, scopes, guard),
            SyntaxKind::NameRef => {
                let name = first_ident_token(node)
                    .map(|token| token.text().to_string())
                    .ok_or(ConstEvalError::NotConstant)?;
                match self.try_resolve_const_value(&name, scopes, guard) {
                    Ok(value) => Ok(value),
                    Err(ConstEvalError::UndefinedName(_)) => {
                        match self.table.resolve_enum_value_by_name(&name) {
                            EnumValueResolution::Resolved(value) => Ok(value),
                            EnumValueResolution::NotFound => {
                                Err(ConstEvalError::UndefinedName(SmolStr::new(name.as_str())))
                            }
                            EnumValueResolution::Ambiguous => {
                                Err(ConstEvalError::AmbiguousName(SmolStr::new(name.as_str())))
                            }
                        }
                    }
                    Err(err) => Err(err),
                }
            }
            SyntaxKind::ParenExpr => node
                .children()
                .next()
                .ok_or(ConstEvalError::NotConstant)
                .and_then(|child| self.try_eval_int_expr(&child, scopes, guard)),
            SyntaxKind::UnaryExpr => {
                let op = unary_op_from_node(node).ok_or(ConstEvalError::NotConstant)?;
                let expr = node.children().next().ok_or(ConstEvalError::NotConstant)?;
                let value = self.try_eval_int_expr(&expr, scopes, guard)?;
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
                let lhs = self.try_eval_int_expr(&children[0], scopes, guard)?;
                let rhs = self.try_eval_int_expr(&children[children.len() - 1], scopes, guard)?;
                match binary_op_from_node(node).ok_or(ConstEvalError::NotConstant)? {
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

    fn try_eval_sizeof_int_expr(
        &mut self,
        node: &SyntaxNode,
        scopes: &[Option<SmolStr>],
        guard: &mut FxHashSet<(Option<SmolStr>, SmolStr)>,
    ) -> Result<i64, ConstEvalError> {
        let type_id = if let Some(type_ref) = node
            .children()
            .find(|child| child.kind() == SyntaxKind::TypeRef)
        {
            self.resolve_type_from_ref(&type_ref)
        } else {
            let expr = node
                .children()
                .find(|child| {
                    matches!(
                        child.kind(),
                        SyntaxKind::Literal
                            | SyntaxKind::NameRef
                            | SyntaxKind::BinaryExpr
                            | SyntaxKind::UnaryExpr
                            | SyntaxKind::CallExpr
                            | SyntaxKind::IndexExpr
                            | SyntaxKind::FieldExpr
                            | SyntaxKind::DerefExpr
                            | SyntaxKind::AddrExpr
                            | SyntaxKind::ParenExpr
                            | SyntaxKind::ThisExpr
                            | SyntaxKind::SuperExpr
                            | SyntaxKind::SizeOfExpr
                    )
                })
                .ok_or(ConstEvalError::NotConstant)?;
            self.sizeof_operand_type_in_scope(&expr, scopes, guard)
                .ok_or(ConstEvalError::NotConstant)?
        };

        let size = self
            .sizeof_type_bytes(type_id)
            .ok_or(ConstEvalError::NotConstant)?;
        i64::try_from(size).map_err(|_| ConstEvalError::IntegerOverflow)
    }

    fn sizeof_operand_type_in_scope(
        &mut self,
        node: &SyntaxNode,
        scopes: &[Option<SmolStr>],
        guard: &mut FxHashSet<(Option<SmolStr>, SmolStr)>,
    ) -> Option<TypeId> {
        match node.kind() {
            SyntaxKind::ParenExpr => node
                .children()
                .next()
                .and_then(|child| self.sizeof_operand_type_in_scope(&child, scopes, guard)),
            SyntaxKind::NameRef => {
                let name = first_ident_token(node)?.text().to_string();
                if let Some(symbol_id) = self.table.resolve(&name, self.table.current_scope()) {
                    let symbol = self.table.get(symbol_id)?;
                    if !symbol.is_type() {
                        return Some(symbol.type_id);
                    }
                }
                let name = SmolStr::new(name);
                let type_id =
                    self.resolve_type_path_at(std::slice::from_ref(&name), Some(node.text_range()));
                (type_id != TypeId::UNKNOWN).then_some(type_id)
            }
            _ => {
                let _ = (scopes, guard);
                None
            }
        }
    }

    fn sizeof_type_bytes(&self, type_id: TypeId) -> Option<u64> {
        let mut stack = Vec::new();
        self.sizeof_type_bytes_inner(type_id, &mut stack)
    }

    fn sizeof_type_bytes_inner(&self, type_id: TypeId, stack: &mut Vec<TypeId>) -> Option<u64> {
        if stack.contains(&type_id) {
            return None;
        }
        stack.push(type_id);

        let ty = self.table.type_by_id(type_id)?;
        let result = match ty {
            Type::Alias { target, .. } => self.sizeof_type_bytes_inner(*target, stack),
            Type::Subrange { base, .. } => self.sizeof_type_bytes_inner(*base, stack),
            Type::Enum { base, .. } => self.sizeof_type_bytes_inner(*base, stack),
            Type::Array {
                element,
                dimensions,
            } => {
                if dimensions
                    .iter()
                    .any(|(lower, upper)| *lower == 0 && *upper == i64::MAX)
                {
                    None
                } else {
                    let element_size = self.sizeof_type_bytes_inner(*element, stack)?;
                    let len = dimensions.iter().try_fold(1u64, |total, (lower, upper)| {
                        let len = upper.checked_sub(*lower)?.checked_add(1)?;
                        let len = u64::try_from(len).ok()?;
                        total.checked_mul(len)
                    })?;
                    element_size.checked_mul(len)
                }
            }
            Type::Struct { fields, .. } => {
                let mut total = 0u64;
                for field in fields {
                    total =
                        total.checked_add(self.sizeof_type_bytes_inner(field.type_id, stack)?)?;
                }
                Some(total)
            }
            Type::Union { variants, .. } => {
                let mut max = 0u64;
                for variant in variants {
                    max = max.max(self.sizeof_type_bytes_inner(variant.type_id, stack)?);
                }
                Some(max)
            }
            Type::String {
                max_len: Some(max_len),
            } => Some(u64::from(*max_len)),
            Type::WString {
                max_len: Some(max_len),
            } => u64::from(*max_len).checked_mul(2),
            Type::Pointer { .. } | Type::Reference { .. } => {
                Some(POINTER_REFERENCE_HANDLE_SIZE_BYTES)
            }
            Type::FunctionBlock { .. }
            | Type::Class { .. }
            | Type::Interface { .. }
            | Type::String { max_len: None }
            | Type::WString { max_len: None } => None,
            _ => ty.bit_size().map(|bits| u64::from(bits.div_ceil(8))),
        };

        let _ = stack.pop();
        result
    }

    pub(super) fn try_resolve_const_value(
        &mut self,
        name: &str,
        scopes: &[Option<SmolStr>],
        guard: &mut FxHashSet<(Option<SmolStr>, SmolStr)>,
    ) -> Result<i64, ConstEvalError> {
        let mut last_err = ConstEvalError::UndefinedName(SmolStr::new(name));
        for scope in scopes {
            match self.try_resolve_const_value_for_scope(name, scope, guard) {
                Ok(value) => return Ok(value),
                Err(ConstEvalError::UndefinedName(_)) => {
                    last_err = ConstEvalError::UndefinedName(SmolStr::new(name));
                }
                Err(err) => return Err(err),
            }
        }
        Err(last_err)
    }

    pub(super) fn try_resolve_const_value_for_scope(
        &mut self,
        name: &str,
        scope: &Option<SmolStr>,
        guard: &mut FxHashSet<(Option<SmolStr>, SmolStr)>,
    ) -> Result<i64, ConstEvalError> {
        let key = const_key(scope, name);
        if let Some(value) = self.const_values.get(&key) {
            return Ok(*value);
        }
        let expr = self
            .const_exprs
            .get(&key)
            .cloned()
            .ok_or_else(|| ConstEvalError::UndefinedName(SmolStr::new(name)))?;
        if !guard.insert(key.clone()) {
            return Err(ConstEvalError::CyclicDependency(key.1));
        }
        let scopes = scope_chain_for_node(&expr);
        let value = self.try_eval_int_expr(&expr, &scopes, guard);
        guard.remove(&key);
        let value = value?;
        self.const_values.insert(key, value);
        Ok(value)
    }

    pub(super) fn report_const_eval_error(
        &mut self,
        err: ConstEvalError,
        range: text_size::TextRange,
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
            ConstEvalError::NotConstant => {}
        }
    }
}
