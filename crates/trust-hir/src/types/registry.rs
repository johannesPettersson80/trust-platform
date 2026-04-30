use rustc_hash::FxHashMap;
use smol_str::SmolStr;

use super::defs::{ArrayDimensionExt, StructField, Type, TypeId, UnionVariant};

/// Type registry for managing all types.
#[derive(Debug, Clone, Default)]
pub struct TypeRegistry {
    /// All types indexed by ID.
    types: FxHashMap<TypeId, Type>,
    /// Name to type ID lookup.
    names: FxHashMap<SmolStr, TypeId>,
    /// Next type ID to assign.
    next_id: u32,
}

impl TypeRegistry {
    /// Creates a new type registry with built-in types.
    #[must_use]
    pub fn new() -> Self {
        let mut registry = Self {
            types: FxHashMap::default(),
            names: FxHashMap::default(),
            next_id: TypeId::USER_TYPES_START,
        };

        registry.register_builtin_types();

        registry
    }

    pub(super) fn register_builtin(&mut self, id: TypeId, name: &str, ty: Type) {
        self.types.insert(id, ty);
        self.names.insert(SmolStr::new(name), id);
    }

    /// Registers a new type and returns its ID.
    pub fn register(&mut self, name: impl Into<SmolStr>, ty: Type) -> TypeId {
        let id = TypeId(self.next_id);
        self.next_id += 1;

        let name = name.into();
        self.insert_name(name, id);
        self.types.insert(id, ty);

        id
    }

    /// Reserves a named user type ID before the full type body is available.
    ///
    /// This is used by lowerers that need to resolve self-referential reference
    /// fields such as `Node.next : REF_TO Node` while the `Node` struct body is
    /// still being lowered.
    pub fn reserve(&mut self, name: impl Into<SmolStr>) -> TypeId {
        let id = TypeId(self.next_id);
        self.next_id += 1;

        self.insert_name(name.into(), id);
        self.types.insert(id, Type::Unknown);
        id
    }

    /// Replaces a previously reserved user type body.
    pub fn replace(&mut self, id: TypeId, ty: Type) {
        self.types.insert(id, ty);
    }

    fn insert_name(&mut self, name: SmolStr, id: TypeId) {
        self.names.insert(name.clone(), id);
        let upper = name.as_str().to_ascii_uppercase();
        if upper != name {
            self.names.insert(SmolStr::new(upper), id);
        }
    }

    /// Registers a struct type with fields.
    pub fn register_struct(
        &mut self,
        name: impl Into<SmolStr>,
        fields: Vec<StructField>,
    ) -> TypeId {
        let name = name.into();
        self.register(name.clone(), Type::Struct { name, fields })
    }

    /// Registers a union type with variants.
    pub fn register_union(
        &mut self,
        name: impl Into<SmolStr>,
        variants: Vec<UnionVariant>,
    ) -> TypeId {
        let name = name.into();
        self.register(name.clone(), Type::Union { name, variants })
    }

    /// Registers an enum type with values.
    pub fn register_enum(
        &mut self,
        name: impl Into<SmolStr>,
        base: TypeId,
        values: Vec<(SmolStr, i64)>,
    ) -> TypeId {
        let name = name.into();
        self.register(name.clone(), Type::Enum { name, base, values })
    }

    /// Registers an array type.
    pub fn register_array(&mut self, element: TypeId, dimensions: Vec<(i64, i64)>) -> TypeId {
        // Generate a unique name for the array type
        let elem_name = self.type_name(element).unwrap_or_else(|| "?".into());
        let dims_str: Vec<String> = dimensions
            .iter()
            .map(ArrayDimensionExt::display_bounds)
            .collect();
        let name = format!("ARRAY[{}] OF {}", dims_str.join(", "), elem_name);
        self.register(
            name,
            Type::Array {
                element,
                dimensions,
            },
        )
    }

    /// Registers a STRING type with a specific length.
    pub fn register_string_with_length(&mut self, max_len: u32) -> TypeId {
        let name = format!("STRING[{}]", max_len);
        self.register(
            name,
            Type::String {
                max_len: Some(max_len),
            },
        )
    }

    /// Registers a WSTRING type with a specific length.
    pub fn register_wstring_with_length(&mut self, max_len: u32) -> TypeId {
        let name = format!("WSTRING[{}]", max_len);
        self.register(
            name,
            Type::WString {
                max_len: Some(max_len),
            },
        )
    }

    /// Registers a pointer type.
    pub fn register_pointer(&mut self, target: TypeId) -> TypeId {
        let target_name = self.type_name(target).unwrap_or_else(|| "?".into());
        let name = format!("POINTER TO {}", target_name);
        self.register(name, Type::Pointer { target })
    }

