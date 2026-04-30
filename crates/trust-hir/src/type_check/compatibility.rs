use super::*;
use crate::semantic::SemanticOutcome;
use crate::types::ArrayDimensionExt;

impl<'a> TypeChecker<'a> {
    pub(super) fn resolve_alias_type(&self, type_id: TypeId) -> TypeId {
        self.symbols.resolve_alias_type(type_id)
    }

    pub(super) fn resolve_alias_type_outcome(&self, type_id: TypeId) -> SemanticOutcome<TypeId> {
        self.symbols.resolve_alias_type_outcome(type_id)
    }

    pub(super) fn resolve_subrange_base(&self, type_id: TypeId) -> TypeId {
        let resolved = match self.resolve_alias_type_outcome(type_id) {
            SemanticOutcome::Resolved(type_id) => type_id,
            _ => type_id,
        };
        match self.symbols.type_by_id(resolved) {
            Some(Type::Subrange { base, .. }) => *base,
            _ => resolved,
        }
    }

    pub(super) fn subrange_bounds(&self, type_id: TypeId) -> Option<(TypeId, i64, i64)> {
        let resolved = self.resolve_alias_type(type_id);
        match self.symbols.type_by_id(resolved) {
            Some(Type::Subrange { base, lower, upper }) => Some((*base, *lower, *upper)),
            _ => None,
        }
    }

    pub(super) fn normalize_subrange<'b>(&'b self, ty: &'b Type) -> &'b Type {
        if let Type::Subrange { base, .. } = ty {
            return self.symbols.type_by_id(*base).unwrap_or(ty);
        }
        ty
    }

    pub(super) fn resolved_type(&self, type_id: TypeId) -> Option<&Type> {
        let resolved = self.resolve_alias_type(type_id);
        self.symbols.type_by_id(resolved)
    }

    pub(super) fn is_generic_type(&self, type_id: TypeId) -> bool {
        matches!(
            self.symbols.type_by_id(type_id),
            Some(
                Type::Any
                    | Type::AnyDerived
                    | Type::AnyElementary
                    | Type::AnyMagnitude
                    | Type::AnyInt
                    | Type::AnyUnsigned
                    | Type::AnySigned
                    | Type::AnyReal
                    | Type::AnyNum
                    | Type::AnyDuration
                    | Type::AnyBit
                    | Type::AnyChars
                    | Type::AnyString
                    | Type::AnyChar
                    | Type::AnyDate
            )
        )
    }

    pub(super) fn is_reference_like_type(&self, type_id: TypeId) -> bool {
        let resolved = self.resolve_alias_type(type_id);
        matches!(
            self.symbols.type_by_id(resolved),
            Some(Type::Reference { .. } | Type::Pointer { .. })
        )
    }

    pub(super) fn is_assignable(&self, target: TypeId, source: TypeId) -> bool {
        let target = self.resolve_alias_type(target);
        let source = self.resolve_alias_type(source);
        if target == source {
            return true;
        }

        if target == TypeId::UNKNOWN || source == TypeId::UNKNOWN {
            return true; // Allow unknown types
        }

        let target_ty = self.symbols.type_by_id(target);
        let source_ty = self.symbols.type_by_id(source);

        match (target_ty, source_ty) {
            (Some(t), Some(s)) => self.types_compatible(t, s),
            _ => false,
        }
    }

