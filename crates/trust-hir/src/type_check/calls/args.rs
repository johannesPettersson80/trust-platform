use super::super::standard::is_execution_param;
use super::super::*;
use super::*;

impl<'a, 'b> CallChecker<'a, 'b> {
    pub(in crate::type_check) fn bind_call_arguments(
        &mut self,
        params: &[ParamInfo],
        node: &SyntaxNode,
    ) -> BoundArgs {
        self.bind_call_arguments_with_aliases(params, node, &[])
    }

    pub(in crate::type_check) fn bind_call_arguments_with_aliases(
        &mut self,
        params: &[ParamInfo],
        node: &SyntaxNode,
        aliases: &[(&str, &str)],
    ) -> BoundArgs {
        let args = self.collect_call_args(node);
        let arg_count = args.len();

        if params.is_empty() {
            if arg_count > 0 {
                self.checker.diagnostics.error(
                    DiagnosticCode::WrongArgumentCount,
                    node.text_range(),
                    format!("expected 0 arguments, found {}", arg_count),
                );
            }
            return BoundArgs {
                assigned: Vec::new(),
                formal_call: false,
            };
        }

        let formal_call = args.is_empty()
            || args
                .iter()
                .any(|arg| arg.assign != CallArgAssign::Positional);

        let mut assigned: Vec<Option<CallArg>> = vec![None; params.len()];

        let param_index = |name: &str| -> Option<usize> {
            if let Some(index) = params
                .iter()
                .position(|param| param.name.eq_ignore_ascii_case(name))
            {
                return Some(index);
            }

            let alias = aliases.iter().find_map(|(alias, canonical)| {
                if alias.eq_ignore_ascii_case(name) {
                    Some(*canonical)
                } else {
                    None
                }
            })?;

            params
                .iter()
                .position(|param| param.name.eq_ignore_ascii_case(alias))
        };

        if formal_call {
            let positional_params: Vec<_> = params
                .iter()
                .enumerate()
                .filter(|(_, param)| !is_execution_param(param))
                .collect();
            let mut positional_iter = positional_params.iter();
            let mut positional_count = 0usize;
            let mut positional_overflow = false;
            let mut saw_formal = false;

            for arg in args {
                if arg.assign == CallArgAssign::Positional {
                    if saw_formal {
                        self.checker.diagnostics.error(
                            DiagnosticCode::InvalidArgumentType,
                            arg.range,
                            "positional arguments must precede formal arguments",
                        );
                        continue;
                    }
                    positional_count += 1;
                    if let Some((param_index, _)) = positional_iter.next() {
                        assigned[*param_index] = Some(arg);
                    } else {
                        positional_overflow = true;
                    }
                    continue;
                }

                saw_formal = true;
                if arg.assign == CallArgAssign::RefAssign {
                    self.checker.diagnostics.error(
                        DiagnosticCode::InvalidArgumentType,
                        arg.range,
                        "parameter assignment cannot use '?='",
                    );
                    continue;
                }

                let Some(name) = &arg.name else {
                    self.checker.diagnostics.error(
                        DiagnosticCode::InvalidArgumentType,
                        arg.range,
                        "formal call arguments must be named",
                    );
                    continue;
                };

                if let Some(index) = param_index(name.as_str()) {
                    if assigned[index].is_some() {
                        self.checker.diagnostics.error(
                            DiagnosticCode::InvalidArgumentType,
                            arg.range,
                            format!("argument '{}' specified more than once", name),
                        );
                    } else {
                        assigned[index] = Some(arg);
                    }
                } else {
                    self.checker.diagnostics.error(
                        DiagnosticCode::CannotResolve,
                        arg.range,
                        format!("unknown parameter '{}'", name),
                    );
                }
            }

            if positional_overflow {
                self.checker.diagnostics.error(
                    DiagnosticCode::WrongArgumentCount,
                    node.text_range(),
                    format!(
                        "expected {} positional arguments, found {}",
                        positional_params.len(),
                        positional_count
                    ),
                );
            }
        } else {
            let positional_params: Vec<_> = params
                .iter()
                .enumerate()
                .filter(|(_, param)| !is_execution_param(param))
                .collect();

            if arg_count != positional_params.len() {
                self.checker.diagnostics.error(
                    DiagnosticCode::WrongArgumentCount,
                    node.text_range(),
                    format!(
                        "expected {} arguments, found {}",
                        positional_params.len(),
                        arg_count
                    ),
                );
            }

            for (arg, (param_index, _)) in args.into_iter().zip(positional_params.iter()) {
                assigned[*param_index] = Some(arg);
            }
        }

        if formal_call {
            for (param, arg) in params.iter().zip(assigned.iter()) {
                if arg.is_none() && matches!(param.direction, ParamDirection::InOut) {
                    self.checker.diagnostics.error(
                        DiagnosticCode::InvalidArgumentType,
                        node.text_range(),
                        format!("missing binding for in-out parameter '{}'", param.name),
                    );
                }
            }
        }

        BoundArgs {
            assigned,
            formal_call,
        }
    }

