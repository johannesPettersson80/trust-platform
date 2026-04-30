use std::sync::Arc;

use indexmap::IndexMap;
use smol_str::SmolStr;
use trust_hir::types::TypeRegistry;
use trust_hir::{Type, TypeId};

use crate::memory::InstanceId;

use super::{
    DateTimeValue, DateValue, Duration, LDateTimeValue, LDateValue, LTimeOfDayValue,
    TimeOfDayValue, ValueRef,
};

/// Array value with bounds tracking.
#[derive(Debug, Clone, PartialEq)]
pub struct ArrayValue {
    pub(crate) elements: Vec<Value>,
    pub(crate) dimensions: Vec<(i64, i64)>,
}

/// Struct value with named fields.
#[derive(Debug, Clone, PartialEq)]
pub struct StructValue {
    pub(crate) type_name: SmolStr,
    pub(crate) fields: IndexMap<SmolStr, Value>,
}

/// Error produced when constructing a compound runtime value from raw data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueConstructionError {
    UnknownType(TypeId),
    UnknownTypeName(SmolStr),
    AliasCycle(TypeId),
    NotStruct(TypeId),
    NotArray(TypeId),
    NotStructOrUnion(TypeId),
    UnsupportedType(TypeId),
    InvalidArrayBounds {
        dimensions: Vec<(i64, i64)>,
    },
    ArrayDimensionsMismatch {
        expected: Vec<(i64, i64)>,
        actual: Vec<(i64, i64)>,
    },
    ArrayElementCountMismatch {
        expected: usize,
        actual: usize,
    },
    ArrayElementTypeMismatch {
        index: usize,
        expected: TypeId,
        actual: &'static str,
    },
    MissingField {
        type_name: SmolStr,
        field_name: SmolStr,
    },
    ExtraField {
        type_name: SmolStr,
        field_name: SmolStr,
    },
    FieldTypeMismatch {
        type_name: SmolStr,
        field_name: SmolStr,
        expected: TypeId,
        actual: &'static str,
    },
    TypeMismatch {
        expected: TypeId,
        actual: &'static str,
    },
    Enum(EnumValueError),
}

