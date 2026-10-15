//! Bounded, opaque persistence checkpoints for admitted physical aggregates.

use super::{
    read_struct_path, scalar, Child, Enumeration, Leaf, PhysicalLayout, PhysicalValue, StructValue,
};
use crate::{
    bytecode::BYTECODE_MAX_TYPE_NESTING,
    error::RuntimeError,
    value::{RefSegment, StorageAbi, Value},
};
use alloc::{sync::Arc, vec::Vec};
use smol_str::SmolStr;

const CHECKPOINT_MAGIC: &[u8; 4] = b"PHV1";
// Both the checkpoint length in RETAIN v3 and its counts/strings use u32
// framing. Actual allocations are further bounded by bytes remaining in the
// input before reservation.
const MAX_WIRE_LENGTH: usize = u32::MAX as usize;

impl StructValue {
    /// Encode the admitted physical layout and backing as an opaque persistence
    /// checkpoint. Logical fields remain the value's ordinary representation.
    pub fn encode_physical_checkpoint(&self) -> Result<Option<Vec<u8>>, RuntimeError> {
        let Some(physical) = &self.physical else {
            return Ok(None);
        };
        let mut out = Vec::new();
        out.extend_from_slice(CHECKPOINT_MAGIC);
        out.push(match physical.layout.abi {
            StorageAbi::LegacyTemporal32 => 0,
            StorageAbi::UniformTemporal64 => 1,
        });
        out.extend_from_slice(&physical.layout.bits.to_le_bytes());
        write_count(&mut out, physical.layout.leaves.len())?;
        write_count(&mut out, physical.layout.children.len())?;
        for leaf in &physical.layout.leaves {
            write_path(&mut out, &leaf.path)?;
            out.extend_from_slice(&leaf.offset.to_le_bytes());
            out.push(leaf.width);
            out.extend_from_slice(&leaf.primitive_id.to_le_bytes());
            if let Some(enumeration) = &leaf.enumeration {
                out.push(1);
                out.push(u8::from(enumeration.signed));
                write_count(&mut out, enumeration.variants.len())?;
                for (name, value) in &enumeration.variants {
                    write_string(&mut out, name)?;
                    out.extend_from_slice(&value.to_le_bytes());
                }
            } else {
                out.push(0);
            }
            if let Some((lower, upper)) = leaf.range {
                out.push(1);
                out.extend_from_slice(&lower.to_le_bytes());
                out.extend_from_slice(&upper.to_le_bytes());
            } else {
                out.push(0);
            }
            check_checkpoint_size(out.len())?;
        }
        for child in &physical.layout.children {
            write_path(&mut out, &child.path)?;
            out.extend_from_slice(&child.offset.to_le_bytes());
            out.extend_from_slice(&child.bits.to_le_bytes());
            check_checkpoint_size(out.len())?;
        }
        write_count(&mut out, physical.bytes.len())?;
        out.extend_from_slice(&physical.bytes);
        check_checkpoint_size(out.len())?;
        Ok(Some(out))
    }

    /// Attach a validated persisted physical checkpoint to this logical
    /// aggregate. Layout identity is retained only after its bytes agree with
    /// every scalar leaf in the value.
    pub fn with_physical_checkpoint(mut self, checkpoint: &[u8]) -> Result<Self, RuntimeError> {
        self.physical = Some(decode_checkpoint(checkpoint, &self)?);
        Ok(self)
    }
}

