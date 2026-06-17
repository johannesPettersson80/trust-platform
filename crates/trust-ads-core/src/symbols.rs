use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::mapping::{AdsDataTypeDescriptor, AdsMappingError};

/// Current ADS symbol snapshot schema version.
pub const SYMBOL_SNAPSHOT_SCHEMA_VERSION: u16 = 1;

/// Symbol capability flag reported by an ADS endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolFlag {
    /// Symbol can be read.
    Read,
    /// Symbol can be written.
    Write,
    /// Symbol is persistent on the ADS endpoint.
    Persistent,
    /// Symbol is retained on the ADS endpoint.
    Retain,
}

/// Direction-neutral ADS symbol descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolDescriptor {
    /// ADS symbol name, for example `MAIN.Temperature`.
    pub name: String,
    /// ADS type descriptor.
    pub data_type: AdsDataTypeDescriptor,
    /// ADS index group.
    pub index_group: u32,
    /// ADS index offset.
    pub index_offset: u32,
    /// ADS byte size.
    pub byte_size: u32,
    /// Symbol capability flags.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub flags: BTreeSet<SymbolFlag>,
}

impl SymbolDescriptor {
    /// Creates a descriptor with no flags.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        data_type: AdsDataTypeDescriptor,
        index_group: u32,
        index_offset: u32,
        byte_size: u32,
    ) -> Self {
        Self {
            name: name.into(),
            data_type,
            index_group,
            index_offset,
            byte_size,
            flags: BTreeSet::new(),
        }
    }

    /// Returns this descriptor with one flag added.
    #[must_use]
    pub fn with_flag(mut self, flag: SymbolFlag) -> Self {
        self.flags.insert(flag);
        self
    }

    /// Validates that the endpoint byte size matches the declared ADS type.
    ///
    /// # Errors
    ///
    /// Returns an error when the type descriptor cannot compute a byte length,
    /// the computed length exceeds `u32`, or the endpoint size disagrees.
    pub fn validate_byte_size(&self) -> Result<(), SymbolSizeError> {
        let expected = u32::try_from(self.data_type.byte_len().map_err(|source| {
            SymbolSizeError::DataType {
                symbol: self.name.clone(),
                source,
            }
        })?)
        .map_err(|_| SymbolSizeError::ByteSizeTooLarge {
            symbol: self.name.clone(),
        })?;
        if expected == self.byte_size {
            Ok(())
        } else {
            Err(SymbolSizeError::ByteSizeMismatch {
                symbol: self.name.clone(),
                expected,
                actual: self.byte_size,
            })
        }
    }
}

/// Error returned when symbol byte-size metadata is inconsistent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolSizeError {
    /// The type descriptor could not compute a byte length.
    DataType {
        /// Symbol name.
        symbol: String,
        /// Underlying type mapping error.
        source: AdsMappingError,
    },
    /// The computed byte length exceeded `u32`.
    ByteSizeTooLarge {
        /// Symbol name.
        symbol: String,
    },
    /// Endpoint byte size disagrees with the descriptor.
    ByteSizeMismatch {
        /// Symbol name.
        symbol: String,
        /// Computed byte size.
        expected: u32,
        /// Endpoint-reported byte size.
        actual: u32,
    },
}

impl core::fmt::Display for SymbolSizeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DataType { symbol, source } => {
                write!(f, "symbol '{symbol}' type descriptor is invalid: {source}")
            }
            Self::ByteSizeTooLarge { symbol } => {
                write!(f, "symbol '{symbol}' computed byte size exceeds u32")
            }
            Self::ByteSizeMismatch {
                symbol,
                expected,
                actual,
            } => write!(
                f,
                "symbol '{symbol}' byte size mismatch: expected {expected}, got {actual}"
            ),
        }
    }
}

impl std::error::Error for SymbolSizeError {}

/// Point access requested by the imported ADS interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointAccess {
    /// ADS drives the local point.
    Read,
    /// Local program publishes the point to ADS.
    Write,
    /// Both read and write directions are enabled.
    ReadWrite,
}

/// Point update mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateMode {
    /// Cyclic sum-up poll.
    Poll,
    /// ADS notification subscription.
    Notify,
}

