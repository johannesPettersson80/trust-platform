use super::*;

/// Program organization units and their bytecode ranges.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PouIndex {
    /// Records in wire order; indices into this table are zero-based.
    pub entries: Vec<PouEntry>,
}

/// CALL_NATIVE selector for a function.
pub const NATIVE_CALL_KIND_FUNCTION: u32 = 0;
/// CALL_NATIVE selector for a function block.
pub const NATIVE_CALL_KIND_FUNCTION_BLOCK: u32 = 1;
/// CALL_NATIVE selector for a method.
pub const NATIVE_CALL_KIND_METHOD: u32 = 2;
/// CALL_NATIVE selector for a standard-library function.
pub const NATIVE_CALL_KIND_STDLIB: u32 = 3;

/// POU signature, body range, local-reference range and dispatch metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PouEntry {
    /// Wire identifier.
    pub id: u32,
    /// Name index in STRING_TABLE.
    pub name_idx: u32,
    /// Wire record kind.
    pub kind: PouKind,
    /// Absolute byte offset within POU_BODIES.
    pub code_offset: u32,
    /// POU body length in bytes.
    pub code_length: u32,
    /// First local reference index in REF_TABLE.
    pub local_ref_start: u32,
    /// Number of consecutive base local references.
    pub local_ref_count: u32,
    /// Optional return type index.
    pub return_type_id: Option<u32>,
    /// Optional enclosing POU identifier.
    pub owner_pou_id: Option<u32>,
    /// Declared parameters in call order.
    pub params: Vec<ParamEntry>,
    /// Inheritance/dispatch metadata for a class-like POU.
    pub class_meta: Option<PouClassMeta>,
}

/// Wire discriminant for a program organization unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PouKind {
    /// Program record.
    Program = 0,
    /// Function Block record.
    FunctionBlock = 1,
    /// Function record.
    Function = 2,
    /// Class record.
    Class = 3,
    /// Method record.
    Method = 4,
}

impl PouKind {
    /// Decode a known wire discriminant; return `None` for an unknown value.
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

    /// Whether the POU carries inheritance/dispatch metadata.
    #[must_use]
    pub fn is_class_like(self) -> bool {
        matches!(self, Self::FunctionBlock | Self::Class)
    }
}

/// Parameter signature including direction and optional constant default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamEntry {
    /// Name index in STRING_TABLE.
    pub name_idx: u32,
    /// Type index in TYPE_TABLE.
    pub type_id: u32,
    /// Parameter direction: 0 input, 1 output, 2 in-out.
    pub direction: u8,
    /// Optional parameter default in CONST_POOL (STBC 1.1 and later).
    pub default_const_idx: Option<u32>,
}

/// Inheritance and dispatch tables for a class-like POU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PouClassMeta {
    /// Optional base-class POU identifier.
    pub parent_pou_id: Option<u32>,
    /// Implemented interface mappings.
    pub interfaces: Vec<InterfaceImpl>,
    /// Method declarations in dispatch order.
    pub methods: Vec<MethodEntry>,
}

/// Method dispatch and access metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodEntry {
    /// Name index in STRING_TABLE.
    pub name_idx: u32,
    /// Referenced POU identifier.
    pub pou_id: u32,
    /// Concrete virtual dispatch slot.
    pub vtable_slot: u32,
    /// Encoded method access policy.
    pub access: u8,
    /// Wire flags; interpretation depends on the enclosing record.
    pub flags: u8,
}

/// Mapping from an implemented interface to concrete virtual slots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceImpl {
    /// Implemented interface type index.
    pub interface_type_id: u32,
    /// Concrete virtual slots in interface method order.
    pub vtable_slots: Vec<u32>,
}