    pub(in crate::type_check) fn check_bound_call_argument_types(
        &mut self,
        params: &[ParamInfo],
        bound: &BoundArgs,
    ) {
        let formal_call = bound.formal_call;
        for (param, arg) in params.iter().zip(bound.assigned.iter()) {
            let Some(arg) = arg else {
                continue;
            };

            if formal_call {
                match param.direction {
                    ParamDirection::Out => {
                        if arg.assign != CallArgAssign::Arrow {
                            self.checker.diagnostics.error(
                                DiagnosticCode::InvalidArgumentType,
                                arg.range,
                                format!("output parameter '{}' must use '=>'", param.name),
                            );
                            continue;
                        }
                    }
                    _ => {
                        if arg.assign == CallArgAssign::Arrow {
                            self.checker.diagnostics.error(
                                DiagnosticCode::InvalidArgumentType,
                                arg.range,
                                format!("parameter '{}' is not VAR_OUTPUT; use ':='", param.name),
                            );
                            continue;
                        }
                    }
                }
            }

            let arg_type = self.checker.expr().check_expression(&arg.expr);

            match param.direction {
                ParamDirection::In => {
                    let context_literal = self
                        .checker
                        .is_contextual_int_literal(param.type_id, &arg.expr)
                        || self
                            .checker
                            .is_contextual_real_literal(param.type_id, &arg.expr);
                    if self.checker.is_assignable(param.type_id, arg_type) || context_literal {
                        self.checker.check_string_literal_assignment(
                            param.type_id,
                            &arg.expr,
                            arg_type,
                        );
                        if !context_literal {
                            self.checker.warn_implicit_conversion(
                                param.type_id,
                                arg_type,
                                arg.range,
                            );
                        }
                    } else {
                        self.checker.diagnostics.error(
                            DiagnosticCode::InvalidArgumentType,
                            arg.range,
                            self.invalid_argument_type_message(param, arg_type),
                        );
                    }
                }
                ParamDirection::Out => {
                    if !self.checker.is_valid_lvalue(&arg.expr) {
                        self.checker.diagnostics.error(
                            DiagnosticCode::InvalidArgumentType,
                            arg.range,
                            format!("output parameter '{}' must be assignable", param.name),
                        );
                        continue;
                    }
                    if self.checker.is_constant_target(&arg.expr) {
                        self.checker.diagnostics.error(
                            DiagnosticCode::InvalidArgumentType,
                            arg.range,
                            format!(
                                "output parameter '{}' cannot bind to a constant",
                                param.name
                            ),
                        );
                        continue;
                    }
                    if let Some(resolved) = self.checker.assignment_target_symbol(&arg.expr) {
                        self.checker
                            .stmt()
                            .check_loop_restriction(resolved.id, arg.range);
                    }
                    if self.checker.is_assignable(arg_type, param.type_id) {
                        self.checker
                            .warn_implicit_conversion(arg_type, param.type_id, arg.range);
                    } else {
                        self.checker.diagnostics.error(
                            DiagnosticCode::InvalidArgumentType,
                            arg.range,
                            self.invalid_output_argument_type_message(param, arg_type),
                        );
                    }
                }
                ParamDirection::InOut => {
                    if !self.checker.is_valid_lvalue(&arg.expr) {
                        self.checker.diagnostics.error(
                            DiagnosticCode::InvalidArgumentType,
                            arg.range,
                            format!("in-out parameter '{}' must be assignable", param.name),
                        );
                        continue;
                    }
                    if self.checker.is_constant_target(&arg.expr) {
                        self.checker.diagnostics.error(
                            DiagnosticCode::InvalidArgumentType,
                            arg.range,
                            format!(
                                "in-out parameter '{}' cannot bind to a constant",
                                param.name
                            ),
                        );
                        continue;
                    }
                    if let Some(resolved) = self.checker.assignment_target_symbol(&arg.expr) {
                        self.checker
                            .stmt()
                            .check_loop_restriction(resolved.id, arg.range);
                    }

                    let to_param = self.checker.is_assignable(param.type_id, arg_type);
                    let from_param = self.checker.is_assignable(arg_type, param.type_id);
                    if to_param && from_param {
                        self.checker
                            .warn_implicit_conversion(param.type_id, arg_type, arg.range);
                        self.checker
                            .warn_implicit_conversion(arg_type, param.type_id, arg.range);
                    } else {
                        self.checker.diagnostics.error(
                            DiagnosticCode::InvalidArgumentType,
                            arg.range,
                            self.invalid_in_out_argument_type_message(param, arg_type),
                        );
                    }
                }
            }
        }
    }