fn decode_checkpoint(bytes: &[u8], value: &StructValue) -> Result<PhysicalValue, RuntimeError> {
    // `try_from` states the 32-bit wire limit and is not a dead comparison on 32-bit targets.
    if u32::try_from(bytes.len()).is_err() {
        return Err(RuntimeError::Overflow);
    }
    let mut reader = CheckpointReader::new(bytes);
    if reader.read_bytes(CHECKPOINT_MAGIC.len())? != CHECKPOINT_MAGIC {
        return Err(RuntimeError::TypeMismatch);
    }
    let abi = match reader.read_u8()? {
        0 => StorageAbi::LegacyTemporal32,
        1 => StorageAbi::UniformTemporal64,
        _ => return Err(RuntimeError::TypeMismatch),
    };
    let bits = reader.read_u32()?;
    let byte_len = bit_extent_bytes(bits)?;
    let leaf_count = reader.read_count(MAX_WIRE_LENGTH, 19)?;
    let child_count = reader.read_count(MAX_WIRE_LENGTH, 17)?;
    let mut leaves = reserve(leaf_count)?;
    for _ in 0..leaf_count {
        let path = reader.read_path()?;
        let offset = reader.read_u32()?;
        let width = reader.read_u8()?;
        let primitive_id = reader.read_u16()?;
        let enumeration = match reader.read_u8()? {
            0 => None,
            1 => {
                let signed = reader.read_bool()?;
                let count = reader.read_count(MAX_WIRE_LENGTH, 12)?;
                if count == 0 {
                    return Err(RuntimeError::TypeMismatch);
                }
                let mut variants = reserve(count)?;
                for _ in 0..count {
                    variants.push((reader.read_string()?, reader.read_i64()?));
                }
                Some(Enumeration { signed, variants })
            }
            _ => return Err(RuntimeError::TypeMismatch),
        };
        let range = match reader.read_u8()? {
            0 => None,
            1 => {
                let lower = reader.read_i64()?;
                let upper = reader.read_i64()?;
                if lower > upper {
                    return Err(RuntimeError::TypeMismatch);
                }
                Some((lower, upper))
            }
            _ => return Err(RuntimeError::TypeMismatch),
        };
        if width == 0
            || width > 64
            || offset
                .checked_add(u32::from(width))
                .is_none_or(|end| end > bits)
            || scalar::width(primitive_id, abi) != Some(width)
            || path.is_empty()
            || path.len() > BYTECODE_MAX_TYPE_NESTING
        {
            return Err(RuntimeError::TypeMismatch);
        }
        leaves.push(Leaf {
            path,
            offset,
            width,
            primitive_id,
            enumeration,
            range,
        });
    }
    let mut children = reserve(child_count)?;
    for _ in 0..child_count {
        let path = reader.read_path()?;
        let offset = reader.read_u32()?;
        let child_bits = reader.read_u32()?;
        if path.is_empty()
            || path.len() > BYTECODE_MAX_TYPE_NESTING
            // Aggregate placement rejects non-byte-aligned struct fields, and
            // admitted struct extents are rounded to full bytes.
            || offset % 8 != 0
            || child_bits % 8 != 0
            || offset.checked_add(child_bits).is_none_or(|end| end > bits)
        {
            return Err(RuntimeError::TypeMismatch);
        }
        children.push(Child {
            path,
            offset,
            bits: child_bits,
        });
    }
    let mut no_charge = |_: usize, _: usize| Ok::<(), RuntimeError>(());
    let layout = Arc::new(PhysicalLayout::new(
        bits,
        abi,
        leaves,
        children,
        &mut no_charge,
    )?);
    let backing_len = reader.read_count(MAX_WIRE_LENGTH, 1)?;
    if backing_len != byte_len {
        return Err(RuntimeError::TypeMismatch);
    }
    let backing_bytes = reader.read_bytes(backing_len)?;
    let mut backing = Vec::new();
    backing
        .try_reserve_exact(backing_len)
        .map_err(|_| RuntimeError::Overflow)?;
    backing.extend_from_slice(backing_bytes);
    reader.expect_finished()?;
    if bits % 8 != 0
        && backing
            .last()
            .is_some_and(|last| last & (!0u8 << (bits % 8)) != 0)
    {
        return Err(RuntimeError::TypeMismatch);
    }
    let physical = PhysicalValue {
        layout,
        bytes: backing,
    };
    validate_logical_leaves(value, &physical)?;
    Ok(physical)
}

