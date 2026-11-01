impl<'a, 'b> StandardChecker<'a, 'b> {
    pub(in crate::type_check) fn check_comparable_args(
        &mut self,
        args: &[(CallArg, TypeId)],
    ) -> bool {
        if args.len() < 2 {
            return true;
        }
        if args
            .iter()
            .any(|(_, ty)| self.base_type_id(*ty) == TypeId::UNKNOWN)
        {
            return false;
        }

        if args.iter().all(|(_, ty)| self.is_numeric_type(*ty)) {
            self.common_numeric_type_for_args(args);
            return true;
        }

        if args.iter().all(|(_, ty)| self.is_bit_string_type(*ty)) {
            self.common_bit_type_for_args(args);
            return true;
        }

        if args.iter().all(|(_, ty)| self.is_chars_type(*ty)) {
            let Some(wide) = self.chars_kind(args[0].1) else {
                return false;
            };
            for (arg, ty) in args.iter().skip(1) {
                if self.chars_kind(*ty) != Some(wide) {
                    self.checker.diagnostics.error(
                        DiagnosticCode::InvalidArgumentType,
                        arg.range,
                        "cannot mix narrow and wide character/string arguments",
                    );
                    return false;
                }
            }
            return true;
        }

        if args.iter().all(|(_, ty)| self.is_time_related_type(*ty)) {
            let base = self.base_type_id(args[0].1);
            for (arg, ty) in args.iter().skip(1) {
                if self.base_type_id(*ty) != base {
                    self.checker.diagnostics.error(
                        DiagnosticCode::InvalidArgumentType,
                        arg.range,
                        "time/date arguments must have the same type",
                    );
                    return false;
                }
            }
            return true;
        }

        if args
            .iter()
            .all(|(_, ty)| matches!(self.checker.resolved_type(*ty), Some(Type::Enum { .. })))
        {
            let base = self.base_type_id(args[0].1);
            for (arg, ty) in args.iter().skip(1) {
                if self.base_type_id(*ty) != base {
                    self.checker.diagnostics.error(
                        DiagnosticCode::InvalidArgumentType,
                        arg.range,
                        "enum arguments must have the same type",
                    );
                    return false;
                }
            }
            return true;
        }

        for (arg, _) in args {
            self.checker.diagnostics.error(
                DiagnosticCode::InvalidArgumentType,
                arg.range,
                "arguments are not comparable",
            );
        }
        false
    }

    pub(in crate::type_check) fn is_conversion_allowed(&self, src: TypeId, dst: TypeId) -> bool {
        let src = self.base_type_id(src);
        let dst = self.base_type_id(dst);
        if src == TypeId::UNKNOWN || dst == TypeId::UNKNOWN {
            return true;
        }

        if src == dst {
            return true;
        }

        if self.is_numeric_type(src) && self.is_numeric_type(dst) {
            return true;
        }

        // CODESYS TO_STRING parity: an enumerated value converts to its
        // member name string (IEC_DEVIATIONS 2026-10-28). Base-less enums
        // included — the runtime enum value carries its variant name.
        if matches!(self.checker.resolved_type(src), Some(Type::Enum { .. }))
            && matches!(
                self.normalize_string_type_id(dst),
                TypeId::STRING | TypeId::WSTRING
            )
        {
            return true;
        }

        if matches!(
            src,
            TypeId::BYTE | TypeId::WORD | TypeId::DWORD | TypeId::LWORD
        ) && matches!(
            dst,
            TypeId::BYTE | TypeId::WORD | TypeId::DWORD | TypeId::LWORD
        ) {
            return true;
        }

        if matches!(
            src,
            TypeId::BOOL | TypeId::BYTE | TypeId::WORD | TypeId::DWORD | TypeId::LWORD
        ) && matches!(
            dst,
            TypeId::SINT
                | TypeId::INT
                | TypeId::DINT
                | TypeId::LINT
                | TypeId::USINT
                | TypeId::UINT
                | TypeId::UDINT
                | TypeId::ULINT
        ) {
            return true;
        }

        if matches!(src, TypeId::DWORD) && dst == TypeId::REAL {
            return true;
        }
        if matches!(src, TypeId::LWORD) && dst == TypeId::LREAL {
            return true;
        }

        if matches!(
            dst,
            TypeId::BYTE | TypeId::WORD | TypeId::DWORD | TypeId::LWORD
        ) && matches!(
            src,
            TypeId::SINT
                | TypeId::INT
                | TypeId::DINT
                | TypeId::LINT
                | TypeId::USINT
                | TypeId::UINT
                | TypeId::UDINT
                | TypeId::ULINT
        ) {
            return true;
        }

        if src == TypeId::REAL && dst == TypeId::DWORD {
            return true;
        }
        if src == TypeId::LREAL && dst == TypeId::LWORD {
            return true;
        }

        let bit_string = |ty: TypeId| {
            matches!(
                ty,
                TypeId::BYTE | TypeId::WORD | TypeId::DWORD | TypeId::LWORD
            )
        };
        // IEC Table 24 rows 4, 8, 12 and 16: the rightmost bit of a bit string
        if dst == TypeId::BOOL && bit_string(src) {
            return true;
        }
        // vendor extensions outside IEC Tables 22-27 (docs/specs/07-standard-functions.md
        // 2.9): REAL/LREAL to a bit string of another width, numbers to BOOL, BOOL to
        // REAL/LREAL
        if matches!(src, TypeId::REAL | TypeId::LREAL) && bit_string(dst) {
            return true;
        }
        if dst == TypeId::BOOL && self.is_numeric_type(src) {
            return true;
        }
        if src == TypeId::BOOL && matches!(dst, TypeId::REAL | TypeId::LREAL) {
            return true;
        }

        if matches!(src, TypeId::LTIME) && dst == TypeId::TIME {
            return true;
        }
        if matches!(src, TypeId::TIME) && dst == TypeId::LTIME {
            return true;
        }
        if matches!(src, TypeId::TIME) && dst == TypeId::DWORD {
            return true;
        }
        if matches!(src, TypeId::DWORD) && dst == TypeId::TIME {
            return true;
        }
        // CODESYS converts durations to any integer (ms magnitude, wrapping
        // like the vendor toolchain) and time-of-day to its ms-of-day
        // magnitude (IEC_DEVIATIONS 2026-10-28).
        if matches!(src, TypeId::TIME | TypeId::LTIME)
            && matches!(
                dst,
                TypeId::SINT
                    | TypeId::INT
                    | TypeId::DINT
                    | TypeId::LINT
                    | TypeId::USINT
                    | TypeId::UINT
                    | TypeId::UDINT
                    | TypeId::ULINT
            )
        {
            return true;
        }
        if matches!(src, TypeId::TOD | TypeId::LTOD) && dst == TypeId::DWORD {
            return true;
        }
        if matches!(src, TypeId::LDT) && dst == TypeId::DT {
            return true;
        }
        if matches!(src, TypeId::LDT) && dst == TypeId::DATE {
            return true;
        }
        if matches!(src, TypeId::LDT) && dst == TypeId::LTOD {
            return true;
        }
        if matches!(src, TypeId::LDT) && dst == TypeId::TOD {
            return true;
        }
        if matches!(src, TypeId::DT) && dst == TypeId::LDT {
            return true;
        }
        if matches!(src, TypeId::DT) && dst == TypeId::DATE {
            return true;
        }
        if matches!(src, TypeId::DT) && dst == TypeId::LTOD {
            return true;
        }
        if matches!(src, TypeId::DT) && dst == TypeId::TOD {
            return true;
        }
        if matches!(src, TypeId::LTOD) && dst == TypeId::TOD {
            return true;
        }
        if matches!(src, TypeId::TOD) && dst == TypeId::LTOD {
            return true;
        }

        let src = self.normalize_string_type_id(src);
        let dst = self.normalize_string_type_id(dst);

        // Capacity-carrying string instances normalize to their family, so
        // TO_STRING over a STRING[n] alias is an identity copy.
        if src == dst {
            return true;
        }

        if matches!(dst, TypeId::STRING | TypeId::WSTRING)
            && (self.is_numeric_type(src)
                || matches!(
                    src,
                    TypeId::BOOL
                        | TypeId::BYTE
                        | TypeId::WORD
                        | TypeId::DWORD
                        | TypeId::LWORD
                        | TypeId::TIME
                        | TypeId::LTIME
                ))
        {
            return true;
        }
        if matches!(src, TypeId::STRING | TypeId::WSTRING) && (self.is_numeric_type(dst)) {
            return true;
        }
        if matches!(dst, TypeId::CHAR | TypeId::WCHAR)
            && (self.is_numeric_type(src)
                || matches!(
                    src,
                    TypeId::BYTE | TypeId::WORD | TypeId::DWORD | TypeId::LWORD
                ))
        {
            return true;
        }
        if matches!(src, TypeId::CHAR | TypeId::WCHAR)
            && (self.is_numeric_type(dst)
                || matches!(
                    dst,
                    TypeId::BYTE | TypeId::WORD | TypeId::DWORD | TypeId::LWORD
                ))
        {
            return true;
        }

        if matches!(src, TypeId::WSTRING) && matches!(dst, TypeId::STRING | TypeId::WCHAR) {
            return true;
        }
        if matches!(src, TypeId::STRING) && matches!(dst, TypeId::WSTRING | TypeId::CHAR) {
            return true;
        }
        if matches!(src, TypeId::WCHAR) && matches!(dst, TypeId::WSTRING | TypeId::CHAR) {
            return true;
        }
        if matches!(src, TypeId::CHAR) && matches!(dst, TypeId::STRING | TypeId::WCHAR) {
            return true;
        }

        false
    }
}
