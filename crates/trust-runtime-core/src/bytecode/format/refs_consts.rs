use super::*;

/// Indexed typed constants.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConstPool {
    /// Records in wire order; indices into this table are zero-based.
    pub entries: Vec<ConstEntry>,
}

/// Typed constant payload; decoded recursively according to TYPE_TABLE.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstEntry {
    /// Type index in TYPE_TABLE.
    pub type_id: u32,
    /// Serialized constant bytes excluding its type/length prefix.
    pub payload: Vec<u8>,
}

/// Indexed references into runtime storage.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RefTable {
    /// Records in wire order; indices into this table are zero-based.
    pub entries: Vec<RefEntry>,
}

/// Reference root and optional field/index path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefEntry {
    /// Storage region containing the reference root.
    pub location: RefLocation,
    /// Frame or instance owner identifier.
    pub owner_id: u32,
    /// Byte position or storage-slot index, as defined by the enclosing record.
    pub offset: u32,
    /// Reference path operations in traversal order.
    pub segments: Vec<RefSegment>,
}

/// Storage region addressed by a reference root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefLocation {
    /// Global storage.
    Global = 0,
    /// Frame-local storage.
    Local = 1,
    /// Function-block/class instance storage.
    Instance = 2,
    /// Process-image storage.
    Io = 3,
    /// Retained storage.
    Retain = 4,
}

impl RefLocation {
    /// Decode a known wire discriminant; return `None` for an unknown value.
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

/// One path operation relative to a reference root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefSegment {
    /// Array indices in dimension order.
    Index(Vec<i64>),
    /// Field record.
    Field {
        /// Field name index in STRING_TABLE.
        name_idx: u32,
    },
}

/// Variable type, retention and initialization metadata.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VarMeta {
    /// Records in wire order; indices into this table are zero-based.
    pub entries: Vec<VarMetaEntry>,
}

/// Metadata for one named storage reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarMetaEntry {
    /// Name index in STRING_TABLE.
    pub name_idx: u32,
    /// Type index in TYPE_TABLE.
    pub type_id: u32,
    /// Reference index in REF_TABLE.
    pub ref_idx: u32,
    /// Retention policy code (0 through 3).
    pub retain: u8,
    /// Optional initializer constant index; forbidden for local VAR_META.
    pub init_const_idx: Option<u32>,
}

/// Constant initializers for retained references.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RetainInit {
    /// Records in wire order; indices into this table are zero-based.
    pub entries: Vec<RetainInitEntry>,
}

/// Retained reference and its initial constant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainInitEntry {
    /// Reference index in REF_TABLE.
    pub ref_idx: u32,
    /// Constant index in CONST_POOL.
    pub const_idx: u32,
}