    pub(in crate::type_check) fn check_typed_args_against_params(
        &mut self,
        params: &[ParamInfo],
        typed: &[Option<(CallArg, TypeId)>],
    ) {
        for (param, arg) in params.iter().zip(typed.iter()) {
            let Some((arg, arg_type)) = arg.as_ref() else {
                continue;
            };
            let arg_type = *arg_type;

            match param.direction {
                ParamDirection::In => {
                    let context_literal = self
                        .checker
                        .is_contextual_int_literal(param.type_id, &arg.expr)
                        || self
                            .checker
                            .is_contextual_real_literal(param.type_id, &arg.expr);
                    if self.checker.is_assignable(param.type_id, arg_type) || context_literal {
                        self.checker.check_string_literal_assignment(
                            param.type_id,
                            &arg.expr,
                            arg_type,
                        );
                        if !context_literal {
                            self.checker.warn_implicit_conversion(
                                param.type_id,
                                arg_type,
                                arg.range,
                            );
                        }
                    } else {
                        self.checker.diagnostics.error(
                            DiagnosticCode::InvalidArgumentType,
                            arg.range,
                            self.invalid_argument_type_message(param, arg_type),
                        );
                    }
                }
                ParamDirection::Out => {
                    if self.checker.is_assignable(arg_type, param.type_id) {
                        self.checker
                            .warn_implicit_conversion(arg_type, param.type_id, arg.range);
                    } else {
                        self.checker.diagnostics.error(
                            DiagnosticCode::InvalidArgumentType,
                            arg.range,
                            self.invalid_output_argument_type_message(param, arg_type),
                        );
                    }
                }
                ParamDirection::InOut => {
                    let to_param = self.checker.is_assignable(param.type_id, arg_type);
                    let from_param = self.checker.is_assignable(arg_type, param.type_id);
                    if to_param && from_param {
                        self.checker
                            .warn_implicit_conversion(param.type_id, arg_type, arg.range);
                        self.checker
                            .warn_implicit_conversion(arg_type, param.type_id, arg.range);
                    } else {
                        self.checker.diagnostics.error(
                            DiagnosticCode::InvalidArgumentType,
                            arg.range,
                            self.invalid_in_out_argument_type_message(param, arg_type),
                        );
                    }
                }
            }
        }
    }

    fn invalid_argument_type_message(&self, param: &ParamInfo, arg_type: TypeId) -> String {
        self.pointer_target_mismatch_message(param, arg_type)
            .unwrap_or_else(|| {
                format!(
                    "expected '{}' for parameter '{}'",
                    self.checker.type_name(param.type_id),
                    param.name
                )
            })
    }

    fn invalid_output_argument_type_message(&self, param: &ParamInfo, arg_type: TypeId) -> String {
        self.pointer_target_mismatch_message(param, arg_type)
            .unwrap_or_else(|| {
                format!(
                    "output parameter '{}' expects '{}'",
                    param.name,
                    self.checker.type_name(param.type_id)
                )
            })
    }

    fn invalid_in_out_argument_type_message(&self, param: &ParamInfo, arg_type: TypeId) -> String {
        self.pointer_target_mismatch_message(param, arg_type)
            .unwrap_or_else(|| {
                format!(
                    "in-out parameter '{}' expects '{}'",
                    param.name,
                    self.checker.type_name(param.type_id)
                )
            })
    }

    fn pointer_target_mismatch_message(
        &self,
        param: &ParamInfo,
        arg_type: TypeId,
    ) -> Option<String> {
        let expected = self.checker.resolve_alias_type(param.type_id);
        let actual = self.checker.resolve_alias_type(arg_type);
        let (
            Some(Type::Pointer {
                target: expected_target,
            }),
            Some(Type::Pointer {
                target: actual_target,
            }),
        ) = (
            self.checker.symbols.type_by_id(expected),
            self.checker.symbols.type_by_id(actual),
        )
        else {
            return None;
        };

        Some(format!(
            "pointer target mismatch for parameter '{}': expected pointer to '{}', got pointer to '{}'",
            param.name,
            self.checker.type_name(*expected_target),
            self.checker.type_name(*actual_target)
        ))
    }

