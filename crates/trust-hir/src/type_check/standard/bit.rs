use super::super::literals::exact_int_literal_value;
use super::super::*;
use super::helpers::{builtin_in_params, builtin_param};

impl<'a, 'b> StandardChecker<'a, 'b> {
    pub(in crate::type_check) fn infer_bit_shift_call(
        &mut self,
        node: &SyntaxNode,
        name: &str,
    ) -> TypeId {
        let params = vec![
            builtin_param("IN", ParamDirection::In),
            builtin_param("N", ParamDirection::In),
        ];
        let call = self.builtin_call(node, params);
        if !call.gives_inputs(2) {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        // The value gives the result its type; a count a formal call omits is 0.
        let Some((arg_in, ty_in)) = call.arg(0) else {
            return self.untyped_call(node, &name.to_ascii_uppercase(), "IN");
        };
        let count = call.arg(1);
        if ty_in == TypeId::UNKNOWN || count.as_ref().is_some_and(|(_, ty)| *ty == TypeId::UNKNOWN)
        {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::CannotResolve, node.text_range());
        }
        // An untyped literal is a bit string literal here (IEC 61131-3 Ed.3 §6.3.2): the call is
        // open, and takes the type where it goes (`bit_literals`).
        let open = self.checker.open_bit_input_type(&arg_in.expr);
        let ty_in = match open {
            Some(least) => least,
            None if self.is_bit_string_type(ty_in) => ty_in,
            None => {
                return self.checker.legacy_diagnostic_type(
                    DiagnosticCode::InvalidArgumentType,
                    arg_in.range,
                    bit_string_input_refusal(&arg_in.expr),
                );
            }
        };
        if let Some((arg_n, ty_n)) = count
            && !self.checker.is_assignable(TypeId::ANY_INT, ty_n)
        {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg_n.range,
                "expected integer shift count",
            );
        }
        if let Some(least) = open {
            self.checker
                .record_open_bit_call(node, vec![arg_in.expr.clone()], least);
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
        // Inputs that are all untyped literals (or open calls) leave the call open; it takes the bit
        // string type where it goes (`bit_literals`). A typed input gives the literals its type.
        let open: Option<Vec<TypeId>> = inputs
            .iter()
            .map(|(arg, _)| self.checker.open_bit_input_type(&arg.expr))
            .collect();
        if let Some(open) = open {
            let least = open
                .into_iter()
                .max_by_key(|ty| {
                    self.checker
                        .resolved_type(*ty)
                        .and_then(Type::bit_size)
                        .unwrap_or(0)
                })
                .unwrap_or(TypeId::BYTE);
            let exprs = inputs.iter().map(|(arg, _)| arg.expr.clone()).collect();
            self.checker.record_open_bit_call(node, exprs, least);
            return least;
        }
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

/// The refusal of `input` where a bit string goes: a negative literal is named, as IEC 61131-3 Ed.3
/// §6.3.2 makes only a based number or a positive one a bit string literal.
fn bit_string_input_refusal(input: &SyntaxNode) -> &'static str {
    if exact_int_literal_value(input).is_some_and(|value| value < 0) {
        "expected bit string input: a negative literal is no bit string literal"
    } else {
        "expected bit string input"
    }
}
