use super::super::*;
use super::helpers::{builtin_in_params, builtin_param};

impl<'a, 'b> StandardChecker<'a, 'b> {
    pub(in crate::type_check) fn infer_len_call(&mut self, node: &SyntaxNode) -> TypeId {
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
        if !self.is_string_type(ty) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg.range,
                "expected STRING or WSTRING input",
            );
        }
        TypeId::INT
    }

    pub(in crate::type_check) fn infer_left_right_call(
        &mut self,
        node: &SyntaxNode,
        name: &str,
    ) -> TypeId {
        let params = vec![
            builtin_param("IN", ParamDirection::In),
            builtin_param("L", ParamDirection::In),
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
        let Some((arg_l, ty_l)) = call.arg(1) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        if ty_in == TypeId::UNKNOWN || ty_l == TypeId::UNKNOWN {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::CannotResolve, node.text_range());
        }
        if !self.is_string_type(ty_in) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg_in.range,
                format!("{} expects STRING or WSTRING", name),
            );
        }
        if !self.is_integer_type(ty_l) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg_l.range,
                "expected integer length",
            );
        }
        self.base_type_id(ty_in)
    }

    pub(in crate::type_check) fn infer_mid_call(&mut self, node: &SyntaxNode) -> TypeId {
        let params = vec![
            builtin_param("IN", ParamDirection::In),
            builtin_param("L", ParamDirection::In),
            builtin_param("P", ParamDirection::In),
        ];
        let call = self.builtin_call(node, params);
        call.check_formal_arg_count(self, node, 3);
        if call.arg_count() != 3 {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        let Some((arg_in, ty_in)) = call.arg(0) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        let Some((arg_l, ty_l)) = call.arg(1) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        let Some((arg_p, ty_p)) = call.arg(2) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        if ty_in == TypeId::UNKNOWN || ty_l == TypeId::UNKNOWN || ty_p == TypeId::UNKNOWN {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::CannotResolve, node.text_range());
        }
        if !self.is_string_type(ty_in) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg_in.range,
                "MID expects STRING or WSTRING",
            );
        }
        if !self.is_integer_type(ty_l) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg_l.range,
                "expected integer length",
            );
        }
        if !self.is_integer_type(ty_p) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg_p.range,
                "expected integer position",
            );
        }
        self.base_type_id(ty_in)
    }

    pub(in crate::type_check) fn infer_concat_call(&mut self, node: &SyntaxNode) -> TypeId {
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
        self.common_string_type_for_args(&inputs)
            .unwrap_or_else(|| {
                self.checker
                    .legacy_suppressed_type(DiagnosticCode::InvalidArgumentType, node.text_range())
            })
    }

    pub(in crate::type_check) fn infer_insert_call(&mut self, node: &SyntaxNode) -> TypeId {
        let params = vec![
            builtin_param("IN1", ParamDirection::In),
            builtin_param("IN2", ParamDirection::In),
            builtin_param("P", ParamDirection::In),
        ];
        let call = self.builtin_call(node, params);
        call.check_formal_arg_count(self, node, 3);
        if call.arg_count() != 3 {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        let Some((arg_in1, ty_in1)) = call.arg(0) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        let Some((arg_in2, ty_in2)) = call.arg(1) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        let Some((arg_p, ty_p)) = call.arg(2) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        if ty_in1 == TypeId::UNKNOWN || ty_in2 == TypeId::UNKNOWN || ty_p == TypeId::UNKNOWN {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::CannotResolve, node.text_range());
        }
        if !self.is_string_type(ty_in1) || !self.is_string_type(ty_in2) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg_in1.range,
                "INSERT expects STRING or WSTRING inputs",
            );
        }
        if self.string_kind(ty_in1) != self.string_kind(ty_in2) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg_in2.range,
                "cannot mix STRING and WSTRING",
            );
        }
        if !self.is_integer_type(ty_p) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg_p.range,
                "expected integer position",
            );
        }
        self.base_type_id(ty_in1)
    }

    pub(in crate::type_check) fn infer_delete_call(&mut self, node: &SyntaxNode) -> TypeId {
        let params = vec![
            builtin_param("IN", ParamDirection::In),
            builtin_param("L", ParamDirection::In),
            builtin_param("P", ParamDirection::In),
        ];
        let call = self.builtin_call(node, params);
        call.check_formal_arg_count(self, node, 3);
        if call.arg_count() != 3 {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        let Some((arg_in, ty_in)) = call.arg(0) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        let Some((arg_l, ty_l)) = call.arg(1) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        let Some((arg_p, ty_p)) = call.arg(2) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        if ty_in == TypeId::UNKNOWN || ty_l == TypeId::UNKNOWN || ty_p == TypeId::UNKNOWN {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::CannotResolve, node.text_range());
        }
        if !self.is_string_type(ty_in) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg_in.range,
                "DELETE expects STRING or WSTRING input",
            );
        }
        if !self.is_integer_type(ty_l) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg_l.range,
                "expected integer length",
            );
        }
        if !self.is_integer_type(ty_p) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg_p.range,
                "expected integer position",
            );
        }
        self.base_type_id(ty_in)
    }

    pub(in crate::type_check) fn infer_replace_call(&mut self, node: &SyntaxNode) -> TypeId {
        let params = vec![
            builtin_param("IN1", ParamDirection::In),
            builtin_param("IN2", ParamDirection::In),
            builtin_param("L", ParamDirection::In),
            builtin_param("P", ParamDirection::In),
        ];
        let call = self.builtin_call(node, params);
        call.check_formal_arg_count(self, node, 4);
        if call.arg_count() != 4 {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        let Some((arg_in1, ty_in1)) = call.arg(0) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        let Some((arg_in2, ty_in2)) = call.arg(1) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        let Some((arg_l, ty_l)) = call.arg(2) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        let Some((arg_p, ty_p)) = call.arg(3) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        if ty_in1 == TypeId::UNKNOWN
            || ty_in2 == TypeId::UNKNOWN
            || ty_l == TypeId::UNKNOWN
            || ty_p == TypeId::UNKNOWN
        {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::CannotResolve, node.text_range());
        }
        if !self.is_string_type(ty_in1) || !self.is_string_type(ty_in2) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg_in1.range,
                "REPLACE expects STRING or WSTRING inputs",
            );
        }
        if self.string_kind(ty_in1) != self.string_kind(ty_in2) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg_in2.range,
                "cannot mix STRING and WSTRING",
            );
        }
        if !self.is_integer_type(ty_l) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg_l.range,
                "expected integer length",
            );
        }
        if !self.is_integer_type(ty_p) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg_p.range,
                "expected integer position",
            );
        }
        self.base_type_id(ty_in1)
    }

    pub(in crate::type_check) fn infer_find_call(&mut self, node: &SyntaxNode) -> TypeId {
        let params = vec![
            builtin_param("IN1", ParamDirection::In),
            builtin_param("IN2", ParamDirection::In),
        ];
        let call = self.builtin_call(node, params);
        call.check_formal_arg_count(self, node, 2);
        if call.arg_count() != 2 {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        let Some((arg_in1, ty_in1)) = call.arg(0) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        let Some((arg_in2, ty_in2)) = call.arg(1) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        if ty_in1 == TypeId::UNKNOWN || ty_in2 == TypeId::UNKNOWN {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::CannotResolve, node.text_range());
        }
        if !self.is_string_type(ty_in1) || !self.is_string_type(ty_in2) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg_in1.range,
                "FIND expects STRING or WSTRING inputs",
            );
        }
        if self.string_kind(ty_in1) != self.string_kind(ty_in2) {
            self.checker.diagnostics.error(
                DiagnosticCode::InvalidArgumentType,
                arg_in2.range,
                "cannot mix STRING and WSTRING",
            );
        }
        TypeId::INT
    }
}
