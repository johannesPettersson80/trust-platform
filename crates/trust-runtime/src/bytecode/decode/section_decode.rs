fn decode_section_data(
    version: BytecodeVersion,
    id: u16,
    payload: &[u8],
) -> Result<SectionData, BytecodeError> {
    let Some(kind) = SectionId::from_raw(id) else {
        return Ok(SectionData::Raw(payload.to_vec()));
    };
    let mut reader = BytecodeReader::new(payload);
    let data = match kind {
        SectionId::StringTable | SectionId::DebugStringTable => {
            let context = match kind {
                SectionId::StringTable => "STRING_TABLE",
                SectionId::DebugStringTable => "DEBUG_STRING_TABLE",
                _ => unreachable!("string table branch"),
            };
            let table = decode_string_table(version, &mut reader, context)?;
            match kind {
                SectionId::StringTable => SectionData::StringTable(table),
                SectionId::DebugStringTable => SectionData::DebugStringTable(table),
                _ => unreachable!("string table branch"),
            }
        }
        SectionId::TypeTable => SectionData::TypeTable(decode_type_table(version, payload)?),
        SectionId::ConstPool => SectionData::ConstPool(decode_const_pool(&mut reader)?),
        SectionId::RefTable => SectionData::RefTable(decode_ref_table(&mut reader)?),
        SectionId::PouIndex => SectionData::PouIndex(decode_pou_index(version, &mut reader)?),
        SectionId::PouBodies => SectionData::PouBodies(payload.to_vec()),
        SectionId::ResourceMeta => SectionData::ResourceMeta(decode_resource_meta(&mut reader)?),
        SectionId::IoMap => SectionData::IoMap(decode_io_map(&mut reader)?),
        SectionId::DebugMap => SectionData::DebugMap(decode_debug_map(&mut reader)?),
        SectionId::VarMeta => SectionData::VarMeta(decode_var_meta(&mut reader)?),
        SectionId::RetainInit => SectionData::RetainInit(decode_retain_init(&mut reader)?),
    };
    Ok(data)
}

fn decode_const_pool(reader: &mut BytecodeReader<'_>) -> Result<ConstPool, BytecodeError> {
    let count = read_bounded_count(reader, 8, "CONST_POOL")?;
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let type_id = reader.read_u32()?;
        let len = reader.read_u32()? as usize;
        let payload = reader.read_bytes(len)?.to_vec();
        entries.push(ConstEntry { type_id, payload });
    }
    Ok(ConstPool { entries })
}

fn decode_ref_table(reader: &mut BytecodeReader<'_>) -> Result<RefTable, BytecodeError> {
    let count = read_bounded_count(reader, 16, "REF_TABLE")?;
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let location = reader.read_u8()?;
        let _flags = reader.read_u8()?;
        let _reserved = reader.read_u16()?;
        let owner_id = reader.read_u32()?;
        let offset = reader.read_u32()?;
        let segment_count = read_bounded_count(reader, 8, "REF_TABLE segment")?;
        let location = RefLocation::from_raw(location)
            .ok_or_else(|| BytecodeError::InvalidSection("invalid ref location".into()))?;
        let mut segments = Vec::with_capacity(segment_count);
        for _ in 0..segment_count {
            let kind = reader.read_u8()?;
            let _reserved = reader.read_bytes(3)?;
            match kind {
                0 => {
                    let count = read_bounded_count(reader, 8, "REF_TABLE index")?;
                    let mut indices = Vec::with_capacity(count);
                    for _ in 0..count {
                        indices.push(reader.read_i64()?);
                    }
                    segments.push(RefSegment::Index(indices));
                }
                1 => {
                    let name_idx = reader.read_u32()?;
                    segments.push(RefSegment::Field { name_idx });
                }
                _ => return Err(BytecodeError::InvalidSection("invalid ref segment".into())),
            }
        }
        entries.push(RefEntry {
            location,
            owner_id,
            offset,
            segments,
        });
    }
    Ok(RefTable { entries })
}

