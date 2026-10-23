//! Artifact-selected logical storage widths, independent of external I/O codecs.

/// Temporal layout contract carried by an admitted artifact version.
/// This selects storage/SIZEOF widths, never the logical temporal value range.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StorageAbi {
    /// STBC 1.x/2.0, or a flagged contextual migration: short temporal types
    /// occupy four bytes; long forms occupy eight. Historical aggregate SIZEOF
    /// sums fields even when their relative addresses contain gaps or aliases.
    LegacyTemporal32,
    /// STBC 3.0: every temporal type occupies eight bytes on every platform.
    #[default]
    UniformTemporal64,
}

impl StorageAbi {
    /// Storage/SIZEOF width of TIME, DATE, TOD and DT in bytes.
    #[must_use]
    pub const fn short_temporal_bytes(self) -> u64 {
        match self {
            Self::LegacyTemporal32 => 4,
            Self::UniformTemporal64 => 8,
        }
    }
}
