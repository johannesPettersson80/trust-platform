use super::super::*;
use super::*;
use crate::semantic::LEGACY_UNKNOWN_TYPE_ID;

impl<'a, 'b> StandardChecker<'a, 'b> {
    pub(in crate::type_check) fn check_standard_function_block_call(
        &mut self,
        name: &str,
        node: &SyntaxNode,
    ) -> bool {
        let upper = name.to_ascii_uppercase();
        let param = |name: &str, type_id: TypeId, direction: ParamDirection| ParamInfo {
            name: SmolStr::new(name),
            type_id,
            direction,
        };

        match upper.as_str() {
            "RS" => {
                let params = vec![
                    param("S", TypeId::BOOL, ParamDirection::In),
                    param("R1", TypeId::BOOL, ParamDirection::In),
                    param("Q1", TypeId::BOOL, ParamDirection::Out),
                ];
                let aliases = [("SET", "S"), ("RESET1", "R1")];
                self.check_standard_fb_fixed_params(&params, node, &aliases);
                true
            }
            "SR" => {
                let params = vec![
                    param("S1", TypeId::BOOL, ParamDirection::In),
                    param("R", TypeId::BOOL, ParamDirection::In),
                    param("Q1", TypeId::BOOL, ParamDirection::Out),
                ];
                let aliases = [("SET1", "S1"), ("RESET", "R")];
                self.check_standard_fb_fixed_params(&params, node, &aliases);
                true
            }
            "CTU" | "CTD" | "CTUD" => {
                self.check_counter_function_block_call(&upper, node);
                true
            }
            "TP" | "TON" | "TOF" => {
                self.check_timer_function_block_call(None, node);
                true
            }
            "TP_LTIME" | "TON_LTIME" | "TOF_LTIME" => {
                self.check_timer_function_block_call(Some(TypeId::LTIME), node);
                true
            }
            _ => false,
        }
    }

    pub(in crate::type_check) fn check_standard_fb_fixed_params(
        &mut self,
        params: &[ParamInfo],
        node: &SyntaxNode,
        aliases: &[(&str, &str)],
    ) {
        let bound = self
            .checker
            .calls()
            .bind_call_arguments_with_aliases(params, node, aliases);
        self.checker
            .calls()
            .check_bound_call_argument_types(params, &bound);
    }

    pub(in crate::type_check) fn check_counter_function_block_call(
        &mut self,
        name: &str,
        node: &SyntaxNode,
    ) {
        let param = |name: &str, type_id: TypeId, direction: ParamDirection| ParamInfo {
            name: SmolStr::new(name),
            type_id,
            direction,
        };

        let (mut params, pv_index, cv_index) = match name {
            "CTU" => (
                vec![
                    param("CU", TypeId::BOOL, ParamDirection::In),
                    param("R", TypeId::BOOL, ParamDirection::In),
                    param("PV", TypeId::ANY_INT, ParamDirection::In),
                    param("Q", TypeId::BOOL, ParamDirection::Out),
                    param("CV", TypeId::ANY_INT, ParamDirection::Out),
                ],
                2,
                4,
            ),
            "CTD" => (
                vec![
                    param("CD", TypeId::BOOL, ParamDirection::In),
                    param("LD", TypeId::BOOL, ParamDirection::In),
                    param("PV", TypeId::ANY_INT, ParamDirection::In),
                    param("Q", TypeId::BOOL, ParamDirection::Out),
                    param("CV", TypeId::ANY_INT, ParamDirection::Out),
                ],
                2,
                4,
            ),
            "CTUD" => (
                vec![
                    param("CU", TypeId::BOOL, ParamDirection::In),
                    param("CD", TypeId::BOOL, ParamDirection::In),
                    param("R", TypeId::BOOL, ParamDirection::In),
                    param("LD", TypeId::BOOL, ParamDirection::In),
                    param("PV", TypeId::ANY_INT, ParamDirection::In),
                    param("QU", TypeId::BOOL, ParamDirection::Out),
                    param("QD", TypeId::BOOL, ParamDirection::Out),
                    param("CV", TypeId::ANY_INT, ParamDirection::Out),
                ],
                4,
                7,
            ),
            _ => return,
        };

        let (_bound, typed) = self.checker.calls().collect_builtin_args(&params, node);

        let pv_type =
            self.counter_value_type(typed.get(pv_index).and_then(|arg| arg.as_ref()), "PV");
        let cv_type =
            self.counter_value_type(typed.get(cv_index).and_then(|arg| arg.as_ref()), "CV");

        let expected = pv_type.or(cv_type).unwrap_or(LEGACY_UNKNOWN_TYPE_ID);
        params[pv_index].type_id = expected;
        params[cv_index].type_id = expected;

        self.checker
            .calls()
            .check_typed_args_against_params(&params, &typed);
    }

