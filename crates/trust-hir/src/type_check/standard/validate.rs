use super::super::*;
use super::helpers::builtin_param;

impl<'a, 'b> StandardChecker<'a, 'b> {
    pub(in crate::type_check) fn infer_is_valid_call(&mut self, node: &SyntaxNode) -> TypeId {
        let params = vec![builtin_param("IN", ParamDirection::In)];
        let call = self.builtin_call(node, params);
        call.check_formal_arg_count(self, node, 1);
        if call.arg_count() != 1 {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        let Some((arg, arg_type)) = call.arg(0) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        if arg_type == TypeId::UNKNOWN {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::CannotResolve, arg.range);
        }
        if !self.is_real_type(arg_type) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg.range,
                "expected REAL or LREAL type",
            );
        }
        TypeId::BOOL
    }

    pub(in crate::type_check) fn infer_is_valid_bcd_call(&mut self, node: &SyntaxNode) -> TypeId {
        let params = vec![builtin_param("IN", ParamDirection::In)];
        let call = self.builtin_call(node, params);
        call.check_formal_arg_count(self, node, 1);
        if call.arg_count() != 1 {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        let Some((arg, arg_type)) = call.arg(0) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        if arg_type == TypeId::UNKNOWN {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::CannotResolve, arg.range);
        }
        if !self.is_bit_string_type(arg_type) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg.range,
                "expected bit string type",
            );
        }
        TypeId::BOOL
    }
}
