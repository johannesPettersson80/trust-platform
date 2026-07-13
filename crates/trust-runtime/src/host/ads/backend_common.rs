use std::time::{SystemTime, UNIX_EPOCH};

use trust_ads_core::{
    AdsDataTypeDescriptor, ArrayDimension, IecDataType, SymbolDescriptor, SymbolFlag,
    TransportSecurity,
};

use super::transport::{AdsHandleRequest, AdsPointAddress, AdsResolvedHandle, AdsTransportError};

// Names mirror Beckhoff AdsDef.h; ads-rs exposes these as raw fields only.
const ADS_SYMBOL_FLAG_PERSISTENT: u32 = 1 << 0;
const ADS_SYMBOL_FLAG_READ_ONLY: u32 = 1 << 5;

const ADS_BASE_TYPE_INT: u32 = 2;
const ADS_BASE_TYPE_DINT: u32 = 3;
const ADS_BASE_TYPE_REAL: u32 = 4;
const ADS_BASE_TYPE_LREAL: u32 = 5;
const ADS_BASE_TYPE_SINT: u32 = 16;
const ADS_BASE_TYPE_USINT: u32 = 17;
const ADS_BASE_TYPE_UINT: u32 = 18;
const ADS_BASE_TYPE_UDINT: u32 = 19;
const ADS_BASE_TYPE_LINT: u32 = 20;
const ADS_BASE_TYPE_ULINT: u32 = 21;
const ADS_BASE_TYPE_STRING: u32 = 30;
const ADS_BASE_TYPE_BOOL: u32 = 33;
const ADS_BASE_TYPE_COMPOUND: u32 = 65;

pub(super) fn validate_route_policy(
    route: &trust_ads_core::AdsRoute,
    backend_name: &str,
) -> Result<(), AdsTransportError> {
    if !matches!(route.security.transport, TransportSecurity::Plain) {
        return Err(AdsTransportError::new(format!(
            "Secure ADS is reserved but not implemented by the {backend_name} backend"
        )));
    }
    if route.security.auto_add_route {
        return Err(AdsTransportError::new(
            "auto_add_route=true is reserved for authoring tools; runtime will not write AMS routes",
        ));
    }
    Ok(())
}

pub(super) fn validate_requested_size(request: &AdsHandleRequest) -> Result<(), AdsTransportError> {
    let expected = checked_byte_len(&request.data_type)?;
    if let AdsPointAddress::Index { size, .. } = request.address {
        let actual = usize::try_from(size).map_err(|_| {
            AdsTransportError::new(format!(
                "ADS point '{}' index size exceeds usize",
                request.point_name
            ))
        })?;
        if actual != expected {
            return Err(AdsTransportError::new(format!(
                "ADS point '{}' index size mismatch: expected {expected}, got {actual}",
                request.point_name
            )));
        }
    }
    Ok(())
}

pub(super) fn read_write_address(handle: &AdsResolvedHandle) -> (u32, u32) {
    match handle.address {
        AdsPointAddress::Symbol(_) => (ads::index::RW_SYMVAL_BYHANDLE, handle.handle),
        AdsPointAddress::Index {
            index_group,
            index_offset,
            ..
        } => (index_group, index_offset),
    }
}

pub(super) fn symbol_descriptor_from_ads(
    symbol: &ads::symbol::Symbol,
    types: &ads::symbol::TypeMap,
) -> Result<Option<SymbolDescriptor>, AdsTransportError> {
    let type_info = types.get(symbol.typ.as_str());
    let Some(data_type) = data_type_descriptor_from_ads(symbol, type_info)? else {
        return Ok(None);
    };
    let byte_size = u32::try_from(symbol.size).map_err(|_| {
        AdsTransportError::new(format!(
            "ADS symbol '{}' size {} exceeds u32",
            symbol.name, symbol.size
        ))
    })?;
    let mut descriptor = SymbolDescriptor::new(
        symbol.name.clone(),
        data_type,
        symbol.ix_group,
        symbol.ix_offset,
        byte_size,
    )
    .with_flag(SymbolFlag::Read);
    if symbol.flags & ADS_SYMBOL_FLAG_READ_ONLY == 0 {
        descriptor = descriptor.with_flag(SymbolFlag::Write);
    }
    if symbol.flags & ADS_SYMBOL_FLAG_PERSISTENT != 0 {
        // ADS exposes remanent symbol state as the Persistent bit; there is no
        // separate RETAIN bit in the symbol header. Mark both so runtime
        // guardrails can treat all remanent remote state conservatively.
        descriptor = descriptor
            .with_flag(SymbolFlag::Persistent)
            .with_flag(SymbolFlag::Retain);
    }
    descriptor
        .validate_byte_size()
        .map_err(|err| AdsTransportError::new(err.to_string()))?;
    Ok(Some(descriptor))
}

