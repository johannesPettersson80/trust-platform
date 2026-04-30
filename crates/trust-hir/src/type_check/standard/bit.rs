use super::super::*;
use super::helpers::{builtin_in_params, builtin_param};

impl<'a, 'b> StandardChecker<'a, 'b> {
    pub(in crate::type_check) fn infer_bit_shift_call(
        &mut self,
        node: &SyntaxNode,
        _name: &str,
    ) -> TypeId {
        let params = vec![
            builtin_param("IN", ParamDirection::In),
            builtin_param("N", ParamDirection::In),
        ];
        let call = self.builtin_call(node, params);
        call.check_formal_arg_count(self, node, 2);
        if call.arg_count() != 2 {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        let Some((arg_in, ty_in)) = call.arg(0) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        let Some((arg_n, ty_n)) = call.arg(1) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        if ty_in == TypeId::UNKNOWN || ty_n == TypeId::UNKNOWN {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::CannotResolve, node.text_range());
        }
        if !self.is_bit_string_type(ty_in) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg_in.range,
                "expected bit string input",
            );
        }
        if !self.is_integer_type(ty_n) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg_n.range,
                "expected integer shift count",
            );
        }
        self.base_type_id(ty_in)
    }

    pub(in crate::type_check) fn infer_variadic_bitwise_call(
        &mut self,
        node: &SyntaxNode,
    ) -> TypeId {
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
        self.common_bit_type_for_args(&inputs).unwrap_or_else(|| {
            self.checker
                .legacy_suppressed_type(DiagnosticCode::InvalidArgumentType, node.text_range())
        })
    }

    pub(in crate::type_check) fn infer_not_call(&mut self, node: &SyntaxNode) -> TypeId {
        let params = vec![builtin_param("IN", ParamDirection::In)];
        let call = self.builtin_call(node, params);
        call.check_formal_arg_count(self, node, 1);
        if call.arg_count() != 1 {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        let Some((arg, ty)) = call.arg(0) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        if ty == TypeId::UNKNOWN {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::CannotResolve, arg.range);
        }
        if !self.is_bit_string_type(ty) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg.range,
                "expected bit string input",
            );
        }
        self.base_type_id(ty)
    }
}
