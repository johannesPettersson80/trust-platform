//! Shared process-image byte representation and typed binding conversion.
use crate::{
    error::RuntimeError,
    io_address::{IoAddress, IoSize, PROCESS_IMAGE_AREA_LIMIT},
    memory::IoArea,
    value::Value,
};
use alloc::{format, vec::Vec};
use smol_str::SmolStr;
mod coercion;
mod prepared;
pub use coercion::{
    coerce_binding_from_io, coerce_binding_to_io, coerce_from_io, coerce_scalar_to_io,
    coerce_to_io, expected_size_for_type, io_value_type_name, IoEnumBinding,
};
pub use prepared::{IoValueCodec, PreparedIoBinding};

/// Read a little-endian flat process-image value; absent buffer bytes read as zero.
/// Hierarchical addresses require their separate value store.
pub fn read_flat_image(buffer: &[u8], address: &IoAddress) -> Result<Value, RuntimeError> {
    validate_flat_address(address)?;
    match address.size {
        IoSize::Bit => {
            let byte = buffer.get(address.byte as usize).copied().unwrap_or(0);
            let bit = (byte >> address.bit) & 1;
            Ok(Value::Bool(bit == 1))
        }
        IoSize::Byte => Ok(Value::Byte(
            buffer.get(address.byte as usize).copied().unwrap_or(0),
        )),
        IoSize::Word => {
            let lo = buffer.get(address.byte as usize).copied().unwrap_or(0);
            let hi = buffer.get(address.byte as usize + 1).copied().unwrap_or(0);
            Ok(Value::Word(u16::from_le_bytes([lo, hi])))
        }
        IoSize::DWord => {
            let mut bytes = [0u8; 4];
            for (idx, byte) in bytes.iter_mut().enumerate() {
                *byte = buffer
                    .get(address.byte as usize + idx)
                    .copied()
                    .unwrap_or(0);
            }
            Ok(Value::DWord(u32::from_le_bytes(bytes)))
        }
        IoSize::LWord => {
            let mut bytes = [0u8; 8];
            for (idx, byte) in bytes.iter_mut().enumerate() {
                *byte = buffer
                    .get(address.byte as usize + idx)
                    .copied()
                    .unwrap_or(0);
            }
            Ok(Value::LWord(u64::from_le_bytes(bytes)))
        }
        IoSize::Bytes(len) => {
            let start = address.byte as usize;
            let end = start.saturating_add(len as usize);
            let bytes = if start < buffer.len() {
                &buffer[start..buffer.len().min(end)]
            } else {
                &[]
            };
            let nul = bytes
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(bytes.len());
            let text =
                core::str::from_utf8(&bytes[..nul]).map_err(|_| RuntimeError::TypeMismatch)?;
            Ok(Value::String(SmolStr::new(text)))
        }
    }
}

/// Encode a flat image value, growing the buffer within the process-image limit.
pub fn write_flat_image(
    buffer: &mut Vec<u8>,
    address: &IoAddress,
    value: Value,
) -> Result<(), RuntimeError> {
    validate_flat_address(address)?;
    match address.size {
        IoSize::Bit => match value {
            Value::Bool(flag) => {
                ensure_len(buffer, address.byte as usize)?;
                let byte = &mut buffer[address.byte as usize];
                if flag {
                    *byte |= 1 << address.bit;
                } else {
                    *byte &= !(1 << address.bit);
                }
                Ok(())
            }
            _ => Err(RuntimeError::TypeMismatch),
        },
        IoSize::Byte => match value {
            Value::Byte(byte) => {
                ensure_len(buffer, address.byte as usize)?;
                buffer[address.byte as usize] = byte;
                Ok(())
            }
            _ => Err(RuntimeError::TypeMismatch),
        },
        IoSize::Word => match value {
            Value::Word(word) => {
                ensure_len(buffer, address.byte as usize + 1)?;
                let [lo, hi] = word.to_le_bytes();
                buffer[address.byte as usize] = lo;
                buffer[address.byte as usize + 1] = hi;
                Ok(())
            }
            _ => Err(RuntimeError::TypeMismatch),
        },
        IoSize::DWord => match value {
            Value::DWord(word) => {
                ensure_len(buffer, address.byte as usize + 3)?;
                let bytes = word.to_le_bytes();
                for (idx, byte) in bytes.iter().enumerate() {
                    buffer[address.byte as usize + idx] = *byte;
                }
                Ok(())
            }
            _ => Err(RuntimeError::TypeMismatch),
        },
        IoSize::LWord => match value {
            Value::LWord(word) => {
                ensure_len(buffer, address.byte as usize + 7)?;
                let bytes = word.to_le_bytes();
                for (idx, byte) in bytes.iter().enumerate() {
                    buffer[address.byte as usize + idx] = *byte;
                }
                Ok(())
            }
            _ => Err(RuntimeError::TypeMismatch),
        },
        IoSize::Bytes(len) => match value {
            Value::String(text) => {
                let bytes = text.as_bytes();
                if bytes.len() > len as usize {
                    return Err(RuntimeError::Overflow);
                }
                let start = address.byte as usize;
                let end = start.checked_add(len as usize).ok_or_else(|| {
                    RuntimeError::InvalidIoAddress("process image address overflow".into())
                })?;
                ensure_len(buffer, end.saturating_sub(1))?;
                buffer[start..end].fill(0);
                buffer[start..start + bytes.len()].copy_from_slice(bytes);
                Ok(())
            }
            _ => Err(RuntimeError::TypeMismatch),
        },
    }
}

