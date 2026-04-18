impl<'a, 'b> StmtChecker<'a, 'b> {

    fn check_for_stmt(&mut self, node: &SyntaxNode) {
        let incoming = self.checker.return_value_definitely_assigned;
        let mut control_symbol = None;
        let mut control_type = None;

        // FOR loop iterator must be integer
        for child in node.children() {
            if matches!(child.kind(), SyntaxKind::NameRef | SyntaxKind::Name) {
                if let Some(name) = self.checker.resolve_ref().get_name_from_ref(&child) {
                    if let Some(symbol_id) = self
                        .checker
                        .symbols
                        .resolve(&name, self.checker.current_scope)
                    {
                        if let Some(symbol) = self.checker.symbols.get(symbol_id) {
                            let iter_type = self.checker.resolve_alias_type(symbol.type_id);
                            if let Some(ty) = self.checker.symbols.type_by_id(iter_type) {
                                if !ty.is_integer() {
                                    self.checker.diagnostics.error(
                                        DiagnosticCode::TypeMismatch,
                                        child.text_range(),
                                        "FOR loop iterator must be integer type",
                                    );
                                }
                            }
                            control_symbol = Some(symbol_id);
                            control_type = Some(iter_type);
                        }
                    }
                }
                break;
            }
        }

        let exprs: Vec<_> = node
            .children()
            .filter(|child| is_expression_kind(child.kind()))
            .collect();

        for (idx, expr) in exprs.iter().enumerate() {
            let expr_type_raw = self.check_expression(expr);
            let expr_type = self.checker.resolve_alias_type(expr_type_raw);
            if let Some(ty) = self.checker.symbols.type_by_id(expr_type) {
                if !ty.is_integer() {
                    self.checker.diagnostics.error(
                        DiagnosticCode::TypeMismatch,
                        expr.text_range(),
                        "FOR loop bounds must be integer type",
                    );
                }
            }

            if let Some(control_type) = control_type {
                let context_literal = self.checker.is_contextual_int_literal(control_type, expr);
                if expr_type != TypeId::UNKNOWN && expr_type != control_type && !context_literal {
                    let label = match idx {
                        0 => "initial value",
                        1 => "final value",
                        _ => "step value",
                    };
                    self.checker.diagnostics.error(
                        DiagnosticCode::TypeMismatch,
                        expr.text_range(),
                        format!(
                            "FOR loop {} must match control variable type '{}'",
                            label,
                            self.checker.type_name(control_type)
                        ),
                    );
                }
            }
        }

        let mut restricted = FxHashSet::default();
        if let Some(control_symbol) = control_symbol {
            restricted.insert(control_symbol);
        }
        if let Some(expr) = exprs.first() {
            if let Some(symbol_id) = self.checker.resolve_ref().resolve_simple_symbol(expr) {
                restricted.insert(symbol_id);
            }
        }
        if let Some(expr) = exprs.get(1) {
            if let Some(symbol_id) = self.checker.resolve_ref().resolve_simple_symbol(expr) {
                restricted.insert(symbol_id);
            }
        }

        self.checker.loop_stack.push(LoopContext { restricted });
        let _ = self.check_statement_children_with_state(node, incoming);
        self.checker.loop_stack.pop();
        self.checker.return_value_definitely_assigned = incoming;
    }


    fn check_while_stmt(&mut self, node: &SyntaxNode) {
        let incoming = self.checker.return_value_definitely_assigned;
        // Check condition is boolean
        if let Some(expr) = first_expression_child(node) {
            let cond_type = self.check_expression(&expr);
            self.checker
                .expr()
                .check_boolean(cond_type, expr.text_range());
        }

        self.checker.loop_stack.push(LoopContext {
            restricted: FxHashSet::default(),
        });
        let _ = self.check_statement_children_with_state(node, incoming);
        self.checker.loop_stack.pop();
        self.checker.return_value_definitely_assigned = incoming;
    }


    fn check_repeat_stmt(&mut self, node: &SyntaxNode) {
        let incoming = self.checker.return_value_definitely_assigned;
        // Check UNTIL condition is boolean
        if let Some(expr) = last_expression_child(node) {
            let cond_type = self.check_expression(&expr);
            self.checker
                .expr()
                .check_boolean(cond_type, expr.text_range());
        }

        self.checker.loop_stack.push(LoopContext {
            restricted: FxHashSet::default(),
        });
        let _ = self.check_statement_children_with_state(node, incoming);
        self.checker.loop_stack.pop();
        self.checker.return_value_definitely_assigned = incoming;
    }


    fn check_case_stmt(&mut self, node: &SyntaxNode) {
        let incoming = self.checker.return_value_definitely_assigned;
        // Get selector type
        let mut selector_type = TypeId::UNKNOWN;
        if let Some(expr) = first_expression_child(node) {
            selector_type = self.check_expression(&expr);
            if selector_type != TypeId::UNKNOWN && !self.is_case_selector_type(selector_type) {
                self.checker.diagnostics.error(
                    DiagnosticCode::TypeMismatch,
                    expr.text_range(),
                    "CASE selector must be an elementary type",
                );
            }
        }

        let mut tracker = CaseLabelTracker::default();
        let mut branch_states = Vec::new();

        // Check case branches
        for child in node.children() {
            if child.kind() == SyntaxKind::CaseBranch {
                branch_states.push(self.check_case_branch(
                    &child,
                    selector_type,
                    &mut tracker,
                    incoming,
                ));
            }
        }

        let has_else = node
            .children()
            .any(|child| child.kind() == SyntaxKind::ElseBranch);
        for child in node.children() {
            if child.kind() == SyntaxKind::ElseBranch {
                branch_states.push(self.check_statement_children_with_state(&child, incoming));
            }
        }
        let exhaustive = has_else || self.case_labels_cover_enum(selector_type, &tracker);
        if !exhaustive {
            branch_states.push(incoming);
        }
        self.checker.return_value_definitely_assigned =
            branch_states.into_iter().all(std::convert::identity);
        if !has_else && !self.case_labels_cover_enum(selector_type, &tracker) {
            self.checker.diagnostics.warning(
                DiagnosticCode::MissingElse,
                node.text_range(),
                "CASE statement has no ELSE branch",
            );
        }
    }


    fn check_exit_stmt(&mut self, node: &SyntaxNode) {
        if self.checker.loop_stack.is_empty() {
            self.checker.diagnostics.error(
                DiagnosticCode::InvalidOperation,
                node.text_range(),
                "EXIT must appear inside a loop",
            );
        }
    }


    fn check_continue_stmt(&mut self, node: &SyntaxNode) {
        if self.checker.loop_stack.is_empty() {
            self.checker.diagnostics.error(
                DiagnosticCode::InvalidOperation,
                node.text_range(),
                "CONTINUE must appear inside a loop",
            );
        }
    }


    fn check_jmp_stmt(&mut self, node: &SyntaxNode) {
        let label_name = node
            .children()
            .find(|n| n.kind() == SyntaxKind::Name)
            .and_then(|name| self.checker.resolve_ref().get_name_from_ref(&name));

        let Some(label_name) = label_name else {
            return;
        };

        if let Some(scope) = self.checker.label_scopes.last_mut() {
            let key = SmolStr::new(label_name.to_ascii_uppercase());
            if scope.labels.contains(&key) {
                return;
            }
            scope
                .pending_jumps
                .push((key, label_name.clone(), node.text_range()));
        } else {
            self.checker.diagnostics.error(
                DiagnosticCode::InvalidOperation,
                node.text_range(),
                "JMP is not valid outside a statement list",
            );
        }
    }


    fn check_label_stmt(&mut self, node: &SyntaxNode) {
        if let Some(label_node) = node.children().find(|n| n.kind() == SyntaxKind::Name) {
            if let Some(name) = self.checker.resolve_ref().get_name_from_ref(&label_node) {
                if let Some(scope) = self.checker.label_scopes.last_mut() {
                    let key = SmolStr::new(name.to_ascii_uppercase());
                    if !scope.labels.insert(key) {
                        self.checker.diagnostics.error(
                            DiagnosticCode::DuplicateDeclaration,
                            label_node.text_range(),
                            format!("duplicate label '{}'", name),
                        );
                    }
                }
            }
        }

        self.check_statement_children(node);
    }

}
