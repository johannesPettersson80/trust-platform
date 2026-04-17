fn collect_io_bindings(
    registry: &TypeRegistry,
    type_id: TypeId,
    reference: crate::value::ValueRef,
    offset_bytes: u64,
    bit_offset: u8,
    out: &mut Vec<IoLeafBinding>,
) -> Result<(), CompileError> {
    let ty = registry
        .get(type_id)
        .ok_or_else(|| CompileError::new("unknown type for I/O binding"))?;
    match ty {
        Type::Alias { target, .. } => {
            collect_io_bindings(registry, *target, reference, offset_bytes, bit_offset, out)
        }
        Type::Subrange { base, .. } => {
            collect_io_bindings(registry, *base, reference, offset_bytes, bit_offset, out)
        }
        Type::Enum { base, .. } => {
            collect_io_bindings(registry, *base, reference, offset_bytes, bit_offset, out)
        }
        Type::Array {
            element,
            dimensions,
        } => {
            let element_size = type_size_bytes(*element, registry)?;
            let lengths: Vec<i64> = dimensions
                .iter()
                .map(|(lower, upper)| upper - lower + 1)
                .collect();
            if lengths.iter().any(|len| *len <= 0) {
                return Err(CompileError::new("invalid array bounds for I/O binding"));
            }
            let mut strides = vec![element_size; lengths.len()];
            let mut stride = element_size;
            for idx in (0..lengths.len()).rev() {
                strides[idx] = stride;
                stride = stride
                    .checked_mul(
                        u64::try_from(lengths[idx]).map_err(|_| {
                            CompileError::new("array length overflow for I/O binding")
                        })?,
                    )
                    .ok_or_else(|| CompileError::new("array stride overflow for I/O binding"))?;
            }

            #[allow(clippy::too_many_arguments)]
            fn walk_array(
                registry: &TypeRegistry,
                element: TypeId,
                dimensions: &[(i64, i64)],
                lengths: &[i64],
                strides: &[u64],
                reference: &crate::value::ValueRef,
                offset_bytes: u64,
                current_dim: usize,
                indices: &mut Vec<i64>,
                bit_offset: u8,
                out: &mut Vec<IoLeafBinding>,
            ) -> Result<(), CompileError> {
                if current_dim == dimensions.len() {
                    let mut ref_with_index = reference.clone();
                    ref_with_index
                        .path
                        .push(crate::value::RefSegment::Index(
                            crate::value::ref_indices_from_iter(indices.iter().copied()),
                        ));
                    return collect_io_bindings(
                        registry,
                        element,
                        ref_with_index,
                        offset_bytes,
                        bit_offset,
                        out,
                    );
                }
                let (lower, _upper) = dimensions[current_dim];
                let stride = strides[current_dim];
                let len = lengths[current_dim];
                for idx in 0..len {
                    let index_value = lower + idx;
                    let offset = stride
                        .checked_mul(u64::try_from(idx).map_err(|_| {
                            CompileError::new("array offset overflow for I/O binding")
                        })?)
                        .ok_or_else(|| {
                            CompileError::new("array offset overflow for I/O binding")
                        })?;
                    let total_offset = offset_bytes.checked_add(offset).ok_or_else(|| {
                        CompileError::new("array offset overflow for I/O binding")
                    })?;
                    indices.push(index_value);
                    walk_array(
                        registry,
                        element,
                        dimensions,
                        lengths,
                        strides,
                        reference,
                        total_offset,
                        current_dim + 1,
                        indices,
                        bit_offset,
                        out,
                    )?;
                    indices.pop();
                }
                Ok(())
            }

            let mut indices = Vec::with_capacity(dimensions.len());
            walk_array(
                registry,
                *element,
                dimensions,
                &lengths,
                &strides,
                &reference,
                offset_bytes,
                0,
                &mut indices,
                bit_offset,
                out,
            )
        }
        Type::Struct { fields, .. } => {
            let mut current_offset = offset_bytes;
            for field in fields {
                let mut field_offset = current_offset;
                let mut field_bit_offset = bit_offset;
                if let Some(address) = &field.address {
                    match parse_field_address(address)? {
                        FieldAddress::Relative {
                            offset_bytes: rel_offset,
                            bit_offset: rel_bits,
                        } => {
                            field_offset =
                                offset_bytes.checked_add(rel_offset).ok_or_else(|| {
                                    CompileError::new("struct offset overflow for I/O binding")
                                })?;
                            field_bit_offset = field_bit_offset.saturating_add(rel_bits);
                        }
                        FieldAddress::Absolute(_) => {
                            return Err(CompileError::new(
                                "absolute direct address not allowed for structured fields with a base address",
                            ));
                        }
                    }
                }

                let mut field_ref = reference.clone();
                field_ref
                    .path
                    .push(crate::value::RefSegment::Field(field.name.clone()));
                collect_io_bindings(
                    registry,
                    field.type_id,
                    field_ref,
                    field_offset,
                    field_bit_offset,
                    out,
                )?;
                let field_size = type_size_bytes(field.type_id, registry)?;
                let field_end = field_offset
                    .checked_add(field_size)
                    .ok_or_else(|| CompileError::new("struct offset overflow for I/O binding"))?;
                if field.address.is_some() {
                    current_offset = current_offset.max(field_end);
                } else {
                    current_offset = field_end;
                }
            }
            Ok(())
        }
        Type::Union { variants, .. } => {
            for variant in variants {
                let mut variant_offset = offset_bytes;
                let mut variant_bit_offset = bit_offset;
                if let Some(address) = &variant.address {
                    match parse_field_address(address)? {
                        FieldAddress::Relative {
                            offset_bytes: rel_offset,
                            bit_offset: rel_bits,
                        } => {
                            variant_offset =
                                offset_bytes.checked_add(rel_offset).ok_or_else(|| {
                                    CompileError::new("union offset overflow for I/O binding")
                                })?;
                            variant_bit_offset = variant_bit_offset.saturating_add(rel_bits);
                        }
                        FieldAddress::Absolute(_) => {
                            return Err(CompileError::new(
                                "absolute direct address not allowed for union fields with a base address",
                            ));
                        }
                    }
                }
                let mut variant_ref = reference.clone();
                variant_ref
                    .path
                    .push(crate::value::RefSegment::Field(variant.name.clone()));
                collect_io_bindings(
                    registry,
                    variant.type_id,
                    variant_ref,
                    variant_offset,
                    variant_bit_offset,
                    out,
                )?;
            }
            Ok(())
        }
        Type::String { .. } | Type::WString { .. } => Err(CompileError::new(
            "AT binding for STRING types is not supported",
        )),
        Type::FunctionBlock { .. }
        | Type::Class { .. }
        | Type::Interface { .. }
        | Type::Pointer { .. }
        | Type::Reference { .. } => Err(CompileError::new(
            "AT binding for this type is not supported",
        )),
        _ => {
            let size = io_size_for_type(type_id, registry)?;
            if bit_offset > 0 && !matches!(size, crate::io::IoSize::Bit) {
                return Err(CompileError::new(
                    "bit offset only allowed for BOOL direct bindings",
                ));
            }
            let value_type = leaf_value_type(type_id, registry)?;
            out.push(IoLeafBinding {
                reference,
                offset_bytes,
                bit_offset,
                size,
                value_type,
            });
            Ok(())
        }
    }
}