fn decode_pou_index(
    version: BytecodeVersion,
    reader: &mut BytecodeReader<'_>,
) -> Result<PouIndex, BytecodeError> {
    let count = read_bounded_count(reader, 40, "POU_INDEX")?;
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let id = reader.read_u32()?;
        let name_idx = reader.read_u32()?;
        let kind = reader.read_u8()?;
        let _flags = reader.read_u8()?;
        let _reserved = reader.read_u16()?;
        let code_offset = reader.read_u32()?;
        let code_length = reader.read_u32()?;
        let local_ref_start = reader.read_u32()?;
        let local_ref_count = reader.read_u32()?;
        let return_type_id = optional_u32(reader.read_u32()?);
        let owner_pou_id = optional_u32(reader.read_u32()?);
        let parameter_bytes = if version.minor >= 1 { 16 } else { 12 };
        let param_count = read_bounded_count(reader, parameter_bytes, "POU_INDEX parameter")?;
        let kind = PouKind::from_raw(kind)
            .ok_or_else(|| BytecodeError::InvalidSection("invalid pou kind".into()))?;

        let mut params = Vec::with_capacity(param_count);
        for _ in 0..param_count {
            let name_idx = reader.read_u32()?;
            let type_id = reader.read_u32()?;
            let direction = reader.read_u8()?;
            let _flags = reader.read_u8()?;
            let _reserved = reader.read_u16()?;
            let default_const_idx = if version.minor >= 1 {
                optional_u32(reader.read_u32()?)
            } else {
                None
            };
            params.push(super::ParamEntry {
                name_idx,
                type_id,
                direction,
                default_const_idx,
            });
        }

        let class_meta = if kind.is_class_like() {
            let parent_pou_id = optional_u32(reader.read_u32()?);
            let interface_count = read_bounded_count(reader, 8, "POU_INDEX interface")?;
            let mut interfaces = Vec::with_capacity(interface_count);
            for _ in 0..interface_count {
                let interface_type_id = reader.read_u32()?;
                let method_count = read_bounded_count(reader, 4, "POU_INDEX vtable slot")?;
                let mut vtable_slots = Vec::with_capacity(method_count);
                for _ in 0..method_count {
                    vtable_slots.push(reader.read_u32()?);
                }
                interfaces.push(InterfaceImpl {
                    interface_type_id,
                    vtable_slots,
                });
            }

            let method_count = read_bounded_count(reader, 16, "POU_INDEX method")?;
            let mut methods = Vec::with_capacity(method_count);
            for _ in 0..method_count {
                let name_idx = reader.read_u32()?;
                let pou_id = reader.read_u32()?;
                let vtable_slot = reader.read_u32()?;
                let access = reader.read_u8()?;
                let flags = reader.read_u8()?;
                let _reserved = reader.read_u16()?;
                methods.push(MethodEntry {
                    name_idx,
                    pou_id,
                    vtable_slot,
                    access,
                    flags,
                });
            }
            Some(PouClassMeta {
                parent_pou_id,
                interfaces,
                methods,
            })
        } else {
            None
        };

        entries.push(PouEntry {
            id,
            name_idx,
            kind,
            code_offset,
            code_length,
            local_ref_start,
            local_ref_count,
            return_type_id,
            owner_pou_id,
            params,
            class_meta,
        });
    }
    Ok(PouIndex { entries })
}

