use std::fmt;

use trust_ads_core::{
    AdsDataTypeDescriptor, PointAccess, SymbolDescriptor, SymbolFlag, SymbolSizeError,
};

use super::contracts::AdsPointConfig;
use super::transport::AdsPointAddress;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AdsPointDescriptorError {
    MissingSymbol {
        point_name: String,
        symbol_name: String,
    },
    SymbolSize {
        point_name: String,
        source: SymbolSizeError,
    },
    TypeMismatch {
        point_name: String,
        expected: String,
        actual: String,
    },
    NotReadable {
        point_name: String,
        symbol_name: String,
    },
    NotWritable {
        point_name: String,
        symbol_name: String,
    },
    RetainReadNotAllowed {
        point_name: String,
        symbol_name: String,
    },
    IndexSizeMismatch {
        point_name: String,
        expected: u32,
        actual: u32,
    },
    IndexSizeTooLarge {
        point_name: String,
    },
    InvalidIndexType {
        point_name: String,
        detail: String,
    },
}

impl fmt::Display for AdsPointDescriptorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSymbol {
                point_name,
                symbol_name,
            } => write!(
                f,
                "ADS point '{point_name}' references symbol '{symbol_name}' missing from snapshot"
            ),
            Self::SymbolSize { point_name, source } => {
                write!(f, "ADS point '{point_name}' symbol size validation failed: {source}")
            }
            Self::TypeMismatch {
                point_name,
                expected,
                actual,
            } => write!(
                f,
                "ADS point '{point_name}' type mismatch: config {expected}, remote {actual}"
            ),
            Self::NotReadable {
                point_name,
                symbol_name,
            } => write!(
                f,
                "ADS point '{point_name}' is configured for read but remote symbol '{symbol_name}' is not readable"
            ),
            Self::NotWritable {
                point_name,
                symbol_name,
            } => write!(
                f,
                "ADS point '{point_name}' is configured for write but remote symbol '{symbol_name}' is not writable"
            ),
            Self::RetainReadNotAllowed {
                point_name,
                symbol_name,
            } => write!(
                f,
                "ADS read binding '{point_name}' targets remanent remote symbol '{symbol_name}'; set allow_retain_read=true"
            ),
            Self::IndexSizeMismatch {
                point_name,
                expected,
                actual,
            } => write!(
                f,
                "ADS point '{point_name}' index size mismatch: expected {expected}, got {actual}"
            ),
            Self::IndexSizeTooLarge { point_name } => write!(
                f,
                "ADS point '{point_name}' computed index byte size exceeds u32"
            ),
            Self::InvalidIndexType { point_name, detail } => write!(
                f,
                "ADS point '{point_name}' index type descriptor is invalid: {detail}"
            ),
        }
    }
}

impl std::error::Error for AdsPointDescriptorError {}

pub(super) fn validate_point_against_symbols<'a>(
    point: &AdsPointConfig,
    symbols: &'a [SymbolDescriptor],
) -> Result<Option<&'a SymbolDescriptor>, AdsPointDescriptorError> {
    match &point.address {
        AdsPointAddress::Symbol(symbol_name) => {
            let symbol = symbols
                .iter()
                .find(|symbol| symbol.name == *symbol_name)
                .ok_or_else(|| AdsPointDescriptorError::MissingSymbol {
                    point_name: point.point_name.clone(),
                    symbol_name: symbol_name.clone(),
                })?;
            validate_point_symbol(point, symbol)?;
            Ok(Some(symbol))
        }
        AdsPointAddress::Index { size, .. } => {
            if let Some(symbol) = remote_symbol_for_point(symbols, point) {
                validate_point_symbol(point, symbol)?;
                Ok(Some(symbol))
            } else {
                validate_index_size(point, *size)?;
                Ok(None)
            }
        }
    }
}

pub(super) fn remote_symbol_for_point<'a>(
    symbols: &'a [SymbolDescriptor],
    point: &AdsPointConfig,
) -> Option<&'a SymbolDescriptor> {
    symbols.iter().find(|symbol| match &point.address {
        AdsPointAddress::Symbol(name) => symbol.name == *name,
        AdsPointAddress::Index {
            index_group,
            index_offset,
            size,
        } => {
            symbol.index_group == *index_group
                && symbol.index_offset == *index_offset
                && symbol.byte_size == *size
        }
    })
}

pub(super) fn ads_type_descriptors_match(
    expected: &AdsDataTypeDescriptor,
    actual: &AdsDataTypeDescriptor,
) -> bool {
    expected.iec_type == actual.iec_type
        && expected.dimensions == actual.dimensions
        && expected.string_len == actual.string_len
        && expected.byte_len().ok() == actual.byte_len().ok()
}

pub(super) fn point_reads(access: PointAccess) -> bool {
    matches!(access, PointAccess::Read | PointAccess::ReadWrite)
}

pub(super) fn point_writes(access: PointAccess) -> bool {
    matches!(access, PointAccess::Write | PointAccess::ReadWrite)
}

fn validate_point_symbol(
    point: &AdsPointConfig,
    symbol: &SymbolDescriptor,
) -> Result<(), AdsPointDescriptorError> {
    symbol
        .validate_byte_size()
        .map_err(|source| AdsPointDescriptorError::SymbolSize {
            point_name: point.point_name.clone(),
            source,
        })?;
    if !ads_type_descriptors_match(&point.data_type, &symbol.data_type) {
        return Err(AdsPointDescriptorError::TypeMismatch {
            point_name: point.point_name.clone(),
            expected: format!("{:?}", point.data_type),
            actual: format!("{:?}", symbol.data_type),
        });
    }
    if point_reads(point.access) && !symbol.flags.contains(&SymbolFlag::Read) {
        return Err(AdsPointDescriptorError::NotReadable {
            point_name: point.point_name.clone(),
            symbol_name: symbol.name.clone(),
        });
    }
    if point_writes(point.access) && !symbol.flags.contains(&SymbolFlag::Write) {
        return Err(AdsPointDescriptorError::NotWritable {
            point_name: point.point_name.clone(),
            symbol_name: symbol.name.clone(),
        });
    }
    let remote_retain = symbol.flags.contains(&SymbolFlag::Retain)
        || symbol.flags.contains(&SymbolFlag::Persistent);
    if point_reads(point.access) && remote_retain && !point.allow_retain_read {
        return Err(AdsPointDescriptorError::RetainReadNotAllowed {
            point_name: point.point_name.clone(),
            symbol_name: symbol.name.clone(),
        });
    }
    Ok(())
}

fn validate_index_size(point: &AdsPointConfig, size: u32) -> Result<(), AdsPointDescriptorError> {
    let expected =
        point
            .data_type
            .byte_len()
            .map_err(|err| AdsPointDescriptorError::InvalidIndexType {
                point_name: point.point_name.clone(),
                detail: err.to_string(),
            })?;
    let expected =
        u32::try_from(expected).map_err(|_| AdsPointDescriptorError::IndexSizeTooLarge {
            point_name: point.point_name.clone(),
        })?;
    if expected == size {
        Ok(())
    } else {
        Err(AdsPointDescriptorError::IndexSizeMismatch {
            point_name: point.point_name.clone(),
            expected,
            actual: size,
        })
    }
}