impl std::fmt::Display for ValueConstructionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownType(type_id) => write!(f, "unknown type id {}", type_id.0),
            Self::UnknownTypeName(type_name) => write!(f, "unknown type '{type_name}'"),
            Self::AliasCycle(type_id) => {
                write!(f, "alias cycle while resolving type id {}", type_id.0)
            }
            Self::NotStruct(type_id) => write!(f, "type id {} is not a struct", type_id.0),
            Self::NotArray(type_id) => write!(f, "type id {} is not an array", type_id.0),
            Self::NotStructOrUnion(type_id) => {
                write!(f, "type id {} is not a struct or union", type_id.0)
            }
            Self::UnsupportedType(type_id) => {
                write!(
                    f,
                    "type id {} cannot be represented as a runtime value",
                    type_id.0
                )
            }
            Self::InvalidArrayBounds { dimensions } => {
                write!(f, "invalid array dimensions {dimensions:?}")
            }
            Self::ArrayDimensionsMismatch { expected, actual } => {
                write!(
                    f,
                    "array dimensions mismatch: expected {expected:?}, got {actual:?}"
                )
            }
            Self::ArrayElementCountMismatch { expected, actual } => {
                write!(
                    f,
                    "array element count mismatch: expected {expected}, got {actual}"
                )
            }
            Self::ArrayElementTypeMismatch {
                index,
                expected,
                actual,
            } => write!(
                f,
                "array element {index} type mismatch: expected type id {}, got {actual}",
                expected.0
            ),
            Self::MissingField {
                type_name,
                field_name,
            } => write!(f, "missing field '{type_name}.{field_name}'"),
            Self::ExtraField {
                type_name,
                field_name,
            } => write!(f, "extra field '{type_name}.{field_name}'"),
            Self::FieldTypeMismatch {
                type_name,
                field_name,
                expected,
                actual,
            } => write!(
                f,
                "field '{type_name}.{field_name}' type mismatch: expected type id {}, got {actual}",
                expected.0
            ),
            Self::TypeMismatch { expected, actual } => {
                write!(
                    f,
                    "value type mismatch: expected type id {}, got {actual}",
                    expected.0
                )
            }
            Self::Enum(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ValueConstructionError {}

impl From<EnumValueError> for ValueConstructionError {
    fn from(error: EnumValueError) -> Self {
        Self::Enum(error)
    }
}

impl ArrayValue {
    pub fn new(
        registry: &TypeRegistry,
        type_id: TypeId,
        elements: Vec<Value>,
    ) -> Result<Self, ValueConstructionError> {
        let (element_type, dimensions) = array_type_parts(registry, type_id)?;
        let value = Self::from_serialized_parts(registry, type_id, dimensions.clone(), elements)?;
        debug_assert_eq!(
            value.dimensions, dimensions,
            "array constructor must preserve canonical dimensions"
        );
        debug_assert!(value.elements.iter().all(|element| value_matches_type(
            registry,
            element_type,
            element
        )));
        Ok(value)
    }

    pub fn from_serialized_parts(
        registry: &TypeRegistry,
        type_id: TypeId,
        dimensions: Vec<(i64, i64)>,
        elements: Vec<Value>,
    ) -> Result<Self, ValueConstructionError> {
        let (element_type, expected_dimensions) = array_type_parts(registry, type_id)?;
        if dimensions != expected_dimensions {
            return Err(ValueConstructionError::ArrayDimensionsMismatch {
                expected: expected_dimensions,
                actual: dimensions,
            });
        }
        let expected_len = array_len(&dimensions)?;
        if elements.len() != expected_len {
            return Err(ValueConstructionError::ArrayElementCountMismatch {
                expected: expected_len,
                actual: elements.len(),
            });
        }
        for (index, element) in elements.iter().enumerate() {
            if !value_matches_type(registry, element_type, element) {
                return Err(ValueConstructionError::ArrayElementTypeMismatch {
                    index,
                    expected: element_type,
                    actual: value_kind(element),
                });
            }
        }
        Ok(Self::from_canonical_parts(elements, expected_dimensions))
    }

    pub fn from_untyped_parts(
        elements: Vec<Value>,
        dimensions: Vec<(i64, i64)>,
    ) -> Result<Self, ValueConstructionError> {
        let expected_len = array_len(&dimensions)?;
        if elements.len() != expected_len {
            return Err(ValueConstructionError::ArrayElementCountMismatch {
                expected: expected_len,
                actual: elements.len(),
            });
        }
        Ok(Self::from_canonical_parts(elements, dimensions))
    }

    pub(crate) fn from_canonical_parts(elements: Vec<Value>, dimensions: Vec<(i64, i64)>) -> Self {
        Self {
            elements,
            dimensions,
        }
    }

    #[must_use]
    pub fn elements(&self) -> &[Value] {
        &self.elements
    }

    pub fn elements_mut(&mut self) -> &mut [Value] {
        &mut self.elements
    }

    #[must_use]
    pub fn dimensions(&self) -> &[(i64, i64)] {
        &self.dimensions
    }

    pub fn set_dimensions(
        &mut self,
        dimensions: Vec<(i64, i64)>,
    ) -> Result<(), ValueConstructionError> {
        let expected_len = array_len(&dimensions)?;
        if self.elements.len() != expected_len {
            return Err(ValueConstructionError::ArrayElementCountMismatch {
                expected: expected_len,
                actual: self.elements.len(),
            });
        }
        self.dimensions = dimensions;
        Ok(())
    }
}

impl StructValue {
    pub fn new(
        registry: &TypeRegistry,
        type_id: TypeId,
        fields: IndexMap<SmolStr, Value>,
    ) -> Result<Self, ValueConstructionError> {
        let struct_type = struct_type_parts(registry, type_id)?;
        let mut canonical_fields = IndexMap::new();
        for field in &struct_type.fields {
            let Some((_, value)) = fields
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(field.name.as_str()))
            else {
                return Err(ValueConstructionError::MissingField {
                    type_name: struct_type.name.clone(),
                    field_name: field.name.clone(),
                });
            };
            if !value_matches_type(registry, field.type_id, value) {
                return Err(ValueConstructionError::FieldTypeMismatch {
                    type_name: struct_type.name.clone(),
                    field_name: field.name.clone(),
                    expected: field.type_id,
                    actual: value_kind(value),
                });
            }
            canonical_fields.insert(field.name.clone(), value.clone());
        }
        for name in fields.keys() {
            if !struct_type
                .fields
                .iter()
                .any(|field| field.name.eq_ignore_ascii_case(name.as_str()))
            {
                return Err(ValueConstructionError::ExtraField {
                    type_name: struct_type.name.clone(),
                    field_name: name.clone(),
                });
            }
        }
        Ok(Self::from_canonical_parts(
            struct_type.name,
            canonical_fields,
        ))
    }

    pub fn from_serialized_parts(
        registry: &TypeRegistry,
        type_name: &str,
        fields: IndexMap<SmolStr, Value>,
    ) -> Result<Self, ValueConstructionError> {
        let type_id = registry
            .lookup(type_name)
            .ok_or_else(|| ValueConstructionError::UnknownTypeName(type_name.into()))?;
        Self::new(registry, type_id, fields)
    }

    pub fn from_untyped_parts(type_name: SmolStr, fields: IndexMap<SmolStr, Value>) -> Self {
        Self::from_canonical_parts(type_name, fields)
    }

    pub(crate) fn from_canonical_parts(
        type_name: SmolStr,
        fields: IndexMap<SmolStr, Value>,
    ) -> Self {
        Self { type_name, fields }
    }

    #[must_use]
    pub fn type_name(&self) -> &SmolStr {
        &self.type_name
    }

    #[must_use]
    pub fn fields(&self) -> &IndexMap<SmolStr, Value> {
        &self.fields
    }

    #[must_use]
    pub fn field(&self, name: &str) -> Option<&Value> {
        self.fields.get(name)
    }

    pub fn field_mut(&mut self, name: &str) -> Option<&mut Value> {
        self.fields.get_mut(name)
    }

    #[must_use]
    pub fn contains_field(&self, name: &str) -> bool {
        self.fields.contains_key(name)
    }

    pub fn set_existing_field(&mut self, name: SmolStr, value: Value) -> bool {
        let Some(slot) = self.fields.get_mut(name.as_str()) else {
            return false;
        };
        *slot = value;
        true
    }
}

