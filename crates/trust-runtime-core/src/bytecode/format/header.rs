use super::*;

/// Four-byte STBC container signature.
pub const MAGIC: [u8; 4] = *b"STBC";
/// Minimum serialized header size in bytes.
pub const HEADER_SIZE: u16 = 24;
/// Serialized section-table record size in bytes.
pub const SECTION_ENTRY_SIZE: usize = 12;
/// Header flag requiring CRC32 of the table and payload region.
pub const HEADER_FLAG_CRC32: u32 = 0x0001;

/// Standard section identifiers defined by specification 12.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionId {
    /// String Table record.
    StringTable = 0x0001,
    /// Type Table record.
    TypeTable = 0x0002,
    /// Const Pool record.
    ConstPool = 0x0003,
    /// Ref Table record.
    RefTable = 0x0004,
    /// Pou Index record.
    PouIndex = 0x0005,
    /// Pou Bodies record.
    PouBodies = 0x0006,
    /// Resource Meta record.
    ResourceMeta = 0x0007,
    /// Io Map record.
    IoMap = 0x0008,
    /// Debug Map record.
    DebugMap = 0x0009,
    /// Debug String Table record.
    DebugStringTable = 0x000A,
    /// Var Meta record.
    VarMeta = 0x000B,
    /// Retain Init record.
    RetainInit = 0x000C,
    /// STBC 2.0 declaration templates.
    StorageLayout = 0x000D,
    /// STBC 2.0 persistent root bindings.
    ConstructionRoots = 0x000E,
    /// STBC 2.0 executable initializer ranges.
    Initializers = 0x000F,
    /// STBC 2.0 named access aliases and permissions.
    AccessBindings = 0x0010,
}

impl SectionId {
    /// Decode a known wire discriminant; return `None` for an unknown value.
    #[must_use]
    pub fn from_raw(id: u16) -> Option<Self> {
        match id {
            0x0001 => Some(Self::StringTable),
            0x0002 => Some(Self::TypeTable),
            0x0003 => Some(Self::ConstPool),
            0x0004 => Some(Self::RefTable),
            0x0005 => Some(Self::PouIndex),
            0x0006 => Some(Self::PouBodies),
            0x0007 => Some(Self::ResourceMeta),
            0x0008 => Some(Self::IoMap),
            0x0009 => Some(Self::DebugMap),
            0x000A => Some(Self::DebugStringTable),
            0x000B => Some(Self::VarMeta),
            0x000C => Some(Self::RetainInit),
            0x000D => Some(Self::StorageLayout),
            0x000E => Some(Self::ConstructionRoots),
            0x000F => Some(Self::Initializers),
            0x0010 => Some(Self::AccessBindings),
            _ => None,
        }
    }

    /// Return the serialized numeric identifier.
    #[must_use]
    pub fn as_raw(self) -> u16 {
        self as u16
    }
}

/// Wire section-table record; offsets are absolute container byte positions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionEntry {
    /// Wire identifier.
    pub id: u16,
    /// Wire flags; interpretation depends on the enclosing record.
    pub flags: u16,
    /// Byte position or storage-slot index, as defined by the enclosing record.
    pub offset: u32,
    /// Payload length in bytes, excluding alignment padding.
    pub length: u32,
}

/// Owned raw section; call validation before relying on its id/payload agreement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// Wire identifier.
    pub id: u16,
    /// Wire flags; interpretation depends on the enclosing record.
    pub flags: u16,
    /// Payload corresponding to the record kind.
    pub data: SectionData,
}

/// Decoded section payload, or preserved bytes for an unknown extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionData {
    /// String Table record.
    StringTable(StringTable),
    /// Debug String Table record.
    DebugStringTable(StringTable),
    /// Type Table record.
    TypeTable(TypeTable),
    /// Const Pool record.
    ConstPool(ConstPool),
    /// Ref Table record.
    RefTable(RefTable),
    /// Pou Index record.
    PouIndex(PouIndex),
    /// Pou Bodies record.
    PouBodies(Vec<u8>),
    /// Resource Meta record.
    ResourceMeta(ResourceMeta),
    /// Io Map record.
    IoMap(IoMap),
    /// Debug Map record.
    DebugMap(DebugMap),
    /// Var Meta record.
    VarMeta(VarMeta),
    /// Retain Init record.
    RetainInit(RetainInit),
    /// STBC 2.0 storage declaration templates.
    StorageLayout(StorageLayout),
    /// STBC 2.0 persistent construction roots.
    ConstructionRoots(ConstructionRoots),
    /// STBC 2.0 executable initialization plans.
    Initializers(InitializerIndex),
    /// STBC 2.0 named access aliases and permissions.
    AccessBindings(AccessBindings),
    /// Uninterpreted payload for an unknown section.
    Raw(Vec<u8>),
}
