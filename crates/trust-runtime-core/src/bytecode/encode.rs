//! Bytecode encoding.

use alloc::vec::Vec;

use super::align4;
mod buffer;
use super::{
    BytecodeError, BytecodeModuleView, BytecodeVersion, SectionData, SectionEntry, TypeData,
    TypeEntry, TypeTable, HEADER_FLAG_CRC32, HEADER_SIZE, MAGIC, SECTION_ENTRY_SIZE,
};
use buffer::{encoded_count, encoded_extent, Buffer};

impl BytecodeModuleView<'_> {
    /// Serialize a bounded container; counts, extents and alignment are checked before writes.
    pub fn encode(&self) -> Result<Vec<u8>, BytecodeError> {
        let section_count = u16::try_from(self.sections.len())
            .map_err(|_| BytecodeError::InvalidHeader("section count overflow".into()))?;
        let section_table_off = HEADER_SIZE as usize;
        let table_len = self
            .sections
            .len()
            .checked_mul(SECTION_ENTRY_SIZE)
            .ok_or_else(encoded_extent)?;
        let mut offset = align4(
            section_table_off
                .checked_add(table_len)
                .ok_or_else(encoded_extent)?,
        )
        .ok_or_else(encoded_extent)?;
        let mut remaining = super::BYTECODE_MAX_CONTAINER_BYTES
            .checked_sub(offset)
            .ok_or_else(encoded_extent)?;
        let mut payloads = Vec::new();
        let mut entries = Vec::new();
        payloads
            .try_reserve_exact(self.sections.len())
            .map_err(|_| encoded_extent())?;
        entries
            .try_reserve_exact(self.sections.len())
            .map_err(|_| encoded_extent())?;
        for section in self.sections {
            let data = encode_section_data(self.version, &section.data, remaining)?;
            let padded = align4(data.len()).ok_or_else(encoded_extent)?;
            remaining = remaining.checked_sub(padded).ok_or_else(encoded_extent)?;
            entries.push(SectionEntry {
                id: section.id,
                flags: section.flags,
                offset: encoded_count(offset)?,
                length: encoded_count(data.len())?,
            });
            offset = offset.checked_add(padded).ok_or_else(encoded_extent)?;
            payloads.push(data);
        }
        let mut bytes = Buffer::new(super::BYTECODE_MAX_CONTAINER_BYTES);
        bytes.extend_from_slice(&MAGIC)?;
        bytes.extend_from_slice(&self.version.major.to_le_bytes())?;
        bytes.extend_from_slice(&self.version.minor.to_le_bytes())?;
        bytes.extend_from_slice(&self.flags.to_le_bytes())?;
        bytes.extend_from_slice(&HEADER_SIZE.to_le_bytes())?;
        bytes.extend_from_slice(&section_count.to_le_bytes())?;
        bytes.extend_from_slice(&encoded_count(section_table_off)?.to_le_bytes())?;
        bytes.extend_from_slice(&0u32.to_le_bytes())?;
        for entry in entries {
            bytes.extend_from_slice(&entry.id.to_le_bytes())?;
            bytes.extend_from_slice(&entry.flags.to_le_bytes())?;
            bytes.extend_from_slice(&entry.offset.to_le_bytes())?;
            bytes.extend_from_slice(&entry.length.to_le_bytes())?;
        }
        bytes.pad_to(align4(bytes.len()).ok_or_else(encoded_extent)?)?;
        for data in payloads {
            bytes.extend_from_slice(&data)?;
            bytes.pad_to(align4(bytes.len()).ok_or_else(encoded_extent)?)?;
        }
        let mut bytes = bytes.into_vec();
        if self.flags & HEADER_FLAG_CRC32 != 0 {
            let checksum = crc32fast::hash(&bytes[section_table_off..]);
            bytes[20..24].copy_from_slice(&checksum.to_le_bytes());
        }
        Ok(bytes)
    }
}