/// Error produced when constructing a runtime enum value from non-canonical data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnumValueError {
    UnknownType(TypeId),
    UnknownTypeName(SmolStr),
    AliasCycle(TypeId),
    NotEnum(TypeId),
    UnknownVariant {
        type_name: SmolStr,
        variant_name: SmolStr,
    },
    NumericMismatch {
        type_name: SmolStr,
        variant_name: SmolStr,
        expected: i64,
        actual: i64,
    },
}

impl std::fmt::Display for EnumValueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownType(type_id) => write!(f, "unknown enum type id {}", type_id.0),
            Self::UnknownTypeName(type_name) => write!(f, "unknown enum type '{type_name}'"),
            Self::AliasCycle(type_id) => {
                write!(f, "alias cycle while resolving enum type id {}", type_id.0)
            }
            Self::NotEnum(type_id) => write!(f, "type id {} is not an enum", type_id.0),
            Self::UnknownVariant {
                type_name,
                variant_name,
            } => write!(f, "unknown enum variant '{type_name}#{variant_name}'"),
            Self::NumericMismatch {
                type_name,
                variant_name,
                expected,
                actual,
            } => write!(
                f,
                "enum variant '{type_name}#{variant_name}' has value {expected}, got {actual}"
            ),
        }
    }
}

impl std::error::Error for EnumValueError {}

/// Enum value storing canonical type identity plus display variant data.
#[derive(Debug, Clone)]
pub struct EnumValue {
    type_name: SmolStr,
    variant_name: SmolStr,
    numeric_value: i64,
}

struct EnumTypeParts<'a> {
    name: SmolStr,
    values: &'a [(SmolStr, i64)],
}

impl EnumValue {
    pub fn new(
        registry: &TypeRegistry,
        type_id: TypeId,
        variant_name: &str,
    ) -> Result<Self, EnumValueError> {
        let enum_type = enum_type_parts(registry, type_id)?;
        let (variant_name, numeric_value) = enum_type
            .values
            .iter()
            .find(|(variant, _)| variant.eq_ignore_ascii_case(variant_name))
            .ok_or_else(|| EnumValueError::UnknownVariant {
                type_name: enum_type.name.clone(),
                variant_name: variant_name.into(),
            })?;
        Ok(Self::from_canonical_parts(
            enum_type.name,
            variant_name.clone(),
            *numeric_value,
        ))
    }

