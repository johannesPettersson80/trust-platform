fn type_size_bytes(type_id: TypeId, registry: &TypeRegistry) -> Result<u64, CompileError> {
    // Process-image stride follows the selected wire codecs and field locations.
    // Neither current storage SIZEOF nor the legacy field-sum rule describes it.
    io_type_extent(type_id, registry, 0)
        .map_err(|err| CompileError::new(format!("unsupported size for I/O binding: {err}")))
}

fn io_type_extent(
    type_id: TypeId,
    registry: &TypeRegistry,
    depth: usize,
) -> Result<u64, CompileError> {
    if depth >= 128 {
        return Err(CompileError::new(
            "recursive or excessively nested I/O type",
        ));
    }
    let ty = registry
        .get(type_id)
        .ok_or_else(|| CompileError::new("unknown I/O type"))?;
    match ty {
        Type::Alias { target, .. } => io_type_extent(*target, registry, depth + 1),
        Type::Enum { base, .. } | Type::Subrange { base, .. } => {
            io_type_extent(*base, registry, depth + 1)
        }
        Type::Array {
            element,
            dimensions,
        } => {
            let count = dimensions
                .iter()
                .try_fold(1_u64, |count, &(lower, upper)| {
                    let length = u64::try_from(i128::from(upper) - i128::from(lower) + 1)
                        .ok()
                        .filter(|length| *length != 0)
                        .ok_or_else(|| CompileError::new("invalid I/O array bounds"))?;
                    count
                        .checked_mul(length)
                        .ok_or_else(|| CompileError::new("I/O array extent overflow"))
                })?;
            io_type_extent(*element, registry, depth + 1)?
                .checked_mul(count)
                .ok_or_else(|| CompileError::new("I/O array extent overflow"))
        }
        Type::Struct { fields, .. } => {
            let mut end = 0_u64;
            for field in fields {
                let start = relative_io_offset(field.address.as_deref(), end)?;
                let next = start
                    .checked_add(io_type_extent(field.type_id, registry, depth + 1)?)
                    .ok_or_else(|| CompileError::new("I/O structure extent overflow"))?;
                end = end.max(next);
            }
            Ok(end)
        }
        Type::Union { variants, .. } => {
            let mut end = 0_u64;
            for variant in variants {
                let start = relative_io_offset(variant.address.as_deref(), 0)?;
                let next = start
                    .checked_add(io_type_extent(variant.type_id, registry, depth + 1)?)
                    .ok_or_else(|| CompileError::new("I/O union extent overflow"))?;
                end = end.max(next);
            }
            Ok(end)
        }
        _ => {
            // The existing address owner supplies the exact leaf codec width,
            // including four-byte short temporals and byte-strided BOOL arrays.
            let address = IoAddress {
                area: crate::memory::IoArea::Memory,
                size: io_size_for_type(type_id, registry)?,
                byte: 0,
                bit: 0,
                path: vec![0],
                wildcard: false,
            };
            address
                .flat_byte_end()
                .map_err(|error| CompileError::new(error.to_string()))?
                .map(u64::from)
                .ok_or_else(|| CompileError::new("invalid flat I/O extent"))
        }
    }
}

fn relative_io_offset(address: Option<&str>, sequential: u64) -> Result<u64, CompileError> {
    match address.map(parse_field_address).transpose()? {
        None => Ok(sequential),
        Some(FieldAddress::Relative { offset_bytes, .. }) => Ok(offset_bytes),
        Some(FieldAddress::Absolute(_)) => Err(CompileError::new(
            "absolute field address cannot define a relative I/O stride",
        )),
    }
}