fn encode_section_data(
    version: BytecodeVersion,
    data: &SectionData,
    limit: usize,
) -> Result<Vec<u8>, BytecodeError> {
    let mut out = Buffer::new(limit);
    match data {
        SectionData::StringTable(table) | SectionData::DebugStringTable(table) => {
            out.extend_from_slice(&encoded_count(table.entries.len())?.to_le_bytes())?;
            for entry in &table.entries {
                let bytes = entry.as_bytes();
                out.extend_from_slice(&encoded_count(bytes.len())?.to_le_bytes())?;
                out.extend_from_slice(bytes)?;
                if version.minor >= 1 {
                    let entry_len = 4usize.checked_add(bytes.len()).ok_or_else(encoded_extent)?;
                    let padded = align4(entry_len).ok_or_else(encoded_extent)?;
                    let target = out
                        .len()
                        .checked_add(padded - entry_len)
                        .ok_or_else(encoded_extent)?;
                    out.pad_to(target)?;
                }
            }
        }
        SectionData::TypeTable(table) => {
            out = encode_type_table(version, table, limit)?;
        }
        SectionData::ConstPool(pool) => {
            out.extend_from_slice(&encoded_count(pool.entries.len())?.to_le_bytes())?;
            for entry in &pool.entries {
                out.extend_from_slice(&entry.type_id.to_le_bytes())?;
                out.extend_from_slice(&encoded_count(entry.payload.len())?.to_le_bytes())?;
                out.extend_from_slice(&entry.payload)?;
            }
        }
        SectionData::RefTable(table) => {
            out.extend_from_slice(&encoded_count(table.entries.len())?.to_le_bytes())?;
            for entry in &table.entries {
                out.push(entry.location as u8)?;
                out.push(0)?;
                out.extend_from_slice(&0u16.to_le_bytes())?;
                out.extend_from_slice(&entry.owner_id.to_le_bytes())?;
                out.extend_from_slice(&entry.offset.to_le_bytes())?;
                out.extend_from_slice(&encoded_count(entry.segments.len())?.to_le_bytes())?;
                for segment in &entry.segments {
                    match segment {
                        super::RefSegment::Index(indices) => {
                            out.push(0)?;
                            out.extend_from_slice(&[0u8; 3])?;
                            out.extend_from_slice(&encoded_count(indices.len())?.to_le_bytes())?;
                            for index in indices {
                                out.extend_from_slice(&index.to_le_bytes())?;
                            }
                        }
                        super::RefSegment::Field { name_idx } => {
                            out.push(1)?;
                            out.extend_from_slice(&[0u8; 3])?;
                            out.extend_from_slice(&name_idx.to_le_bytes())?;
                        }
                    }
                }
            }
        }
        SectionData::PouIndex(index) => {
            out.extend_from_slice(&encoded_count(index.entries.len())?.to_le_bytes())?;
            for entry in &index.entries {
                out.extend_from_slice(&entry.id.to_le_bytes())?;
                out.extend_from_slice(&entry.name_idx.to_le_bytes())?;
                out.push(entry.kind as u8)?;
                out.push(0)?;
                out.extend_from_slice(&0u16.to_le_bytes())?;
                out.extend_from_slice(&entry.code_offset.to_le_bytes())?;
                out.extend_from_slice(&entry.code_length.to_le_bytes())?;
                out.extend_from_slice(&entry.local_ref_start.to_le_bytes())?;
                out.extend_from_slice(&entry.local_ref_count.to_le_bytes())?;
                out.extend_from_slice(&entry.return_type_id.unwrap_or(u32::MAX).to_le_bytes())?;
                out.extend_from_slice(&entry.owner_pou_id.unwrap_or(u32::MAX).to_le_bytes())?;
                out.extend_from_slice(&encoded_count(entry.params.len())?.to_le_bytes())?;
                for param in &entry.params {
                    out.extend_from_slice(&param.name_idx.to_le_bytes())?;
                    out.extend_from_slice(&param.type_id.to_le_bytes())?;
                    out.push(param.direction)?;
                    out.push(0)?;
                    out.extend_from_slice(&0u16.to_le_bytes())?;
                    if version.minor >= 1 {
                        out.extend_from_slice(
                            &param.default_const_idx.unwrap_or(u32::MAX).to_le_bytes(),
                        )?;
                    }
                }
                if let Some(meta) = &entry.class_meta {
                    out.extend_from_slice(&meta.parent_pou_id.unwrap_or(u32::MAX).to_le_bytes())?;
                    out.extend_from_slice(&encoded_count(meta.interfaces.len())?.to_le_bytes())?;
                    for interface in &meta.interfaces {
                        out.extend_from_slice(&interface.interface_type_id.to_le_bytes())?;
                        out.extend_from_slice(
                            &encoded_count(interface.vtable_slots.len())?.to_le_bytes(),
                        )?;
                        for slot in &interface.vtable_slots {
                            out.extend_from_slice(&slot.to_le_bytes())?;
                        }
                    }
                    out.extend_from_slice(&encoded_count(meta.methods.len())?.to_le_bytes())?;
                    for method in &meta.methods {
                        out.extend_from_slice(&method.name_idx.to_le_bytes())?;
                        out.extend_from_slice(&method.pou_id.to_le_bytes())?;
                        out.extend_from_slice(&method.vtable_slot.to_le_bytes())?;
                        out.push(method.access)?;
                        out.push(method.flags)?;
                        out.extend_from_slice(&0u16.to_le_bytes())?;
                    }
                } else if entry.kind.is_class_like() {
                    out.extend_from_slice(&u32::MAX.to_le_bytes())?;
                    out.extend_from_slice(&0u32.to_le_bytes())?;
                    out.extend_from_slice(&0u32.to_le_bytes())?;
                }
            }
        }
        SectionData::PouBodies(bodies) => out.extend_from_slice(bodies)?,
        SectionData::ResourceMeta(meta) => {
            out.extend_from_slice(&encoded_count(meta.resources.len())?.to_le_bytes())?;
            for resource in &meta.resources {
                out.extend_from_slice(&resource.name_idx.to_le_bytes())?;
                out.extend_from_slice(&resource.inputs_size.to_le_bytes())?;
                out.extend_from_slice(&resource.outputs_size.to_le_bytes())?;
                out.extend_from_slice(&resource.memory_size.to_le_bytes())?;
                out.extend_from_slice(&encoded_count(resource.tasks.len())?.to_le_bytes())?;
                for task in &resource.tasks {
                    out.extend_from_slice(&task.name_idx.to_le_bytes())?;
                    out.extend_from_slice(&task.priority.to_le_bytes())?;
                    out.extend_from_slice(&task.interval_nanos.to_le_bytes())?;
                    out.extend_from_slice(&task.single_name_idx.unwrap_or(u32::MAX).to_le_bytes())?;
                    out.extend_from_slice(
                        &encoded_count(task.program_name_idx.len())?.to_le_bytes(),
                    )?;
                    for idx in &task.program_name_idx {
                        out.extend_from_slice(&idx.to_le_bytes())?;
                    }
                    out.extend_from_slice(&encoded_count(task.fb_ref_idx.len())?.to_le_bytes())?;
                    for idx in &task.fb_ref_idx {
                        out.extend_from_slice(&idx.to_le_bytes())?;
                    }
                }
            }
        }
        SectionData::IoMap(map) => {
            out.extend_from_slice(&encoded_count(map.bindings.len())?.to_le_bytes())?;
            for binding in &map.bindings {
                out.extend_from_slice(&binding.address_str_idx.to_le_bytes())?;
                out.extend_from_slice(&binding.ref_idx.to_le_bytes())?;
                out.extend_from_slice(&binding.type_id.unwrap_or(u32::MAX).to_le_bytes())?;
            }
        }
        SectionData::DebugMap(map) => {
            out.extend_from_slice(&encoded_count(map.entries.len())?.to_le_bytes())?;
            for entry in &map.entries {
                out.extend_from_slice(&entry.pou_id.to_le_bytes())?;
                out.extend_from_slice(&entry.code_offset.to_le_bytes())?;
                out.extend_from_slice(&entry.file_idx.to_le_bytes())?;
                out.extend_from_slice(&entry.line.to_le_bytes())?;
                out.extend_from_slice(&entry.column.to_le_bytes())?;
                out.push(entry.kind)?;
                out.extend_from_slice(&[0u8; 3])?;
            }
        }
        SectionData::VarMeta(meta) => {
            out.extend_from_slice(&encoded_count(meta.entries.len())?.to_le_bytes())?;
            for entry in &meta.entries {
                out.extend_from_slice(&entry.name_idx.to_le_bytes())?;
                out.extend_from_slice(&entry.type_id.to_le_bytes())?;
                out.extend_from_slice(&entry.ref_idx.to_le_bytes())?;
                out.push(entry.retain)?;
                out.push(0)?;
                out.extend_from_slice(&0u16.to_le_bytes())?;
                out.extend_from_slice(&entry.init_const_idx.unwrap_or(u32::MAX).to_le_bytes())?;
            }
        }
        SectionData::RetainInit(retain) => {
            out.extend_from_slice(&encoded_count(retain.entries.len())?.to_le_bytes())?;
            for entry in &retain.entries {
                out.extend_from_slice(&entry.ref_idx.to_le_bytes())?;
                out.extend_from_slice(&entry.const_idx.to_le_bytes())?;
            }
        }
        SectionData::Raw(raw) => out.extend_from_slice(raw)?,
    }
    Ok(out.into_vec())
}