    pub(super) fn types_compatible(&self, target: &Type, source: &Type) -> bool {
        let target = self.normalize_subrange(target);
        let source = self.normalize_subrange(source);

        match (target, source) {
            // Same type
            (a, b) if a == b => true,

            (
                Type::Array {
                    element: te,
                    dimensions: td,
                },
                Type::Array {
                    element: se,
                    dimensions: sd,
                },
            ) => {
                if td.len() != sd.len() {
                    return false;
                }
                for ((t_lower, t_upper), (s_lower, s_upper)) in td.iter().zip(sd.iter()) {
                    let wildcard_target = (*t_lower, *t_upper).is_wildcard();
                    let wildcard_source = (*s_lower, *s_upper).is_wildcard();
                    if wildcard_target || wildcard_source {
                        continue;
                    }
                    if t_lower != s_lower || t_upper != s_upper {
                        return false;
                    }
                }
                let Some(target_element) = self.symbols.type_by_id(*te) else {
                    return false;
                };
                let Some(source_element) = self.symbols.type_by_id(*se) else {
                    return false;
                };
                self.types_compatible(target_element, source_element)
            }

            (Type::Pointer { target: tt }, Type::Pointer { target: ts }) => {
                let Some(target) = self.symbols.type_by_id(self.resolve_alias_type(*tt)) else {
                    return false;
                };
                let Some(source) = self.symbols.type_by_id(self.resolve_alias_type(*ts)) else {
                    return false;
                };
                self.types_compatible(target, source)
            }

            (Type::Reference { target: tt }, Type::Reference { target: ts }) => {
                self.reference_types_compatible(*tt, *ts)
            }

            // NULL assignment to references/pointers
            (Type::Pointer { .. } | Type::Reference { .. }, Type::Null) => true,

            // Interface assignment compatibility
            (Type::Interface { .. }, Type::Null) => true,
            (
                Type::Interface { name: target },
                Type::Class { name: source }
                | Type::FunctionBlock { name: source }
                | Type::Interface { name: source },
            ) => {
                let Some(target_id) = self
                    .symbols
                    .resolve_global_or_qualified_name(target.as_str())
                else {
                    return false;
                };
                let Some(source_id) = self
                    .symbols
                    .resolve_global_or_qualified_name(source.as_str())
                else {
                    return false;
                };
                self.is_interface_assignable(target_id, source_id)
            }

            // Numeric widening (safe conversions)
            (Type::Int, Type::SInt) => true,
            (Type::DInt, Type::SInt | Type::Int) => true,
            (Type::LInt, Type::SInt | Type::Int | Type::DInt) => true,
            (Type::UInt, Type::USInt) => true,
            (Type::UDInt, Type::USInt | Type::UInt) => true,
            (Type::ULInt, Type::USInt | Type::UInt | Type::UDInt) => true,
            (Type::Real, Type::SInt | Type::Int | Type::DInt) => true,
            (Type::LReal, Type::SInt | Type::Int | Type::DInt | Type::LInt | Type::Real) => true,

            // Bit string widening
            (Type::Word, Type::Byte) => true,
            (Type::DWord, Type::Byte | Type::Word) => true,
            (Type::LWord, Type::Byte | Type::Word | Type::DWord) => true,

            // String types are compatible regardless of declared length.
            (Type::String { .. }, Type::String { .. }) => true,
            (Type::WString { .. }, Type::WString { .. }) => true,

            // Generic type matching
            (Type::Any, _) => true,
            (Type::AnyDerived, t) if t.is_derived() => true,
            (Type::AnyElementary, t) if t.is_elementary() => true,
            (Type::AnyMagnitude, t) if t.is_numeric() || t.is_duration() => true,
            (Type::AnyInt, t) if t.is_integer() => true,
            (Type::AnyUnsigned, t) if t.is_unsigned() => true,
            (Type::AnySigned, t) if t.is_signed() => true,
            (Type::AnyReal, t) if t.is_float() => true,
            (Type::AnyNum, t) if t.is_numeric() => true,
            (Type::AnyDuration, t) if t.is_duration() => true,
            (Type::AnyBit, t) if t.is_bit_string() => true,
            (Type::AnyChars, t) if t.is_chars() => true,
            (Type::AnyString, t) if t.is_string() => true,
            (Type::AnyChar, t) if t.is_char() => true,
            (Type::AnyDate, t) if t.is_date() => true,

            _ => false,
        }
    }

