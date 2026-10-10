use super::*;

pub(super) fn validate_type_table(
    strings: &StringTable,
    types: &TypeTable,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    for entry in &types.entries {
        budget.work(1)?;
        let matches = matches!(
            (&entry.kind, &entry.data),
            (TypeKind::Primitive, TypeData::Primitive { .. })
                | (TypeKind::Array, TypeData::Array { .. })
                | (TypeKind::Struct, TypeData::Struct { .. })
                | (TypeKind::Union, TypeData::Union { .. })
                | (TypeKind::Enum, TypeData::Enum { .. })
                | (TypeKind::Alias, TypeData::Alias { .. })
                | (TypeKind::Subrange, TypeData::Subrange { .. })
                | (TypeKind::Reference, TypeData::Reference { .. })
                | (
                    TypeKind::FunctionBlock | TypeKind::Class,
                    TypeData::Pou { .. }
                )
                | (TypeKind::Interface, TypeData::Interface { .. })
        );
        if !matches {
            return Err(BytecodeError::from(RejectionReason::TypePayloadMismatch));
        }
        if let Some(name_idx) = entry.name_idx {
            ensure_string_index(strings, name_idx)?;
        }
        match &entry.data {
            TypeData::Array { elem_type_id, dims } => {
                ensure_type_index(types, *elem_type_id)?;
                for (lower, upper) in dims {
                    budget.work(1)?;
                    if lower > upper {
                        return Err(BytecodeError::from(RejectionReason::InvalidArrayBounds));
                    }
                }
            }
            TypeData::Struct { fields } | TypeData::Union { fields } => {
                for field in fields {
                    budget.work(1)?;
                    ensure_string_index(strings, field.name_idx)?;
                    ensure_type_index(types, field.type_id)?;
                }
            }
            TypeData::Enum {
                base_type_id,
                variants,
            } => {
                ensure_type_index(types, *base_type_id)?;
                for variant in variants {
                    budget.work(1)?;
                    ensure_string_index(strings, variant.name_idx)?;
                }
            }
            TypeData::Alias { target_type_id }
            | TypeData::Subrange {
                base_type_id: target_type_id,
                ..
            }
            | TypeData::Reference { target_type_id } => {
                ensure_type_index(types, *target_type_id)?;
            }
            TypeData::Pou { .. } => {}
            TypeData::Interface { methods } => {
                for method in methods {
                    budget.work(1)?;
                    ensure_string_index(strings, method.name_idx)?;
                }
            }
            TypeData::Primitive { .. } => {}
        }
    }
    Ok(())
}