    pub fn from_serialized_parts(
        registry: &TypeRegistry,
        type_name: &str,
        variant_name: &str,
        numeric_value: i64,
    ) -> Result<Self, EnumValueError> {
        let type_id = registry
            .lookup(type_name)
            .ok_or_else(|| EnumValueError::UnknownTypeName(type_name.into()))?;
        Self::new_with_numeric(registry, type_id, variant_name, numeric_value)
    }

    pub fn new_with_numeric(
        registry: &TypeRegistry,
        type_id: TypeId,
        variant_name: &str,
        numeric_value: i64,
    ) -> Result<Self, EnumValueError> {
        let value = Self::new(registry, type_id, variant_name)?;
        if value.numeric_value != numeric_value {
            return Err(EnumValueError::NumericMismatch {
                type_name: value.type_name.clone(),
                variant_name: value.variant_name.clone(),
                expected: value.numeric_value,
                actual: numeric_value,
            });
        }
        Ok(value)
    }

    pub(crate) fn from_canonical_parts(
        type_name: SmolStr,
        variant_name: SmolStr,
        numeric_value: i64,
    ) -> Self {
        Self {
            type_name,
            variant_name,
            numeric_value,
        }
    }

    #[must_use]
    pub fn type_name(&self) -> &SmolStr {
        &self.type_name
    }

    #[must_use]
    pub fn variant_name(&self) -> &SmolStr {
        &self.variant_name
    }

    #[must_use]
    pub fn numeric_value(&self) -> i64 {
        self.numeric_value
    }
}

impl PartialEq for EnumValue {
    fn eq(&self, other: &Self) -> bool {
        self.type_name == other.type_name && self.numeric_value == other.numeric_value
    }
}

impl Eq for EnumValue {}

fn enum_type_parts(
    registry: &TypeRegistry,
    type_id: TypeId,
) -> Result<EnumTypeParts<'_>, EnumValueError> {
    let mut current = type_id;
    let mut seen = Vec::new();
    loop {
        if seen.contains(&current) {
            return Err(EnumValueError::AliasCycle(current));
        }
        seen.push(current);
        match registry.get(current) {
            Some(Type::Alias { target, .. }) => current = *target,
            Some(Type::Enum { name, values, .. }) => {
                return Ok(EnumTypeParts {
                    name: name.clone(),
                    values: values.as_slice(),
                });
            }
            Some(_) => return Err(EnumValueError::NotEnum(current)),
            None => return Err(EnumValueError::UnknownType(current)),
        }
    }
}

struct StructTypeParts {
    name: SmolStr,
    fields: Vec<StructFieldSpec>,
}

struct StructFieldSpec {
    name: SmolStr,
    type_id: TypeId,
}

fn struct_type_parts(
    registry: &TypeRegistry,
    type_id: TypeId,
) -> Result<StructTypeParts, ValueConstructionError> {
    let mut current = type_id;
    let mut seen = Vec::new();
    loop {
        if seen.contains(&current) {
            return Err(ValueConstructionError::AliasCycle(current));
        }
        seen.push(current);
        match registry.get(current) {
            Some(Type::Alias { target, .. }) => current = *target,
            Some(Type::Struct { name, fields }) => {
                return Ok(StructTypeParts {
                    name: name.clone(),
                    fields: fields
                        .iter()
                        .map(|field| StructFieldSpec {
                            name: field.name.clone(),
                            type_id: field.type_id,
                        })
                        .collect(),
                });
            }
            Some(Type::Union { name, variants }) => {
                return Ok(StructTypeParts {
                    name: name.clone(),
                    fields: variants
                        .iter()
                        .map(|variant| StructFieldSpec {
                            name: variant.name.clone(),
                            type_id: variant.type_id,
                        })
                        .collect(),
                });
            }
            Some(_) => return Err(ValueConstructionError::NotStructOrUnion(current)),
            None => return Err(ValueConstructionError::UnknownType(current)),
        }
    }
}

fn array_type_parts(
    registry: &TypeRegistry,
    type_id: TypeId,
) -> Result<(TypeId, Vec<(i64, i64)>), ValueConstructionError> {
    let mut current = type_id;
    let mut seen = Vec::new();
    loop {
        if seen.contains(&current) {
            return Err(ValueConstructionError::AliasCycle(current));
        }
        seen.push(current);
        match registry.get(current) {
            Some(Type::Alias { target, .. }) => current = *target,
            Some(Type::Array {
                element,
                dimensions,
            }) => return Ok((*element, dimensions.clone())),
            Some(_) => return Err(ValueConstructionError::NotArray(current)),
            None => return Err(ValueConstructionError::UnknownType(current)),
        }
    }
}