fn encode_type_table(
    version: BytecodeVersion,
    table: &TypeTable,
    limit: usize,
) -> Result<Buffer, BytecodeError> {
    let mut out = Buffer::new(limit);
    out.extend_from_slice(&encoded_count(table.entries.len())?.to_le_bytes())?;
    if version.minor >= 1 {
        for offset in type_offsets(&table.entries, limit)? {
            out.extend_from_slice(&offset.to_le_bytes())?;
        }
    }
    for entry in &table.entries {
        encode_type_entry(entry, &mut out)?;
    }
    Ok(out)
}

fn encode_type_entry(entry: &TypeEntry, out: &mut Buffer) -> Result<(), BytecodeError> {
    out.push(entry.kind as u8)?;
    out.push(0)?;
    out.extend_from_slice(&0u16.to_le_bytes())?;
    let name_idx = entry.name_idx.unwrap_or(u32::MAX);
    out.extend_from_slice(&name_idx.to_le_bytes())?;
    match &entry.data {
        TypeData::Primitive {
            prim_id,
            max_length,
        } => {
            out.extend_from_slice(&prim_id.to_le_bytes())?;
            out.extend_from_slice(&max_length.to_le_bytes())?;
        }
        TypeData::Array { elem_type_id, dims } => {
            out.extend_from_slice(&elem_type_id.to_le_bytes())?;
            out.extend_from_slice(&encoded_count(dims.len())?.to_le_bytes())?;
            for (lower, upper) in dims {
                out.extend_from_slice(&lower.to_le_bytes())?;
                out.extend_from_slice(&upper.to_le_bytes())?;
            }
        }
        TypeData::Struct { fields } | TypeData::Union { fields } => {
            out.extend_from_slice(&encoded_count(fields.len())?.to_le_bytes())?;
            for field in fields {
                out.extend_from_slice(&field.name_idx.to_le_bytes())?;
                out.extend_from_slice(&field.type_id.to_le_bytes())?;
            }
        }
        TypeData::Enum {
            base_type_id,
            variants,
        } => {
            out.extend_from_slice(&base_type_id.to_le_bytes())?;
            out.extend_from_slice(&encoded_count(variants.len())?.to_le_bytes())?;
            for variant in variants {
                out.extend_from_slice(&variant.name_idx.to_le_bytes())?;
                out.extend_from_slice(&variant.value.to_le_bytes())?;
            }
        }
        TypeData::Alias { target_type_id } => {
            out.extend_from_slice(&target_type_id.to_le_bytes())?;
        }
        TypeData::Subrange {
            base_type_id,
            lower,
            upper,
        } => {
            out.extend_from_slice(&base_type_id.to_le_bytes())?;
            out.extend_from_slice(&lower.to_le_bytes())?;
            out.extend_from_slice(&upper.to_le_bytes())?;
        }
        TypeData::Reference { target_type_id } => {
            out.extend_from_slice(&target_type_id.to_le_bytes())?;
        }
        TypeData::Pou { pou_id } => {
            out.extend_from_slice(&pou_id.to_le_bytes())?;
        }
        TypeData::Interface { methods } => {
            out.extend_from_slice(&encoded_count(methods.len())?.to_le_bytes())?;
            for method in methods {
                out.extend_from_slice(&method.name_idx.to_le_bytes())?;
                out.extend_from_slice(&method.slot.to_le_bytes())?;
            }
        }
    }
    Ok(())
}

fn type_offsets(entries: &[TypeEntry], limit: usize) -> Result<Vec<u32>, BytecodeError> {
    let mut cursor = entries
        .len()
        .checked_mul(4)
        .and_then(|n| n.checked_add(4))
        .ok_or_else(encoded_extent)?;
    if cursor > limit {
        return Err(encoded_extent());
    }
    let mut offsets = Vec::new();
    offsets
        .try_reserve_exact(entries.len())
        .map_err(|_| encoded_extent())?;
    for entry in entries {
        offsets.push(encoded_count(cursor)?);
        let mut bytes = Buffer::new(limit - cursor);
        encode_type_entry(entry, &mut bytes)?;
        cursor = cursor.checked_add(bytes.len()).ok_or_else(encoded_extent)?;
    }
    Ok(offsets)
}

/// Compute checked TYPE_TABLE offsets for compiler-side producers without retaining encoded entries.
/// Fails before truncating a count or exceeding the maximum container extent.
pub fn compute_type_offsets_for_entries(entries: &[TypeEntry]) -> Result<Vec<u32>, BytecodeError> {
    type_offsets(entries, super::BYTECODE_MAX_CONTAINER_BYTES)
}