    /// Registers a reference type.
    pub fn register_reference(&mut self, target: TypeId) -> TypeId {
        let target_name = self.type_name(target).unwrap_or_else(|| "?".into());
        let name = format!("REF_TO {}", target_name);
        self.register(name, Type::Reference { target })
    }

    /// Gets the name of a type by ID.
    #[must_use]
    pub fn type_name(&self, id: TypeId) -> Option<SmolStr> {
        // Check built-in types first
        if let Some(name) = id.builtin_name() {
            return Some(SmolStr::new(name));
        }
        if let Some(name) = self.types.get(&id).and_then(canonical_type_name) {
            return Some(name);
        }
        // Look up in registered names
        self.names
            .iter()
            .find(|(_, &tid)| tid == id)
            .map(|(name, _)| name.clone())
    }

    /// Gets a type by ID.
    #[must_use]
    pub fn get(&self, id: TypeId) -> Option<&Type> {
        self.types.get(&id)
    }

    /// Looks up a type by name.
    #[must_use]
    pub fn lookup(&self, name: &str) -> Option<TypeId> {
        // Case-insensitive lookup for built-in types
        self.names
            .get(name)
            .or_else(|| self.names.get(&SmolStr::new(name.to_uppercase())))
            .copied()
    }
}

fn canonical_type_name(ty: &Type) -> Option<SmolStr> {
    match ty {
        Type::Struct { name, .. }
        | Type::Union { name, .. }
        | Type::Enum { name, .. }
        | Type::FunctionBlock { name }
        | Type::Class { name }
        | Type::Interface { name }
        | Type::Alias { name, .. } => Some(name.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // IEC 61131-3 Ed.3 Table 10 (elementary data types)
    fn test_type_registry() {
        let registry = TypeRegistry::new();

        assert_eq!(registry.lookup("INT"), Some(TypeId::INT));
        assert_eq!(registry.lookup("int"), Some(TypeId::INT));
        assert_eq!(registry.lookup("BOOL"), Some(TypeId::BOOL));
    }

    #[test]
    fn type_name_prefers_canonical_user_type_name() {
        let mut registry = TypeRegistry::new();
        let type_id = registry.register_enum(
            "Solo",
            TypeId::INT,
            vec![(SmolStr::new("S0"), 0), (SmolStr::new("S1"), 1)],
        );

        assert_eq!(registry.lookup("SOLO"), Some(type_id));
        assert_eq!(registry.type_name(type_id).as_deref(), Some("Solo"));
    }

    #[test]
    // IEC 61131-3 Ed.3 Figure 5 (generic type hierarchy)
    fn test_type_compatibility() {
        let mut registry = TypeRegistry::new();

        // Same type is compatible
        assert!(registry.is_assignable(TypeId::INT, TypeId::INT));

        // Widening is compatible
        assert!(registry.is_assignable(TypeId::DINT, TypeId::INT));
        assert!(registry.is_assignable(TypeId::REAL, TypeId::INT));

        // Narrowing is not compatible
        assert!(!registry.is_assignable(TypeId::INT, TypeId::DINT));

        // Generic types
        assert!(registry.is_assignable(TypeId::ANY_ELEMENTARY, TypeId::BOOL));
        assert!(registry.is_assignable(TypeId::ANY_UNSIGNED, TypeId::UDINT));
        assert!(!registry.is_assignable(TypeId::ANY_UNSIGNED, TypeId::DINT));
        assert!(registry.is_assignable(TypeId::ANY_DURATION, TypeId::TIME));
        assert!(registry.is_assignable(TypeId::ANY_DATE, TypeId::DATE));
        assert!(registry.is_assignable(TypeId::ANY_CHAR, TypeId::CHAR));
        assert!(registry.is_assignable(TypeId::ANY_CHARS, TypeId::WSTRING));

        let struct_id = registry.register_struct(
            "Point",
            vec![StructField {
                name: "x".into(),
                type_id: TypeId::INT,
                address: None,
                default_initializer: None,
            }],
        );
        assert!(registry.is_assignable(TypeId::ANY_DERIVED, struct_id));
    }

    #[test]
    fn missing_array_element_type_identity_is_not_assignable() {
        let mut registry = TypeRegistry::new();
        let target = registry.register(
            "BadTargetArray",
            Type::Array {
                element: TypeId(9001),
                dimensions: vec![(0, 1)],
            },
        );
        let source = registry.register(
            "BadSourceArray",
            Type::Array {
                element: TypeId(9002),
                dimensions: vec![(0, 1)],
            },
        );

        assert!(
            !registry.is_assignable(target, source),
            "missing array element TypeIds must not silently substitute Type::Unknown and become compatible"
        );
    }
}