fn array_len(dimensions: &[(i64, i64)]) -> Result<usize, ValueConstructionError> {
    let mut total: i128 = 1;
    for (lower, upper) in dimensions {
        if upper < lower {
            return Err(ValueConstructionError::InvalidArrayBounds {
                dimensions: dimensions.to_vec(),
            });
        }
        total *= (*upper as i128) - (*lower as i128) + 1;
    }
    usize::try_from(total).map_err(|_| ValueConstructionError::InvalidArrayBounds {
        dimensions: dimensions.to_vec(),
    })
}

fn value_matches_type(registry: &TypeRegistry, type_id: TypeId, value: &Value) -> bool {
    let Some(ty) = registry.get(type_id) else {
        return false;
    };
    match ty {
        Type::Alias { target, .. } => value_matches_type(registry, *target, value),
        Type::Bool => matches!(value, Value::Bool(_)),
        Type::SInt => matches!(value, Value::SInt(_)),
        Type::Int => matches!(value, Value::Int(_)),
        Type::DInt => matches!(value, Value::DInt(_)),
        Type::LInt => matches!(value, Value::LInt(_)),
        Type::USInt => matches!(value, Value::USInt(_)),
        Type::UInt => matches!(value, Value::UInt(_)),
        Type::UDInt => matches!(value, Value::UDInt(_)),
        Type::ULInt => matches!(value, Value::ULInt(_)),
        Type::Real => matches!(value, Value::Real(_)),
        Type::LReal => matches!(value, Value::LReal(_)),
        Type::Byte => matches!(value, Value::Byte(_)),
        Type::Word => matches!(value, Value::Word(_)),
        Type::DWord => matches!(value, Value::DWord(_)),
        Type::LWord => matches!(value, Value::LWord(_)),
        Type::Time => matches!(value, Value::Time(_)),
        Type::LTime => matches!(value, Value::LTime(_)),
        Type::Date => matches!(value, Value::Date(_)),
        Type::LDate => matches!(value, Value::LDate(_)),
        Type::Tod => matches!(value, Value::Tod(_)),
        Type::LTod => matches!(value, Value::LTod(_)),
        Type::Dt => matches!(value, Value::Dt(_)),
        Type::Ldt => matches!(value, Value::Ldt(_)),
        Type::String { .. } => matches!(value, Value::String(_)),
        Type::WString { .. } => matches!(value, Value::WString(_)),
        Type::Char => matches!(value, Value::Char(_)),
        Type::WChar => matches!(value, Value::WChar(_)),
        Type::Reference { .. } | Type::Pointer { .. } => {
            matches!(value, Value::Reference(_) | Value::Null)
        }
        Type::Subrange { base, lower, upper } => {
            integer_value_in_range(value, *base, *lower, *upper)
        }
        Type::Enum { .. } => matches_enum_type(registry, type_id, value),
        Type::Struct { .. } | Type::Union { .. } => matches_struct_type(registry, type_id, value),
        Type::Array { .. } => matches_array_type(registry, type_id, value),
        Type::Null => matches!(value, Value::Null),
        Type::Unknown
        | Type::Void
        | Type::FunctionBlock { .. }
        | Type::Class { .. }
        | Type::Interface { .. }
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
        | Type::AnyDate => false,
    }
}

fn integer_value_in_range(value: &Value, base: TypeId, lower: i64, upper: i64) -> bool {
    let Some(value) = integer_value(value, base) else {
        return false;
    };
    (lower..=upper).contains(&value)
}

fn integer_value(value: &Value, base: TypeId) -> Option<i64> {
    match (base, value) {
        (TypeId::SINT, Value::SInt(value)) => Some(i64::from(*value)),
        (TypeId::INT, Value::Int(value)) => Some(i64::from(*value)),
        (TypeId::DINT, Value::DInt(value)) => Some(i64::from(*value)),
        (TypeId::LINT, Value::LInt(value)) => Some(*value),
        (TypeId::USINT, Value::USInt(value)) => Some(i64::from(*value)),
        (TypeId::UINT, Value::UInt(value)) => Some(i64::from(*value)),
        (TypeId::UDINT, Value::UDInt(value)) => Some(i64::from(*value)),
        (TypeId::ULINT, Value::ULInt(value)) => i64::try_from(*value).ok(),
        _ => None,
    }
}

