use super::{
    coerce_binding_from_io, coerce_binding_to_io, expected_size_for_type,
    validate_process_image_address, IoEnumBinding,
};
use crate::{
    bytecode::{IoBinding, TypeData, TypeTable},
    error::RuntimeError,
    io_address::{IoAddress, IoSize},
    stdlib::conversions::ConversionType,
    value::Value,
};
use alloc::vec::Vec;
use smol_str::SmolStr;

/// Reusable declared-type conversion for an image binding; not an execution admission token.
#[derive(Debug, Clone, Default)]
pub struct IoValueCodec {
    /// Scalar wire category, or no conversion for an untyped binding.
    pub wire_type: Option<ConversionType>,
    /// Declared enum identity and admitted variants, when applicable.
    pub enum_type: Option<IoEnumBinding>,
    /// Maximum narrow-string payload length, when applicable.
    pub string_capacity: Option<u16>,
}
impl IoValueCodec {
    /// Resolve a codec through aliases and scalar/enum declarations.
    pub fn from_type(
        type_id: Option<u32>,
        types: &TypeTable,
        strings: &[SmolStr],
    ) -> Result<Self, RuntimeError> {
        let Some(mut ty) = type_id else {
            return Ok(Self::default());
        };
        let mut result = Self::default();
        for _ in 0..=crate::bytecode::BYTECODE_MAX_CONST_NESTING {
            let entry = types
                .entries
                .get(ty as usize)
                .ok_or(RuntimeError::TypeMismatch)?;
            match &entry.data {
                TypeData::Alias { target_type_id } => ty = *target_type_id,
                TypeData::Subrange { base_type_id, .. } => ty = *base_type_id,
                TypeData::Enum {
                    base_type_id,
                    variants,
                } => {
                    let name = entry
                        .name_idx
                        .and_then(|idx| strings.get(idx as usize))
                        .cloned()
                        .ok_or(RuntimeError::TypeMismatch)?;
                    let mut names = Vec::with_capacity(variants.len());
                    for variant in variants {
                        names.push((
                            strings
                                .get(variant.name_idx as usize)
                                .cloned()
                                .ok_or(RuntimeError::TypeMismatch)?,
                            variant.value,
                        ));
                    }
                    result.enum_type = Some(IoEnumBinding {
                        type_name: name,
                        variants: names,
                    });
                    ty = *base_type_id;
                }
                TypeData::Primitive {
                    prim_id,
                    max_length,
                } => {
                    result.wire_type = Some(match prim_id {
                        1 => ConversionType::Bool,
                        2 => ConversionType::Byte,
                        3 => ConversionType::Word,
                        4 => ConversionType::DWord,
                        5 => ConversionType::LWord,
                        6 => ConversionType::SInt,
                        7 => ConversionType::Int,
                        8 => ConversionType::DInt,
                        9 => ConversionType::LInt,
                        10 => ConversionType::USInt,
                        11 => ConversionType::UInt,
                        12 => ConversionType::UDInt,
                        13 => ConversionType::ULInt,
                        14 => ConversionType::Real,
                        15 => ConversionType::LReal,
                        16 => ConversionType::Time,
                        24 => {
                            if *max_length == 0 {
                                return Err(RuntimeError::TypeMismatch);
                            }
                            result.string_capacity = Some(*max_length);
                            ConversionType::String
                        }
                        26 => ConversionType::Char,
                        27 => ConversionType::WChar,
                        _ => return Err(RuntimeError::TypeMismatch),
                    });
                    return Ok(result);
                }
                _ => return Err(RuntimeError::TypeMismatch),
            }
        }
        Err(RuntimeError::TypeMismatch)
    }
    /// Validate the binding address and apply declared string capacity.
    pub fn address(&self, mut address: IoAddress) -> Result<IoAddress, RuntimeError> {
        if let Some(capacity) = self.string_capacity {
            address.size = IoSize::Bytes(u32::from(capacity));
        } else if let Some(ty) = self.wire_type {
            if expected_size_for_type(ty) != Some(address.size) {
                return Err(RuntimeError::TypeMismatch);
            }
        }
        validate_process_image_address(&address)?;
        Ok(address)
    }
    /// Convert a sampled image value to the declared IEC value.
    pub fn decode(&self, value: Value) -> Result<Value, RuntimeError> {
        coerce_binding_from_io(value, self.wire_type, self.enum_type.as_ref())
    }
    /// Convert a declared IEC value to its image representation.
    pub fn encode(&self, value: Value, size: IoSize) -> Result<Value, RuntimeError> {
        coerce_binding_to_io(value, self.wire_type, self.enum_type.as_ref(), size)
    }
}

/// Resolved image address and codec from artifact tables; execution admission remains separate.
#[derive(Debug, Clone)]
pub struct PreparedIoBinding {
    /// Validated physical or hierarchical image address.
    pub address: IoAddress,
    /// Artifact reference identifying the bound storage location.
    pub ref_idx: u32,
    /// Declared artifact type, if the binding is typed.
    pub type_id: Option<u32>,
    /// Prepared value conversion for this binding.
    pub codec: IoValueCodec,
}
impl PreparedIoBinding {
    /// Resolve and validate the address, type and codec of one artifact binding.
    pub fn from_tables(
        binding: &IoBinding,
        types: &TypeTable,
        strings: &[SmolStr],
    ) -> Result<Self, RuntimeError> {
        let text = strings
            .get(binding.address_str_idx as usize)
            .ok_or(RuntimeError::TypeMismatch)?;
        let codec = IoValueCodec::from_type(binding.type_id, types, strings)?;
        let address = codec.address(IoAddress::parse(text)?)?;
        Ok(Self {
            address,
            ref_idx: binding.ref_idx,
            type_id: binding.type_id,
            codec,
        })
    }
}
