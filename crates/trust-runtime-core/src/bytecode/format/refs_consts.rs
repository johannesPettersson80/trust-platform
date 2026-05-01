#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConstPool {
    pub entries: Vec<ConstEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstEntry {
    pub type_id: u32,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RefTable {
    pub entries: Vec<RefEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefEntry {
    pub location: RefLocation,
    pub owner_id: u32,
    pub offset: u32,
    pub segments: Vec<RefSegment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefLocation {
    Global = 0,
    Local = 1,
    Instance = 2,
    Io = 3,
    Retain = 4,
}

impl RefLocation {
    #[must_use]
    pub fn from_raw(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Global),
            1 => Some(Self::Local),
            2 => Some(Self::Instance),
            3 => Some(Self::Io),
            4 => Some(Self::Retain),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefSegment {
    Index(Vec<i64>),
    Field { name_idx: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VarMeta {
    pub entries: Vec<VarMetaEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarMetaEntry {
    pub name_idx: u32,
    pub type_id: u32,
    pub ref_idx: u32,
    pub retain: u8,
    pub init_const_idx: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RetainInit {
    pub entries: Vec<RetainInitEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainInitEntry {
    pub ref_idx: u32,
    pub const_idx: u32,
}