fn matches_enum_type(registry: &TypeRegistry, type_id: TypeId, value: &Value) -> bool {
    let Value::Enum(enum_value) = value else {
        return false;
    };
    EnumValue::new_with_numeric(
        registry,
        type_id,
        enum_value.variant_name().as_str(),
        enum_value.numeric_value(),
    )
    .is_ok_and(|declared| declared.type_name() == enum_value.type_name())
}

fn matches_struct_type(registry: &TypeRegistry, type_id: TypeId, value: &Value) -> bool {
    let Value::Struct(struct_value) = value else {
        return false;
    };
    let Ok(declared) = struct_type_parts(registry, type_id) else {
        return false;
    };
    if struct_value.type_name() != &declared.name {
        return false;
    }
    if struct_value.fields().len() != declared.fields.len() {
        return false;
    }
    declared.fields.iter().all(|field| {
        struct_value
            .fields()
            .get(&field.name)
            .is_some_and(|value| value_matches_type(registry, field.type_id, value))
    })
}

fn matches_array_type(registry: &TypeRegistry, type_id: TypeId, value: &Value) -> bool {
    let Value::Array(array_value) = value else {
        return false;
    };
    let Ok((element_type, dimensions)) = array_type_parts(registry, type_id) else {
        return false;
    };
    if array_value.dimensions() != dimensions.as_slice() {
        return false;
    }
    let Ok(expected_len) = array_len(&dimensions) else {
        return false;
    };
    array_value.elements().len() == expected_len
        && array_value
            .elements()
            .iter()
            .all(|element| value_matches_type(registry, element_type, element))
}

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Bool(_) => "BOOL",
        Value::SInt(_) => "SINT",
        Value::Int(_) => "INT",
        Value::DInt(_) => "DINT",
        Value::LInt(_) => "LINT",
        Value::USInt(_) => "USINT",
        Value::UInt(_) => "UINT",
        Value::UDInt(_) => "UDINT",
        Value::ULInt(_) => "ULINT",
        Value::Real(_) => "REAL",
        Value::LReal(_) => "LREAL",
        Value::Byte(_) => "BYTE",
        Value::Word(_) => "WORD",
        Value::DWord(_) => "DWORD",
        Value::LWord(_) => "LWORD",
        Value::Time(_) => "TIME",
        Value::LTime(_) => "LTIME",
        Value::Date(_) => "DATE",
        Value::LDate(_) => "LDATE",
        Value::Tod(_) => "TOD",
        Value::LTod(_) => "LTOD",
        Value::Dt(_) => "DT",
        Value::Ldt(_) => "LDT",
        Value::String(_) => "STRING",
        Value::WString(_) => "WSTRING",
        Value::Char(_) => "CHAR",
        Value::WChar(_) => "WCHAR",
        Value::Array(_) => "ARRAY",
        Value::Struct(_) => "STRUCT",
        Value::Enum(_) => "ENUM",
        Value::Reference(_) => "REFERENCE",
        Value::Instance(_) => "INSTANCE",
        Value::Null => "NULL",
    }
}

/// Runtime value representation for IEC 61131-3 types.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),

    SInt(i8),
    Int(i16),
    DInt(i32),
    LInt(i64),

    USInt(u8),
    UInt(u16),
    UDInt(u32),
    ULInt(u64),

    Real(f32),
    LReal(f64),

    Byte(u8),
    Word(u16),
    DWord(u32),
    LWord(u64),

    Time(Duration),
    LTime(Duration),
    Date(DateValue),
    LDate(LDateValue),
    Tod(TimeOfDayValue),
    LTod(LTimeOfDayValue),
    Dt(DateTimeValue),
    Ldt(LDateTimeValue),

    String(SmolStr),
    WString(String),
    Char(u8),
    WChar(u16),

    Array(Box<ArrayValue>),
    Struct(Arc<StructValue>),
    Enum(Box<EnumValue>),

    Reference(Option<ValueRef>),
    Instance(InstanceId),

    Null,
}

pub(crate) fn normalize_assignment_for_target(target: &Value, value: Value) -> Value {
    match (target, value) {
        (Value::Reference(_), Value::Null) => Value::Reference(None),
        (_, value) => value,
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Bool(value)
    }
}

