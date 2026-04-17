use super::const_utils::*;
use super::*;
use crate::types::POINTER_REFERENCE_HANDLE_SIZE_BYTES;

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
            SyntaxKind::SizeOfExpr => self.eval_sizeof_int_expr(node, scopes, guard),
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

    fn eval_sizeof_int_expr(
        &mut self,
        node: &SyntaxNode,
        scopes: &[Option<SmolStr>],
        guard: &mut FxHashSet<(Option<SmolStr>, SmolStr)>,
    ) -> Option<i64> {
        let type_id = if let Some(type_ref) = node
            .children()
            .find(|child| child.kind() == SyntaxKind::TypeRef)
        {
            self.resolve_type_from_ref(&type_ref)
        } else {
            let expr = node.children().find(|child| {
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
            })?;
            self.sizeof_operand_type_in_scope(&expr, scopes, guard)?
        };

        let size = self.sizeof_type_bytes(type_id)?;
        i64::try_from(size).ok()
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
                let type_id = self.resolve_type_path(std::slice::from_ref(&name));
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