fn validate_logical_leaves(
    value: &StructValue,
    physical: &PhysicalValue,
) -> Result<(), RuntimeError> {
    for leaf in &physical.layout.leaves {
        let logical = read_struct_path(value, &leaf.path).ok_or(RuntimeError::TypeMismatch)?;
        let logical_bits = scalar::checked_bits(logical, logical, leaf, physical.layout.abi)?
            & width_mask(leaf.width);
        let stored_bits = read_bits(&physical.bytes, leaf.offset, leaf.width);
        if logical_bits != stored_bits {
            return Err(RuntimeError::TypeMismatch);
        }
    }
    for child in &physical.layout.children {
        let Some(Value::Struct(child_value)) = read_struct_path(value, &child.path) else {
            return Err(RuntimeError::TypeMismatch);
        };
        let Some(child_physical) = &child_value.physical else {
            return Err(RuntimeError::TypeMismatch);
        };
        if child_physical.layout.bits != child.bits {
            return Err(RuntimeError::TypeMismatch);
        }
        if child_physical.layout.abi != physical.layout.abi {
            return Err(RuntimeError::TypeMismatch);
        }
        if child_physical.layout.leaves.len()
            != physical.layout.leaves_below_path(&child.path).len()
        {
            return Err(RuntimeError::TypeMismatch);
        }
        for nested_leaf in &child_physical.layout.leaves {
            let path = join_path(&child.path, &nested_leaf.path)?;
            let parent_leaf = physical
                .layout
                .leaf_at_path(&path)
                .ok_or(RuntimeError::TypeMismatch)?;
            if parent_leaf.offset != child.offset + nested_leaf.offset
                || parent_leaf.width != nested_leaf.width
                || parent_leaf.primitive_id != nested_leaf.primitive_id
                || parent_leaf.enumeration != nested_leaf.enumeration
                || parent_leaf.range != nested_leaf.range
            {
                return Err(RuntimeError::TypeMismatch);
            }
        }
        if child_physical.layout.children.len()
            != physical.layout.children_below_path(&child.path).len()
        {
            return Err(RuntimeError::TypeMismatch);
        }
        for nested_child in &child_physical.layout.children {
            let path = join_path(&child.path, &nested_child.path)?;
            let parent_child = physical
                .layout
                .child_at_path(&path)
                .ok_or(RuntimeError::TypeMismatch)?;
            if parent_child.offset != child.offset + nested_child.offset
                || parent_child.bits != nested_child.bits
            {
                return Err(RuntimeError::TypeMismatch);
            }
        }
        for byte_index in 0..child.bits.div_ceil(8) {
            let bit = byte_index * 8;
            let width =
                u8::try_from((child.bits - bit).min(8)).map_err(|_| RuntimeError::Overflow)?;
            if read_bits(&physical.bytes, child.offset + bit, width)
                != read_bits(&child_physical.bytes, bit, width)
            {
                return Err(RuntimeError::TypeMismatch);
            }
        }
    }
    Ok(())
}

fn width_mask(width: u8) -> u64 {
    if width == 64 {
        u64::MAX
    } else {
        (1_u64 << width) - 1
    }
}

fn join_path(
    prefix: &[RefSegment],
    suffix: &[RefSegment],
) -> Result<Vec<RefSegment>, RuntimeError> {
    let length = prefix
        .len()
        .checked_add(suffix.len())
        .ok_or(RuntimeError::Overflow)?;
    if length > BYTECODE_MAX_TYPE_NESTING {
        return Err(RuntimeError::TypeMismatch);
    }
    let mut path = reserve(length)?;
    path.extend_from_slice(prefix);
    path.extend_from_slice(suffix);
    Ok(path)
}

fn read_bits(bytes: &[u8], start: u32, width: u8) -> u64 {
    let mut value = 0;
    for bit in 0..u32::from(width) {
        let position = start + bit;
        if let Some(byte) = bytes.get((position / 8) as usize) {
            value |= u64::from((byte >> (position % 8)) & 1) << bit;
        }
    }
    value
}

fn write_path(out: &mut Vec<u8>, path: &[RefSegment]) -> Result<(), RuntimeError> {
    if path.len() > BYTECODE_MAX_TYPE_NESTING {
        return Err(RuntimeError::Overflow);
    }
    write_count(out, path.len())?;
    for segment in path {
        match segment {
            RefSegment::Field(name) => {
                out.push(1);
                write_string(out, name)?;
            }
            RefSegment::Index(indices) => {
                out.push(2);
                if indices.is_empty() {
                    return Err(RuntimeError::TypeMismatch);
                }
                write_count(out, indices.len())?;
                for index in indices {
                    out.extend_from_slice(&index.to_le_bytes());
                }
            }
        }
        check_checkpoint_size(out.len())?;
    }
    Ok(())
}

