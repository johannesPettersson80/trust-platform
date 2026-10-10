use super::*;

/// Indexed UTF-8 strings shared by bytecode records.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StringTable {
    /// Records in wire order; indices into this table are zero-based.
    pub entries: Vec<SmolStr>,
}

/// Indexed type definitions and decoded wire offsets.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypeTable {
    /// Decoded byte offsets within TYPE_TABLE; encoding recomputes them.
    pub offsets: Vec<u32>,
    /// Records in wire order; indices into this table are zero-based.
    pub entries: Vec<TypeEntry>,
}

/// Raw type descriptor; kind and payload agreement is checked by validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeEntry {
    /// Wire record kind.
    pub kind: TypeKind,
    /// Name index in STRING_TABLE.
    pub name_idx: Option<u32>,
    /// Payload corresponding to the record kind.
    pub data: TypeData,
}

/// Wire discriminant for a type descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeKind {
    /// Primitive record.
    Primitive = 0,
    /// Array record.
    Array = 1,
    /// Struct record.
    Struct = 2,
    /// Enum record.
    Enum = 3,
    /// Alias record.
    Alias = 4,
    /// Subrange record.
    Subrange = 5,
    /// Reference record.
    Reference = 6,
    /// Union record.
    Union = 7,
    /// Function Block record.
    FunctionBlock = 8,
    /// Class record.
    Class = 9,
    /// Interface record.
    Interface = 10,
}

impl TypeKind {
    /// Decode a known wire discriminant; return `None` for an unknown value.
    #[must_use]
    pub fn from_raw(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Primitive),
            1 => Some(Self::Array),
            2 => Some(Self::Struct),
            3 => Some(Self::Enum),
            4 => Some(Self::Alias),
            5 => Some(Self::Subrange),
            6 => Some(Self::Reference),
            7 => Some(Self::Union),
            8 => Some(Self::FunctionBlock),
            9 => Some(Self::Class),
            10 => Some(Self::Interface),
            _ => None,
        }
    }
}

/// Type-specific payload; all type indices refer to TYPE_TABLE.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeData {
    /// Primitive record.
    Primitive {
        /// Primitive type identifier from specification 12.
        prim_id: u16,
        /// Declared maximum STRING/WSTRING length, or zero when inapplicable.
        max_length: u16,
    },
    /// Array record.
    Array {
        /// Array element type index.
        elem_type_id: u32,
        /// Inclusive lower and upper bounds in dimension order.
        dims: Vec<(i64, i64)>,
    },
    /// Struct record.
    Struct {
        /// Fields in declaration order.
        fields: Vec<Field>,
    },
    /// Enum record.
    Enum {
        /// Underlying type index.
        base_type_id: u32,
        /// Enumeration names and values in declaration order.
        variants: Vec<EnumVariant>,
    },
    /// Alias record.
    Alias {
        /// Referenced or aliased type index.
        target_type_id: u32,
    },
    /// Subrange record.
    Subrange {
        /// Underlying type index.
        base_type_id: u32,
        /// Inclusive lower bound.
        lower: i64,
        /// Inclusive upper bound.
        upper: i64,
    },
    /// Reference record.
    Reference {
        /// Referenced or aliased type index.
        target_type_id: u32,
    },
    /// Union record.
    Union {
        /// Fields in declaration order.
        fields: Vec<Field>,
    },
    /// Function-block or class POU identity.
    Pou {
        /// Referenced POU identifier.
        pou_id: u32,
    },
    /// Interface record.
    Interface {
        /// Method declarations in dispatch order.
        methods: Vec<InterfaceMethod>,
    },
}

/// Named field in a structure or union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// Name index in STRING_TABLE.
    pub name_idx: u32,
    /// Type index in TYPE_TABLE.
    pub type_id: u32,
}

/// Named enumeration value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariant {
    /// Name index in STRING_TABLE.
    pub name_idx: u32,
    /// Signed enumeration value.
    pub value: i64,
}

/// Interface method name and dispatch slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceMethod {
    /// Name index in STRING_TABLE.
    pub name_idx: u32,
    /// Virtual dispatch slot.
    pub slot: u32,
}
