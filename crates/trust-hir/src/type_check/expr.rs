use super::helpers::direct_address_type;
use super::literals::{
    int_literal_info, is_long_date_literal, is_long_dt_literal, is_long_time_literal,
    is_long_tod_literal, smallest_int_type_for_literal,
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
            this_type: None,
            super_type: None,
            loop_stack: Vec::new(),
            label_scopes: Vec::new(),
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
        match node.kind() {
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
            _ => TypeId::UNKNOWN,
        }
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
            if let Some(type_id) = self.checker.symbols.lookup_type(type_name) {
                return type_id;
            }
            return TypeId::UNKNOWN;
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
        TypeId::UNKNOWN
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
            None => return TypeId::UNKNOWN,
        };

        match self
            .checker
            .resolve()
            .resolve_name_in_context(&name, node.text_range())
        {
            Some(resolved) => {
                let Some(symbol) = self.checker.symbols.get(resolved.id) else {
                    return TypeId::UNKNOWN;
                };
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
            None => {
                self.checker.diagnostics.error(
                    DiagnosticCode::UndefinedVariable,
                    node.text_range(),
                    format!("undefined identifier '{}'", name),
                );
                TypeId::UNKNOWN
            }
        }
    }

    fn infer_binary_expr(&mut self, node: &SyntaxNode) -> TypeId {
        let children: Vec<_> = node.children().collect();
        if children.len() < 2 {
            return TypeId::UNKNOWN;
        }

        let lhs_node = &children[0];
        let rhs_node = &children[children.len() - 1];
        let lhs_type = self.check_expression(lhs_node);
        let rhs_type = self.check_expression(rhs_node);

        let op = BinaryOp::from_node(node);

        if op.is_comparison() {
            self.check_comparable(lhs_type, rhs_type, node.text_range());
            // Warn about floating-point equality comparison (common pitfall)
            if matches!(op, BinaryOp::Eq | BinaryOp::Neq) {
                let lhs_r = self.checker.resolve_alias_type(lhs_type);
                let rhs_r = self.checker.resolve_alias_type(rhs_type);
                if lhs_r == TypeId::REAL || lhs_r == TypeId::LREAL
                    || rhs_r == TypeId::REAL || rhs_r == TypeId::LREAL
                {
                    self.checker.diagnostics.warning(
                        DiagnosticCode::NondeterministicIo,
                        node.text_range(),
                        "floating-point equality comparison may produce unexpected results due to rounding; consider using a tolerance check (e.g., ABS(a - b) < 0.001)",
                    );
                }
            }
            TypeId::BOOL
        } else if op.is_logical() {
            // IEC 61131-3: AND/OR/XOR work on BOOL and bit-string types
            // (BYTE, WORD, DWORD, LWORD). For bit-strings, the result type
            // is the operand type, not BOOL.
            let lhs_resolved = self.checker.resolve_alias_type(lhs_type);
            let rhs_resolved = self.checker.resolve_alias_type(rhs_type);
            let lhs_is_bit = self.checker.resolved_type(lhs_resolved).is_some_and(|ty| ty.is_bit_string());
            let rhs_is_bit = self.checker.resolved_type(rhs_resolved).is_some_and(|ty| ty.is_bit_string());
            if lhs_is_bit && rhs_is_bit {
                // Bit-string logical: result is the wider type (or same if equal)
                lhs_resolved
            } else if lhs_is_bit || rhs_is_bit {
                // Mixed bit-string and non-bit-string: type error
                self.checker.diagnostics.error(
                    DiagnosticCode::TypeMismatch,
                    node.text_range(),
                    "mismatched types for logical operation",
                );
                TypeId::UNKNOWN
            } else {
                self.check_boolean(lhs_type, node.text_range());
                self.check_boolean(rhs_type, node.text_range());
                TypeId::BOOL
            }
        } else if op.is_arithmetic() {
            // Warn about division/modulo by zero literal
            if matches!(op, BinaryOp::Div | BinaryOp::Mod) {
                if is_zero_literal(rhs_node) {
                    self.checker.diagnostics.warning(
                        DiagnosticCode::OutOfRange,
                        node.text_range(),
                        "division by zero",
                    );
                }
            }
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
            TypeId::UNKNOWN
        }
    }

    fn infer_unary_expr(&mut self, node: &SyntaxNode) -> TypeId {
        let operand = match node.children().next() {
            Some(child) => self.check_expression(&child),
            None => return TypeId::UNKNOWN,
        };

        let op = UnaryOp::from_node(node);

        match op {
            UnaryOp::Neg => {
                if let Some(ty) = self.checker.resolved_type(operand) {
                    if ty.is_numeric() {
                        return operand;
                    }
                }
                self.checker.diagnostics.error(
                    DiagnosticCode::TypeMismatch,
                    node.text_range(),
                    "negation requires numeric type",
                );
                TypeId::UNKNOWN
            }
            UnaryOp::Not => {
                let resolved = self.checker.resolve_alias_type(operand);
                let is_bit = self.checker.resolved_type(resolved).is_some_and(|ty| ty.is_bit_string());
                if is_bit {
                    resolved // NOT on BYTE/WORD/DWORD → same type
                } else {
                    self.check_boolean(operand, node.text_range());
                    TypeId::BOOL
                }
            }
            UnaryOp::Unknown => TypeId::UNKNOWN,
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

    pub(super) fn common_numeric_type(
        &mut self,
        lhs: TypeId,
        rhs: TypeId,
        range: TextRange,
    ) -> TypeId {
        let lhs = self.checker.resolve_alias_type(lhs);
        let rhs = self.checker.resolve_alias_type(rhs);
        let lhs_ty = self.checker.symbols.type_by_id(lhs);
        let rhs_ty = self.checker.symbols.type_by_id(rhs);

        match (lhs_ty, rhs_ty) {
            (Some(l), Some(r)) if l.is_numeric() && r.is_numeric() => {
                // Return the wider type
                self.checker.wider_numeric(lhs, rhs)
            }
            (None, _) | (_, None) => {
                // Unknown types - return UNKNOWN
                TypeId::UNKNOWN
            }
            _ => {
                self.checker.diagnostics.error(
                    DiagnosticCode::TypeMismatch,
                    range,
                    "operands must be numeric types",
                );
                TypeId::UNKNOWN
            }
        }
    }
}

/// Check if a syntax node is a literal integer zero (0).
fn is_zero_literal(node: &SyntaxNode) -> bool {
    if node.kind() != SyntaxKind::Literal {
        return false;
    }
    for element in node.descendants_with_tokens() {
        let token = match element.into_token() {
            Some(token) => token,
            None => continue,
        };
        match token.kind() {
            SyntaxKind::IntLiteral => return token.text() == "0",
            SyntaxKind::RealLiteral => {
                let text = token.text();
                return text == "0.0" || text == "0.";
            }
            _ => {}
        }
    }
    false
}
