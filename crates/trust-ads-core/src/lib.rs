//! Direction-neutral Beckhoff ADS domain types.
//!
//! This crate owns ADS symbol, route, quality, and IEC value mapping records
//! that can be reused by ADS client and server products. It deliberately keeps
//! runtime binding and TwinCAT transport concerns out of this layer.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

/// ADS/IEC value mapping helpers.
pub mod mapping;
/// Per-point communication quality records.
pub mod quality;
/// ADS route and security policy records.
pub mod routing;
/// ADS symbol and import snapshot records.
pub mod symbols;

pub use mapping::{
    ads_bytes_from_value, value_from_ads_bytes, AdsDataTypeDescriptor, AdsMappingError,
    ArrayDimension, IecDataType,
};
pub use quality::{PointQuality, PointStatus, QualityState};
pub use routing::{AdsRoute, AdsSecurityPolicy, AmsNetId, TransportSecurity};
pub use symbols::{
    ImportedPointDescriptor, PointAccess, SymbolDescriptor, SymbolFlag, SymbolSizeError,
    SymbolSnapshot, UpdateMode, SYMBOL_SNAPSHOT_SCHEMA_VERSION,
};
