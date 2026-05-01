pub(crate) const MAGIC: [u8; 4] = *b"STBC";
pub(crate) const HEADER_SIZE: u16 = 24;
pub(crate) const SECTION_ENTRY_SIZE: usize = 12;
pub(crate) const HEADER_FLAG_CRC32: u32 = 0x0001;

/// Bytecode decoder errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BytecodeError {
    #[error("invalid bytecode magic")]
    InvalidMagic,
    #[error("unsupported bytecode version {major}.{minor}")]
    UnsupportedVersion { major: u16, minor: u16 },
    #[error("invalid bytecode header: {0}")]
    InvalidHeader(SmolStr),
    #[error("invalid bytecode checksum (expected {expected:#010x}, got {actual:#010x})")]
    InvalidChecksum { expected: u32, actual: u32 },
    #[error("invalid section table: {0}")]
    InvalidSectionTable(SmolStr),
    #[error("section out of bounds")]
    SectionOutOfBounds,
    #[error("section overlap")]
    SectionOverlap,
    #[error("section alignment error")]
    SectionAlignment,
    #[error("unexpected end of input")]
    UnexpectedEof,
    #[error("invalid section data: {0}")]
    InvalidSection(SmolStr),
    #[error("missing required section: {0}")]
    MissingSection(SmolStr),
    #[error("invalid opcode 0x{0:02X}")]
    InvalidOpcode(u8),
    #[error("invalid jump target {0}")]
    InvalidJumpTarget(i32),
    #[error("invalid POU id {0}")]
    InvalidPouId(u32),
    #[error("invalid index {index} for {kind}")]
    InvalidIndex { kind: SmolStr, index: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionId {
    StringTable = 0x0001,
    TypeTable = 0x0002,
    ConstPool = 0x0003,
    RefTable = 0x0004,
    PouIndex = 0x0005,
    PouBodies = 0x0006,
    ResourceMeta = 0x0007,
    IoMap = 0x0008,
    DebugMap = 0x0009,
    DebugStringTable = 0x000A,
    VarMeta = 0x000B,
    RetainInit = 0x000C,
}

impl SectionId {
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
            _ => None,
        }
    }

    #[must_use]
    pub fn as_raw(self) -> u16 {
        self as u16
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionEntry {
    pub id: u16,
    pub flags: u16,
    pub offset: u32,
    pub length: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub id: u16,
    pub flags: u16,
    pub data: SectionData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionData {
    StringTable(StringTable),
    DebugStringTable(StringTable),
    TypeTable(TypeTable),
    ConstPool(ConstPool),
    RefTable(RefTable),
    PouIndex(PouIndex),
    PouBodies(Vec<u8>),
    ResourceMeta(ResourceMeta),
    IoMap(IoMap),
    DebugMap(DebugMap),
    VarMeta(VarMeta),
    RetainInit(RetainInit),
    Raw(Vec<u8>),
}
