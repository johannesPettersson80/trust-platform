impl<'a, 'b> StandardChecker<'a, 'b> {
    pub(in crate::type_check) fn infer_conversion_function_call(
        &mut self,
        name: &str,
        node: &SyntaxNode,
    ) -> Option<TypeId> {
        let upper = name;

        if upper.eq_ignore_ascii_case("TRUNC") {
            let Some((arg, arg_type)) = self.collect_single_conversion_arg(node) else {
                return Some(TypeId::UNKNOWN);
            };
            if !self.is_real_type(arg_type) {
                self.checker.diagnostics.error(
                    DiagnosticCode::InvalidArgumentType,
                    arg.range,
                    "expected REAL or LREAL input",
                );
                return Some(TypeId::UNKNOWN);
            }
            return Some(TypeId::DINT);
        }

        if let Some(dst_name) = upper.strip_prefix("TRUNC_") {
            let dst = TypeId::from_builtin_name(dst_name)?;
            let Some((arg, arg_type)) = self.collect_single_conversion_arg(node) else {
                return Some(TypeId::UNKNOWN);
            };
            if !self.is_real_type(arg_type) {
                self.checker.diagnostics.error(
                    DiagnosticCode::InvalidArgumentType,
                    arg.range,
                    "expected REAL or LREAL input",
                );
                return Some(TypeId::UNKNOWN);
            }
            if !self.is_integer_type(dst) {
                self.checker.diagnostics.error(
                    DiagnosticCode::InvalidArgumentType,
                    arg.range,
                    format!("invalid TRUNC target '{}'", self.checker.type_name(dst)),
                );
                return Some(TypeId::UNKNOWN);
            }
            return Some(dst);
        }

        if let Some((src_name, dst_name)) = upper.split_once("_TRUNC_") {
            let src = TypeId::from_builtin_name(src_name)?;
            let dst = TypeId::from_builtin_name(dst_name)?;
            let Some((arg, arg_type)) = self.collect_single_conversion_arg(node) else {
                return Some(TypeId::UNKNOWN);
            };
            if !self.expect_assignable_in_param(src, &arg, arg_type) {
                return Some(TypeId::UNKNOWN);
            }
            if !self.is_real_type(src) || !self.is_integer_type(dst) {
                self.checker.diagnostics.error(
                    DiagnosticCode::InvalidArgumentType,
                    arg.range,
                    "invalid TRUNC conversion",
                );
                return Some(TypeId::UNKNOWN);
            }
            return Some(dst);
        }

        if let Some(dst_name) = upper.strip_prefix("TO_BCD_") {
            let dst = TypeId::from_builtin_name(dst_name)?;
            let Some((arg, arg_type)) = self.collect_single_conversion_arg(node) else {
                return Some(TypeId::UNKNOWN);
            };
            if !self.is_unsigned_int_type(arg_type) {
                self.checker.diagnostics.error(
                    DiagnosticCode::InvalidArgumentType,
                    arg.range,
                    "expected unsigned integer input",
                );
                return Some(TypeId::UNKNOWN);
            }
            if !self.is_bit_string_type(dst) || dst == TypeId::BOOL {
                self.checker.diagnostics.error(
                    DiagnosticCode::InvalidArgumentType,
                    arg.range,
                    format!("invalid BCD target '{}'", self.checker.type_name(dst)),
                );
                return Some(TypeId::UNKNOWN);
            }
            return Some(dst);
        }

        if let Some((dst_name, src_name)) = upper.split_once("_TO_BCD_") {
            let dst = TypeId::from_builtin_name(dst_name)?;
            let src = TypeId::from_builtin_name(src_name)?;
            let Some((arg, arg_type)) = self.collect_single_conversion_arg(node) else {
                return Some(TypeId::UNKNOWN);
            };
            if !self.expect_assignable_in_param(dst, &arg, arg_type) {
                return Some(TypeId::UNKNOWN);
            }
            if !self.is_unsigned_int_type(dst)
                || !self.is_bit_string_type(src)
                || src == TypeId::BOOL
            {
                self.checker.diagnostics.error(
                    DiagnosticCode::InvalidArgumentType,
                    arg.range,
                    "invalid BCD conversion",
                );
                return Some(TypeId::UNKNOWN);
            }
            return Some(src);
        }

        if let Some(dst_name) = upper.strip_prefix("BCD_TO_") {
            let dst = TypeId::from_builtin_name(dst_name)?;
            let Some((arg, arg_type)) = self.collect_single_conversion_arg(node) else {
                return Some(TypeId::UNKNOWN);
            };
            if !self.is_bit_string_type(arg_type) || arg_type == TypeId::BOOL {
                self.checker.diagnostics.error(
                    DiagnosticCode::InvalidArgumentType,
                    arg.range,
                    "expected BYTE/WORD/DWORD/LWORD input",
                );
                return Some(TypeId::UNKNOWN);
            }
            if !self.is_unsigned_int_type(dst) {
                self.checker.diagnostics.error(
                    DiagnosticCode::InvalidArgumentType,
                    arg.range,
                    format!("invalid BCD target '{}'", self.checker.type_name(dst)),
                );
                return Some(TypeId::UNKNOWN);
            }
            return Some(dst);
        }

        if let Some((src_name, dst_name)) = upper.split_once("_BCD_TO_") {
            let src = TypeId::from_builtin_name(src_name)?;
            let dst = TypeId::from_builtin_name(dst_name)?;
            let Some((arg, arg_type)) = self.collect_single_conversion_arg(node) else {
                return Some(TypeId::UNKNOWN);
            };
            if !self.expect_assignable_in_param(src, &arg, arg_type) {
                return Some(TypeId::UNKNOWN);
            }
            if !self.is_bit_string_type(src)
                || src == TypeId::BOOL
                || !self.is_unsigned_int_type(dst)
            {
                self.checker.diagnostics.error(
                    DiagnosticCode::InvalidArgumentType,
                    arg.range,
                    "invalid BCD conversion",
                );
                return Some(TypeId::UNKNOWN);
            }
            return Some(dst);
        }

        if let Some(dst_name) = upper.strip_prefix("TO_") {
            let dst = TypeId::from_builtin_name(dst_name)?;
            let Some((arg, arg_type)) = self.collect_single_conversion_arg(node) else {
                return Some(TypeId::UNKNOWN);
            };
            if !self.is_conversion_allowed(arg_type, dst) {
                self.checker.diagnostics.error(
                    DiagnosticCode::InvalidArgumentType,
                    arg.range,
                    format!(
                        "cannot convert '{}' to '{}'",
                        self.checker.type_name(arg_type),
                        self.checker.type_name(dst)
                    ),
                );
                return Some(TypeId::UNKNOWN);
            }
            return Some(dst);
        }

        if let Some((src_name, dst_name)) = upper.split_once("_TO_") {
            let src = TypeId::from_builtin_name(src_name)?;
            let dst = TypeId::from_builtin_name(dst_name)?;
            let Some((arg, arg_type)) = self.collect_single_conversion_arg(node) else {
                return Some(TypeId::UNKNOWN);
            };
            if !self.expect_assignable_in_param(src, &arg, arg_type) {
                return Some(TypeId::UNKNOWN);
            }
            if !self.is_conversion_allowed(src, dst) {
                self.checker.diagnostics.error(
                    DiagnosticCode::InvalidArgumentType,
                    arg.range,
                    format!(
                        "cannot convert '{}' to '{}'",
                        self.checker.type_name(src),
                        self.checker.type_name(dst)
                    ),
                );
                return Some(TypeId::UNKNOWN);
            }
            return Some(dst);
        }

        None
    }


    fn collect_single_conversion_arg(&mut self, node: &SyntaxNode) -> Option<(CallArg, TypeId)> {
        let params = vec![builtin_param("IN", ParamDirection::In)];
        let call = self.builtin_call(node, params);
        call.check_formal_arg_count(self, node, 1);
        let (arg, arg_type) = call.arg(0)?;
        (arg_type != TypeId::UNKNOWN).then_some((arg, arg_type))
    }


    fn expect_assignable_in_param(
        &mut self,
        expected: TypeId,
        arg: &CallArg,
        arg_type: TypeId,
    ) -> bool {
        if arg_type == TypeId::UNKNOWN {
            return false;
        }
        if !self.checker.is_assignable(expected, arg_type) {
            self.checker.diagnostics.error(
                DiagnosticCode::InvalidArgumentType,
                arg.range,
                format!(
                    "expected '{}' for parameter 'IN'",
                    self.checker.type_name(expected)
                ),
            );
            return false;
        }
        true
    }

}
