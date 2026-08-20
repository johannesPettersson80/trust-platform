use super::defs::{Type, TypeId};
use super::registry::TypeRegistry;
use rustc_hash::FxHashSet;

pub(crate) fn is_accuracy_preserving_implicit_conversion(target: &Type, source: &Type) -> bool {
    matches!(
        (target, source),
        (Type::Int, Type::SInt)
            | (Type::DInt, Type::SInt | Type::Int)
            | (Type::LInt, Type::SInt | Type::Int | Type::DInt)
            | (Type::UInt, Type::USInt)
            | (Type::UDInt, Type::USInt | Type::UInt)
            | (Type::ULInt, Type::USInt | Type::UInt | Type::UDInt)
            | (Type::Real, Type::SInt | Type::Int)
            | (
                Type::LReal,
                Type::SInt | Type::Int | Type::DInt | Type::Real
            )
            | (Type::Word, Type::Byte)
            | (Type::DWord, Type::Byte | Type::Word)
            | (Type::LWord, Type::Byte | Type::Word | Type::DWord)
    )
}

impl TypeRegistry {
    /// Checks if two types are compatible for assignment.
    #[must_use]
    pub fn is_assignable(&self, target: TypeId, source: TypeId) -> bool {
        if self.get(target).is_some_and(is_generic_type) {
            return self.generic_accepts(target, source);
        }

        if target == source {
            return true;
        }

        let target_ty = self.get(target);
        let source_ty = self.get(source);

        match (target_ty, source_ty) {
            (Some(t), Some(s)) => self.types_compatible(t, s),
            _ => false,
        }
    }

    fn generic_accepts(&self, target: TypeId, source: TypeId) -> bool {
        let Some(source) = self.resolve_alias(source) else {
            return false;
        };
        if !is_concrete_value_type(source) {
            return false;
        }

        let family_source = match source {
            Type::Subrange { base, .. } => {
                let Some(base) = self.resolve_alias(*base) else {
                    return false;
                };
                base
            }
            source => source,
        };

        match self.get(target) {
            Some(Type::Any) => true,
            Some(Type::AnyDerived) => source.is_derived(),
            Some(Type::AnyElementary) => source.is_elementary(),
            Some(Type::AnyMagnitude) => family_source.is_numeric() || family_source.is_duration(),
            Some(Type::AnyInt) => family_source.is_integer(),
            Some(Type::AnyUnsigned) => family_source.is_unsigned(),
            Some(Type::AnySigned) => family_source.is_signed(),
            Some(Type::AnyReal) => family_source.is_float(),
            Some(Type::AnyNum) => family_source.is_numeric(),
            Some(Type::AnyDuration) => family_source.is_duration(),
            Some(Type::AnyBit) => family_source.is_bit_string(),
            Some(Type::AnyChars) => family_source.is_chars(),
            Some(Type::AnyString) => family_source.is_string(),
            Some(Type::AnyChar) => family_source.is_char(),
            Some(Type::AnyDate) => family_source.is_date(),
            _ => false,
        }
    }

    fn resolve_alias(&self, type_id: TypeId) -> Option<&Type> {
        let mut current = type_id;
        let mut visited = FxHashSet::default();
        while visited.insert(current) {
            match self.get(current)? {
                Type::Alias { target, .. } => current = *target,
                ty => return Some(ty),
            }
        }
        None
    }

    fn types_compatible(&self, target: &Type, source: &Type) -> bool {
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
                    let wildcard_target = *t_lower == 0 && *t_upper == i64::MAX;
                    let wildcard_source = *s_lower == 0 && *s_upper == i64::MAX;
                    if wildcard_target || wildcard_source {
                        continue;
                    }
                    if t_lower != s_lower || t_upper != s_upper {
                        return false;
                    }
                }
                let Some(target_element) = self.get(*te) else {
                    return false;
                };
                let Some(source_element) = self.get(*se) else {
                    return false;
                };
                self.types_compatible(target_element, source_element)
            }

            // NULL assignment to references/pointers
            (Type::Pointer { .. } | Type::Reference { .. }, Type::Null) => true,

            // String types are compatible regardless of declared length.
            (Type::String { .. }, Type::String { .. }) => true,
            (Type::WString { .. }, Type::WString { .. }) => true,

            // IEC 61131-3 6.6.1.6: implicit conversion preserves value and accuracy.
            (target, source) if is_accuracy_preserving_implicit_conversion(target, source) => true,

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

    fn normalize_subrange<'a>(&'a self, ty: &'a Type) -> &'a Type {
        if let Type::Subrange { base, .. } = ty {
            return self.get(*base).unwrap_or(ty);
        }
        ty
    }
}

fn is_generic_type(ty: &Type) -> bool {
    matches!(
        ty,
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
}

fn is_concrete_value_type(ty: &Type) -> bool {
    !matches!(
        ty,
        Type::Unknown
            | Type::Void
            | Type::Null
            | Type::Any
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
            | Type::Alias { .. }
    )
}
