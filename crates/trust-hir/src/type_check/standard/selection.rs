use super::super::*;
use super::helpers::{builtin_in_params, builtin_param};

impl<'a, 'b> StandardChecker<'a, 'b> {
    pub(in crate::type_check) fn infer_sel_call(&mut self, node: &SyntaxNode) -> TypeId {
        let params = vec![
            builtin_param("G", ParamDirection::In),
            builtin_param("IN0", ParamDirection::In),
            builtin_param("IN1", ParamDirection::In),
        ];
        let call = self.builtin_call(node, params);
        call.check_formal_arg_count(self, node, 3);
        if call.arg_count() != 3 {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        let Some((arg_g, ty_g)) = call.arg(0) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        if ty_g == TypeId::UNKNOWN {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::CannotResolve, arg_g.range);
        }
        if self.base_type_id(ty_g) != TypeId::BOOL {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg_g.range,
                "expected BOOL selector",
            );
        }
        let inputs = call.args_from(1);
        self.common_any_type_for_args(&inputs).unwrap_or_else(|| {
            self.checker
                .legacy_suppressed_type(DiagnosticCode::InvalidArgumentType, node.text_range())
        })
    }

    pub(in crate::type_check) fn infer_min_max_call(&mut self, node: &SyntaxNode) -> TypeId {
        let arg_count = self.checker.calls().collect_call_args(node).len();
        if arg_count < 2 {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::WrongArgumentCount,
                node.text_range(),
                format!("expected at least 2 arguments, found {}", arg_count),
            );
        }
        let params = builtin_in_params("IN", 1, arg_count);
        let call = self.builtin_call(node, params);
        let inputs = call.args_from(0);
        self.common_elementary_type_for_args(&inputs)
            .unwrap_or_else(|| {
                self.checker
                    .legacy_suppressed_type(DiagnosticCode::InvalidArgumentType, node.text_range())
            })
    }

    pub(in crate::type_check) fn infer_limit_call(&mut self, node: &SyntaxNode) -> TypeId {
        let params = vec![
            builtin_param("MN", ParamDirection::In),
            builtin_param("IN", ParamDirection::In),
            builtin_param("MX", ParamDirection::In),
        ];
        let call = self.builtin_call(node, params);
        call.check_formal_arg_count(self, node, 3);
        if call.arg_count() != 3 {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        let inputs = call.args_from(0);
        self.common_elementary_type_for_args(&inputs)
            .unwrap_or_else(|| {
                self.checker
                    .legacy_suppressed_type(DiagnosticCode::InvalidArgumentType, node.text_range())
            })
    }

    pub(in crate::type_check) fn infer_mux_call(&mut self, node: &SyntaxNode) -> TypeId {
        let arg_count = self.checker.calls().collect_call_args(node).len();
        if arg_count < 3 {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::WrongArgumentCount,
                node.text_range(),
                format!("expected at least 3 arguments, found {}", arg_count),
            );
        }
        let mut params = vec![builtin_param("K", ParamDirection::In)];
        params.extend(builtin_in_params("IN", 0, arg_count - 1));
        let call = self.builtin_call(node, params);
        let Some((arg_k, ty_k)) = call.arg(0) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        if ty_k == TypeId::UNKNOWN {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::CannotResolve, arg_k.range);
        }
        if !self.is_integer_type(ty_k) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg_k.range,
                "expected integer selector",
            );
        }
        let inputs = call.args_from(1);
        self.common_any_type_for_args(&inputs).unwrap_or_else(|| {
            self.checker
                .legacy_suppressed_type(DiagnosticCode::InvalidArgumentType, node.text_range())
        })
    }
}