fn validate_flat_address(address: &IoAddress) -> Result<(), RuntimeError> {
    validate_process_image_address(address)?;
    if address.path.len() > 1 {
        return Err(RuntimeError::InvalidIoAddress(
            "hierarchical address requires its separate value store".into(),
        ));
    }
    Ok(())
}

/// Owned identity for a hierarchical process-image value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IoAddressKey {
    area: IoArea,
    size: IoSize,
    path: Vec<u32>,
    bit: u8,
}

impl From<&IoAddress> for IoAddressKey {
    fn from(address: &IoAddress) -> Self {
        Self {
            area: address.area,
            size: address.size,
            path: address.path.clone(),
            bit: address.bit,
        }
    }
}

/// Ensure an addressed byte exists, rejecting growth beyond the process-image limit.
pub fn ensure_len(buffer: &mut Vec<u8>, index: usize) -> Result<(), RuntimeError> {
    if index >= PROCESS_IMAGE_AREA_LIMIT {
        return process_image_area_limit_error("process image address");
    }
    if buffer.len() <= index {
        buffer.resize(index + 1, 0);
    }
    Ok(())
}

/// Validate the area, bit selection and extent of a flat or hierarchical address.
pub fn validate_process_image_address(address: &IoAddress) -> Result<(), RuntimeError> {
    if address.wildcard {
        return Err(RuntimeError::InvalidIoAddress(
            "wildcard process image address is not concrete".into(),
        ));
    }
    if matches!(address.size, IoSize::Bit) && address.bit >= 8 {
        return Err(RuntimeError::InvalidIoAddress(
            "process image bit exceeds byte width".into(),
        ));
    }
    if address.path.len() > 1 {
        return Ok(());
    }
    let byte = address.byte as usize;
    let extra = match address.size {
        IoSize::Bit | IoSize::Byte => 0,
        IoSize::Word => 1,
        IoSize::DWord => 3,
        IoSize::LWord => 7,
        IoSize::Bytes(0) => {
            return Err(RuntimeError::InvalidIoAddress(
                "zero-length process image byte window".into(),
            ));
        }
        IoSize::Bytes(len) => usize::try_from(len - 1).map_err(|_| {
            RuntimeError::InvalidIoAddress("process image byte window is too large".into())
        })?,
    };
    let last_byte = byte
        .checked_add(extra)
        .ok_or_else(|| RuntimeError::InvalidIoAddress("process image address overflow".into()))?;
    if last_byte >= PROCESS_IMAGE_AREA_LIMIT {
        return process_image_area_limit_error("process image address");
    }
    Ok(())
}

/// Reject an image area whose declared size exceeds the shared limit.
pub fn validate_process_image_area_len(area: &str, len: usize) -> Result<(), RuntimeError> {
    if len > PROCESS_IMAGE_AREA_LIMIT {
        return process_image_area_limit_error(area);
    }
    Ok(())
}

/// Construct the shared image-size limit error for the named area.
pub fn process_image_area_limit_error(area: &str) -> Result<(), RuntimeError> {
    Err(RuntimeError::InvalidIoAddress(
        format!(
            "{area} exceeds {} byte process image area limit",
            PROCESS_IMAGE_AREA_LIMIT
        )
        .into(),
    ))
}

impl IoAddressKey {
    /// Logical bytes copied when cloning the owned hierarchical path.
    pub fn clone_allocation_bytes(&self) -> Option<usize> {
        self.path.len().checked_mul(core::mem::size_of::<u32>())
    }
}