pub(super) fn validate_const_pool(
    types: &TypeTable,
    pool: &ConstPool,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    for entry in &pool.entries {
        validate_const_payload_with_budget(types, entry, budget)?;
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn validate_const_payload(
    types: &TypeTable,
    entry: &ConstEntry,
) -> Result<(), BytecodeError> {
    validate_const_payload_with_budget(
        types,
        entry,
        &mut ValidationBudget::new(ValidationLimits::default()),
    )
}

fn validate_const_payload_with_budget(
    types: &TypeTable,
    entry: &ConstEntry,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let type_id = entry.type_id;
    let payload = &entry.payload;
    let entry = types
        .entries
        .get(type_id as usize)
        .ok_or_else(|| BytecodeError::InvalidIndex {
            kind: "type".into(),
            index: type_id,
        })?;
    let mut reader = BytecodeReader::new(payload);
    validate_const_payload_entry(types, entry, &mut reader, 0, budget)?;
    if reader.remaining() != 0 {
        return Err(BytecodeError::from(RejectionReason::ConstPayloadLength));
    }
    Ok(())
}

pub(super) fn validate_const_payload_entry(
    types: &TypeTable,
    entry: &TypeEntry,
    reader: &mut BytecodeReader<'_>,
    depth: u8,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    budget.work(1)?;
    if depth > crate::bytecode::BYTECODE_MAX_CONST_NESTING {
        return Err(BytecodeError::from(
            RejectionReason::ConstTypeRecursionOverflow,
        ));
    }
    match &entry.data {
        TypeData::Primitive { prim_id, .. } => {
            match prim_id {
                1 => {
                    reader.read_u8()?;
                }
                2 | 6 | 10 | 26 => {
                    reader.read_u8()?;
                }
                3 | 7 | 11 | 27 => {
                    reader.read_u16()?;
                }
                4 | 8 | 12 => {
                    reader.read_u32()?;
                }
                5 | 9 | 13 | 15 | 16 | 17 | 18 | 19 | 20 | 21 | 22 | 23 => {
                    reader.read_u64()?;
                }
                14 => {
                    reader.read_u32()?;
                }
                24 => {
                    let payload = reader.read_bytes(reader.remaining())?;
                    budget.work(payload.len())?;
                    core::str::from_utf8(payload).map_err(|err| {
                        BytecodeError::section_diagnostic(SectionDiagnostic::StringUtf8(err))
                    })?;
                }
                25 => {
                    let payload = reader.read_bytes(reader.remaining())?;
                    budget.work(payload.len())?;
                    if payload.len() % 2 != 0 {
                        return Err(BytecodeError::from(
                            RejectionReason::InvalidWstringConstPayloadLength,
                        ));
                    }
                    let (units, remainder) = payload.as_chunks::<2>();
                    debug_assert!(remainder.is_empty());
                    for decoded in
                        char::decode_utf16(units.iter().map(|chunk| u16::from_le_bytes(*chunk)))
                    {
                        decoded.map_err(|_| BytecodeError::section_static("invalid WSTRING const UTF-16: invalid utf-16: lone surrogate found"))?;
                    }
                }
                _ => {
                    return Err(BytecodeError::from(RejectionReason::UnknownPrimitive));
                }
            }
        }
        TypeData::Array { elem_type_id, dims } => {
            let count = reader.read_u32()? as usize;
            budget.work(
                dims.len()
                    .checked_mul(2)
                    .ok_or(RejectionReason::ValidationWorkLimit)?,
            )?;
            let expected = const_array_element_count(dims)?;
            if count != expected {
                return Err(BytecodeError::section_diagnostic(
                    SectionDiagnostic::ArrayCount {
                        expected,
                        actual: count,
                    },
                ));
            }
            let elem = types.entries.get(*elem_type_id as usize).ok_or_else(|| {
                BytecodeError::InvalidIndex {
                    kind: "type".into(),
                    index: *elem_type_id,
                }
            })?;
            for _ in 0..count {
                validate_const_child_payload(types, elem, reader, depth + 1, budget)?;
            }
        }
        TypeData::Struct { fields } | TypeData::Union { fields } => {
            let count = reader.read_u32()? as usize;
            if count != fields.len() {
                return Err(BytecodeError::from(
                    RejectionReason::StructUnionConstantCountMismatch,
                ));
            }
            for field in fields {
                budget.work(1)?;
                let field_type = types.entries.get(field.type_id as usize).ok_or_else(|| {
                    BytecodeError::InvalidIndex {
                        kind: "type".into(),
                        index: field.type_id,
                    }
                })?;
                validate_const_child_payload(types, field_type, reader, depth + 1, budget)?;
            }
        }
        TypeData::Enum { .. } => {
            reader.read_i64()?;
        }
        TypeData::Alias { target_type_id } => {
            let target = types.entries.get(*target_type_id as usize).ok_or_else(|| {
                BytecodeError::InvalidIndex {
                    kind: "type".into(),
                    index: *target_type_id,
                }
            })?;
            validate_const_payload_entry(types, target, reader, depth + 1, budget)?;
        }
        TypeData::Subrange { base_type_id, .. } => {
            let base = types.entries.get(*base_type_id as usize).ok_or_else(|| {
                BytecodeError::InvalidIndex {
                    kind: "type".into(),
                    index: *base_type_id,
                }
            })?;
            validate_const_payload_entry(types, base, reader, depth + 1, budget)?;
        }
        TypeData::Reference { .. } => {
            if reader.read_u32()? != u32::MAX {
                return Err(BytecodeError::from(
                    RejectionReason::ReferenceConstPayloadMustEncodeNull,
                ));
            }
        }
        _ => {
            return Err(BytecodeError::from(RejectionReason::UnsupportedConstType));
        }
    }
    Ok(())
}

pub(super) fn validate_const_child_payload(
    types: &TypeTable,
    entry: &TypeEntry,
    reader: &mut BytecodeReader<'_>,
    depth: u8,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let len = reader.read_u32()? as usize;
    let payload = reader.read_bytes(len)?;
    let mut child = BytecodeReader::new(payload);
    validate_const_payload_entry(types, entry, &mut child, depth, budget)?;
    if child.remaining() != 0 {
        return Err(BytecodeError::from(
            RejectionReason::ConstChildPayloadLength,
        ));
    }
    Ok(())
}

pub(super) fn const_array_element_count(dims: &[(i64, i64)]) -> Result<usize, BytecodeError> {
    if dims
        .iter()
        .any(|(lower, upper)| *lower == 0 && *upper == i64::MAX)
    {
        return Ok(0);
    }
    let mut count = 1_i128;
    for (lower, upper) in dims {
        if lower > upper {
            return Err(BytecodeError::from(RejectionReason::InvalidArrayBounds));
        }
        count = count
            .checked_mul(i128::from(*upper) - i128::from(*lower) + 1)
            .ok_or_else(|| BytecodeError::from(RejectionReason::ArrayConstantSizeOverflow))?;
    }
    usize::try_from(count)
        .map_err(|_| BytecodeError::from(RejectionReason::ArrayConstantSizeOverflow))
}

pub(super) fn validate_ref_table(
    strings: &StringTable,
    table: &RefTable,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    for entry in &table.entries {
        budget.work(1)?;
        for segment in &entry.segments {
            budget.work(1)?;
            if let RefSegment::Field { name_idx } = segment {
                ensure_string_index(strings, *name_idx)?;
            }
        }
    }
    Ok(())
}