/// Imported ADS point descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedPointDescriptor {
    /// Local declared point name generated or selected by tooling.
    pub point_name: String,
    /// Remote ADS symbol name.
    pub symbol_name: String,
    /// ADS type descriptor.
    pub data_type: AdsDataTypeDescriptor,
    /// Access direction.
    pub access: PointAccess,
    /// Update mode.
    pub mode: UpdateMode,
}

/// Deterministic ADS symbol snapshot for one route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolSnapshot {
    /// Snapshot schema version.
    pub schema_version: u16,
    /// Stable route name.
    pub route_name: String,
    /// Captured symbols.
    pub symbols: Vec<SymbolDescriptor>,
}

impl SymbolSnapshot {
    /// Creates a snapshot and sorts symbols into canonical order.
    #[must_use]
    pub fn new(route_name: impl Into<String>, symbols: Vec<SymbolDescriptor>) -> Self {
        let mut snapshot = Self {
            schema_version: SYMBOL_SNAPSHOT_SCHEMA_VERSION,
            route_name: route_name.into(),
            symbols,
        };
        snapshot.canonicalize();
        snapshot
    }

    /// Sorts symbols into canonical order.
    pub fn canonicalize(&mut self) {
        self.symbols
            .sort_by(|left, right| symbol_sort_key(left).cmp(&symbol_sort_key(right)));
    }

    /// Serializes this snapshot as deterministic pretty JSON with trailing newline.
    ///
    /// # Errors
    ///
    /// Returns a serde error if serialization fails.
    pub fn to_deterministic_json(&self) -> Result<String, serde_json::Error> {
        let mut canonical = self.clone();
        canonical.canonicalize();
        let mut json = serde_json::to_string_pretty(&canonical)?;
        json.push('\n');
        Ok(json)
    }
}

fn symbol_sort_key(symbol: &SymbolDescriptor) -> (&str, u32, u32) {
    (
        symbol.name.as_str(),
        symbol.index_group,
        symbol.index_offset,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping::{AdsDataTypeDescriptor, IecDataType};

    #[test]
    fn snapshot_serialization_is_byte_identical_after_reordering() {
        let temp = SymbolDescriptor::new(
            "MAIN.Temperature",
            AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
            0x4020,
            8,
            4,
        )
        .with_flag(SymbolFlag::Read);
        let ready = SymbolDescriptor::new(
            "GVL.LineReady",
            AdsDataTypeDescriptor::scalar("BOOL", IecDataType::Bool),
            0x4020,
            0,
            1,
        )
        .with_flag(SymbolFlag::Read)
        .with_flag(SymbolFlag::Write);

        let left = SymbolSnapshot::new("line1", vec![temp.clone(), ready.clone()]);
        let right = SymbolSnapshot::new("line1", vec![ready, temp]);

        assert_eq!(
            left.to_deterministic_json().expect("serialize left"),
            right.to_deterministic_json().expect("serialize right")
        );
    }

    #[test]
    fn imported_point_model_round_trips() {
        let point = ImportedPointDescriptor {
            point_name: "line1_temp".to_string(),
            symbol_name: "MAIN.Temperature".to_string(),
            data_type: AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
            access: PointAccess::Read,
            mode: UpdateMode::Notify,
        };
        let json = serde_json::to_string(&point).expect("serialize point");

        assert_eq!(
            serde_json::from_str::<ImportedPointDescriptor>(&json).expect("deserialize point"),
            point
        );
    }

    #[test]
    fn validates_endpoint_byte_size_against_type_descriptor() {
        let symbol = SymbolDescriptor::new(
            "MAIN.Temperature",
            AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
            0x4020,
            0,
            4,
        );
        assert_eq!(symbol.validate_byte_size(), Ok(()));

        let mismatched = SymbolDescriptor::new(
            "MAIN.Temperature",
            AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
            0x4020,
            0,
            8,
        );
        assert!(matches!(
            mismatched.validate_byte_size(),
            Err(SymbolSizeError::ByteSizeMismatch {
                expected: 4,
                actual: 8,
                ..
            })
        ));
    }

    #[test]
    fn validates_array_byte_size_against_type_descriptor() {
        let symbol = SymbolDescriptor::new(
            "GVL.StatusWords",
            AdsDataTypeDescriptor::scalar("WORD", IecDataType::Word)
                .with_dimensions(vec![crate::mapping::ArrayDimension { lower: 1, upper: 4 }]),
            0x4020,
            0,
            8,
        );

        assert_eq!(symbol.validate_byte_size(), Ok(()));
    }
}
