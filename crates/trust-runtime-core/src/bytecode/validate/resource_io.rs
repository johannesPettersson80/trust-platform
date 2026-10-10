use super::*;

pub(super) fn validate_resource_meta(
    tables: &ValidationContext<'_>,
    meta: &ResourceMeta,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let strings = tables.strings;
    let ref_table = tables.ref_table;
    for resource in &meta.resources {
        budget.work(1)?;
        ensure_string_index(strings, resource.name_idx)?;
        for task in &resource.tasks {
            budget.work(1)?;
            ensure_string_index(strings, task.name_idx)?;
            if let Some(single_idx) = task.single_name_idx {
                ensure_string_index(strings, single_idx)?;
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
                if tables
                    .named_pou(PouKind::Program, name.as_str(), budget)?
                    .is_none()
                {
                    return Err(BytecodeError::InvalidSection(
                        format!("task references unknown program '{name}'").into(),
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
