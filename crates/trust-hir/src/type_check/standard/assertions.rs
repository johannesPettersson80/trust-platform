use super::super::*;
use super::helpers::builtin_param;

impl<'a, 'b> StandardChecker<'a, 'b> {
    pub(in crate::type_check) fn infer_assert_true_call(&mut self, node: &SyntaxNode) -> TypeId {
        self.infer_assert_bool_call(node, "ASSERT_TRUE")
    }

    pub(in crate::type_check) fn infer_assert_false_call(&mut self, node: &SyntaxNode) -> TypeId {
        self.infer_assert_bool_call(node, "ASSERT_FALSE")
    }

    pub(in crate::type_check) fn infer_assert_equal_call(&mut self, node: &SyntaxNode) -> TypeId {
        self.infer_assert_comparable_call(node, "EXPECTED", "ACTUAL")
    }

    pub(in crate::type_check) fn infer_assert_not_equal_call(
        &mut self,
        node: &SyntaxNode,
    ) -> TypeId {
        self.infer_assert_comparable_call(node, "EXPECTED", "ACTUAL")
    }

    pub(in crate::type_check) fn infer_assert_greater_call(&mut self, node: &SyntaxNode) -> TypeId {
        self.infer_assert_comparable_call(node, "VALUE", "BOUND")
    }

    pub(in crate::type_check) fn infer_assert_less_call(&mut self, node: &SyntaxNode) -> TypeId {
        self.infer_assert_comparable_call(node, "VALUE", "BOUND")
    }

    pub(in crate::type_check) fn infer_assert_greater_or_equal_call(
        &mut self,
        node: &SyntaxNode,
    ) -> TypeId {
        self.infer_assert_comparable_call(node, "VALUE", "BOUND")
    }

    pub(in crate::type_check) fn infer_assert_less_or_equal_call(
        &mut self,
        node: &SyntaxNode,
    ) -> TypeId {
        self.infer_assert_comparable_call(node, "VALUE", "BOUND")
    }

    pub(in crate::type_check) fn infer_assert_near_call(&mut self, node: &SyntaxNode) -> TypeId {
        let params = vec![
            builtin_param("EXPECTED", ParamDirection::In),
            builtin_param("ACTUAL", ParamDirection::In),
            builtin_param("DELTA", ParamDirection::In),
        ];
        let call = self.builtin_call(node, params);
        call.check_formal_arg_count(self, node, 3);
        if call.arg_count() != 3 {
            return TypeId::UNKNOWN;
        }
        let inputs = call.args_from(0);
        if inputs.len() != 3 {
            return TypeId::UNKNOWN;
        }
        self.common_numeric_type_for_args(&inputs)
            .map(|_| TypeId::VOID)
            .unwrap_or(TypeId::UNKNOWN)
    }

    fn infer_assert_bool_call(&mut self, node: &SyntaxNode, name: &str) -> TypeId {
        let params = vec![builtin_param("IN", ParamDirection::In)];
        let call = self.builtin_call(node, params);
        call.check_formal_arg_count(self, node, 1);
        if call.arg_count() != 1 {
            return TypeId::UNKNOWN;
        }
        let Some((arg, arg_type)) = call.arg(0) else {
            return TypeId::UNKNOWN;
        };
        if !self.checker.is_assignable(TypeId::BOOL, arg_type) {
            self.checker.diagnostics.error(
                DiagnosticCode::InvalidArgumentType,
                arg.range,
                format!("{name} expects BOOL input"),
            );
            return TypeId::UNKNOWN;
        }
        TypeId::VOID
    }

    fn infer_assert_comparable_call(
        &mut self,
        node: &SyntaxNode,
        left_name: &str,
        right_name: &str,
    ) -> TypeId {
        let params = vec![
            builtin_param(left_name, ParamDirection::In),
            builtin_param(right_name, ParamDirection::In),
        ];
        let call = self.builtin_call(node, params);
        call.check_formal_arg_count(self, node, 2);
        if call.arg_count() != 2 {
            return TypeId::UNKNOWN;
        }
        let inputs = call.args_from(0);
        if inputs.len() != 2 {
            return TypeId::UNKNOWN;
        }
        if !self.check_comparable_args(&inputs) {
            return TypeId::UNKNOWN;
        }
        TypeId::VOID
    }
}
