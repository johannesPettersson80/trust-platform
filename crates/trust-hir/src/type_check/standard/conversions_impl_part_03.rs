impl<'a, 'b> StandardChecker<'a, 'b> {

    pub(in crate::type_check) fn common_real_type_for_args(
        &mut self,
        args: &[(CallArg, TypeId)],
    ) -> Option<TypeId> {
        let mut any_lreal = false;
        for (arg, ty) in args {
            let base = self.base_type_id(*ty);
            if base == TypeId::UNKNOWN {
                return None;
            }
            if !self.is_real_type(base) {
                self.checker.diagnostics.error(
                    DiagnosticCode::InvalidArgumentType,
                    arg.range,
                    "expected REAL or LREAL type",
                );
                return None;
            }
            if base == TypeId::LREAL {
                any_lreal = true;
            }
        }
        let common = if any_lreal {
            TypeId::LREAL
        } else {
            TypeId::REAL
        };
        for (arg, ty) in args {
            let base = self.base_type_id(*ty);
            if base != common {
                self.checker
                    .warn_implicit_conversion(common, base, arg.range);
            }
        }
        Some(common)
    }


    pub(in crate::type_check) fn common_bit_type_for_args(
        &mut self,
        args: &[(CallArg, TypeId)],
    ) -> Option<TypeId> {
        let mut common: Option<(TypeId, u32)> = None;
        for (arg, ty) in args {
            let base = self.base_type_id(*ty);
            if base == TypeId::UNKNOWN {
                return None;
            }
            if !self.is_bit_string_type(base) {
                self.checker.diagnostics.error(
                    DiagnosticCode::InvalidArgumentType,
                    arg.range,
                    "expected bit string type",
                );
                return None;
            }
            let Some(size) = self.checker.resolved_type(base).and_then(Type::bit_size) else {
                continue;
            };
            common = Some(match common {
                None => (base, size),
                Some((current, current_size)) => {
                    if size > current_size {
                        (base, size)
                    } else {
                        (current, current_size)
                    }
                }
            });
        }
        let (common, _) = common?;
        for (arg, ty) in args {
            let base = self.base_type_id(*ty);
            if base != common {
                self.checker
                    .warn_implicit_conversion(common, base, arg.range);
            }
        }
        Some(common)
    }


    pub(in crate::type_check) fn common_string_type_for_args(
        &mut self,
        args: &[(CallArg, TypeId)],
    ) -> Option<TypeId> {
        let mut common: Option<TypeId> = None;
        for (arg, ty) in args {
            let base = self.base_type_id(*ty);
            if base == TypeId::UNKNOWN {
                return None;
            }
            if !self.is_string_type(base) {
                self.checker.diagnostics.error(
                    DiagnosticCode::InvalidArgumentType,
                    arg.range,
                    "expected STRING or WSTRING type",
                );
                return None;
            }
            match common {
                None => common = Some(base),
                Some(current) => {
                    if self.is_string_type(current) && self.is_string_type(base) {
                        let current_is_wide = self.string_kind(current);
                        let base_is_wide = self.string_kind(base);
                        if current_is_wide != base_is_wide {
                            self.checker.diagnostics.error(
                                DiagnosticCode::InvalidArgumentType,
                                arg.range,
                                "cannot mix STRING and WSTRING",
                            );
                            return None;
                        }
                    }
                }
            }
        }
        let common = common?;
        for (arg, ty) in args {
            let base = self.base_type_id(*ty);
            if base != common {
                self.checker
                    .warn_implicit_conversion(common, base, arg.range);
            }
        }
        Some(common)
    }


    pub(in crate::type_check) fn common_any_type_for_args(
        &mut self,
        args: &[(CallArg, TypeId)],
    ) -> Option<TypeId> {
        if args.is_empty() {
            return None;
        }
        if args.iter().any(|(_, ty)| self.base_type_id(*ty) == TypeId::UNKNOWN) {
            return None;
        }
        if args.iter().all(|(_, ty)| self.is_elementary_type(*ty)) {
            return self.common_elementary_type_for_args(args);
        }

        let base = self.base_type_id(args[0].1);
        for (arg, ty) in args.iter().skip(1) {
            let other = self.base_type_id(*ty);
            if base != other {
                self.checker.diagnostics.error(
                    DiagnosticCode::InvalidArgumentType,
                    arg.range,
                    "arguments must have the same type",
                );
                return None;
            }
        }
        Some(base)
    }


    pub(in crate::type_check) fn common_elementary_type_for_args(
        &mut self,
        args: &[(CallArg, TypeId)],
    ) -> Option<TypeId> {
        if args.is_empty() {
            return None;
        }
        if args.iter().any(|(_, ty)| self.base_type_id(*ty) == TypeId::UNKNOWN) {
            return None;
        }

        if args.iter().all(|(_, ty)| self.is_numeric_type(*ty)) {
            return self.common_numeric_type_for_args(args);
        }

        if args.iter().all(|(_, ty)| self.is_bit_string_type(*ty)) {
            return self.common_bit_type_for_args(args);
        }

        if args.iter().all(|(_, ty)| self.is_string_type(*ty)) {
            return self.common_string_type_for_args(args);
        }

        if args.iter().all(|(_, ty)| self.is_time_related_type(*ty)) {
            let base = self.base_type_id(args[0].1);
            for (arg, ty) in args.iter().skip(1) {
                let other = self.base_type_id(*ty);
                if base != other {
                    self.checker.diagnostics.error(
                        DiagnosticCode::InvalidArgumentType,
                        arg.range,
                        "time/date arguments must have the same type",
                    );
                    return None;
                }
            }
            return Some(base);
        }

        if args
            .iter()
            .all(|(_, ty)| matches!(self.checker.resolved_type(*ty), Some(Type::Enum { .. })))
        {
            let base = self.base_type_id(args[0].1);
            for (arg, ty) in args.iter().skip(1) {
                let other = self.base_type_id(*ty);
                if base != other {
                    self.checker.diagnostics.error(
                        DiagnosticCode::InvalidArgumentType,
                        arg.range,
                        "enum arguments must have the same type",
                    );
                    return None;
                }
            }
            return Some(base);
        }

        for (arg, _) in args {
            self.checker.diagnostics.error(
                DiagnosticCode::InvalidArgumentType,
                arg.range,
                "expected elementary type",
            );
        }
        None
    }

}