impl From<i16> for Value {
    fn from(value: i16) -> Self {
        Value::Int(value)
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Self {
        Value::DInt(value)
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::LInt(value)
    }
}

impl From<u8> for Value {
    fn from(value: u8) -> Self {
        Value::USInt(value)
    }
}

impl From<u16> for Value {
    fn from(value: u16) -> Self {
        Value::UInt(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trust_hir::types::StructField;

    #[test]
    fn enum_value_new_resolves_alias_to_canonical_enum_type() {
        let mut registry = TypeRegistry::new();
        let base = registry.register_enum(
            "Solo",
            TypeId::INT,
            vec![("S0".into(), 0), ("S1".into(), 1)],
        );
        let alias = registry.register(
            "AliasSolo",
            Type::Alias {
                name: "AliasSolo".into(),
                target: base,
            },
        );

        let from_base = EnumValue::new(&registry, base, "S1").expect("base enum value");
        let from_alias = EnumValue::new(&registry, alias, "s1").expect("alias enum value");

        assert_eq!(from_alias.type_name().as_str(), "Solo");
        assert_eq!(from_alias.variant_name().as_str(), "S1");
        assert_eq!(from_alias.numeric_value(), 1);
        assert_eq!(from_alias, from_base);
    }

    #[test]
    fn enum_value_from_serialized_parts_canonicalizes_and_validates_numeric_value() {
        let mut registry = TypeRegistry::new();
        registry.register_enum(
            "Solo",
            TypeId::INT,
            vec![("S0".into(), 0), ("S1".into(), 1)],
        );

        let value = EnumValue::from_serialized_parts(&registry, "SOLO", "s1", 1)
            .expect("serialized enum value");
        assert_eq!(value.type_name().as_str(), "Solo");
        assert_eq!(value.variant_name().as_str(), "S1");

        let error = EnumValue::from_serialized_parts(&registry, "SOLO", "S1", 0)
            .expect_err("numeric mismatch should fail");
        assert!(matches!(error, EnumValueError::NumericMismatch { .. }));
    }

    #[test]
    fn struct_value_new_canonicalizes_alias_fields_and_rejects_type_drift() {
        let mut registry = TypeRegistry::new();
        let point = registry.register_struct(
            "Point",
            vec![
                StructField {
                    name: "x".into(),
                    type_id: TypeId::INT,
                    address: None,
                    default_initializer: None,
                },
                StructField {
                    name: "y".into(),
                    type_id: TypeId::INT,
                    address: None,
                    default_initializer: None,
                },
            ],
        );
        let alias = registry.register(
            "PointAlias",
            Type::Alias {
                name: "PointAlias".into(),
                target: point,
            },
        );
        let fields = [("Y".into(), Value::Int(2)), ("X".into(), Value::Int(1))]
            .into_iter()
            .collect();

        let value = StructValue::new(&registry, alias, fields).expect("alias-backed struct");

        assert_eq!(value.type_name().as_str(), "Point");
        assert_eq!(
            value.fields().keys().cloned().collect::<Vec<_>>(),
            vec![SmolStr::new("x"), SmolStr::new("y")]
        );
        assert_eq!(value.fields().get("x"), Some(&Value::Int(1)));
        assert_eq!(value.fields().get("y"), Some(&Value::Int(2)));

        let bad_fields = [("x".into(), Value::Bool(true)), ("y".into(), Value::Int(2))]
            .into_iter()
            .collect();
        let error =
            StructValue::new(&registry, point, bad_fields).expect_err("wrong field type must fail");
        assert!(matches!(
            error,
            ValueConstructionError::FieldTypeMismatch { .. }
        ));

        let missing_error = StructValue::new(
            &registry,
            point,
            [(SmolStr::new("x"), Value::Int(1))].into_iter().collect(),
        )
        .expect_err("missing field must fail");
        assert!(matches!(
            missing_error,
            ValueConstructionError::MissingField { .. }
        ));

        let extra_error = StructValue::new(
            &registry,
            point,
            [
                (SmolStr::new("x"), Value::Int(1)),
                (SmolStr::new("y"), Value::Int(2)),
                (SmolStr::new("z"), Value::Int(3)),
            ]
            .into_iter()
            .collect(),
        )
        .expect_err("extra field must fail");
        assert!(matches!(
            extra_error,
            ValueConstructionError::ExtraField { .. }
        ));
    }

    #[test]
    fn struct_value_mutator_updates_existing_fields_only() {
        let mut value = StructValue::from_untyped_parts(
            "Point".into(),
            [
                (SmolStr::new("x"), Value::Int(1)),
                (SmolStr::new("y"), Value::Int(2)),
            ]
            .into_iter()
            .collect(),
        );

        assert!(value.contains_field("x"));
        assert_eq!(value.field("x"), Some(&Value::Int(1)));
        assert!(value.set_existing_field("x".into(), Value::Int(10)));
        assert!(!value.set_existing_field("z".into(), Value::Int(99)));
        assert_eq!(value.field("x"), Some(&Value::Int(10)));
        assert!(!value.contains_field("z"));
    }

    #[test]
    fn array_value_new_canonicalizes_alias_and_rejects_shape_or_type_drift() {
        let mut registry = TypeRegistry::new();
        let base = registry.register_array(TypeId::INT, vec![(1, 2)]);
        let alias = registry.register(
            "IntArrayAlias",
            Type::Alias {
                name: "IntArrayAlias".into(),
                target: base,
            },
        );

        let value = ArrayValue::new(&registry, alias, vec![Value::Int(1), Value::Int(2)])
            .expect("alias-backed array");

        assert_eq!(value.dimensions(), &[(1, 2)]);
        assert_eq!(value.elements(), &[Value::Int(1), Value::Int(2)]);

        let count_error = ArrayValue::new(&registry, base, vec![Value::Int(1)])
            .expect_err("wrong element count must fail");
        assert!(matches!(
            count_error,
            ValueConstructionError::ArrayElementCountMismatch { .. }
        ));

        let type_error = ArrayValue::new(&registry, base, vec![Value::Int(1), Value::Bool(false)])
            .expect_err("wrong element type must fail");
        assert!(matches!(
            type_error,
            ValueConstructionError::ArrayElementTypeMismatch { .. }
        ));

        let bounds_error = ArrayValue::from_untyped_parts(Vec::new(), vec![(2, 1)])
            .expect_err("invalid array bounds must fail");
        assert!(matches!(
            bounds_error,
            ValueConstructionError::InvalidArrayBounds { .. }
        ));
    }

    #[test]
    fn array_value_mutators_preserve_shape_contract() {
        let mut value =
            ArrayValue::from_untyped_parts(vec![Value::Int(1), Value::Int(2)], vec![(1, 2)])
                .expect("array value");

        value.elements_mut()[1] = Value::Int(20);
        assert_eq!(value.elements(), &[Value::Int(1), Value::Int(20)]);
        value
            .set_dimensions(vec![(0, 1)])
            .expect("same element count dimensions");
        assert_eq!(value.dimensions(), &[(0, 1)]);

        let error = value
            .set_dimensions(vec![(0, 2)])
            .expect_err("different element count must fail");
        assert!(matches!(
            error,
            ValueConstructionError::ArrayElementCountMismatch { .. }
        ));
    }

    #[test]
    fn array_value_new_validates_array_of_struct_elements() {
        let mut registry = TypeRegistry::new();
        let point = registry.register_struct(
            "Point",
            vec![StructField {
                name: "x".into(),
                type_id: TypeId::INT,
                address: None,
                default_initializer: None,
            }],
        );
        let point_array = registry.register_array(point, vec![(1, 2)]);
        let first = StructValue::new(
            &registry,
            point,
            [(SmolStr::new("x"), Value::Int(1))].into_iter().collect(),
        )
        .expect("first point");
        let second = StructValue::new(
            &registry,
            point,
            [(SmolStr::new("x"), Value::Int(2))].into_iter().collect(),
        )
        .expect("second point");

        let value = ArrayValue::new(
            &registry,
            point_array,
            vec![
                Value::Struct(Arc::new(first)),
                Value::Struct(Arc::new(second)),
            ],
        )
        .expect("array of structs");

        assert_eq!(value.dimensions(), &[(1, 2)]);
        assert_eq!(value.elements().len(), 2);

        let error = ArrayValue::new(
            &registry,
            point_array,
            vec![
                Value::Struct(Arc::new(StructValue::from_untyped_parts(
                    "Point".into(),
                    [(SmolStr::new("x"), Value::Int(1))].into_iter().collect(),
                ))),
                Value::Bool(false),
            ],
        )
        .expect_err("array element type drift must fail");
        assert!(matches!(
            error,
            ValueConstructionError::ArrayElementTypeMismatch { .. }
        ));
    }
}
