use super::super::*;
use super::*;
use crate::semantic::SemanticOutcome;
use crate::symbols::VarQualifier;

impl<'a, 'b> CallChecker<'a, 'b> {
    pub(in crate::type_check) fn infer_ref_call(&mut self, node: &SyntaxNode) -> TypeId {
        let outcome = self.infer_ref_call_outcome(node);
        self.checker.legacy_type_from_outcome(outcome)
    }

    fn infer_ref_call_outcome(&mut self, node: &SyntaxNode) -> SemanticOutcome<TypeId> {
        let args = self.collect_call_args(node);
        if args.len() != 1 {
            return self.checker.diagnostic_type_outcome(
                DiagnosticCode::WrongArgumentCount,
                node.text_range(),
                format!("expected 1 argument, found {}", args.len()),
            );
        }
        let expr = &args[0].expr;
        if !self.checker.is_valid_lvalue(expr) {
            return self.checker.diagnostic_type_outcome(
                DiagnosticCode::InvalidOperation,
                expr.text_range(),
                "REF expects an assignable operand",
            );
        }
        if self.checker.is_constant_target(expr) {
            return self.checker.diagnostic_type_outcome(
                DiagnosticCode::InvalidOperation,
                expr.text_range(),
                "REF cannot take a reference to a constant",
            );
        }

        if let Some(resolved) = self.checker.resolve().resolve_lvalue_root(expr) {
            if let Some(symbol) = self.checker.symbols.get(resolved.id) {
                if matches!(
                    symbol.kind,
                    SymbolKind::Variable {
                        qualifier: VarQualifier::Temp
                    }
                ) {
                    return self.checker.diagnostic_type_outcome(
                        DiagnosticCode::InvalidOperation,
                        expr.text_range(),
                        "REF cannot take a reference to a temporary variable",
                    );
                }

                if let Some(current_id) = self.checker.current_pou_symbol {
                    if let Some(owner) = self.checker.symbols.get(current_id) {
                        let is_function_like = matches!(
                            owner.kind,
                            SymbolKind::Function { .. } | SymbolKind::Method { .. }
                        );
                        if is_function_like && symbol.parent == Some(current_id) {
                            return self.checker.diagnostic_type_outcome(
                                DiagnosticCode::InvalidOperation,
                                expr.text_range(),
                                "REF cannot take a reference to function-local variables",
                            );
                        }
                    }
                }
            }
        }

        let target_type = self.checker.expr().check_expression(expr);
        if target_type == TypeId::UNKNOWN {
            return self
                .checker
                .suppressed_type_outcome(DiagnosticCode::CannotResolve, expr.text_range());
        }

        SemanticOutcome::Resolved(self.checker.symbols.register_reference_type(target_type))
    }

    pub(in crate::type_check) fn infer_new_call(&mut self, node: &SyntaxNode) -> TypeId {
        let outcome = self.infer_new_call_outcome(node);
        self.checker.legacy_type_from_outcome(outcome)
    }

    fn infer_new_call_outcome(&mut self, node: &SyntaxNode) -> SemanticOutcome<TypeId> {
        let args = self.collect_call_args(node);
        if args.len() != 1 {
            return self.checker.diagnostic_type_outcome(
                DiagnosticCode::WrongArgumentCount,
                node.text_range(),
                format!("expected 1 argument, found {}", args.len()),
            );
        }

        let arg = &args[0];
        if arg.assign != CallArgAssign::Positional {
            return self.checker.diagnostic_type_outcome(
                DiagnosticCode::InvalidArgumentType,
                arg.range,
                "NEW expects a single positional type argument",
            );
        }

        let target_type = match self
            .checker
            .resolve_ref()
            .resolve_type_from_expr_outcome(&arg.expr)
        {
            SemanticOutcome::Resolved(type_id) => type_id,
            SemanticOutcome::Ambiguous { .. } => {
                return self.checker.diagnostic_type_outcome(
                    DiagnosticCode::CannotResolve,
                    arg.range,
                    "NEW type argument is ambiguous; qualify the type name",
                );
            }
            SemanticOutcome::WrongKind { .. } => {
                return self.checker.diagnostic_type_outcome(
                    DiagnosticCode::InvalidArgumentType,
                    arg.range,
                    "NEW expects a type name",
                );
            }
            SemanticOutcome::Unknown { .. } => {
                return self.checker.diagnostic_type_outcome(
                    DiagnosticCode::UndefinedType,
                    arg.range,
                    "NEW expects a type name",
                );
            }
            SemanticOutcome::SuppressedCascade { primary, range } => {
                return SemanticOutcome::SuppressedCascade { primary, range };
            }
            SemanticOutcome::InvariantViolation { message, .. } => {
                return self.checker.diagnostic_type_outcome(
                    DiagnosticCode::CannotResolve,
                    arg.range,
                    message.to_string(),
                );
            }
        };

        SemanticOutcome::Resolved(self.checker.symbols.register_reference_type(target_type))
    }

    pub(in crate::type_check) fn infer_ref_delete_call(&mut self, node: &SyntaxNode) -> TypeId {
        let outcome = self.infer_ref_delete_call_outcome(node);
        self.checker.legacy_type_from_outcome(outcome)
    }

    fn infer_ref_delete_call_outcome(&mut self, node: &SyntaxNode) -> SemanticOutcome<TypeId> {
        let args = self.collect_call_args(node);
        if args.len() != 1 {
            return self.checker.diagnostic_type_outcome(
                DiagnosticCode::WrongArgumentCount,
                node.text_range(),
                format!("expected 1 argument, found {}", args.len()),
            );
        }

        let arg = &args[0];
        if arg.assign != CallArgAssign::Positional {
            return self.checker.diagnostic_type_outcome(
                DiagnosticCode::InvalidArgumentType,
                arg.range,
                "__DELETE expects a single positional argument",
            );
        }

        let arg_type = self.checker.expr().check_expression(&arg.expr);
        let resolved = self.checker.resolve_alias_type(arg_type);
        if resolved == TypeId::UNKNOWN {
            return self
                .checker
                .suppressed_type_outcome(DiagnosticCode::CannotResolve, arg.range);
        }
        match self.checker.symbols.type_by_id(resolved) {
            Some(Type::Reference { .. } | Type::Pointer { .. }) | Some(Type::Null) => {}
            _ => {
                return self.checker.diagnostic_type_outcome(
                    DiagnosticCode::InvalidArgumentType,
                    arg.range,
                    "__DELETE expects a REF_TO, POINTER TO, or NULL argument",
                );
            }
        }

        SemanticOutcome::Resolved(TypeId::VOID)
    }
}
