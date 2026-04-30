impl<'a, 'b> StmtChecker<'a, 'b> {

    fn check_case_branch(
        &mut self,
        node: &SyntaxNode,
        selector_type: TypeId,
        tracker: &mut CaseLabelTracker,
        initial_state: bool,
    ) -> bool {
        let saved = self.checker.return_value_definitely_assigned;
        self.checker.return_value_definitely_assigned = initial_state;

        // Check that case labels are compatible with selector type
        for child in node.children() {
            match child.kind() {
                SyntaxKind::CaseLabel => self.check_case_label(&child, selector_type, tracker),
                SyntaxKind::Subrange => self.check_case_subrange(&child, selector_type, tracker),
                _ if is_expression_kind(child.kind()) => {
                    self.check_case_label_expr(&child, selector_type, tracker);
                }
                _ if is_statement_kind(child.kind()) => self.check_statement(&child),
                _ => {}
            }
        }

        let exit_state = self.checker.return_value_definitely_assigned;
        self.checker.return_value_definitely_assigned = saved;
        exit_state
    }


    fn check_case_label(
        &mut self,
        node: &SyntaxNode,
        selector_type: TypeId,
        tracker: &mut CaseLabelTracker,
    ) {
        if let Some(subrange) = node.children().find(|n| n.kind() == SyntaxKind::Subrange) {
            self.check_case_subrange(&subrange, selector_type, tracker);
            return;
        }

        if let Some(expr) = node.children().find(|n| is_expression_kind(n.kind())) {
            self.check_case_label_expr(&expr, selector_type, tracker);
        }
    }


    fn check_case_subrange(
        &mut self,
        node: &SyntaxNode,
        selector_type: TypeId,
        tracker: &mut CaseLabelTracker,
    ) {
        let expr_children: Vec<_> = node
            .children()
            .filter(|n| is_expression_kind(n.kind()))
            .collect();
        let selector_type = self.checker.resolve_alias_type(selector_type);
        if expr_children.len() == 2
            && matches!(
                self.checker.symbols.type_by_id(selector_type),
                Some(Type::String { .. } | Type::WString { .. } | Type::AnyString)
            )
        {
            self.checker.diagnostics.error(
                DiagnosticCode::InvalidOperation,
                node.text_range(),
                "CASE subranges are not supported for STRING/WSTRING selectors",
            );
            return;
        }

        let mut bounds = Vec::new();
        let mut has_label = false;
        for child in expr_children {
            has_label = true;
            if !self.is_case_label_expr(&child) {
                self.checker.diagnostics.error(
                    DiagnosticCode::InvalidOperation,
                    child.text_range(),
                    "case label must be a literal, enum value, or constant",
                );
                continue;
            }
            let label_type = self.check_expression(&child);
            if !self.checker.is_assignable(selector_type, label_type)
                && !self
                    .checker
                    .is_contextual_int_literal(selector_type, &child)
                && !self
                    .checker
                    .is_contextual_real_literal(selector_type, &child)
            {
                self.checker.diagnostics.error(
                    DiagnosticCode::TypeMismatch,
                    child.text_range(),
                    "case label type must match selector type",
                );
            }
            if self.case_label_tracks_integer_value(selector_type) {
                if let Some(value) = self.checker.require_const_int_expr(
                    &child,
                    "case label must be a literal, enum value, or constant",
                ) {
                    bounds.push(value);
                }
            }
        }

        match bounds.len() {
            1 => self.record_case_label_value(tracker, bounds[0], node.text_range()),
            2 => self.record_case_label_range(tracker, bounds[0], bounds[1], node.text_range()),
            _ => {}
        }

        if !has_label && node.kind() == SyntaxKind::Subrange {
            self.checker.diagnostics.error(
                DiagnosticCode::TypeMismatch,
                node.text_range(),
                "case label type must match selector type",
            );
        }
    }


    fn check_case_label_expr(
        &mut self,
        expr: &SyntaxNode,
        selector_type: TypeId,
        tracker: &mut CaseLabelTracker,
    ) {
        if !self.is_case_label_expr(expr) {
            self.checker.diagnostics.error(
                DiagnosticCode::InvalidOperation,
                expr.text_range(),
                "case label must be a literal, enum value, or constant",
            );
            return;
        }

        let label_type = self.check_expression(expr);
        if !self.checker.is_assignable(selector_type, label_type)
            && !self.checker.is_contextual_int_literal(selector_type, expr)
            && !self.checker.is_contextual_real_literal(selector_type, expr)
        {
            self.checker.diagnostics.error(
                DiagnosticCode::TypeMismatch,
                expr.text_range(),
                "case label type must match selector type",
            );
        }

        if self.case_label_tracks_integer_value(selector_type) {
            if let Some(value) = self.checker.require_const_int_expr(
                expr,
                "case label must be a literal, enum value, or constant",
            ) {
                self.record_case_label_value(tracker, value, expr.text_range());
            }
        }
    }


    fn is_case_label_expr(&mut self, expr: &SyntaxNode) -> bool {
        match expr.kind() {
            SyntaxKind::Literal => true,
            SyntaxKind::ParenExpr => expr
                .children()
                .find(|child| is_expression_kind(child.kind()))
                .is_some_and(|child| self.is_case_label_expr(&child)),
            SyntaxKind::UnaryExpr => {
                let is_neg = expr
                    .descendants_with_tokens()
                    .filter_map(|e| e.into_token())
                    .any(|token| token.kind() == SyntaxKind::Minus);
                if !is_neg {
                    return false;
                }
                expr.children()
                    .find(|child| is_expression_kind(child.kind()))
                    .is_some_and(|child| self.is_case_label_expr(&child))
            }
            SyntaxKind::NameRef => {
                let Some(name) = self.checker.resolve_ref().get_name_from_ref(expr) else {
                    return false;
                };
                let Some(symbol_id) = self
                    .checker
                    .symbols
                    .resolve(&name, self.checker.current_scope)
                else {
                    return false;
                };
                let Some(symbol) = self.checker.symbols.get(symbol_id) else {
                    return false;
                };
                matches!(
                    symbol.kind,
                    SymbolKind::Constant | SymbolKind::EnumValue { .. }
                )
            }
            _ => false,
        }
    }


    fn is_case_selector_type(&self, type_id: TypeId) -> bool {
        let resolved = self.checker.resolve_alias_type(type_id);
        matches!(
            self.checker.symbols.type_by_id(resolved),
            Some(
                Type::Bool
                    | Type::SInt
                    | Type::Int
                    | Type::DInt
                    | Type::LInt
                    | Type::USInt
                    | Type::UInt
                    | Type::UDInt
                    | Type::ULInt
                    | Type::Real
                    | Type::LReal
                    | Type::Byte
                    | Type::Word
                    | Type::DWord
                    | Type::LWord
                    | Type::Time
                    | Type::LTime
                    | Type::Date
                    | Type::LDate
                    | Type::Tod
                    | Type::LTod
                    | Type::Dt
                    | Type::Ldt
                    | Type::String { .. }
                    | Type::WString { .. }
                    | Type::Char
                    | Type::WChar
                    | Type::Enum { .. }
                    | Type::Subrange { .. }
                    | Type::Any
                    | Type::AnyInt
                    | Type::AnyReal
                    | Type::AnyNum
                    | Type::AnyBit
                    | Type::AnyString
                    | Type::AnyDate
            )
        )
    }

    fn case_label_tracks_integer_value(&self, type_id: TypeId) -> bool {
        let resolved = self.checker.resolve_alias_type(type_id);
        matches!(
            self.checker.symbols.type_by_id(resolved),
            Some(
                Type::Bool
                    | Type::SInt
                    | Type::Int
                    | Type::DInt
                    | Type::LInt
                    | Type::USInt
                    | Type::UInt
                    | Type::UDInt
                    | Type::ULInt
                    | Type::Byte
                    | Type::Word
                    | Type::DWord
                    | Type::LWord
                    | Type::Enum { .. }
                    | Type::Subrange { .. }
                    | Type::AnyInt
                    | Type::AnyBit
            )
        )
    }


    fn case_labels_cover_enum(&self, selector_type: TypeId, tracker: &CaseLabelTracker) -> bool {
        let resolved = self.checker.resolve_alias_type(selector_type);
        let Some(Type::Enum { values, .. }) = self.checker.symbols.type_by_id(resolved) else {
            return false;
        };
        if values.is_empty() {
            return false;
        }
        values.iter().all(|(_, value)| tracker.covers(*value))
    }

}
