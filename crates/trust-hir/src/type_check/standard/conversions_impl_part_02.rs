impl<'a, 'b> StandardChecker<'a, 'b> {

    pub(in crate::type_check) fn check_formal_arg_count(
        &mut self,
        bound: &BoundArgs,
        node: &SyntaxNode,
        actual: usize,
        expected: usize,
    ) {
        if bound.formal_call && actual != expected {
            self.checker.diagnostics.error(
                DiagnosticCode::WrongArgumentCount,
                node.text_range(),
                format!("expected {} arguments, found {}", expected, actual),
            );
        }
    }


    pub(in crate::type_check) fn base_type_id(&self, type_id: TypeId) -> TypeId {
        let resolved = self.checker.resolve_alias_type(type_id);
        self.checker.resolve_subrange_base(resolved)
    }


    pub(in crate::type_check) fn is_numeric_type(&self, type_id: TypeId) -> bool {
        self.checker
            .resolved_type(type_id)
            .is_some_and(|ty| ty.is_numeric())
    }


    pub(in crate::type_check) fn is_integer_type(&self, type_id: TypeId) -> bool {
        self.checker
            .resolved_type(type_id)
            .is_some_and(|ty| ty.is_integer())
    }


    pub(in crate::type_check) fn is_unsigned_int_type(&self, type_id: TypeId) -> bool {
        matches!(
            self.base_type_id(type_id),
            TypeId::USINT | TypeId::UINT | TypeId::UDINT | TypeId::ULINT
        )
    }


    pub(in crate::type_check) fn is_real_type(&self, type_id: TypeId) -> bool {
        self.checker
            .resolved_type(type_id)
            .is_some_and(|ty| ty.is_float())
    }


    pub(in crate::type_check) fn is_bit_string_type(&self, type_id: TypeId) -> bool {
        self.checker
            .resolved_type(type_id)
            .is_some_and(|ty| ty.is_bit_string())
    }


    pub(in crate::type_check) fn is_string_type(&self, type_id: TypeId) -> bool {
        self.checker
            .resolved_type(type_id)
            .is_some_and(|ty| ty.is_string())
    }

    pub(in crate::type_check) fn is_chars_type(&self, type_id: TypeId) -> bool {
        self.checker
            .resolved_type(type_id)
            .is_some_and(|ty| ty.is_chars())
    }


    pub(in crate::type_check) fn string_kind(&self, type_id: TypeId) -> Option<bool> {
        match self.checker.resolved_type(type_id)? {
            Type::String { .. } => Some(false),
            Type::WString { .. } => Some(true),
            _ => None,
        }
    }

    pub(in crate::type_check) fn chars_kind(&self, type_id: TypeId) -> Option<bool> {
        match self.checker.resolved_type(type_id)? {
            Type::String { .. } | Type::Char => Some(false),
            Type::WString { .. } | Type::WChar => Some(true),
            _ => None,
        }
    }


    pub(in crate::type_check) fn normalize_string_type_id(&self, type_id: TypeId) -> TypeId {
        match self.string_kind(type_id) {
            Some(true) => TypeId::WSTRING,
            Some(false) => TypeId::STRING,
            None => type_id,
        }
    }


    pub(in crate::type_check) fn is_time_related_type(&self, type_id: TypeId) -> bool {
        self.checker
            .resolved_type(type_id)
            .is_some_and(|ty| ty.is_time())
    }


    pub(in crate::type_check) fn is_time_duration_type(&self, type_id: TypeId) -> bool {
        matches!(self.base_type_id(type_id), TypeId::TIME | TypeId::LTIME)
    }


    pub(in crate::type_check) fn is_elementary_type(&self, type_id: TypeId) -> bool {
        let resolved = self.base_type_id(type_id);
        matches!(
            self.checker.symbols.type_by_id(resolved),
            Some(
                Type::Bool
                    | Type::SInt
                    | Type::Int
                    | Type::DInt
                    | Type::LInt
                    | Type::USInt
                    | Type::UInt
                    | Type::UDInt
                    | Type::ULInt
                    | Type::Real
                    | Type::LReal
                    | Type::Byte
                    | Type::Word
                    | Type::DWord
                    | Type::LWord
                    | Type::Time
                    | Type::LTime
                    | Type::Date
                    | Type::LDate
                    | Type::Tod
                    | Type::LTod
                    | Type::Dt
                    | Type::Ldt
                    | Type::String { .. }
                    | Type::WString { .. }
                    | Type::Char
                    | Type::WChar
                    | Type::Enum { .. }
                    | Type::Subrange { .. }
            )
        )
    }


    pub(in crate::type_check) fn common_numeric_type_for_args(
        &mut self,
        args: &[(CallArg, TypeId)],
    ) -> Option<TypeId> {
        let mut common: Option<TypeId> = None;
        let mut saw_untyped_real = false;
        for (arg, ty) in args {
            let base = self.base_type_id(*ty);
            if base == TypeId::UNKNOWN {
                return None;
            }
            if !self.is_numeric_type(base) {
                self.checker.diagnostics.error(
                    DiagnosticCode::InvalidArgumentType,
                    arg.range,
                    "expected numeric type",
                );
                return None;
            }
            if is_untyped_real_literal_expr(&arg.expr) {
                saw_untyped_real = true;
                continue;
            }
            common = Some(match common {
                None => base,
                Some(current) => self.checker.wider_numeric(current, base),
            });
        }
        let common = match common {
            Some(base) => {
                if saw_untyped_real {
                    let base_ty = self.checker.symbols.type_by_id(base);
                    if base_ty.is_some_and(|ty| ty.is_float()) {
                        base
                    } else {
                        TypeId::LREAL
                    }
                } else {
                    base
                }
            }
            None => {
                if saw_untyped_real {
                    TypeId::LREAL
                } else {
                    return None;
                }
            }
        };
        for (arg, ty) in args {
            let base = self.base_type_id(*ty);
            if is_untyped_real_literal_expr(&arg.expr) && common == TypeId::REAL {
                continue;
            }
            if base != common {
                self.checker
                    .warn_implicit_conversion(common, base, arg.range);
            }
        }
        Some(common)
    }


    pub(in crate::type_check) fn common_integer_type_for_args(
        &mut self,
        args: &[(CallArg, TypeId)],
    ) -> Option<TypeId> {
        let mut common: Option<TypeId> = None;
        for (arg, ty) in args {
            let base = self.base_type_id(*ty);
            if base == TypeId::UNKNOWN {
                return None;
            }
            if !self.is_integer_type(base) {
                self.checker.diagnostics.error(
                    DiagnosticCode::InvalidArgumentType,
                    arg.range,
                    "expected integer type",
                );
                return None;
            }
            common = Some(match common {
                None => base,
                Some(current) => self.checker.wider_numeric(current, base),
            });
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

}
