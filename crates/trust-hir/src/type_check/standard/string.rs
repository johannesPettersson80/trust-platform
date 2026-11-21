use super::super::calls::CallArg;
use super::super::*;
use super::helpers::{builtin_in_params, builtin_param, BuiltinCall};

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
        if !call.gives_inputs(2) {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        // The text gives the result its type; a length that a formal call omits is 0.
        let Some((arg_in, ty_in)) = call.arg(0) else {
            return self.untyped_call(node, &name.to_ascii_uppercase(), "IN");
        };
        let length = call.arg(1);
        if ty_in == TypeId::UNKNOWN || unknown(length.as_ref()) {
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
        if let Some(refused) = self.refused_count(length.as_ref(), "length") {
            return refused;
        }
        self.string_value_type(ty_in)
    }

    pub(in crate::type_check) fn infer_mid_call(&mut self, node: &SyntaxNode) -> TypeId {
        self.infer_text_part_call(node, "MID", "MID expects STRING or WSTRING")
    }

    pub(in crate::type_check) fn infer_delete_call(&mut self, node: &SyntaxNode) -> TypeId {
        self.infer_text_part_call(node, "DELETE", "DELETE expects STRING or WSTRING input")
    }

    /// `MID(IN, L, P)` or `DELETE(IN, L, P)`: the text gives the result its type; a length or a
    /// position that a formal call omits is 0.
    fn infer_text_part_call(&mut self, node: &SyntaxNode, name: &str, refusal: &str) -> TypeId {
        let params = vec![
            builtin_param("IN", ParamDirection::In),
            builtin_param("L", ParamDirection::In),
            builtin_param("P", ParamDirection::In),
        ];
        let call = self.builtin_call(node, params);
        if !call.gives_inputs(3) {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        let Some((arg_in, ty_in)) = call.arg(0) else {
            return self.untyped_call(node, name, "IN");
        };
        let (length, position) = (call.arg(1), call.arg(2));
        if ty_in == TypeId::UNKNOWN || unknown(length.as_ref()) || unknown(position.as_ref()) {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::CannotResolve, node.text_range());
        }
        if !self.is_string_type(ty_in) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg_in.range,
                refusal,
            );
        }
        if let Some(refused) = self.refused_count(length.as_ref(), "length") {
            return refused;
        }
        if let Some(refused) = self.refused_count(position.as_ref(), "position") {
            return refused;
        }
        self.string_value_type(ty_in)
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
        if !call.gives_inputs(3) {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        let position = call.arg(2);
        if unknown(call.arg(0).as_ref())
            || unknown(call.arg(1).as_ref())
            || unknown(position.as_ref())
        {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::CannotResolve, node.text_range());
        }
        let Some(text) = self.two_texts(node, &call, "INSERT") else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::InvalidArgumentType, node.text_range());
        };
        if let Some(refused) = self.refused_count(position.as_ref(), "position") {
            return refused;
        }
        text
    }

    pub(in crate::type_check) fn infer_replace_call(&mut self, node: &SyntaxNode) -> TypeId {
        let params = vec![
            builtin_param("IN1", ParamDirection::In),
            builtin_param("IN2", ParamDirection::In),
            builtin_param("L", ParamDirection::In),
            builtin_param("P", ParamDirection::In),
        ];
        let call = self.builtin_call(node, params);
        if !call.gives_inputs(4) {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        let (length, position) = (call.arg(2), call.arg(3));
        if unknown(call.arg(0).as_ref())
            || unknown(call.arg(1).as_ref())
            || unknown(length.as_ref())
            || unknown(position.as_ref())
        {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::CannotResolve, node.text_range());
        }
        let Some(text) = self.two_texts(node, &call, "REPLACE") else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::InvalidArgumentType, node.text_range());
        };
        if let Some(refused) = self.refused_count(length.as_ref(), "length") {
            return refused;
        }
        if let Some(refused) = self.refused_count(position.as_ref(), "position") {
            return refused;
        }
        text
    }

    pub(in crate::type_check) fn infer_find_call(&mut self, node: &SyntaxNode) -> TypeId {
        let params = vec![
            builtin_param("IN1", ParamDirection::In),
            builtin_param("IN2", ParamDirection::In),
        ];
        let call = self.builtin_call(node, params);
        if !call.gives_inputs(2) {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        match self.two_texts(node, &call, "FIND") {
            Some(_) => TypeId::INT,
            None => self
                .checker
                .legacy_suppressed_type(DiagnosticCode::InvalidArgumentType, node.text_range()),
        }
    }

    /// The texts `IN1` and `IN2` of `INSERT`, `REPLACE` or `FIND`: the value type of the one given
    /// first, which an omitted one, the empty text, takes (specification 07 §1). `None` once the
    /// call is refused: no text given, a text that is no string, a `STRING` with a `WSTRING`, or an
    /// unresolved one.
    fn two_texts(&mut self, node: &SyntaxNode, call: &BuiltinCall, name: &str) -> Option<TypeId> {
        let (first, second) = (call.arg(0), call.arg(1));
        let Some((given, given_ty)) = first.clone().or_else(|| second.clone()) else {
            self.untyped_call(node, name, "IN1 or IN2");
            return None;
        };
        if unknown(first.as_ref()) || unknown(second.as_ref()) {
            self.checker
                .legacy_suppressed_type(DiagnosticCode::CannotResolve, node.text_range());
            return None;
        }
        let texts = [first.as_ref(), second.as_ref()];
        if texts
            .into_iter()
            .flatten()
            .any(|(_, ty)| !self.is_string_type(*ty))
        {
            self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                given.range,
                format!("{name} expects STRING or WSTRING inputs"),
            );
            return None;
        }
        if let (Some((_, ty1)), Some((arg2, ty2))) = (&first, &second)
            && self.string_kind(*ty1) != self.string_kind(*ty2)
        {
            if name == "FIND" {
                // FIND's result is an INT whatever its texts are, so the call keeps its type.
                self.checker.diagnostics.error(
                    DiagnosticCode::InvalidArgumentType,
                    arg2.range,
                    "cannot mix STRING and WSTRING",
                );
                return Some(TypeId::INT);
            }
            self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg2.range,
                "cannot mix STRING and WSTRING",
            );
            return None;
        }
        Some(self.string_value_type(given_ty))
    }

    /// Refuses the length or position `input` of a string function, which counts characters,
    /// unless it is an integer; an omitted one, 0, is no refusal.
    fn refused_count(&mut self, input: Option<&(CallArg, TypeId)>, what: &str) -> Option<TypeId> {
        let (arg, ty) = input?;
        if self.is_integer_type(*ty) {
            return None;
        }
        Some(self.checker.legacy_diagnostic_type(
            DiagnosticCode::InvalidArgumentType,
            arg.range,
            format!("expected integer {what}"),
        ))
    }
}

/// Whether `input` is given and its type unresolved, which the call refuses without a word of its
/// own.
fn unknown(input: Option<&(CallArg, TypeId)>) -> bool {
    input.is_some_and(|(_, ty)| *ty == TypeId::UNKNOWN)
}
