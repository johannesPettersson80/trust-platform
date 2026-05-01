#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PouIndex {
    pub entries: Vec<PouEntry>,
}

pub const NATIVE_CALL_KIND_FUNCTION: u32 = 0;
pub const NATIVE_CALL_KIND_FUNCTION_BLOCK: u32 = 1;
pub const NATIVE_CALL_KIND_METHOD: u32 = 2;
pub const NATIVE_CALL_KIND_STDLIB: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PouEntry {
    pub id: u32,
    pub name_idx: u32,
    pub kind: PouKind,
    pub code_offset: u32,
    pub code_length: u32,
    pub local_ref_start: u32,
    pub local_ref_count: u32,
    pub return_type_id: Option<u32>,
    pub owner_pou_id: Option<u32>,
    pub params: Vec<ParamEntry>,
    pub class_meta: Option<PouClassMeta>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PouKind {
    Program = 0,
    FunctionBlock = 1,
    Function = 2,
    Class = 3,
    Method = 4,
}

impl PouKind {
    #[must_use]
    pub fn from_raw(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Program),
            1 => Some(Self::FunctionBlock),
            2 => Some(Self::Function),
            3 => Some(Self::Class),
            4 => Some(Self::Method),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_class_like(self) -> bool {
        matches!(self, Self::FunctionBlock | Self::Class)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamEntry {
    pub name_idx: u32,
    pub type_id: u32,
    pub direction: u8,
    pub default_const_idx: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PouClassMeta {
    pub parent_pou_id: Option<u32>,
    pub interfaces: Vec<InterfaceImpl>,
    pub methods: Vec<MethodEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodEntry {
    pub name_idx: u32,
    pub pou_id: u32,
    pub vtable_slot: u32,
    pub access: u8,
    pub flags: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceImpl {
    pub interface_type_id: u32,
    pub vtable_slots: Vec<u32>,
}
