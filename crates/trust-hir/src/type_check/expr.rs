use super::calls::NameResolveOutcome;
use super::helpers::direct_address_type;
use super::literals::{
    int_literal_info, is_long_date_literal, is_long_dt_literal, is_long_time_literal,
    is_long_tod_literal, is_zero_numeric_literal_expr, smallest_int_type_for_literal,
};
use super::*;

impl<'a> TypeChecker<'a> {
    /// Creates a new type checker.
    pub fn new(
        symbols: &'a mut SymbolTable,
        diagnostics: &'a mut DiagnosticBuilder,
        current_scope: ScopeId,
    ) -> Self {
        Self {
            symbols,
            diagnostics,
            current_scope,
            current_function_return: None,
            current_pou_symbol: None,
            saw_return_value: false,
            return_value_definitely_assigned: false,
            this_type: None,
            super_type: None,
            loop_stack: Vec::new(),
            label_scopes: Vec::new(),
            expression_types: FxHashMap::default(),
        }
    }

    /// Sets the expected return type for return statement checking.
    pub fn set_return_type(&mut self, return_type: Option<TypeId>) {
        self.current_function_return = return_type;
    }

    /// Sets the current POU symbol for return-value tracking.
    pub fn set_current_pou(&mut self, symbol_id: Option<SymbolId>) {
        self.current_pou_symbol = symbol_id;
    }

    /// Sets the current scope for name resolution.
    pub fn set_scope(&mut self, scope: ScopeId) {
        self.current_scope = scope;
    }

    /// Sets the receiver types for THIS/SUPER expressions.
    pub fn set_receiver_types(&mut self, this_type: Option<TypeId>, super_type: Option<TypeId>) {
        self.this_type = this_type;
        self.super_type = super_type;
    }
}

impl<'a, 'b> ExprChecker<'a, 'b> {
    /// Infers the type of an expression.
    pub(crate) fn check_expression(&mut self, node: &SyntaxNode) -> TypeId {
        let type_id = match node.kind() {
            SyntaxKind::Literal => self.infer_literal(node),
            SyntaxKind::NameRef => self.infer_name_ref(node),
            SyntaxKind::BinaryExpr => self.infer_binary_expr(node),
            SyntaxKind::UnaryExpr => self.infer_unary_expr(node),
            SyntaxKind::CallExpr => self.checker.calls().infer_call_expr(node),
            SyntaxKind::IndexExpr => self.checker.calls().infer_index_expr(node),
            SyntaxKind::FieldExpr => self.checker.calls().infer_field_expr(node),
            SyntaxKind::DerefExpr => self.checker.calls().infer_deref_expr(node),
            SyntaxKind::AddrExpr => self.checker.calls().infer_addr_expr(node),
            SyntaxKind::ParenExpr => self.checker.infer_paren_expr(node),
            SyntaxKind::ThisExpr => self.checker.infer_this_expr(node),
            SyntaxKind::SuperExpr => self.checker.infer_super_expr(node),
            SyntaxKind::SizeOfExpr => self.checker.infer_size_of_expr(node),
            _ => self
                .checker
                .legacy_type_from_outcome(self.checker.unknown_type_outcome(node.text_range())),
        };
        self.checker.record_expression_type(node, type_id)
    }

    fn infer_literal(&mut self, node: &SyntaxNode) -> TypeId {
        // Check for typed literal prefix (e.g., DINT#123)
        for token in node
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
        {
            if token.kind() != SyntaxKind::TypedLiteralPrefix {
                continue;
            }

            let type_name = token.text().strip_suffix('#').unwrap_or(token.text());
            if let Some(type_id) = TypeId::from_builtin_name(type_name) {
                return type_id;
            }
            if let Some(type_id) = self.checker.symbols.lookup_registered_type_name(type_name) {
                return type_id;
            }
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::UndefinedType,
                node.text_range(),
                format!("undefined typed literal prefix '{}'", type_name),
            );
        }