    pub(in crate::type_check) fn collect_builtin_args(
        &mut self,
        params: &[ParamInfo],
        node: &SyntaxNode,
    ) -> (BoundArgs, Vec<Option<(CallArg, TypeId)>>) {
        let bound = self.bind_call_arguments(params, node);
        let mut typed_args = Vec::with_capacity(params.len());

        for (param, arg) in params.iter().zip(bound.assigned.iter()) {
            let Some(arg) = arg else {
                typed_args.push(None);
                continue;
            };

            if bound.formal_call {
                match param.direction {
                    ParamDirection::Out => {
                        if arg.assign != CallArgAssign::Arrow {
                            self.checker.diagnostics.error(
                                DiagnosticCode::InvalidArgumentType,
                                arg.range,
                                format!("output parameter '{}' must use '=>'", param.name),
                            );
                            typed_args.push(None);
                            continue;
                        }
                    }
                    _ => {
                        if arg.assign == CallArgAssign::Arrow {
                            self.checker.diagnostics.error(
                                DiagnosticCode::InvalidArgumentType,
                                arg.range,
                                format!("parameter '{}' is not VAR_OUTPUT; use ':='", param.name),
                            );
                            typed_args.push(None);
                            continue;
                        }
                    }
                }
            }

            if matches!(param.direction, ParamDirection::Out | ParamDirection::InOut) {
                if !self.checker.is_valid_lvalue(&arg.expr) {
                    self.checker.diagnostics.error(
                        DiagnosticCode::InvalidArgumentType,
                        arg.range,
                        format!("parameter '{}' must be assignable", param.name),
                    );
                    typed_args.push(None);
                    continue;
                }
                if self.checker.is_constant_target(&arg.expr) {
                    self.checker.diagnostics.error(
                        DiagnosticCode::InvalidArgumentType,
                        arg.range,
                        format!("parameter '{}' cannot bind to a constant", param.name),
                    );
                    typed_args.push(None);
                    continue;
                }
            }

            let arg_type = self.checker.expr().check_expression(&arg.expr);
            typed_args.push(Some((arg.clone(), arg_type)));
        }

        (bound, typed_args)
    }

    pub(in crate::type_check) fn callable_parameters(
        &self,
        symbol_id: SymbolId,
        kind: &SymbolKind,
    ) -> Vec<ParamInfo> {
        let mut ids: Vec<SymbolId> = match kind {
            SymbolKind::Function { parameters, .. } | SymbolKind::Method { parameters, .. } => {
                parameters.clone()
            }
            _ => Vec::new(),
        };

        if ids.is_empty() {
            ids = self
                .checker
                .symbols
                .iter()
                .filter(|sym| {
                    sym.parent == Some(symbol_id)
                        && matches!(sym.kind, SymbolKind::Parameter { .. })
                })
                .map(|sym| sym.id)
                .collect();
        }

        ids.sort_by_key(|id| id.0);

        ids.into_iter()
            .filter_map(|id| {
                self.checker.symbols.get(id).and_then(|sym| match sym.kind {
                    SymbolKind::Parameter { direction } => Some(ParamInfo {
                        name: sym.name.clone(),
                        type_id: sym.type_id,
                        direction,
                    }),
                    _ => None,
                })
            })
            .collect()
    }

    pub(in crate::type_check) fn collect_call_args(&self, node: &SyntaxNode) -> Vec<CallArg> {
        let mut args = Vec::new();
        let Some(arg_list) = node
            .children()
            .find(|child| child.kind() == SyntaxKind::ArgList)
        else {
            return args;
        };

        for arg in arg_list.children().filter(|n| n.kind() == SyntaxKind::Arg) {
            let name = arg
                .children()
                .find(|child| child.kind() == SyntaxKind::Name)
                .and_then(|child| self.checker.resolve_ref().get_name_from_ref(&child));
            let expr = arg
                .children()
                .find(|child| is_expression_kind(child.kind()));
            if let Some(expr) = expr {
                let assign = call_arg_assign(&arg);
                args.push(CallArg {
                    name,
                    expr,
                    range: arg.text_range(),
                    assign,
                });
            }
        }

        args
    }
}

fn call_arg_assign(node: &SyntaxNode) -> CallArgAssign {
    for token in node
        .children_with_tokens()
        .filter_map(|element| element.into_token())
    {
        match token.kind() {
            SyntaxKind::Assign => return CallArgAssign::Assign,
            SyntaxKind::Arrow => return CallArgAssign::Arrow,
            SyntaxKind::RefAssign => return CallArgAssign::RefAssign,
            _ => {}
        }
    }
    CallArgAssign::Positional
}
