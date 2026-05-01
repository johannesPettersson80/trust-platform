#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StringTable {
    pub entries: Vec<SmolStr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypeTable {
    pub offsets: Vec<u32>,
    pub entries: Vec<TypeEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeEntry {
    pub kind: TypeKind,
    pub name_idx: Option<u32>,
    pub data: TypeData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeKind {
    Primitive = 0,
    Array = 1,
    Struct = 2,
    Enum = 3,
    Alias = 4,
    Subrange = 5,
    Reference = 6,
    Union = 7,
    FunctionBlock = 8,
    Class = 9,
    Interface = 10,
}

impl TypeKind {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeData {
    Primitive {
        prim_id: u16,
        max_length: u16,
    },
    Array {
        elem_type_id: u32,
        dims: Vec<(i64, i64)>,
    },
    Struct {
        fields: Vec<Field>,
    },
    Enum {
        base_type_id: u32,
        variants: Vec<EnumVariant>,
    },
    Alias {
        target_type_id: u32,
    },
    Subrange {
        base_type_id: u32,
        lower: i64,
        upper: i64,
    },
    Reference {
        target_type_id: u32,
    },
    Union {
        fields: Vec<Field>,
    },
    Pou {
        pou_id: u32,
    },
    Interface {
        methods: Vec<InterfaceMethod>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name_idx: u32,
    pub type_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariant {
    pub name_idx: u32,
    pub value: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceMethod {
    pub name_idx: u32,
    pub slot: u32,
}
