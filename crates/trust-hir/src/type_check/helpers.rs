use super::literals::string_literal_info;
use super::literals::{is_untyped_int_literal_expr, is_untyped_real_literal_expr};
use super::*;
use crate::semantic::LEGACY_UNKNOWN_TYPE_ID;

impl<'a> TypeChecker<'a> {
    // ========== Helper Methods ==========

    pub(super) fn wider_numeric(&self, a: TypeId, b: TypeId) -> TypeId {
        // Widening order for numeric types
        const WIDENING_ORDER: &[TypeId] = &[
            TypeId::SINT,
            TypeId::INT,
            TypeId::DINT,
            TypeId::LINT,
            TypeId::USINT,
            TypeId::UINT,
            TypeId::UDINT,
            TypeId::ULINT,
            TypeId::REAL,
            TypeId::LREAL,
        ];

        let a = self.resolve_subrange_base(a);
        let b = self.resolve_subrange_base(b);

        let pos_a = WIDENING_ORDER.iter().position(|&t| t == a);
        let pos_b = WIDENING_ORDER.iter().position(|&t| t == b);

        match (pos_a, pos_b) {
            (Some(pa), Some(pb)) => WIDENING_ORDER[pa.max(pb)],
            (Some(_), None) => a,
            (None, Some(_)) => b,
            (None, None) => LEGACY_UNKNOWN_TYPE_ID,
        }
    }

    pub(super) fn is_contextual_int_literal(&self, expected: TypeId, expr: &SyntaxNode) -> bool {
        let expected = self.resolve_alias_type(expected);
        let Some(ty) = self.symbols.type_by_id(expected) else {
            return false;
        };
        let normalized = self.normalize_subrange(ty);
        if !normalized.is_integer() {
            return false;
        }
        is_untyped_int_literal_expr(expr)
    }

    pub(super) fn is_contextual_real_literal(&self, expected: TypeId, expr: &SyntaxNode) -> bool {
        let expected = self.resolve_alias_type(expected);
        let Some(ty) = self.symbols.type_by_id(expected) else {
            return false;
        };
        let normalized = self.normalize_subrange(ty);
        if !normalized.is_float() {
            return false;
        }
        is_untyped_real_literal_expr(expr)
    }

    pub(super) fn warn_implicit_conversion(
        &mut self,
        target: TypeId,
        source: TypeId,
        range: TextRange,
    ) {
        let target = self.resolve_alias_type(target);
        let source = self.resolve_alias_type(source);

        if target == source || target == TypeId::UNKNOWN || source == TypeId::UNKNOWN {
            return;
        }
        if self.is_string_family_implicit_ok(target, source) {
            return;
        }
        if self.is_generic_type(target) || self.is_generic_type(source) {
            return;
        }

        self.diagnostics.warning(
            DiagnosticCode::ImplicitConversion,
            range,
            format!(
                "implicit conversion from '{}' to '{}'",
                self.type_name(source),
                self.type_name(target)
            ),
        );
    }

    fn is_string_family_implicit_ok(&self, target: TypeId, source: TypeId) -> bool {
        let Some(target_ty) = self.symbols.type_by_id(target) else {
            return false;
        };
        let Some(source_ty) = self.symbols.type_by_id(source) else {
            return false;
        };
        matches!(target_ty, Type::String { .. }) && matches!(source_ty, Type::String { .. })
            || matches!(target_ty, Type::WString { .. })
                && matches!(source_ty, Type::WString { .. })
    }

    pub(super) fn check_string_literal_assignment(
        &mut self,
        target_type: TypeId,
        value: &SyntaxNode,
        value_type: TypeId,
    ) {
        let Some((target_is_wide, max_len)) = self.string_max_len(target_type) else {
            return;
        };
        let Some(literal) = string_literal_info(value) else {
            return;
        };
        if literal.is_wide != target_is_wide {
            return;
        }
        if literal.len > max_len {
            let target_name = self.type_name(target_type);
            let value_name = self.type_name(value_type);
            self.diagnostics.error(
                DiagnosticCode::OutOfRange,
                value.text_range(),
                format!(
                    "{} literal length {} exceeds {} capacity",
                    value_name, literal.len, target_name
                ),
            );
        }
    }

    pub(super) fn string_max_len(&self, type_id: TypeId) -> Option<(bool, u32)> {
        let resolved = self.resolve_alias_type(type_id);
        match self.symbols.type_by_id(resolved)? {
            Type::String { max_len: Some(max) } => Some((false, *max)),
            Type::WString { max_len: Some(max) } => Some((true, *max)),
            _ => None,
        }
    }

    pub(super) fn type_name(&self, id: TypeId) -> SmolStr {
        self.symbols
            .type_name(id)
            .unwrap_or_else(|| SmolStr::new("?"))
    }
}

pub(super) fn direct_address_type(text: &str) -> TypeId {
    let bytes = text.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'%' {
        return LEGACY_UNKNOWN_TYPE_ID;
    }

    let size = bytes
        .get(2)
        .copied()
        .filter(|b| b.is_ascii_alphabetic())
        .map(|b| b.to_ascii_uppercase());
    match size {
        Some(b'X') => TypeId::BOOL,
        Some(b'B') => TypeId::BYTE,
        Some(b'W') => TypeId::WORD,
        Some(b'D') => TypeId::DWORD,
        Some(b'L') => TypeId::LWORD,
        Some(_) => LEGACY_UNKNOWN_TYPE_ID,
        None => TypeId::BOOL,
    }
}
