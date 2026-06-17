use std::collections::BTreeSet;

use trust_ads_core::{AdsDataTypeDescriptor, SymbolDescriptor, SymbolFlag, SymbolSnapshot};

/// Default ADS index group for truST-owned server symbols.
pub const DEFAULT_SERVER_SYMBOL_INDEX_GROUP: u32 = 0x4020;

/// Runtime-neutral symbol declaration before the ADS server assigns addresses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerSymbolSpec {
    /// ADS symbol name exposed to external clients.
    pub name: String,
    /// ADS/IEC type descriptor.
    pub data_type: AdsDataTypeDescriptor,
    /// Symbol capability flags.
    pub flags: BTreeSet<SymbolFlag>,
}

impl ServerSymbolSpec {
    /// Creates a readable server symbol spec.
    #[must_use]
    pub fn readable(name: impl Into<String>, data_type: AdsDataTypeDescriptor) -> Self {
        Self {
            name: name.into(),
            data_type,
            flags: BTreeSet::from([SymbolFlag::Read]),
        }
    }

    /// Adds one symbol flag.
    #[must_use]
    pub fn with_flag(mut self, flag: SymbolFlag) -> Self {
        self.flags.insert(flag);
        self
    }
}

/// Errors raised while building the server symbol table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerSymbolError {
    /// Symbol name was empty.
    EmptyName,
    /// Two exposed symbols used the same ADS name.
    DuplicateName {
        /// Duplicated symbol name.
        name: String,
    },
    /// Type descriptor could not compute an ADS byte length.
    InvalidType {
        /// Symbol name.
        name: String,
        /// Human-readable cause.
        cause: String,
    },
    /// Computed symbol size does not fit in ADS `u32`.
    SymbolTooLarge {
        /// Symbol name.
        name: String,
    },
    /// Assigned index offset overflowed ADS `u32`.
    OffsetOverflow {
        /// Symbol name being assigned when overflow happened.
        name: String,
    },
    /// Deterministic JSON serialization failed.
    SnapshotSerialization {
        /// Human-readable cause.
        cause: String,
    },
}

impl core::fmt::Display for ServerSymbolError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyName => write!(f, "ADS server symbol name cannot be empty"),
            Self::DuplicateName { name } => {
                write!(f, "duplicate ADS server symbol name '{name}'")
            }
            Self::InvalidType { name, cause } => {
                write!(f, "ADS server symbol '{name}' has invalid type: {cause}")
            }
            Self::SymbolTooLarge { name } => {
                write!(f, "ADS server symbol '{name}' byte size exceeds u32")
            }
            Self::OffsetOverflow { name } => {
                write!(
                    f,
                    "ADS server symbol offset overflow while assigning '{name}'"
                )
            }
            Self::SnapshotSerialization { cause } => {
                write!(f, "failed to serialize ADS server symbol snapshot: {cause}")
            }
        }
    }
}

impl std::error::Error for ServerSymbolError {}

/// Builds a deterministic ADS symbol snapshot with server-owned addresses.
///
/// The input order is deliberately ignored. Symbols are sorted by ADS name
/// before contiguous offsets are assigned, so the same exposed set produces the
/// same `(index_group, index_offset)` layout.
///
/// # Errors
///
/// Returns an error for empty or duplicate names, invalid type descriptors, or
/// ADS address/size overflow.
pub fn build_server_symbol_snapshot(
    route_name: impl Into<String>,
    specs: impl IntoIterator<Item = ServerSymbolSpec>,
    index_group: u32,
) -> Result<SymbolSnapshot, ServerSymbolError> {
    let mut specs = specs.into_iter().collect::<Vec<_>>();
    specs.sort_by(|left, right| left.name.cmp(&right.name));

    let mut seen = BTreeSet::new();
    let mut offset = 0_u32;
    let mut symbols = Vec::with_capacity(specs.len());
    for spec in specs {
        if spec.name.is_empty() {
            return Err(ServerSymbolError::EmptyName);
        }
        if !seen.insert(spec.name.clone()) {
            return Err(ServerSymbolError::DuplicateName { name: spec.name });
        }

        let byte_len =
            spec.data_type
                .byte_len()
                .map_err(|source| ServerSymbolError::InvalidType {
                    name: spec.name.clone(),
                    cause: source.to_string(),
                })?;
        let byte_size = u32::try_from(byte_len).map_err(|_| ServerSymbolError::SymbolTooLarge {
            name: spec.name.clone(),
        })?;
        let next_offset =
            offset
                .checked_add(byte_size)
                .ok_or_else(|| ServerSymbolError::OffsetOverflow {
                    name: spec.name.clone(),
                })?;

        let mut symbol =
            SymbolDescriptor::new(spec.name, spec.data_type, index_group, offset, byte_size);
        symbol.flags = spec.flags;
        symbols.push(symbol);
        offset = next_offset;
    }

    Ok(SymbolSnapshot::new(route_name, symbols))
}

/// Tracks the local ADS symbol version exposed by `ADSIGRP_SYM_VERSION`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolVersionCounter {
    version: u32,
    last_layout_json: Option<String>,
}