    pub(super) fn reference_types_compatible(&self, target: TypeId, source: TypeId) -> bool {
        let target = self.resolve_subrange_base(self.resolve_alias_type(target));
        let source = self.resolve_subrange_base(self.resolve_alias_type(source));
        if target == source {
            return true;
        }

        let Some(target_symbol) = self.type_owner_symbol(target) else {
            return false;
        };
        let Some(source_symbol) = self.type_owner_symbol(source) else {
            return false;
        };

        if self.is_interface_assignable(target_symbol, source_symbol) {
            return true;
        }
        self.resolve_ref()
            .is_same_or_derived(source_symbol, target_symbol)
    }

    pub(super) fn type_owner_symbol(&self, type_id: TypeId) -> Option<SymbolId> {
        let resolved = self.resolve_alias_type(type_id);
        let name = match self.symbols.type_by_id(resolved)? {
            Type::Class { name } | Type::FunctionBlock { name } | Type::Interface { name } => name,
            _ => return None,
        };
        self.symbols.resolve_global_or_qualified_name(name.as_str())
    }

    fn is_interface_assignable(&self, target_id: SymbolId, source_id: SymbolId) -> bool {
        let Some(target_symbol) = self.symbols.get(target_id) else {
            return false;
        };
        if !matches!(target_symbol.kind, SymbolKind::Interface) {
            return false;
        }
        let Some(source_symbol) = self.symbols.get(source_id) else {
            return false;
        };
        if matches!(source_symbol.kind, SymbolKind::Interface) {
            return self.resolve_ref().is_same_or_derived(source_id, target_id);
        }
        self.implements_interface(source_id, target_id)
    }

    fn implements_interface(&self, owner_id: SymbolId, interface_id: SymbolId) -> bool {
        let mut visited = FxHashSet::default();
        let mut current = Some(owner_id);

        while let Some(symbol_id) = current {
            if !visited.insert(symbol_id) {
                break;
            }

            if let Some(interfaces) = self.symbols.implements_names(symbol_id) {
                for name in interfaces {
                    let Some(iface_id) = self
                        .symbols
                        .resolve_oop_reference_for_owner(symbol_id, name.as_str())
                    else {
                        continue;
                    };
                    if self
                        .resolve_ref()
                        .is_same_or_derived(iface_id, interface_id)
                    {
                        return true;
                    }
                }
            }

            current = self.symbols.extends_name(symbol_id).and_then(|name| {
                self.symbols
                    .resolve_oop_reference_for_owner(symbol_id, name.as_str())
            });
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::DiagnosticBuilder;

    #[test]
    fn missing_array_element_type_identity_is_not_assignment_compatible() {
        let mut symbols = SymbolTable::new();
        let target = symbols.register_type(
            "BadTargetArray",
            Type::Array {
                element: TypeId(9001),
                dimensions: vec![(0, 1)],
            },
        );
        let source = symbols.register_type(
            "BadSourceArray",
            Type::Array {
                element: TypeId(9002),
                dimensions: vec![(0, 1)],
            },
        );
        let mut diagnostics = DiagnosticBuilder::new();
        let checker = TypeChecker::new(&mut symbols, &mut diagnostics, ScopeId::GLOBAL);

        assert!(
            !checker.is_assignable(target, source),
            "missing array element TypeIds must not silently substitute Type::Unknown and become compatible"
        );
    }

    #[test]
    fn missing_pointer_target_type_identity_is_not_assignment_compatible() {
        let mut symbols = SymbolTable::new();
        let target = symbols.register_type(
            "BadTargetPointer",
            Type::Pointer {
                target: TypeId(9101),
            },
        );
        let source = symbols.register_type(
            "BadSourcePointer",
            Type::Pointer {
                target: TypeId(9102),
            },
        );
        let mut diagnostics = DiagnosticBuilder::new();
        let checker = TypeChecker::new(&mut symbols, &mut diagnostics, ScopeId::GLOBAL);

        assert!(
            !checker.is_assignable(target, source),
            "missing pointer target TypeIds must not silently substitute Type::Unknown and become compatible"
        );
    }
}
