use super::super::*;
use super::helpers::{builtin_in_params, builtin_param};

impl<'a, 'b> StandardChecker<'a, 'b> {
    pub(in crate::type_check) fn infer_unary_numeric_call(&mut self, node: &SyntaxNode) -> TypeId {
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
        if !self.is_numeric_type(arg_type) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg.range,
                "expected numeric type",
            );
        }
        self.base_type_id(arg_type)
    }

    pub(in crate::type_check) fn infer_unary_real_call(&mut self, node: &SyntaxNode) -> TypeId {
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
        if self.base_type_id(arg_type) == TypeId::LREAL {
            TypeId::LREAL
        } else {
            TypeId::REAL
        }
    }

    pub(in crate::type_check) fn infer_atan2_call(&mut self, node: &SyntaxNode) -> TypeId {
        let params = vec![
            builtin_param("Y", ParamDirection::In),
            builtin_param("X", ParamDirection::In),
        ];
        let call = self.builtin_call(node, params);
        call.check_formal_arg_count(self, node, 2);
        if call.arg_count() != 2 {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        let inputs = call.args_from(0);
        self.common_real_type_for_args(&inputs).unwrap_or_else(|| {
            self.checker
                .legacy_suppressed_type(DiagnosticCode::InvalidArgumentType, node.text_range())
        })
    }

    pub(in crate::type_check) fn infer_add_call(&mut self, node: &SyntaxNode) -> TypeId {
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
        if inputs.len() != arg_count {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }

        if inputs.iter().any(|(_, ty)| self.is_time_related_type(*ty)) {
            if arg_count != 2 {
                return self.checker.legacy_diagnostic_type(
                    DiagnosticCode::WrongArgumentCount,
                    node.text_range(),
                    format!("expected 2 arguments, found {}", arg_count),
                );
            }
            let lhs = inputs[0].1;
            let rhs = inputs[1].1;
            if let Some(result) = self.time_add_result(lhs, rhs) {
                return result;
            }
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                node.text_range(),
                "invalid time/date operands for ADD",
            );
        }

        self.common_numeric_type_for_args(&inputs)
            .unwrap_or_else(|| {
                self.checker
                    .legacy_suppressed_type(DiagnosticCode::InvalidArgumentType, node.text_range())
            })
    }

    pub(in crate::type_check) fn infer_sub_call(&mut self, node: &SyntaxNode) -> TypeId {
        let params = builtin_in_params("IN", 1, 2);
        let call = self.builtin_call(node, params);
        call.check_formal_arg_count(self, node, 2);
        if call.arg_count() != 2 {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        let inputs = call.args_from(0);
        if inputs.len() != 2 {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }

        if inputs.iter().any(|(_, ty)| self.is_time_related_type(*ty)) {
            let lhs = inputs[0].1;
            let rhs = inputs[1].1;
            if let Some(result) = self.time_sub_result(lhs, rhs) {
                return result;
            }
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                node.text_range(),
                "invalid time/date operands for SUB",
            );
        }

        self.common_numeric_type_for_args(&inputs)
            .unwrap_or_else(|| {
                self.checker
                    .legacy_suppressed_type(DiagnosticCode::InvalidArgumentType, node.text_range())
            })
    }

    pub(in crate::type_check) fn infer_mul_call(&mut self, node: &SyntaxNode) -> TypeId {
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
        if inputs.len() != arg_count {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }

        if inputs.iter().any(|(_, ty)| self.is_time_duration_type(*ty)) {
            if arg_count != 2 {
                return self.checker.legacy_diagnostic_type(
                    DiagnosticCode::WrongArgumentCount,
                    node.text_range(),
                    format!("expected 2 arguments, found {}", arg_count),
                );
            }
            let lhs = inputs[0].1;
            let rhs = inputs[1].1;
            if let Some(result) = self.time_mul_result(lhs, rhs) {
                return result;
            }
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                node.text_range(),
                "invalid operands for MUL",
            );
        }

        self.common_numeric_type_for_args(&inputs)
            .unwrap_or_else(|| {
                self.checker
                    .legacy_suppressed_type(DiagnosticCode::InvalidArgumentType, node.text_range())
            })
    }

    pub(in crate::type_check) fn infer_div_call(&mut self, node: &SyntaxNode) -> TypeId {
        let params = builtin_in_params("IN", 1, 2);
        let call = self.builtin_call(node, params);
        call.check_formal_arg_count(self, node, 2);
        if call.arg_count() != 2 {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        let inputs = call.args_from(0);
        if inputs.len() != 2 {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }

        if inputs.iter().any(|(_, ty)| self.is_time_duration_type(*ty)) {
            let lhs = inputs[0].1;
            let rhs = inputs[1].1;
            if let Some(result) = self.time_div_result(lhs, rhs) {
                return result;
            }
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                node.text_range(),
                "invalid operands for DIV",
            );
        }

        self.common_numeric_type_for_args(&inputs)
            .unwrap_or_else(|| {
                self.checker
                    .legacy_suppressed_type(DiagnosticCode::InvalidArgumentType, node.text_range())
            })
    }

    pub(in crate::type_check) fn infer_mod_call(&mut self, node: &SyntaxNode) -> TypeId {
        let params = builtin_in_params("IN", 1, 2);
        let call = self.builtin_call(node, params);
        call.check_formal_arg_count(self, node, 2);
        if call.arg_count() != 2 {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        let inputs = call.args_from(0);
        if inputs.len() != 2 {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        self.common_integer_type_for_args(&inputs)
            .unwrap_or_else(|| {
                self.checker
                    .legacy_suppressed_type(DiagnosticCode::InvalidArgumentType, node.text_range())
            })
    }

    pub(in crate::type_check) fn infer_expt_call(&mut self, node: &SyntaxNode) -> TypeId {
        let params = builtin_in_params("IN", 1, 2);
        let call = self.builtin_call(node, params);
        call.check_formal_arg_count(self, node, 2);
        if call.arg_count() != 2 {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        let inputs = call.args_from(0);
        if inputs.len() != 2 {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        let (arg1, ty1) = &inputs[0];
        let (arg2, ty2) = &inputs[1];
        if *ty1 == TypeId::UNKNOWN || *ty2 == TypeId::UNKNOWN {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::CannotResolve, node.text_range());
        }
        if !self.is_real_type(*ty1) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg1.range,
                "expected REAL or LREAL input",
            );
        }
        if !self.is_numeric_type(*ty2) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg2.range,
                "expected numeric exponent",
            );
        }
        if self.base_type_id(*ty1) == TypeId::LREAL {
            TypeId::LREAL
        } else {
            TypeId::REAL
        }
    }

    pub(in crate::type_check) fn infer_move_call(&mut self, node: &SyntaxNode) -> TypeId {
        let params = vec![builtin_param("IN", ParamDirection::In)];
        let call = self.builtin_call(node, params);
        call.check_formal_arg_count(self, node, 1);
        if call.arg_count() != 1 {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        let Some((_, arg_type)) = call.arg(0) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        self.base_type_id(arg_type)
    }
}
