impl<'a, 'b> StmtChecker<'a, 'b> {

    fn check_return_stmt(&mut self, node: &SyntaxNode) {
        let return_expr = node.children().find(|n| is_expression_kind(n.kind()));
        if return_expr.is_some() {
            self.checker.saw_return_value = true;
            self.checker.return_value_definitely_assigned = true;
        }

        match (self.checker.current_function_return, return_expr) {
            (Some(expected), Some(expr)) if expected != TypeId::VOID => {
                let actual = self.check_expression(&expr);
                if !self.checker.is_assignable(expected, actual)
                    && !self.checker.is_contextual_int_literal(expected, &expr)
                    && !self.checker.is_contextual_real_literal(expected, &expr)
                {
                    self.checker.diagnostics.error(
                        DiagnosticCode::InvalidReturnType,
                        expr.text_range(),
                        format!(
                            "return type mismatch: expected '{}', found '{}'",
                            self.checker.type_name(expected),
                            self.checker.type_name(actual)
                        ),
                    );
                }
            }
            (Some(expected), None)
                if expected != TypeId::VOID
                    && !self.checker.return_value_definitely_assigned =>
            {
                self.checker.diagnostics.error(
                    DiagnosticCode::MissingReturn,
                    node.text_range(),
                    "missing return value",
                );
            }
            (None, Some(expr)) | (Some(TypeId::VOID), Some(expr)) => {
                self.checker.diagnostics.error(
                    DiagnosticCode::InvalidReturnType,
                    expr.text_range(),
                    "unexpected return value in procedure",
                );
            }
            _ => {}
        }
    }


    /// Emits missing return diagnostics after statement checks.
    pub fn finish_return_checks(&mut self, node: &SyntaxNode) {
        if let Some(expected) = self.checker.current_function_return {
            if expected != TypeId::VOID && !self.checker.saw_return_value {
                self.checker.diagnostics.error(
                    DiagnosticCode::MissingReturn,
                    node.text_range(),
                    "missing return value",
                );
            }
        }
    }


    fn check_expr_stmt(&mut self, node: &SyntaxNode) {
        // Just type-check the expression to catch any errors
        if let Some(expr) = node.children().next() {
            self.check_expression(&expr);
        }
    }


    fn check_statement_children(&mut self, node: &SyntaxNode) {
        for child in node.children() {
            if is_statement_kind(child.kind()) {
                self.check_statement(&child);
            }
        }
    }


    fn check_statement_children_with_state(&mut self, node: &SyntaxNode, initial_state: bool) -> bool {
        let saved = self.checker.return_value_definitely_assigned;
        self.checker.return_value_definitely_assigned = initial_state;
        self.check_statement_children(node);
        let exit_state = self.checker.return_value_definitely_assigned;
        self.checker.return_value_definitely_assigned = saved;
        exit_state
    }


    pub(super) fn check_loop_restriction(&mut self, symbol_id: SymbolId, range: TextRange) {
        for ctx in &self.checker.loop_stack {
            if ctx.restricted.contains(&symbol_id) {
                self.checker.diagnostics.error(
                    DiagnosticCode::InvalidOperation,
                    range,
                    "FOR loop control variables must not be modified in the loop body",
                );
                break;
            }
        }
    }


    fn record_case_label_value(
        &mut self,
        tracker: &mut CaseLabelTracker,
        value: i64,
        range: TextRange,
    ) {
        if tracker.ints.contains_key(&value)
            || tracker
                .ranges
                .iter()
                .any(|(lower, upper)| value >= *lower && value <= *upper)
        {
            self.checker.diagnostics.error(
                DiagnosticCode::InvalidOperation,
                range,
                "duplicate CASE label",
            );
            return;
        }

        tracker.ints.insert(value, range);
    }


    fn record_case_label_range(
        &mut self,
        tracker: &mut CaseLabelTracker,
        start: i64,
        end: i64,
        range: TextRange,
    ) {
        let (lower, upper) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        let overlaps_value = tracker
            .ints
            .keys()
            .any(|value| *value >= lower && *value <= upper);
        let overlaps_range = tracker
            .ranges
            .iter()
            .any(|(r_lower, r_upper)| !(upper < *r_lower || lower > *r_upper));

        if overlaps_value || overlaps_range {
            self.checker.diagnostics.error(
                DiagnosticCode::InvalidOperation,
                range,
                "duplicate CASE label",
            );
            return;
        }

        tracker.ranges.push((lower, upper));
    }


    fn check_ref_assignment(&mut self, target: &SyntaxNode, value: &SyntaxNode) {
        let target_type = self.checker.expr().check_expression(target);
        let value_type = self.checker.expr().check_expression(value);

        if target_type == TypeId::UNKNOWN || value_type == TypeId::UNKNOWN {
            return;
        }

        let Some(target_ref) = self.reference_target_type(target_type) else {
            self.checker.diagnostics.error(
                DiagnosticCode::TypeMismatch,
                target.text_range(),
                "reference assignment requires REF_TO or POINTER TO target",
            );
            return;
        };

        if value_type == TypeId::NULL {
            return;
        }

        let Some(source_ref) = self.reference_target_type(value_type) else {
            self.checker.diagnostics.error(
                DiagnosticCode::TypeMismatch,
                value.text_range(),
                "reference assignment requires REF_TO or POINTER TO source",
            );
            return;
        };

        if !self
            .checker
            .reference_types_compatible(target_ref, source_ref)
        {
            let target_name = self.checker.type_name(target_ref);
            let source_name = self.checker.type_name(source_ref);
            self.checker.diagnostics.error(
                DiagnosticCode::TypeMismatch,
                value.text_range(),
                format!(
                    "reference assignment requires compatible types: '{}' vs '{}'",
                    target_name, source_name
                ),
            );
        }
    }


    fn reference_target_type(&self, type_id: TypeId) -> Option<TypeId> {
        let resolved = self.checker.resolve_alias_type(type_id);
        match self.checker.symbols.type_by_id(resolved)? {
            Type::Reference { target } | Type::Pointer { target } => Some(*target),
            _ => None,
        }
    }


    fn check_subrange_assignment(
        &mut self,
        target_type: TypeId,
        value: &SyntaxNode,
        value_type: TypeId,
    ) {
        let Some((_, lower, upper)) = self.checker.subrange_bounds(target_type) else {
            return;
        };

        if let Some((_, value_lower, value_upper)) = self.checker.subrange_bounds(value_type) {
            if value_lower >= lower && value_upper <= upper {
                return;
            }
        }

        if let Some(value_int) = self.checker.eval_const_int_expr_or_report(value) {
            if value_int < lower || value_int > upper {
                self.checker.diagnostics.error(
                    DiagnosticCode::OutOfRange,
                    value.text_range(),
                    format!("value {} outside subrange {}..{}", value_int, lower, upper),
                );
            }
        }
    }

}