    pub(in crate::type_check) fn counter_value_type(
        &mut self,
        arg: Option<&(CallArg, TypeId)>,
        param_name: &str,
    ) -> Option<TypeId> {
        let (arg, arg_type) = arg?;
        let base = self.base_type_id(*arg_type);
        if base == TypeId::UNKNOWN {
            return None;
        }

        if !matches!(
            base,
            TypeId::INT | TypeId::DINT | TypeId::LINT | TypeId::UDINT | TypeId::ULINT
        ) {
            self.checker.diagnostics.error(
                DiagnosticCode::InvalidArgumentType,
                arg.range,
                format!(
                    "expected INT, DINT, LINT, UDINT, or ULINT for parameter '{}'",
                    param_name
                ),
            );
            return None;
        }

        Some(base)
    }

    pub(in crate::type_check) fn check_timer_function_block_call(
        &mut self,
        fixed_type: Option<TypeId>,
        node: &SyntaxNode,
    ) {
        let param = |name: &str, type_id: TypeId, direction: ParamDirection| ParamInfo {
            name: SmolStr::new(name),
            type_id,
            direction,
        };

        let mut params = vec![
            param("IN", TypeId::BOOL, ParamDirection::In),
            param("PT", TypeId::TIME, ParamDirection::In),
            param("Q", TypeId::BOOL, ParamDirection::Out),
            param("ET", TypeId::TIME, ParamDirection::Out),
        ];

        let (_bound, typed) = self.checker.calls().collect_builtin_args(&params, node);

        let pt_type = self.timer_value_type(typed.get(1).and_then(|arg| arg.as_ref()), "PT");
        let et_type = self.timer_value_type(typed.get(3).and_then(|arg| arg.as_ref()), "ET");

        let expected = if let Some(fixed) = fixed_type {
            if let Some(pt) = pt_type {
                if pt != fixed {
                    if let Some((arg, _)) = typed.get(1).and_then(|arg| arg.as_ref()) {
                        self.checker.diagnostics.error(
                            DiagnosticCode::InvalidArgumentType,
                            arg.range,
                            format!(
                                "expected '{}' for parameter 'PT'",
                                self.checker.type_name(fixed)
                            ),
                        );
                    }
                }
            }
            if let Some(et) = et_type {
                if et != fixed {
                    if let Some((arg, _)) = typed.get(3).and_then(|arg| arg.as_ref()) {
                        self.checker.diagnostics.error(
                            DiagnosticCode::InvalidArgumentType,
                            arg.range,
                            format!(
                                "expected '{}' for parameter 'ET'",
                                self.checker.type_name(fixed)
                            ),
                        );
                    }
                }
            }
            fixed
        } else {
            pt_type.or(et_type).unwrap_or(LEGACY_UNKNOWN_TYPE_ID)
        };

        params[1].type_id = expected;
        params[3].type_id = expected;

        self.checker
            .calls()
            .check_typed_args_against_params(&params, &typed);
    }

    pub(in crate::type_check) fn timer_value_type(
        &mut self,
        arg: Option<&(CallArg, TypeId)>,
        param_name: &str,
    ) -> Option<TypeId> {
        let (arg, arg_type) = arg?;
        let base = self.base_type_id(*arg_type);
        if base == TypeId::UNKNOWN {
            return None;
        }

        if !matches!(base, TypeId::TIME | TypeId::LTIME) {
            self.checker.diagnostics.error(
                DiagnosticCode::InvalidArgumentType,
                arg.range,
                format!("expected TIME or LTIME for parameter '{}'", param_name),
            );
            return None;
        }

        Some(base)
    }
}