        // Infer from literal token type
        for token in node
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
        {
            match token.kind() {
                SyntaxKind::IntLiteral => {
                    if let Some(info) = int_literal_info(node) {
                        return smallest_int_type_for_literal(info.value, info.is_based);
                    }
                    return TypeId::DINT;
                }
                SyntaxKind::RealLiteral => return TypeId::LREAL,
                SyntaxKind::StringLiteral => return TypeId::STRING,
                SyntaxKind::WideStringLiteral => return TypeId::WSTRING,
                SyntaxKind::KwTrue | SyntaxKind::KwFalse => return TypeId::BOOL,
                SyntaxKind::KwNull => return TypeId::NULL,
                SyntaxKind::TimeLiteral => {
                    return if is_long_time_literal(token.text()) {
                        TypeId::LTIME
                    } else {
                        TypeId::TIME
                    };
                }
                SyntaxKind::DateLiteral => {
                    return if is_long_date_literal(token.text()) {
                        TypeId::LDATE
                    } else {
                        TypeId::DATE
                    };
                }
                SyntaxKind::TimeOfDayLiteral => {
                    return if is_long_tod_literal(token.text()) {
                        TypeId::LTOD
                    } else {
                        TypeId::TOD
                    };
                }
                SyntaxKind::DateAndTimeLiteral => {
                    return if is_long_dt_literal(token.text()) {
                        TypeId::LDT
                    } else {
                        TypeId::DT
                    };
                }
                _ => continue,
            }
        }
        self.checker
            .legacy_type_from_outcome(self.checker.unknown_type_outcome(node.text_range()))
    }

    fn infer_name_ref(&mut self, node: &SyntaxNode) -> TypeId {
        if let Some(token) = node
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|token| token.kind() == SyntaxKind::DirectAddress)
        {
            return direct_address_type(token.text());
        }

        let name = match self.checker.resolve_ref().get_name_from_ref(node) {
            Some(n) => n,
            None => {
                return self.checker.legacy_type_from_outcome(
                    self.checker.unknown_type_outcome(node.text_range()),
                );
            }
        };

        match self
            .checker
            .resolve()
            .resolve_name_in_context_outcome(&name, node.text_range())
        {
            NameResolveOutcome::Resolved(resolved) => {
                let Some(symbol) = self.checker.symbols.get(resolved.id) else {
                    return self.checker.legacy_type_from_outcome(
                        self.checker.unknown_type_outcome(node.text_range()),
                    );
                };
                if self.checker.is_return_target(node) {
                    if let Some(return_type) = self.checker.current_function_return {
                        return return_type;
                    }
                }
                if let Some(role) = non_value_role(&symbol.kind) {
                    return self.checker.legacy_diagnostic_type(
                        DiagnosticCode::InvalidOperation,
                        node.text_range(),
                        format!("{role} '{}' cannot be used as a value", symbol.name),
                    );
                }
                if resolved.accessible {
                    if let SymbolKind::Property { has_get, .. } = symbol.kind {
                        if !has_get {
                            self.checker.diagnostics.error(
                                DiagnosticCode::InvalidOperation,
                                node.text_range(),
                                format!("property '{}' has no getter", symbol.name),
                            );
                        }
                    }
                }
                symbol.type_id
            }
            NameResolveOutcome::Ambiguous => self
                .checker
                .legacy_suppressed_type(DiagnosticCode::CannotResolve, node.text_range()),
            NameResolveOutcome::NotFound => self.checker.legacy_diagnostic_type(
                DiagnosticCode::UndefinedVariable,
                node.text_range(),
                format!("undefined identifier '{}'", name),
            ),
        }
    }

    fn infer_binary_expr(&mut self, node: &SyntaxNode) -> TypeId {
        let children: Vec<_> = node.children().collect();
        if children.len() < 2 {
            return self
                .checker
                .legacy_type_from_outcome(self.checker.unknown_type_outcome(node.text_range()));
        }

        let lhs_node = &children[0];
        let rhs_node = &children[children.len() - 1];
        let lhs_type = self.check_expression(lhs_node);
        let rhs_type = self.check_expression(rhs_node);

        let op = BinaryOp::from_node(node);

        if op.is_comparison() {
            self.warn_float_equality(lhs_type, rhs_type, op, node.text_range());
            self.check_comparable(lhs_type, rhs_type, node.text_range());
            TypeId::BOOL
        } else if op.is_logical() {
            self.common_bit_string_type(
                lhs_type,
                lhs_node.text_range(),
                rhs_type,
                rhs_node.text_range(),
                node.text_range(),
            )
        } else if op.is_arithmetic() {
            self.warn_literal_zero_divisor(op, rhs_node);
            if let (Some(lhs_ty), Some(rhs_ty)) = (
                self.checker
                    .symbols
                    .type_by_id(self.checker.resolve_alias_type(lhs_type)),
                self.checker
                    .symbols
                    .type_by_id(self.checker.resolve_alias_type(rhs_type)),
            ) {
                if lhs_ty.is_float() && super::literals::is_untyped_real_literal_expr(rhs_node) {
                    return lhs_type;
                }
                if rhs_ty.is_float() && super::literals::is_untyped_real_literal_expr(lhs_node) {
                    return rhs_type;
                }
            }
            self.common_numeric_type(lhs_type, rhs_type, node.text_range())
        } else {
            self.checker
                .legacy_type_from_outcome(self.checker.unknown_type_outcome(node.text_range()))
        }
    }

    fn infer_unary_expr(&mut self, node: &SyntaxNode) -> TypeId {
        let operand = match node.children().next() {
            Some(child) => self.check_expression(&child),
            None => {
                return self.checker.legacy_type_from_outcome(
                    self.checker.unknown_type_outcome(node.text_range()),
                );
            }
        };

        let op = UnaryOp::from_node(node);

        match op {
            UnaryOp::Neg => {
                if operand == TypeId::UNKNOWN {
                    return self
                        .checker
                        .legacy_suppressed_type(DiagnosticCode::CannotResolve, node.text_range());
                }
                if let Some(ty) = self.checker.resolved_type(operand) {
                    if ty.is_numeric() {
                        return operand;
                    }
                }
                self.checker.legacy_diagnostic_type(
                    DiagnosticCode::TypeMismatch,
                    node.text_range(),
                    "negation requires numeric type",
                )
            }
            UnaryOp::Not => self.unary_bit_string_type(operand, node.text_range()),
            UnaryOp::Unknown => self
                .checker
                .legacy_type_from_outcome(self.checker.unknown_type_outcome(node.text_range())),
        }
    }

    pub(super) fn check_boolean(&mut self, type_id: TypeId, range: TextRange) {
        let type_id = self.checker.resolve_alias_type(type_id);
        if type_id != TypeId::BOOL && type_id != TypeId::UNKNOWN {
            self.checker.diagnostics.error(
                DiagnosticCode::TypeMismatch,
                range,
                "expected BOOL type",
            );
        }
    }

    pub(super) fn check_comparable(&mut self, lhs: TypeId, rhs: TypeId, range: TextRange) {
        let lhs = self.checker.resolve_subrange_base(lhs);
        let rhs = self.checker.resolve_subrange_base(rhs);
        if (lhs == TypeId::NULL && self.checker.is_reference_like_type(rhs))
            || (rhs == TypeId::NULL && self.checker.is_reference_like_type(lhs))
        {
            return;
        }
        // Most types are comparable to themselves
        if lhs == rhs {
            return;
        }

        // Numeric types are comparable to each other
        if let (Some(l), Some(r)) = (
            self.checker.symbols.type_by_id(lhs),
            self.checker.symbols.type_by_id(rhs),
        ) {
            if l.is_numeric() && r.is_numeric() {
                return;
            }
            if matches!((l, r), (Type::String { .. }, Type::String { .. }))
                || matches!((l, r), (Type::WString { .. }, Type::WString { .. }))
            {
                return;
            }
        }

        // Unknown types are allowed (might be resolved later)
        if lhs == TypeId::UNKNOWN || rhs == TypeId::UNKNOWN {
            return;
        }

        self.checker.diagnostics.error(
            DiagnosticCode::TypeMismatch,
            range,
            "types are not comparable",
        );
    }

    pub(super) fn common_bit_string_type(
        &mut self,
        lhs: TypeId,
        lhs_range: TextRange,
        rhs: TypeId,
        rhs_range: TextRange,
        range: TextRange,
    ) -> TypeId {
        let lhs = self.checker.resolve_subrange_base(lhs);
        let rhs = self.checker.resolve_subrange_base(rhs);
        if lhs == TypeId::UNKNOWN || rhs == TypeId::UNKNOWN {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::CannotResolve, range);
        }
        let lhs_ty = self.checker.resolved_type(lhs);
        let rhs_ty = self.checker.resolved_type(rhs);

        match (lhs_ty, rhs_ty) {
            (Some(l), Some(r)) if l.is_bit_string() && r.is_bit_string() => {
                let lhs_size = l.bit_size().unwrap_or(0);
                let rhs_size = r.bit_size().unwrap_or(0);
                let common = if lhs_size >= rhs_size { lhs } else { rhs };

                if lhs != common {
                    self.checker
                        .warn_implicit_conversion(common, lhs, lhs_range);
                }
                if rhs != common {
                    self.checker
                        .warn_implicit_conversion(common, rhs, rhs_range);
                }

                common
            }
            (None, _) | (_, None) => self
                .checker
                .legacy_type_from_outcome(self.checker.unknown_type_outcome(range)),
            _ => self.checker.legacy_diagnostic_type(
                DiagnosticCode::TypeMismatch,
                range,
                "operands must be BOOL or bit-string types",
            ),
        }
    }

    pub(super) fn unary_bit_string_type(&mut self, operand: TypeId, range: TextRange) -> TypeId {
        let operand = self.checker.resolve_subrange_base(operand);
        if operand == TypeId::UNKNOWN {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::CannotResolve, range);
        }
        match self.checker.resolved_type(operand) {
            Some(ty) if ty.is_bit_string() => operand,
            None => self
                .checker
                .legacy_type_from_outcome(self.checker.unknown_type_outcome(range)),
            _ => self.checker.legacy_diagnostic_type(
                DiagnosticCode::TypeMismatch,
                range,
                "NOT requires BOOL or bit-string type",
            ),
        }
    }

    fn warn_float_equality(&mut self, lhs: TypeId, rhs: TypeId, op: BinaryOp, range: TextRange) {
        if !matches!(op, BinaryOp::Eq | BinaryOp::Neq) {
            return;
        }

        let lhs = self.checker.resolve_subrange_base(lhs);
        let rhs = self.checker.resolve_subrange_base(rhs);
        let lhs_is_float = self.checker.resolved_type(lhs).is_some_and(Type::is_float);
        let rhs_is_float = self.checker.resolved_type(rhs).is_some_and(Type::is_float);

        if lhs_is_float || rhs_is_float {
            self.checker.diagnostics.warning(
                DiagnosticCode::FloatingPointEquality,
                range,
                "floating-point equality comparison may produce unexpected results",
            );
        }
    }

    fn warn_literal_zero_divisor(&mut self, op: BinaryOp, rhs_node: &SyntaxNode) {
        if !matches!(op, BinaryOp::Div | BinaryOp::Mod) || !is_zero_numeric_literal_expr(rhs_node) {
            return;
        }

        let message = if matches!(op, BinaryOp::Div) {
            "division by literal zero will fault at runtime"
        } else {
            "MOD by literal zero will fault at runtime"
        };
        self.checker.diagnostics.warning(
            DiagnosticCode::LiteralDivisionByZero,
            rhs_node.text_range(),
            message,
        );
    }

    pub(super) fn common_numeric_type(
        &mut self,
        lhs: TypeId,
        rhs: TypeId,
        range: TextRange,
    ) -> TypeId {
        let lhs = self.checker.resolve_alias_type(lhs);
        let rhs = self.checker.resolve_alias_type(rhs);
        if lhs == TypeId::UNKNOWN || rhs == TypeId::UNKNOWN {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::CannotResolve, range);
        }
        let lhs_ty = self.checker.symbols.type_by_id(lhs);
        let rhs_ty = self.checker.symbols.type_by_id(rhs);

        match (lhs_ty, rhs_ty) {
            (Some(l), Some(r)) if l.is_numeric() && r.is_numeric() => {
                // Return the wider type
                self.checker.wider_numeric(lhs, rhs)
            }
            (None, _) | (_, None) => self
                .checker
                .legacy_type_from_outcome(self.checker.unknown_type_outcome(range)),
            _ => self.checker.legacy_diagnostic_type(
                DiagnosticCode::TypeMismatch,
                range,
                "operands must be numeric types",
            ),
        }
    }
}
