use super::super::*;
use super::helpers::{builtin_in_params, builtin_param};

impl<'a, 'b> StandardChecker<'a, 'b> {
    pub(in crate::type_check) fn infer_time_call(&mut self, node: &SyntaxNode) -> TypeId {
        let call = self.builtin_call(node, Vec::new());
        let has_arg_text = node
            .children()
            .find(|child| child.kind() == SyntaxKind::ArgList)
            .map(|arg_list| {
                let text = arg_list.text().to_string();
                let inner = text
                    .strip_prefix('(')
                    .and_then(|inner| inner.strip_suffix(')'))
                    .unwrap_or(text.as_str());
                !inner.trim().is_empty()
            })
            .unwrap_or(false);
        if call.arg_count() != 0 || has_arg_text {
            let found = call.arg_count().max(usize::from(has_arg_text));
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::WrongArgumentCount,
                node.text_range(),
                format!("expected 0 arguments, found {}", found),
            );
        }
        TypeId::TIME
    }

    pub(in crate::type_check) fn infer_time_named_arith_call(
        &mut self,
        node: &SyntaxNode,
        name: &str,
    ) -> TypeId {
        let (lhs, rhs, result) = match name {
            "ADD_TIME" => (TypeId::TIME, TypeId::TIME, TypeId::TIME),
            "ADD_LTIME" => (TypeId::LTIME, TypeId::LTIME, TypeId::LTIME),
            "ADD_TOD_TIME" => (TypeId::TOD, TypeId::TIME, TypeId::TOD),
            "ADD_LTOD_LTIME" => (TypeId::LTOD, TypeId::LTIME, TypeId::LTOD),
            "ADD_DT_TIME" => (TypeId::DT, TypeId::TIME, TypeId::DT),
            "ADD_LDT_LTIME" => (TypeId::LDT, TypeId::LTIME, TypeId::LDT),
            "SUB_TIME" => (TypeId::TIME, TypeId::TIME, TypeId::TIME),
            "SUB_LTIME" => (TypeId::LTIME, TypeId::LTIME, TypeId::LTIME),
            "SUB_DATE_DATE" => (TypeId::DATE, TypeId::DATE, TypeId::TIME),
            "SUB_LDATE_LDATE" => (TypeId::LDATE, TypeId::LDATE, TypeId::LTIME),
            "SUB_TOD_TIME" => (TypeId::TOD, TypeId::TIME, TypeId::TOD),
            "SUB_LTOD_LTIME" => (TypeId::LTOD, TypeId::LTIME, TypeId::LTOD),
            "SUB_TOD_TOD" => (TypeId::TOD, TypeId::TOD, TypeId::TIME),
            "SUB_LTOD_LTOD" => (TypeId::LTOD, TypeId::LTOD, TypeId::LTIME),
            "SUB_DT_TIME" => (TypeId::DT, TypeId::TIME, TypeId::DT),
            "SUB_LDT_LTIME" => (TypeId::LDT, TypeId::LTIME, TypeId::LDT),
            "SUB_DT_DT" => (TypeId::DT, TypeId::DT, TypeId::TIME),
            "SUB_LDT_LDT" => (TypeId::LDT, TypeId::LDT, TypeId::LTIME),
            _ => {
                return self
                    .checker
                    .legacy_suppressed_type(DiagnosticCode::CannotResolve, node.text_range());
            }
        };

        let params = builtin_in_params("IN", 1, 2);
        let call = self.builtin_call(node, params);
        call.check_formal_arg_count(self, node, 2);
        if call.arg_count() != 2 {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        let Some((arg1, ty1)) = call.arg(0) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        let Some((arg2, ty2)) = call.arg(1) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        if ty1 == TypeId::UNKNOWN || ty2 == TypeId::UNKNOWN {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::CannotResolve, node.text_range());
        }
        if !self.checker.is_assignable(lhs, ty1) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg1.range,
                format!("expected '{}'", self.checker.type_name(lhs)),
            );
        }
        if !self.checker.is_assignable(rhs, ty2) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg2.range,
                format!("expected '{}'", self.checker.type_name(rhs)),
            );
        }
        result
    }

    pub(in crate::type_check) fn infer_time_named_mul_div_call(
        &mut self,
        node: &SyntaxNode,
        name: &str,
    ) -> TypeId {
        let (time_type, result) = match name {
            "MUL_TIME" | "DIV_TIME" => (TypeId::TIME, TypeId::TIME),
            "MUL_LTIME" | "DIV_LTIME" => (TypeId::LTIME, TypeId::LTIME),
            _ => {
                return self
                    .checker
                    .legacy_suppressed_type(DiagnosticCode::CannotResolve, node.text_range());
            }
        };

        let params = builtin_in_params("IN", 1, 2);
        let call = self.builtin_call(node, params);
        call.check_formal_arg_count(self, node, 2);
        if call.arg_count() != 2 {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        let Some((arg1, ty1)) = call.arg(0) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        let Some((arg2, ty2)) = call.arg(1) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        if ty1 == TypeId::UNKNOWN || ty2 == TypeId::UNKNOWN {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::CannotResolve, node.text_range());
        }
        if !self.checker.is_assignable(time_type, ty1) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg1.range,
                format!("expected '{}'", self.checker.type_name(time_type)),
            );
        }
        if !self.is_numeric_type(ty2) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg2.range,
                "expected numeric factor",
            );
        }
        result
    }

    pub(in crate::type_check) fn infer_concat_date_time_call(
        &mut self,
        node: &SyntaxNode,
        name: &str,
    ) -> TypeId {
        let (params, expected_types, result) = match name {
            "CONCAT_DATE_TOD" => (
                vec![
                    builtin_param("DATE", ParamDirection::In),
                    builtin_param("TOD", ParamDirection::In),
                ],
                vec![TypeId::DATE, TypeId::TOD],
                TypeId::DT,
            ),
            "CONCAT_DATE_LTOD" => (
                vec![
                    builtin_param("DATE", ParamDirection::In),
                    builtin_param("LTOD", ParamDirection::In),
                ],
                vec![TypeId::DATE, TypeId::LTOD],
                TypeId::LDT,
            ),
            "CONCAT_DATE" => (
                vec![
                    builtin_param("YEAR", ParamDirection::In),
                    builtin_param("MONTH", ParamDirection::In),
                    builtin_param("DAY", ParamDirection::In),
                ],
                vec![TypeId::ANY_INT, TypeId::ANY_INT, TypeId::ANY_INT],
                TypeId::DATE,
            ),
            "CONCAT_TOD" => (
                vec![
                    builtin_param("HOUR", ParamDirection::In),
                    builtin_param("MINUTE", ParamDirection::In),
                    builtin_param("SECOND", ParamDirection::In),
                    builtin_param("MILLISECOND", ParamDirection::In),
                ],
                vec![
                    TypeId::ANY_INT,
                    TypeId::ANY_INT,
                    TypeId::ANY_INT,
                    TypeId::ANY_INT,
                ],
                TypeId::TOD,
            ),
            "CONCAT_LTOD" => (
                vec![
                    builtin_param("HOUR", ParamDirection::In),
                    builtin_param("MINUTE", ParamDirection::In),
                    builtin_param("SECOND", ParamDirection::In),
                    builtin_param("MILLISECOND", ParamDirection::In),
                ],
                vec![
                    TypeId::ANY_INT,
                    TypeId::ANY_INT,
                    TypeId::ANY_INT,
                    TypeId::ANY_INT,
                ],
                TypeId::LTOD,
            ),
            "CONCAT_DT" => (
                vec![
                    builtin_param("YEAR", ParamDirection::In),
                    builtin_param("MONTH", ParamDirection::In),
                    builtin_param("DAY", ParamDirection::In),
                    builtin_param("HOUR", ParamDirection::In),
                    builtin_param("MINUTE", ParamDirection::In),
                    builtin_param("SECOND", ParamDirection::In),
                    builtin_param("MILLISECOND", ParamDirection::In),
                ],
                vec![
                    TypeId::ANY_INT,
                    TypeId::ANY_INT,
                    TypeId::ANY_INT,
                    TypeId::ANY_INT,
                    TypeId::ANY_INT,
                    TypeId::ANY_INT,
                    TypeId::ANY_INT,
                ],
                TypeId::DT,
            ),
            "CONCAT_LDT" => (
                vec![
                    builtin_param("YEAR", ParamDirection::In),
                    builtin_param("MONTH", ParamDirection::In),
                    builtin_param("DAY", ParamDirection::In),
                    builtin_param("HOUR", ParamDirection::In),
                    builtin_param("MINUTE", ParamDirection::In),
                    builtin_param("SECOND", ParamDirection::In),
                    builtin_param("MILLISECOND", ParamDirection::In),
                ],
                vec![
                    TypeId::ANY_INT,
                    TypeId::ANY_INT,
                    TypeId::ANY_INT,
                    TypeId::ANY_INT,
                    TypeId::ANY_INT,
                    TypeId::ANY_INT,
                    TypeId::ANY_INT,
                ],
                TypeId::LDT,
            ),
            _ => {
                return self
                    .checker
                    .legacy_suppressed_type(DiagnosticCode::CannotResolve, node.text_range());
            }
        };

        let call = self.builtin_call(node, params);
        call.check_formal_arg_count(self, node, expected_types.len());
        if call.arg_count() != expected_types.len() {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        for (index, expected) in expected_types.iter().enumerate() {
            let Some((arg, ty)) = call.arg(index) else {
                return self
                    .checker
                    .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
            };
            if ty == TypeId::UNKNOWN {
                return self
                    .checker
                    .legacy_suppressed_type(DiagnosticCode::CannotResolve, arg.range);
            }
            if *expected == TypeId::ANY_INT {
                if !self.is_integer_type(ty) {
                    return self.checker.legacy_diagnostic_type(
                        DiagnosticCode::InvalidArgumentType,
                        arg.range,
                        "expected integer component",
                    );
                }
            } else if !self.checker.is_assignable(*expected, ty) {
                return self.checker.legacy_diagnostic_type(
                    DiagnosticCode::InvalidArgumentType,
                    arg.range,
                    format!("expected '{}'", self.checker.type_name(*expected)),
                );
            }
        }
        result
    }

    pub(in crate::type_check) fn infer_split_date_time_call(
        &mut self,
        node: &SyntaxNode,
        name: &str,
    ) -> TypeId {
        let (params, input_type, outputs) = match name {
            "SPLIT_DATE" => (
                vec![
                    builtin_param("IN", ParamDirection::In),
                    builtin_param("YEAR", ParamDirection::Out),
                    builtin_param("MONTH", ParamDirection::Out),
                    builtin_param("DAY", ParamDirection::Out),
                ],
                TypeId::DATE,
                3,
            ),
            "SPLIT_TOD" => (
                vec![
                    builtin_param("IN", ParamDirection::In),
                    builtin_param("HOUR", ParamDirection::Out),
                    builtin_param("MINUTE", ParamDirection::Out),
                    builtin_param("SECOND", ParamDirection::Out),
                    builtin_param("MILLISECOND", ParamDirection::Out),
                ],
                TypeId::TOD,
                4,
            ),
            "SPLIT_LTOD" => (
                vec![
                    builtin_param("IN", ParamDirection::In),
                    builtin_param("HOUR", ParamDirection::Out),
                    builtin_param("MINUTE", ParamDirection::Out),
                    builtin_param("SECOND", ParamDirection::Out),
                    builtin_param("MILLISECOND", ParamDirection::Out),
                ],
                TypeId::LTOD,
                4,
            ),
            "SPLIT_DT" => (
                vec![
                    builtin_param("IN", ParamDirection::In),
                    builtin_param("YEAR", ParamDirection::Out),
                    builtin_param("MONTH", ParamDirection::Out),
                    builtin_param("DAY", ParamDirection::Out),
                    builtin_param("HOUR", ParamDirection::Out),
                    builtin_param("MINUTE", ParamDirection::Out),
                    builtin_param("SECOND", ParamDirection::Out),
                    builtin_param("MILLISECOND", ParamDirection::Out),
                ],
                TypeId::DT,
                7,
            ),
            "SPLIT_LDT" => (
                vec![
                    builtin_param("IN", ParamDirection::In),
                    builtin_param("YEAR", ParamDirection::Out),
                    builtin_param("MONTH", ParamDirection::Out),
                    builtin_param("DAY", ParamDirection::Out),
                    builtin_param("HOUR", ParamDirection::Out),
                    builtin_param("MINUTE", ParamDirection::Out),
                    builtin_param("SECOND", ParamDirection::Out),
                    builtin_param("MILLISECOND", ParamDirection::Out),
                ],
                TypeId::LDT,
                7,
            ),
            _ => {
                return self
                    .checker
                    .legacy_suppressed_type(DiagnosticCode::CannotResolve, node.text_range());
            }
        };

        let call = self.builtin_call(node, params);
        call.check_formal_arg_count(self, node, outputs + 1);
        if call.arg_count() != outputs + 1 {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        }
        let Some((arg_in, ty_in)) = call.arg(0) else {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::WrongArgumentCount, node.text_range());
        };
        if ty_in == TypeId::UNKNOWN {
            return self
                .checker
                .legacy_suppressed_type(DiagnosticCode::CannotResolve, arg_in.range);
        }
        if !self.checker.is_assignable(input_type, ty_in) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg_in.range,
                format!("expected '{}'", self.checker.type_name(input_type)),
            );
        }
        for (arg, ty) in call.args_from(1) {
            if ty == TypeId::UNKNOWN {
                return self
                    .checker
                    .legacy_suppressed_type(DiagnosticCode::CannotResolve, arg.range);
            }
            if !self.is_integer_type(ty) {
                self.checker.diagnostics.error(
                    DiagnosticCode::InvalidArgumentType,
                    arg.range,
                    "expected integer output",
                );
            }
        }
        TypeId::VOID
    }

    pub(in crate::type_check) fn infer_day_of_week_call(&mut self, node: &SyntaxNode) -> TypeId {
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
        if !self.checker.is_assignable(TypeId::DATE, ty) {
            return self.checker.legacy_diagnostic_type(
                DiagnosticCode::InvalidArgumentType,
                arg.range,
                "expected DATE input",
            );
        }
        TypeId::INT
    }

    pub(in crate::type_check) fn time_add_result(
        &self,
        lhs: TypeId,
        rhs: TypeId,
    ) -> Option<TypeId> {
        let lhs = self.base_type_id(lhs);
        let rhs = self.base_type_id(rhs);
        match (lhs, rhs) {
            (TypeId::TIME, TypeId::TIME) => Some(TypeId::TIME),
            (TypeId::LTIME, TypeId::LTIME) => Some(TypeId::LTIME),
            (TypeId::TOD, TypeId::TIME) | (TypeId::TIME, TypeId::TOD) => Some(TypeId::TOD),
            (TypeId::LTOD, TypeId::LTIME) | (TypeId::LTIME, TypeId::LTOD) => Some(TypeId::LTOD),
            (TypeId::DT, TypeId::TIME) | (TypeId::TIME, TypeId::DT) => Some(TypeId::DT),
            (TypeId::LDT, TypeId::LTIME) | (TypeId::LTIME, TypeId::LDT) => Some(TypeId::LDT),
            _ => None,
        }
    }

    pub(in crate::type_check) fn time_sub_result(
        &self,
        lhs: TypeId,
        rhs: TypeId,
    ) -> Option<TypeId> {
        let lhs = self.base_type_id(lhs);
        let rhs = self.base_type_id(rhs);
        match (lhs, rhs) {
            (TypeId::TIME, TypeId::TIME) => Some(TypeId::TIME),
            (TypeId::LTIME, TypeId::LTIME) => Some(TypeId::LTIME),
            (TypeId::TOD, TypeId::TIME) => Some(TypeId::TOD),
            (TypeId::LTOD, TypeId::LTIME) => Some(TypeId::LTOD),
            (TypeId::TOD, TypeId::TOD) => Some(TypeId::TIME),
            (TypeId::LTOD, TypeId::LTOD) => Some(TypeId::LTIME),
            (TypeId::DT, TypeId::TIME) => Some(TypeId::DT),
            (TypeId::LDT, TypeId::LTIME) => Some(TypeId::LDT),
            (TypeId::DT, TypeId::DT) => Some(TypeId::TIME),
            (TypeId::LDT, TypeId::LDT) => Some(TypeId::LTIME),
            (TypeId::DATE, TypeId::DATE) => Some(TypeId::TIME),
            (TypeId::LDATE, TypeId::LDATE) => Some(TypeId::LTIME),
            _ => None,
        }
    }

    pub(in crate::type_check) fn time_mul_result(
        &self,
        lhs: TypeId,
        rhs: TypeId,
    ) -> Option<TypeId> {
        let lhs = self.base_type_id(lhs);
        let rhs = self.base_type_id(rhs);
        match (lhs, rhs) {
            (TypeId::TIME, _) if self.is_numeric_type(rhs) => Some(TypeId::TIME),
            (_, TypeId::TIME) if self.is_numeric_type(lhs) => Some(TypeId::TIME),
            (TypeId::LTIME, _) if self.is_numeric_type(rhs) => Some(TypeId::LTIME),
            (_, TypeId::LTIME) if self.is_numeric_type(lhs) => Some(TypeId::LTIME),
            _ => None,
        }
    }

    pub(in crate::type_check) fn time_div_result(
        &self,
        lhs: TypeId,
        rhs: TypeId,
    ) -> Option<TypeId> {
        let lhs = self.base_type_id(lhs);
        let rhs = self.base_type_id(rhs);
        match (lhs, rhs) {
            (TypeId::TIME, _) if self.is_numeric_type(rhs) => Some(TypeId::TIME),
            (TypeId::LTIME, _) if self.is_numeric_type(rhs) => Some(TypeId::LTIME),
            _ => None,
        }
    }
}
