fn type_size_bytes(type_id: TypeId, registry: &TypeRegistry) -> Result<u64, CompileError> {
    crate::value::size_of_type(type_id, registry)
        .map_err(|err| CompileError::new(format!("unsupported size for I/O binding: {err:?}")))
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
        | Type::Time
        | Type::Date
        | Type::Tod
        | Type::Dt => Ok(crate::io::IoSize::DWord),
        Type::LInt
        | Type::ULInt
        | Type::LWord
        | Type::LReal
        | Type::LTime
        | Type::LDate
        | Type::LTod
        | Type::Ldt => Ok(crate::io::IoSize::LWord),
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
