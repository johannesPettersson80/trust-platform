use super::*;

pub(super) fn validate_resource_meta(
    tables: &ValidationContext<'_>,
    meta: &ResourceMeta,
    construction: Option<&crate::bytecode::StorageLayout>,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    if construction.is_some()
        && (meta.resources.len() != 1
            || meta.resources.iter().any(|resource| {
                [
                    resource.inputs_size,
                    resource.outputs_size,
                    resource.memory_size,
                ]
                .iter()
                .any(|size| *size as usize > crate::io_address::PROCESS_IMAGE_AREA_LIMIT)
            }))
    {
        return Err(RejectionReason::InvalidConstructionRecord.into());
    }
    let strings = tables.strings;
    let ref_table = tables.ref_table;
    let mut program_roots = Vec::new();
    let mut globals = Vec::new();
    if let Some(layout) = construction {
        for declaration in &layout.entries {
            budget.work(1)?;
            if declaration.owner == crate::bytecode::StorageOwner::Global {
                ensure_string_index(strings, declaration.name_idx)?;
                budget.push(
                    &mut globals,
                    (
                        strings.entries[declaration.name_idx as usize].as_str(),
                        declaration.type_id,
                    ),
                )?;
            }
            if declaration.role == crate::bytecode::StorageRole::ProgramRoot {
                ensure_string_index(strings, declaration.name_idx)?;
                budget.push(
                    &mut program_roots,
                    strings.entries[declaration.name_idx as usize].as_str(),
                )?;
            }
        }
        budget.sort_by(&mut program_roots, &mut |a, b, budget| {
            budget.compare_names(a, b)
        })?;
        budget.sort_by(&mut globals, &mut |a, b, budget| {
            budget.compare_names(a.0, b.0)
        })?;
    }
    for resource in &meta.resources {
        budget.work(1)?;
        ensure_string_index(strings, resource.name_idx)?;
        for task in &resource.tasks {
            budget.work(1)?;
            ensure_string_index(strings, task.name_idx)?;
            if let Some(single_idx) = task.single_name_idx {
                ensure_string_index(strings, single_idx)?;
                if construction.is_some() {
                    let name = strings.entries[single_idx as usize].as_str();
                    let at = budget
                        .search_by(&globals, |global, budget| {
                            budget.compare_names(global.0, name)
                        })?
                        .map_err(|_| RejectionReason::InvalidConstructionRecord)?;
                    let ty = globals[at]
                        .1
                        .ok_or(RejectionReason::InvalidConstructionRecord)?;
                    if resolved_primitive_id(tables.types, ty, budget)? != Some(1) {
                        return Err(RejectionReason::InvalidConstructionRecord.into());
                    }
                }
            }
            for idx in &task.program_name_idx {
                budget.work(1)?;
                ensure_string_index(strings, *idx)?;
                let name = strings.entries.get(*idx as usize).ok_or_else(|| {
                    BytecodeError::InvalidIndex {
                        kind: "string".into(),
                        index: *idx,
                    }
                })?;
                let exists = if construction.is_some() {
                    budget
                        .search_by(&program_roots, |root, budget| {
                            budget.compare_names(root, name.as_str())
                        })?
                        .is_ok()
                } else {
                    tables
                        .named_pou(PouKind::Program, name.as_str(), budget)?
                        .is_some()
                };
                if !exists {
                    return Err(BytecodeError::section_diagnostic(
                        SectionDiagnostic::UnknownProgram(name.clone()),
                    ));
                }
            }
            for idx in &task.fb_ref_idx {
                budget.work(1)?;
                if *idx as usize >= ref_table.entries.len() {
                    return Err(BytecodeError::InvalidIndex {
                        kind: "ref".into(),
                        index: *idx,
                    });
                }
                if construction.is_some() {
                    let ty = tables
                        .ref_type(*idx, budget)?
                        .ok_or(RejectionReason::InvalidConstructionRecord)?;
                    if tables.function_block_type(ty, budget)?.is_none() {
                        return Err(RejectionReason::SourceFreeReceiverNotFunctionBlock.into());
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn validate_io_map(
    strings: &StringTable,
    types: &TypeTable,
    ref_table: &RefTable,
    map: &IoMap,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    for binding in &map.bindings {
        budget.work(1)?;
        ensure_string_index(strings, binding.address_str_idx)?;
        if binding.ref_idx as usize >= ref_table.entries.len() {
            return Err(BytecodeError::InvalidIndex {
                kind: "ref".into(),
                index: binding.ref_idx,
            });
        }
        if let Some(type_id) = binding.type_id {
            ensure_type_index(types, type_id)?;
        }
    }
    Ok(())
}
