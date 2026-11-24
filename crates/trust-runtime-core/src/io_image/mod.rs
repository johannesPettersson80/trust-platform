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

/// A copy of the flat process images, `%I`, `%Q` and `%M`, which a runtime keeps to read its
/// bindings after a scan without the engine (checklist P1-6). A copy keeps its storage, so copying
/// images of the sizes copied before allocates nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlatImages {
    /// The input image.
    pub inputs: Vec<u8>,
    /// The output image.
    pub outputs: Vec<u8>,
    /// The marker image.
    pub memory: Vec<u8>,
}

impl FlatImages {
    /// The image of `area`.
    #[must_use]
    pub fn area(&self, area: IoArea) -> &[u8] {
        match area {
            IoArea::Input => &self.inputs,
            IoArea::Output => &self.outputs,
            IoArea::Memory => &self.memory,
        }
    }

    /// The raw value at the flat `address`, read as the engine reads it from its process image: an
    /// address that is not concrete, is hierarchical or reaches beyond its area's image is refused.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::InvalidIoAddress`] for such an address.
    pub fn read(&self, address: &IoAddress) -> Result<Value, RuntimeError> {
        validate_process_image_address(address)?;
        let image = self.area(address.area);
        check_image_window(address, image.len())?;
        read_flat_image(image, address)
    }
}

/// Refuse a flat `address` that reaches beyond `len` bytes, the length of its area's image.
pub(crate) fn check_image_window(address: &IoAddress, len: usize) -> Result<(), RuntimeError> {
    if address
        .flat_byte_end()?
        .is_some_and(|end| end as usize > len)
    {
        return Err(RuntimeError::InvalidIoAddress(
            "address exceeds admitted process image".into(),
        ));
    }
    Ok(())
}

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

/// The bytes that `value` gives a flat process image at an address of `size`, little-endian as
/// [`write_flat_image`] writes them, into `out`: for a bit one byte, 0 or 1, and for a text its
/// bytes padded with zeros to the address's length (specification 48 Appendix C, C.6). `None` for
/// a value of another type than the address takes, and for bytes that `out` cannot hold.
pub fn flat_image_bytes(size: IoSize, value: &Value, out: &mut [u8]) -> Option<usize> {
    let mut fixed = [0_u8; 8];
    let width = match (size, value) {
        (IoSize::Bit, Value::Bool(flag)) => {
            fixed[0] = u8::from(*flag);
            1
        }
        (IoSize::Byte, Value::Byte(byte)) => {
            fixed[0] = *byte;
            1
        }
        (IoSize::Word, Value::Word(word)) => {
            fixed[..2].copy_from_slice(&word.to_le_bytes());
            2
        }
        (IoSize::DWord, Value::DWord(word)) => {
            fixed[..4].copy_from_slice(&word.to_le_bytes());
            4
        }
        (IoSize::LWord, Value::LWord(word)) => {
            fixed = word.to_le_bytes();
            8
        }
        (IoSize::Bytes(len), Value::String(text)) => {
            let out = out.get_mut(..usize::try_from(len).ok()?)?;
            let text = text.as_bytes();
            out.fill(0);
            out.get_mut(..text.len())?.copy_from_slice(text);
            return Some(out.len());
        }
        _ => return None,
    };
    out.get_mut(..width)?.copy_from_slice(&fixed[..width]);
    Some(width)
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
        IoSize::Byte | IoSize::Word | IoSize::DWord | IoSize::LWord => {
            let mut bytes = [0_u8; 8];
            let width = flat_image_bytes(address.size, &value, &mut bytes)
                .ok_or(RuntimeError::TypeMismatch)?;
            let start = address.byte as usize;
            ensure_len(buffer, start + width - 1)?;
            buffer[start..start + width].copy_from_slice(&bytes[..width]);
            Ok(())
        }
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
    /// Recover this concrete image identity for non-executable authoring/export.
    /// The first path component is the existing address's byte/module component.
    pub fn address(&self) -> Result<IoAddress, RuntimeError> {
        let byte = *self
            .path
            .first()
            .ok_or_else(|| RuntimeError::InvalidIoAddress("empty image identity".into()))?;
        let mut path = Vec::new();
        path.try_reserve_exact(self.path.len())
            .map_err(|_| RuntimeError::Overflow)?;
        path.extend_from_slice(&self.path);
        let address = IoAddress {
            area: self.area,
            size: self.size,
            byte,
            bit: self.bit,
            path,
            wildcard: false,
        };
        validate_process_image_address(&address)?;
        Ok(address)
    }

    /// Logical bytes copied when cloning the owned hierarchical path.
    pub fn clone_allocation_bytes(&self) -> Option<usize> {
        self.path.len().checked_mul(core::mem::size_of::<u32>())
    }
}

#[cfg(test)]
mod address_identity_tests {
    use super::*;

    #[test]
    fn authoring_export_preserves_flat_and_hierarchical_image_identity() {
        for text in ["%IX7.3", "%QB9", "%MW10", "%QL16", "%IX2.4.7", "%QD1.3"] {
            let original = IoAddress::parse(text).unwrap();
            let key = IoAddressKey::from(&original);
            let exported = key.address().unwrap();
            assert_eq!(exported, original, "{text}");
            assert_eq!(IoAddressKey::from(&exported), key, "{text}");
        }
    }
}

#[cfg(test)]
mod flat_bytes_tests {
    use super::*;

    #[test]
    fn a_values_image_bytes_are_little_endian_and_a_bits_one_byte() {
        // Specification 48 Appendix C, C.6: an address's change carries the bytes the value gives
        // the image, little-endian as `write_flat_image` writes them, a bit as one byte 0 or 1 and a
        // text padded with zeros. Catches big-endian bytes, a bit given as its byte's mask, a value
        // of another type taken, and bytes written past `out`.
        let mut out = [0xFF_u8; 8];
        let bytes = |size, value: Value, out: &mut [u8]| flat_image_bytes(size, &value, out);
        assert_eq!(bytes(IoSize::Bit, Value::Bool(true), &mut out), Some(1));
        assert_eq!(out[0], 1);
        assert_eq!(bytes(IoSize::Bit, Value::Bool(false), &mut out), Some(1));
        assert_eq!(out[0], 0);
        assert_eq!(bytes(IoSize::Byte, Value::Byte(0xA5), &mut out), Some(1));
        assert_eq!(out[0], 0xA5);
        assert_eq!(bytes(IoSize::Word, Value::Word(0x1234), &mut out), Some(2));
        assert_eq!(&out[..2], &[0x34, 0x12]);
        assert_eq!(
            bytes(IoSize::DWord, Value::DWord(0x0102_0304), &mut out),
            Some(4)
        );
        assert_eq!(&out[..4], &[4, 3, 2, 1]);
        assert_eq!(
            bytes(IoSize::LWord, Value::LWord(0x0102_0304_0506_0708), &mut out),
            Some(8)
        );
        assert_eq!(out, [8, 7, 6, 5, 4, 3, 2, 1]);
        assert_eq!(
            bytes(IoSize::Bytes(4), Value::String("ab".into()), &mut out),
            Some(4)
        );
        assert_eq!(&out[..4], b"ab\0\0");
        assert_eq!(bytes(IoSize::Word, Value::Int(5), &mut out), None);
        assert_eq!(
            bytes(IoSize::Bytes(2), Value::String("abc".into()), &mut out),
            None
        );
        let mut short = [0_u8; 1];
        assert_eq!(bytes(IoSize::Word, Value::Word(1), &mut short), None);
    }
}
