pub(super) fn collect_direct_field_bindings(
    registry: &TypeRegistry,
    reference: &crate::value::ValueRef,
    type_id: TypeId,
    name: &SmolStr,
    wildcards: &mut Vec<WildcardRequirement>,
    out: &mut Vec<InstanceBinding>,
) -> Result<(), CompileError> {
    let ty = registry
        .get(type_id)
        .ok_or_else(|| CompileError::new("unknown type for direct field binding"))?;
    match ty {
        Type::Alias { target, .. } => {
            collect_direct_field_bindings(registry, reference, *target, name, wildcards, out)
        }
        Type::Subrange { base, .. } => {
            collect_direct_field_bindings(registry, reference, *base, name, wildcards, out)
        }
        Type::Enum { base, .. } => {
            collect_direct_field_bindings(registry, reference, *base, name, wildcards, out)
        }
        Type::Struct { fields, .. } => {
            for field in fields {
                let mut field_ref = reference.clone();
                field_ref
                    .path
                    .push(crate::value::RefSegment::Field(field.name.clone()));
                let field_name = join_instance_path(name, &field.name);
                if let Some(address) = &field.address {
                    match parse_field_address(address)? {
                        FieldAddress::Absolute(address) => {
                            if address.wildcard {
                                wildcards.push(WildcardRequirement {
                                    name: field_name,
                                    reference: field_ref,
                                    area: address.area,
                                });
                            } else {
                                out.push(InstanceBinding {
                                    reference: field_ref,
                                    type_id: field.type_id,
                                    address,
                                    display_name: field_name,
                                });
                            }
                        }
                        FieldAddress::Relative { .. } => {
                            continue;
                        }
                    }
                    continue;
                }
                collect_direct_field_bindings(
                    registry,
                    &field_ref,
                    field.type_id,
                    &field_name,
                    wildcards,
                    out,
                )?;
            }
            Ok(())
        }
        Type::Union { variants, .. } => {
            for variant in variants {
                let mut variant_ref = reference.clone();
                variant_ref
                    .path
                    .push(crate::value::RefSegment::Field(variant.name.clone()));
                let variant_name = join_instance_path(name, &variant.name);
                if let Some(address) = &variant.address {
                    match parse_field_address(address)? {
                        FieldAddress::Absolute(address) => {
                            if address.wildcard {
                                wildcards.push(WildcardRequirement {
                                    name: variant_name,
                                    reference: variant_ref,
                                    area: address.area,
                                });
                            } else {
                                out.push(InstanceBinding {
                                    reference: variant_ref,
                                    type_id: variant.type_id,
                                    address,
                                    display_name: variant_name,
                                });
                            }
                        }
                        FieldAddress::Relative { .. } => {
                            continue;
                        }
                    }
                    continue;
                }
                collect_direct_field_bindings(
                    registry,
                    &variant_ref,
                    variant.type_id,
                    &variant_name,
                    wildcards,
                    out,
                )?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
