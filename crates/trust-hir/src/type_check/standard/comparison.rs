use super::super::*;
use super::helpers::builtin_in_params;

impl<'a, 'b> StandardChecker<'a, 'b> {
    pub(in crate::type_check) fn infer_comparison_call(
        &mut self,
        node: &SyntaxNode,
        name: &str,
    ) -> TypeId {
        // NE compares two inputs, IN1 and IN2, of which a formal call may omit one: it is compared
        // with the default value of the other's type (IEC 61131-3 Ed.3 §6.6.1.4.2).
        if name.eq_ignore_ascii_case("NE") {
            let call = self.builtin_call(node, builtin_in_params("IN", 1, 2));
            if !call.gives_inputs(2) {
                return self
                    .checker
                    .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
            }
            let inputs = call.args_from(0);
            if inputs.is_empty() {
                return self.untyped_call(node, "NE", "IN1 or IN2");
            }
            self.check_comparable_args(&inputs);
            return TypeId::BOOL;
        }
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
        self.check_comparable_args(&inputs);
        TypeId::BOOL
    }
}
