use super::*;

mod args;
mod exprs;
mod resolve;
mod special;
mod standard_fbs;

pub(in crate::type_check) use self::resolve::{NameLookupResult, NameResolveOutcome};

#[derive(Debug, Clone)]
pub(super) struct CallArg {
    pub(super) name: Option<SmolStr>,
    pub(super) expr: SyntaxNode,
    pub(super) range: TextRange,
    pub(super) assign: CallArgAssign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CallArgAssign {
    Positional,
    Assign,
    Arrow,
    RefAssign,
}

#[derive(Debug, Clone)]
pub(super) struct ParamInfo {
    pub(super) name: SmolStr,
    pub(super) type_id: TypeId,
    pub(super) direction: ParamDirection,
}

#[derive(Debug, Clone)]
pub(super) struct BoundArgs {
    pub(super) assigned: Vec<Option<CallArg>>,
    pub(super) formal_call: bool,
}

#[derive(Debug, Clone)]
pub(super) struct CallTargetInfo {
    return_type: TypeId,
    param_owner: SymbolId,
    kind: SymbolKind,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ResolvedSymbol {
    pub(super) id: SymbolId,
    pub(super) accessible: bool,
}

impl<'a, 'b> CallChecker<'a, 'b> {
    pub(super) fn infer_call_expr(&mut self, node: &SyntaxNode) -> TypeId {
        let children: Vec<_> = node.children().collect();
        if children.is_empty() {
            return self
                .checker
                .legacy_type_from_outcome(self.checker.unknown_type_outcome(node.text_range()));
        }

        let callee = &children[0];

        // If it's a name reference, look up the symbol
        if callee.kind() == SyntaxKind::NameRef {
            if let Some(name) = self.checker.resolve_ref().get_name_from_ref(callee) {
                if name.eq_ignore_ascii_case("REF") {
                    return self.infer_ref_call(node);
                }
                if name.eq_ignore_ascii_case("NEW") || name.eq_ignore_ascii_case("__NEW") {
                    return self.infer_new_call(node);
                }
                if name.eq_ignore_ascii_case("__DELETE") {
                    return self.infer_ref_delete_call(node);
                }
                if name.eq_ignore_ascii_case("TIME") {
                    if let Some(result) = self
                        .checker
                        .standard()
                        .infer_standard_function_call(&name, node)
                    {
                        return result;
                    }
                }
                match self
                    .checker
                    .resolve()
                    .resolve_name_in_context_outcome(&name, callee.text_range())
                {
                    NameResolveOutcome::Resolved(resolved) => {
                        if !resolved.accessible {
                            return self.checker.legacy_suppressed_type(
                                DiagnosticCode::CannotResolve,
                                callee.text_range(),
                            );
                        }
                        let Some(call_target) =
                            self.checker.resolve_ref().resolve_call_target(resolved.id)
                        else {
                            return self.checker.legacy_diagnostic_type(
                                DiagnosticCode::UndefinedFunction,
                                callee.text_range(),
                                format!("'{}' is not callable", name),
                            );
                        };

                        self.check_call_arguments(call_target.param_owner, &call_target.kind, node);
                        return call_target.return_type;
                    }
                    NameResolveOutcome::Ambiguous => {
                        return self.checker.legacy_suppressed_type(
                            DiagnosticCode::CannotResolve,
                            callee.text_range(),
                        );
                    }
                    NameResolveOutcome::NotFound => {}
                }

                if let Some(result) = self
                    .checker
                    .standard()
                    .infer_standard_function_call(&name, node)
                {
                    return result;
                }

                return self.checker.legacy_diagnostic_type(
                    DiagnosticCode::UndefinedFunction,
                    callee.text_range(),
                    format!("undefined function '{}'", name),
                );
            }
        }

        if callee.kind() == SyntaxKind::FieldExpr {
            if let Some(symbol_id) = self
                .checker
                .resolve_ref()
                .resolve_namespace_qualified_symbol(callee)
            {
                if let Some(call_target) = self.checker.resolve_ref().resolve_call_target(symbol_id)
                {
                    self.check_call_arguments(call_target.param_owner, &call_target.kind, node);
                    return call_target.return_type;
                }
                return self.checker.legacy_diagnostic_type(
                    DiagnosticCode::UndefinedFunction,
                    callee.text_range(),
                    "qualified name is not callable",
                );
            }
            let field_children: Vec<_> = callee.children().collect();
            if field_children.len() >= 2 {
                let base = &field_children[0];
                let member = &field_children[1];
                let base_type = self.checker.expr().check_expression(base);
                if let Some(name) = self.checker.resolve_ref().get_name_from_ref(member) {
                    if let Some(resolved) = self.checker.resolve().resolve_member_symbol_in_type(
                        base_type,
                        &name,
                        member.text_range(),
                    ) {
                        if !resolved.accessible {
                            return self.checker.legacy_suppressed_type(
                                DiagnosticCode::CannotResolve,
                                member.text_range(),
                            );
                        }
                        let Some(call_target) =
                            self.checker.resolve_ref().resolve_call_target(resolved.id)
                        else {
                            return self.checker.legacy_diagnostic_type(
                                DiagnosticCode::UndefinedFunction,
                                member.text_range(),
                                format!("'{}' is not callable", name),
                            );
                        };

                        self.check_call_arguments(call_target.param_owner, &call_target.kind, node);
                        return call_target.return_type;
                    }
                }
            }

            let callee_type = self.checker.expr().check_expression(callee);
            if callee_type != TypeId::UNKNOWN {
                if let Some(call_target) = self
                    .checker
                    .resolve_ref()
                    .resolve_call_target_from_type(callee_type)
                {
                    self.check_call_arguments(call_target.param_owner, &call_target.kind, node);
                    return call_target.return_type;
                }

                return self.checker.legacy_diagnostic_type(
                    DiagnosticCode::UndefinedFunction,
                    callee.text_range(),
                    "field expression is not callable",
                );
            }
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::CannotResolve, callee.text_range());
        }

        let callee_type = self.checker.expr().check_expression(callee);
        if let Some(call_target) = self
            .checker
            .resolve_ref()
            .resolve_call_target_from_type(callee_type)
        {
            self.check_call_arguments(call_target.param_owner, &call_target.kind, node);
            return call_target.return_type;
        }

        if callee_type != TypeId::UNKNOWN {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::UndefinedFunction,
                callee.text_range(),
                "expression is not callable",
            );
        }

        self.checker
            .legacy_suppressed_type(DiagnosticCode::CannotResolve, callee.text_range())
    }

    pub(super) fn check_call_arguments(
        &mut self,
        symbol_id: SymbolId,
        kind: &SymbolKind,
        node: &SyntaxNode,
    ) {
        if matches!(kind, SymbolKind::FunctionBlock) {
            let standard_fb = self
                .checker
                .symbols
                .get(symbol_id)
                .map(|symbol| (symbol.range.is_empty(), symbol.name.as_str().to_owned()));
            if let Some((true, name)) = standard_fb {
                if self
                    .checker
                    .standard()
                    .check_standard_function_block_call(&name, node)
                {
                    return;
                }
            }
        }

        let params = self.callable_parameters(symbol_id, kind);
        let bound = self.bind_call_arguments(&params, node);
        self.check_bound_call_argument_types(&params, &bound);
    }
}