fn io_size_for_type(
    type_id: TypeId,
    registry: &TypeRegistry,
) -> Result<crate::io::IoSize, CompileError> {
    let ty = registry
        .get(type_id)
        .ok_or_else(|| CompileError::new("unknown type for I/O binding"))?;
    match ty {
        Type::Alias { target, .. } => io_size_for_type(*target, registry),
        Type::Subrange { base, .. } => io_size_for_type(*base, registry),
        Type::Enum { base, .. } => io_size_for_type(*base, registry),
        Type::Bool => Ok(crate::io::IoSize::Bit),
        Type::SInt | Type::USInt | Type::Byte | Type::Char => Ok(crate::io::IoSize::Byte),
        Type::Int | Type::UInt | Type::Word | Type::WChar => Ok(crate::io::IoSize::Word),
        Type::DInt
        | Type::UDInt
        | Type::DWord
        | Type::Real
        | Type::Time => Ok(crate::io::IoSize::DWord),
        // IEC 61131-3 Table 10 gives DATE, TOD and DT no fixed width, so no direct address can
        // carry them. The long forms LDATE, LTOD and LDT are the supported direct-address types.
        Type::Date => Err(CompileError::new(
            "DATE has no fixed direct-address width (IEC 61131-3 Table 10): bind the long form LDATE, or convert explicitly to a long form",
        )),
        Type::Tod => Err(CompileError::new(
            "TOD has no fixed direct-address width (IEC 61131-3 Table 10): bind the long form LTOD, or convert explicitly to a long form",
        )),
        Type::Dt => Err(CompileError::new(
            "DT has no fixed direct-address width (IEC 61131-3 Table 10): bind the long form LDT, or convert explicitly to a long form",
        )),
        Type::LInt
        | Type::ULInt
        | Type::LWord
        | Type::LReal
        | Type::LTime
        | Type::LDate
        | Type::LTod
        | Type::Ldt => Ok(crate::io::IoSize::LWord),
        Type::String { max_len } => max_len
            .map(u32::from)
            .map(crate::io::IoSize::Bytes)
            .ok_or_else(|| CompileError::new("STRING direct I/O binding requires STRING[n]")),
        _ => Err(CompileError::new("unsupported type for I/O binding")),
    }
}

fn leaf_value_type(type_id: TypeId, registry: &TypeRegistry) -> Result<TypeId, CompileError> {
    let ty = registry
        .get(type_id)
        .ok_or_else(|| CompileError::new("unknown type for I/O binding"))?;
    match ty {
        Type::Alias { target, .. } => leaf_value_type(*target, registry),
        Type::Subrange { base, .. } => Ok(*base),
        Type::Enum { base, .. } => Ok(*base),
        _ => Ok(type_id),
    }
}

fn offset_address(
    base: &IoAddress,
    offset_bytes: u64,
    size: crate::io::IoSize,
    bit_offset: u8,
) -> Result<IoAddress, CompileError> {
    let mut address = base.clone();
    address.size = size;
    address.wildcard = false;

    let offset_bytes_u32 = u32::try_from(offset_bytes)
        .map_err(|_| CompileError::new("I/O address offset overflow"))?;

    if matches!(size, crate::io::IoSize::Bit) {
        let total_bits = u64::from(base.bit) + offset_bytes * 8 + u64::from(bit_offset);
        let add_bytes = total_bits / 8;
        let bit = (total_bits % 8) as u8;
        let add_bytes_u32 = u32::try_from(add_bytes)
            .map_err(|_| CompileError::new("I/O address offset overflow"))?;
        address.bit = bit;
        if address.path.len() > 1 {
            let mut path = address.path.clone();
            let last = path
                .last_mut()
                .ok_or_else(|| CompileError::new("invalid I/O address path"))?;
            *last = last
                .checked_add(add_bytes_u32)
                .ok_or_else(|| CompileError::new("I/O address offset overflow"))?;
            address.path = path;
            address.byte = address.path[0];
        } else {
            address.byte = base
                .byte
                .checked_add(add_bytes_u32)
                .ok_or_else(|| CompileError::new("I/O address offset overflow"))?;
            address.path = vec![address.byte];
        }
        return Ok(address);
    }

    address.bit = 0;
    if address.path.len() > 1 {
        let mut path = address.path.clone();
        let last = path
            .last_mut()
            .ok_or_else(|| CompileError::new("invalid I/O address path"))?;
        *last = last
            .checked_add(offset_bytes_u32)
            .ok_or_else(|| CompileError::new("I/O address offset overflow"))?;
        address.path = path;
        address.byte = address.path[0];
    } else {
        address.byte = base
            .byte
            .checked_add(offset_bytes_u32)
            .ok_or_else(|| CompileError::new("I/O address offset overflow"))?;
        address.path = vec![address.byte];
    }
    Ok(address)
}