impl Default for SymbolVersionCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolVersionCounter {
    /// Creates a counter starting at version `1`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            version: 1,
            last_layout_json: None,
        }
    }

    /// Returns the current symbol version.
    #[must_use]
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Observes a snapshot and bumps only when its deterministic layout changes.
    ///
    /// The first observation establishes the baseline and keeps version `1`.
    ///
    /// # Errors
    ///
    /// Returns an error if deterministic snapshot serialization fails.
    pub fn observe_snapshot(
        &mut self,
        snapshot: &SymbolSnapshot,
    ) -> Result<u32, ServerSymbolError> {
        let json = snapshot.to_deterministic_json().map_err(|source| {
            ServerSymbolError::SnapshotSerialization {
                cause: source.to_string(),
            }
        })?;
        if let Some(previous) = self.last_layout_json.as_deref() {
            if previous != json {
                self.version = self.version.wrapping_add(1).max(1);
            }
        }
        self.last_layout_json = Some(json);
        Ok(self.version)
    }
}

#[cfg(test)]
mod tests {
    use trust_ads_core::{AdsDataTypeDescriptor, IecDataType, SymbolFlag};

    use super::{
        build_server_symbol_snapshot, ServerSymbolError, ServerSymbolSpec, SymbolVersionCounter,
        DEFAULT_SERVER_SYMBOL_INDEX_GROUP,
    };

    fn real(name: &str) -> ServerSymbolSpec {
        ServerSymbolSpec::readable(
            name,
            AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
        )
    }

    fn bool_spec(name: &str) -> ServerSymbolSpec {
        ServerSymbolSpec::readable(
            name,
            AdsDataTypeDescriptor::scalar("BOOL", IecDataType::Bool),
        )
    }

    #[test]
    fn symbol_assignment_is_stable_across_input_order() {
        let left = build_server_symbol_snapshot(
            "server",
            vec![real("GVL.Setpoint"), bool_spec("GVL.Ready")],
            DEFAULT_SERVER_SYMBOL_INDEX_GROUP,
        )
        .expect("snapshot");
        let right = build_server_symbol_snapshot(
            "server",
            vec![bool_spec("GVL.Ready"), real("GVL.Setpoint")],
            DEFAULT_SERVER_SYMBOL_INDEX_GROUP,
        )
        .expect("snapshot");

        assert_eq!(left, right);
        assert_eq!(left.symbols[0].name, "GVL.Ready");
        assert_eq!(left.symbols[0].index_offset, 0);
        assert_eq!(left.symbols[1].name, "GVL.Setpoint");
        assert_eq!(left.symbols[1].index_offset, 1);
        assert_eq!(left.symbols[1].byte_size, 4);
    }

    #[test]
    fn duplicate_symbol_names_are_rejected() {
        let error = build_server_symbol_snapshot(
            "server",
            vec![real("GVL.Setpoint"), real("GVL.Setpoint")],
            DEFAULT_SERVER_SYMBOL_INDEX_GROUP,
        )
        .expect_err("duplicate rejected");

        assert_eq!(
            error,
            ServerSymbolError::DuplicateName {
                name: "GVL.Setpoint".to_string()
            }
        );
    }

    #[test]
    fn flags_are_preserved_in_assigned_symbols() {
        let snapshot = build_server_symbol_snapshot(
            "server",
            vec![real("GVL.Setpoint").with_flag(SymbolFlag::Write)],
            DEFAULT_SERVER_SYMBOL_INDEX_GROUP,
        )
        .expect("snapshot");

        assert!(snapshot.symbols[0].flags.contains(&SymbolFlag::Read));
        assert!(snapshot.symbols[0].flags.contains(&SymbolFlag::Write));
    }

    #[test]
    fn deterministic_snapshot_json_is_stable() {
        let snapshot = build_server_symbol_snapshot(
            "server",
            vec![real("GVL.Setpoint"), bool_spec("GVL.Ready")],
            DEFAULT_SERVER_SYMBOL_INDEX_GROUP,
        )
        .expect("snapshot");

        let first = snapshot.to_deterministic_json().expect("first json");
        let second = snapshot.to_deterministic_json().expect("second json");

        assert_eq!(first, second);
        assert!(first.ends_with('\n'));
    }

    #[test]
    fn symbol_version_bumps_only_when_layout_changes() {
        let snapshot = build_server_symbol_snapshot(
            "server",
            vec![real("GVL.Setpoint")],
            DEFAULT_SERVER_SYMBOL_INDEX_GROUP,
        )
        .expect("snapshot");
        let same_snapshot = build_server_symbol_snapshot(
            "server",
            vec![real("GVL.Setpoint")],
            DEFAULT_SERVER_SYMBOL_INDEX_GROUP,
        )
        .expect("same snapshot");
        let changed_snapshot = build_server_symbol_snapshot(
            "server",
            vec![real("GVL.Setpoint"), bool_spec("GVL.Ready")],
            DEFAULT_SERVER_SYMBOL_INDEX_GROUP,
        )
        .expect("changed snapshot");
        let mut counter = SymbolVersionCounter::new();

        assert_eq!(counter.observe_snapshot(&snapshot).expect("first"), 1);
        assert_eq!(counter.observe_snapshot(&same_snapshot).expect("same"), 1);
        assert_eq!(
            counter
                .observe_snapshot(&changed_snapshot)
                .expect("changed"),
            2
        );
        assert_eq!(
            counter
                .observe_snapshot(&changed_snapshot)
                .expect("same changed"),
            2
        );
    }
}