fn write_string(out: &mut Vec<u8>, value: &str) -> Result<(), RuntimeError> {
    write_count(out, value.len())?;
    out.extend_from_slice(value.as_bytes());
    check_checkpoint_size(out.len())
}

fn write_count(out: &mut Vec<u8>, value: usize) -> Result<(), RuntimeError> {
    let value = u32::try_from(value).map_err(|_| RuntimeError::Overflow)?;
    out.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

fn check_checkpoint_size(size: usize) -> Result<(), RuntimeError> {
    if u32::try_from(size).is_err() {
        return Err(RuntimeError::Overflow);
    }
    Ok(())
}

fn bit_extent_bytes(bits: u32) -> Result<usize, RuntimeError> {
    usize::try_from(u64::from(bits).div_ceil(8)).map_err(|_| RuntimeError::Overflow)
}

fn reserve<T>(count: usize) -> Result<Vec<T>, RuntimeError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| RuntimeError::Overflow)?;
    Ok(values)
}

struct CheckpointReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> CheckpointReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_bytes(&mut self, count: usize) -> Result<&'a [u8], RuntimeError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(RuntimeError::Overflow)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(RuntimeError::TypeMismatch)?;
        self.offset = end;
        Ok(bytes)
    }

    fn read_u8(&mut self) -> Result<u8, RuntimeError> {
        Ok(self.read_bytes(1)?[0])
    }

    fn read_bool(&mut self) -> Result<bool, RuntimeError> {
        match self.read_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(RuntimeError::TypeMismatch),
        }
    }

    fn read_u16(&mut self) -> Result<u16, RuntimeError> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32(&mut self) -> Result<u32, RuntimeError> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_i64(&mut self) -> Result<i64, RuntimeError> {
        let bytes = self.read_bytes(8)?;
        Ok(i64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_count(
        &mut self,
        limit: usize,
        minimum_record_bytes: usize,
    ) -> Result<usize, RuntimeError> {
        let value = usize::try_from(self.read_u32()?).map_err(|_| RuntimeError::Overflow)?;
        if value > limit
            || (minimum_record_bytes != 0
                && value > self.bytes.len().saturating_sub(self.offset) / minimum_record_bytes)
        {
            return Err(RuntimeError::Overflow);
        }
        Ok(value)
    }

    fn read_string(&mut self) -> Result<SmolStr, RuntimeError> {
        let len = self.read_count(MAX_WIRE_LENGTH, 1)?;
        let text =
            core::str::from_utf8(self.read_bytes(len)?).map_err(|_| RuntimeError::TypeMismatch)?;
        Ok(SmolStr::new(text))
    }

    fn read_path(&mut self) -> Result<Vec<RefSegment>, RuntimeError> {
        let count = self.read_count(BYTECODE_MAX_TYPE_NESTING, 5)?;
        if count == 0 {
            return Err(RuntimeError::TypeMismatch);
        }
        let mut path = reserve(count)?;
        for _ in 0..count {
            match self.read_u8()? {
                1 => path.push(RefSegment::Field(self.read_string()?)),
                2 => {
                    let count = self.read_count(MAX_WIRE_LENGTH, 8)?;
                    if count == 0 {
                        return Err(RuntimeError::TypeMismatch);
                    }
                    let mut indices = reserve(count)?;
                    for _ in 0..count {
                        indices.push(self.read_i64()?);
                    }
                    path.push(RefSegment::Index(indices));
                }
                _ => return Err(RuntimeError::TypeMismatch),
            }
        }
        Ok(path)
    }

    fn expect_finished(&self) -> Result<(), RuntimeError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(RuntimeError::TypeMismatch)
        }
    }
}

#[cfg(test)]
#[path = "checkpoint_tests.rs"]
mod tests;