fn data_type_descriptor_from_ads(
    symbol: &ads::symbol::Symbol,
    type_info: Option<&ads::symbol::Type>,
) -> Result<Option<AdsDataTypeDescriptor>, AdsTransportError> {
    let base_type = type_info
        .filter(|_| symbol.base_type == ADS_BASE_TYPE_COMPOUND)
        .map_or(symbol.base_type, |data_type| data_type.base_type);
    let source_name = type_info.map_or(symbol.typ.as_str(), |data_type| data_type.name.as_str());
    let Some(iec_type) = iec_type_from_ads(base_type, source_name) else {
        return Ok(None);
    };
    let dimensions = type_info
        .map(|data_type| {
            data_type
                .array
                .iter()
                .map(|(lower, upper)| ArrayDimension {
                    lower: i64::from(*lower),
                    upper: i64::from(*upper),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let string_len = if matches!(iec_type, IecDataType::String) {
        Some(string_capacity(symbol, type_info, dimensions.as_slice())?)
    } else {
        None
    };
    Ok(Some(AdsDataTypeDescriptor {
        source_name: symbol.typ.clone(),
        iec_type,
        dimensions,
        string_len,
    }))
}

fn iec_type_from_ads(base_type: u32, type_name: &str) -> Option<IecDataType> {
    let leaf_name = scalar_type_name(type_name);
    match leaf_name.as_str() {
        "BOOL" => Some(IecDataType::Bool),
        "SINT" => Some(IecDataType::Sint),
        "INT" => Some(IecDataType::Int),
        "DINT" => Some(IecDataType::Dint),
        "LINT" => Some(IecDataType::Lint),
        "USINT" => Some(IecDataType::Usint),
        "UINT" => Some(IecDataType::Uint),
        "UDINT" => Some(IecDataType::Udint),
        "ULINT" => Some(IecDataType::Ulint),
        "REAL" => Some(IecDataType::Real),
        "LREAL" => Some(IecDataType::Lreal),
        "BYTE" => Some(IecDataType::Byte),
        "WORD" => Some(IecDataType::Word),
        "DWORD" => Some(IecDataType::Dword),
        "LWORD" => Some(IecDataType::Lword),
        value if value.starts_with("STRING") => Some(IecDataType::String),
        _ => match base_type {
            ADS_BASE_TYPE_INT => Some(IecDataType::Int),
            ADS_BASE_TYPE_DINT => Some(IecDataType::Dint),
            ADS_BASE_TYPE_REAL => Some(IecDataType::Real),
            ADS_BASE_TYPE_LREAL => Some(IecDataType::Lreal),
            ADS_BASE_TYPE_SINT => Some(IecDataType::Sint),
            ADS_BASE_TYPE_USINT => Some(IecDataType::Usint),
            ADS_BASE_TYPE_UINT => Some(IecDataType::Uint),
            ADS_BASE_TYPE_UDINT => Some(IecDataType::Udint),
            ADS_BASE_TYPE_LINT => Some(IecDataType::Lint),
            ADS_BASE_TYPE_ULINT => Some(IecDataType::Ulint),
            ADS_BASE_TYPE_STRING => Some(IecDataType::String),
            ADS_BASE_TYPE_BOOL => Some(IecDataType::Bool),
            _ => None,
        },
    }
}

fn scalar_type_name(type_name: &str) -> String {
    let upper = type_name.trim().to_ascii_uppercase();
    if let Some((_, leaf)) = upper.rsplit_once(" OF ") {
        leaf.trim().to_string()
    } else {
        upper
    }
}

fn string_capacity(
    symbol: &ads::symbol::Symbol,
    type_info: Option<&ads::symbol::Type>,
    dimensions: &[ArrayDimension],
) -> Result<u16, AdsTransportError> {
    let total_size = type_info.map_or(symbol.size, |data_type| data_type.size);
    let elements = dimensions.iter().try_fold(1usize, |acc, dimension| {
        acc.checked_mul(dimension.len().map_err(map_mapping_error)?)
            .ok_or_else(|| AdsTransportError::new("ADS STRING array element count overflowed"))
    })?;
    if elements == 0 || total_size < elements {
        return Err(AdsTransportError::new(format!(
            "ADS STRING symbol '{}' has invalid size metadata",
            symbol.name
        )));
    }
    let scalar_size = total_size / elements;
    if scalar_size == 0 || !total_size.is_multiple_of(elements) {
        return Err(AdsTransportError::new(format!(
            "ADS STRING symbol '{}' size is not divisible by array elements",
            symbol.name
        )));
    }
    u16::try_from(scalar_size - 1).map_err(|_| {
        AdsTransportError::new(format!(
            "ADS STRING symbol '{}' capacity exceeds u16",
            symbol.name
        ))
    })
}

pub(super) fn checked_byte_len(
    descriptor: &AdsDataTypeDescriptor,
) -> Result<usize, AdsTransportError> {
    let byte_len = descriptor.byte_len().map_err(map_mapping_error)?;
    u32::try_from(byte_len)
        .map_err(|_| AdsTransportError::new("ADS point byte length exceeds u32"))?;
    Ok(byte_len)
}

pub(super) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

pub(super) fn map_mapping_error(error: impl std::fmt::Display) -> AdsTransportError {
    AdsTransportError::new(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persistent_ads_symbol_sets_retain_guardrail_flag() {
        let symbol = ads::symbol::Symbol {
            name: "GVL.RetainedSetpoint".to_string(),
            ix_group: 0x4020,
            ix_offset: 0,
            typ: "DINT".to_string(),
            size: 4,
            base_type: ADS_BASE_TYPE_DINT,
            flags: ADS_SYMBOL_FLAG_PERSISTENT,
        };
        let descriptor = symbol_descriptor_from_ads(&symbol, &ads::symbol::TypeMap::default())
            .expect("symbol descriptor")
            .expect("supported scalar symbol");

        assert!(descriptor.flags.contains(&SymbolFlag::Persistent));
        assert!(descriptor.flags.contains(&SymbolFlag::Retain));
        assert!(descriptor.flags.contains(&SymbolFlag::Write));
    }

    #[test]
    fn readonly_ads_symbol_does_not_get_write_flag() {
        let symbol = ads::symbol::Symbol {
            name: "GVL.ReadOnlyStatus".to_string(),
            ix_group: 0x4020,
            ix_offset: 0,
            typ: "DINT".to_string(),
            size: 4,
            base_type: ADS_BASE_TYPE_DINT,
            flags: ADS_SYMBOL_FLAG_READ_ONLY,
        };
        let descriptor = symbol_descriptor_from_ads(&symbol, &ads::symbol::TypeMap::default())
            .expect("symbol descriptor")
            .expect("supported scalar symbol");

        assert!(descriptor.flags.contains(&SymbolFlag::Read));
        assert!(!descriptor.flags.contains(&SymbolFlag::Write));
    }

    #[test]
    fn unsupported_compound_symbol_is_not_a_bindable_descriptor() {
        let symbol = ads::symbol::Symbol {
            name: "GVL.LibraryVersion".to_string(),
            ix_group: 0x4020,
            ix_offset: 0,
            typ: "ST_LibVersion".to_string(),
            size: 16,
            base_type: ADS_BASE_TYPE_COMPOUND,
            flags: 0,
        };

        let descriptor = symbol_descriptor_from_ads(&symbol, &ads::symbol::TypeMap::default())
            .expect("unsupported complex symbols are skipped, not fatal");

        assert!(descriptor.is_none());
    }
}
