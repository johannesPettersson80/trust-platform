use super::super::*;
use super::helpers::builtin_in_params;

impl<'a, 'b> StandardChecker<'a, 'b> {
    pub(in crate::type_check) fn infer_comparison_call(
        &mut self,
        node: &SyntaxNode,
        name: &str,
    ) -> TypeId {
        let arg_count = self.checker.calls().collect_call_args(node).len();
        if arg_count < 2 {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::WrongArgumentCount,
                node.text_range(),
                format!("expected at least 2 arguments, found {}", arg_count),
            );
        }
        if name.eq_ignore_ascii_case("NE") && arg_count != 2 {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::WrongArgumentCount,
                node.text_range(),
                format!("expected 2 arguments, found {}", arg_count),
            );
        }
        let params = builtin_in_params("IN", 1, arg_count);
        let call = self.builtin_call(node, params);
        let inputs = call.args_from(0);
        self.check_comparable_args(&inputs);
        TypeId::BOOL
    }
}
