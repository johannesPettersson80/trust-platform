use super::defs::{Type, TypeId};
use super::registry::TypeRegistry;

impl TypeRegistry {
    /// Checks if two types are compatible for assignment.
    #[must_use]
    pub fn is_assignable(&self, target: TypeId, source: TypeId) -> bool {
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
