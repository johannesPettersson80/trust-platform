impl<'a, 'b> StmtChecker<'a, 'b> {
    /// Checks CODESYS's `__TRY` (specification 06 §15): its statements, the `__CATCH` variable, which is
    /// a variable of the type `__SYSTEM.ExceptionCode`, and each clause's statements. A `RETURN`, and an
    /// `EXIT` or a `CONTINUE` of a loop around the construct, is refused in the `__FINALLY` statements,
    /// and a `JMP` or a label anywhere in the construct.
    fn check_try_stmt(&mut self, node: &SyntaxNode) {
        self.checker.try_depth += 1;
        for child in node.children() {
            match child.kind() {
                SyntaxKind::CatchClause => self.check_catch_clause(&child),
                SyntaxKind::FinallyClause => {
                    self.checker.finally_loops.push(self.checker.loop_stack.len());
                    for statement in child.children() {
                        self.check_statement(&statement);
                    }
                    self.checker.finally_loops.pop();
                }
                _ => self.check_statement(&child),
            }
        }
        self.checker.try_depth -= 1;
    }

    /// Checks a `__CATCH(variable)` clause: the variable, then the clause's statements.
    fn check_catch_clause(&mut self, clause: &SyntaxNode) {
        let mut statements = Vec::new();
        for child in clause.children() {
            if child.kind().is_statement_node() {
                statements.push(child);
                continue;
            }
            let variable_type = self.check_expression(&child);
            let exception_code = self
                .checker
                .symbols
                .lookup_registered_type_name(crate::symbols::EXCEPTION_CODE_TYPE);
            let is_exception_code = exception_code.is_some_and(|code| {
                self.checker.symbols.resolve_alias_type(variable_type) == code
            });
            if !self.checker.is_valid_lvalue(&child) || !is_exception_code {
                self.checker.diagnostics.error(
                    DiagnosticCode::TypeMismatch,
                    child.text_range(),
                    "the __CATCH variable must be a variable of the type __SYSTEM.ExceptionCode",
                );
            }
        }
        for statement in statements {
            self.check_statement(&statement);
        }
    }

    /// Refuses `what` (`RETURN`, `EXIT`, `CONTINUE`) when it would leave a `__FINALLY` clause: a `RETURN`
    /// anywhere in one, an `EXIT` or a `CONTINUE` of a loop around it.
    fn refuse_leaving_finally(&mut self, node: &SyntaxNode, what: &str, loop_bound: bool) {
        let Some(&loops) = self.checker.finally_loops.last() else {
            return;
        };
        if !loop_bound || self.checker.loop_stack.len() == loops {
            self.checker.diagnostics.error(
                DiagnosticCode::InvalidOperation,
                node.text_range(),
                format!("{what} may not leave the statements after __FINALLY"),
            );
        }
    }

    /// Refuses a `JMP` or a label inside a `__TRY` construct.
    fn refuse_jump_in_try(&mut self, node: &SyntaxNode, what: &str) {
        if self.checker.try_depth > 0 {
            self.checker.diagnostics.error(
                DiagnosticCode::InvalidOperation,
                node.text_range(),
                format!("{what} is not valid inside __TRY"),
            );
        }
    }
}