fn decode_resource_meta(reader: &mut BytecodeReader<'_>) -> Result<ResourceMeta, BytecodeError> {
    let resource_count = read_bounded_count(reader, 20, "RESOURCE_META")?;
    let mut resources = Vec::with_capacity(resource_count);
    for _ in 0..resource_count {
        let name_idx = reader.read_u32()?;
        let inputs_size = reader.read_u32()?;
        let outputs_size = reader.read_u32()?;
        let memory_size = reader.read_u32()?;
        let task_count = read_bounded_count(reader, 28, "RESOURCE_META task")?;
        let mut tasks = Vec::with_capacity(task_count);
        for _ in 0..task_count {
            let name_idx = reader.read_u32()?;
            let priority = reader.read_u32()?;
            let interval_nanos = reader.read_i64()?;
            let single_name_idx = optional_u32(reader.read_u32()?);
            let program_count = read_bounded_count(reader, 4, "RESOURCE_META program")?;
            let mut program_name_idx = Vec::with_capacity(program_count);
            for _ in 0..program_count {
                program_name_idx.push(reader.read_u32()?);
            }
            let fb_ref_count =
                read_bounded_count(reader, 4, "RESOURCE_META function-block reference")?;
            let mut fb_ref_idx = Vec::with_capacity(fb_ref_count);
            for _ in 0..fb_ref_count {
                fb_ref_idx.push(reader.read_u32()?);
            }
            tasks.push(super::TaskEntry {
                name_idx,
                priority,
                interval_nanos,
                single_name_idx,
                program_name_idx,
                fb_ref_idx,
            });
        }
        resources.push(ResourceEntry {
            name_idx,
            inputs_size,
            outputs_size,
            memory_size,
            tasks,
        });
    }
    Ok(ResourceMeta { resources })
}

fn decode_io_map(reader: &mut BytecodeReader<'_>) -> Result<IoMap, BytecodeError> {
    let binding_count = read_bounded_count(reader, 12, "IO_MAP")?;
    let mut bindings = Vec::with_capacity(binding_count);
    for _ in 0..binding_count {
        let address_str_idx = reader.read_u32()?;
        let ref_idx = reader.read_u32()?;
        let type_id = optional_u32(reader.read_u32()?);
        bindings.push(IoBinding {
            address_str_idx,
            ref_idx,
            type_id,
        });
    }
    Ok(IoMap { bindings })
}

fn decode_debug_map(reader: &mut BytecodeReader<'_>) -> Result<DebugMap, BytecodeError> {
    let entry_count = read_bounded_count(reader, 24, "DEBUG_MAP")?;
    let mut entries = Vec::with_capacity(entry_count);
    for _ in 0..entry_count {
        let pou_id = reader.read_u32()?;
        let code_offset = reader.read_u32()?;
        let file_idx = reader.read_u32()?;
        let line = reader.read_u32()?;
        let column = reader.read_u32()?;
        let kind = reader.read_u8()?;
        let _reserved = reader.read_bytes(3)?;
        entries.push(DebugEntry {
            pou_id,
            code_offset,
            file_idx,
            line,
            column,
            kind,
        });
    }
    Ok(DebugMap { entries })
}

fn decode_var_meta(reader: &mut BytecodeReader<'_>) -> Result<VarMeta, BytecodeError> {
    let entry_count = read_bounded_count(reader, 20, "VAR_META")?;
    let mut entries = Vec::with_capacity(entry_count);
    for _ in 0..entry_count {
        let name_idx = reader.read_u32()?;
        let type_id = reader.read_u32()?;
        let ref_idx = reader.read_u32()?;
        let retain = reader.read_u8()?;
        let _flags = reader.read_u8()?;
        let _reserved = reader.read_u16()?;
        let init_const_idx = optional_u32(reader.read_u32()?);
        entries.push(VarMetaEntry {
            name_idx,
            type_id,
            ref_idx,
            retain,
            init_const_idx,
        });
    }
    Ok(VarMeta { entries })
}

fn decode_retain_init(reader: &mut BytecodeReader<'_>) -> Result<RetainInit, BytecodeError> {
    let entry_count = read_bounded_count(reader, 8, "RETAIN_INIT")?;
    let mut entries = Vec::with_capacity(entry_count);
    for _ in 0..entry_count {
        let ref_idx = reader.read_u32()?;
        let const_idx = reader.read_u32()?;
        entries.push(RetainInitEntry { ref_idx, const_idx });
    }
    Ok(RetainInit { entries })
}

fn optional_u32(value: u32) -> Option<u32> {
    if value == u32::MAX {
        None
    } else {
        Some(value)
    }
}
