//! Named access bindings preserve visibility and write permissions after source-free loading.

use super::*;

pub(super) fn validate_aliases(
    module: &BytecodeModuleView<'_>,
    tables: &ValidationContext<'_>,
    construction: &ConstructionTables<'_>,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let aliases = match module.section(SectionId::AccessBindings) {
        Some(SectionData::AccessBindings(value)) => value,
        _ => return Err(BytecodeError::MissingSection("ACCESS_BINDINGS".into())),
    };
    if aliases.entries.len() > BYTECODE_MAX_CONSTRUCTION_RECORDS {
        return Err(RejectionReason::InvalidConstructionRecord.into());
    }
    let mut names = Vec::new();
    let mut globals = Vec::new();
    for declaration in &construction.layout.entries {
        budget.work(1)?;
        if declaration.owner == StorageOwner::Global {
            budget.push(
                &mut globals,
                tables.strings.entries[declaration.name_idx as usize].as_str(),
            )?;
        }
    }
    budget.sort_by(&mut globals, |a, b, budget| budget.compare_names(a, b))?;
    for entry in &aliases.entries {
        budget.work(1)?;
        ensure_string_index(tables.strings, entry.name_idx)?;
        if budget
            .search_by(&globals, |name, budget| {
                budget.compare_names(
                    name,
                    tables.strings.entries[entry.name_idx as usize].as_str(),
                )
            })?
            .is_ok()
        {
            return Err(RejectionReason::InvalidConstructionRecord.into());
        }
        ensure_type_index(tables.types, entry.type_id)?;
        ensure_ref_index(tables.ref_table, entry.ref_idx)?;
        if entry.reserved != 0 || entry.flags & !1 != 0 {
            return Err(RejectionReason::InvalidConstructionRecord.into());
        }
        let reference = &tables.ref_table.entries[entry.ref_idx as usize];
        let declaration = construction
            .paths
            .declaration_for_reference(reference, budget)?
            .and_then(|id| construction.layout.entries.get(id))
            .ok_or(RejectionReason::InvalidConstructionRecord)?;
        if matches!(
            declaration.role,
            StorageRole::External
                | StorageRole::Scratch
                | StorageRole::NativeState
                | StorageRole::EdgePhase
        ) || declaration.flags & 8 != 0
            || (entry.flags & 1 != 0 && declaration.flags & 1 != 0)
        {
            return Err(RejectionReason::InvalidConstructionRecord.into());
        }
        let base = declaration
            .type_id
            .ok_or(RejectionReason::InvalidConstructionRecord)?;
        let selected =
            construction
                .paths
                .select_path(tables, base, &reference.segments, budget, |id| {
                    let member = &construction.layout.entries[id];
                    if matches!(
                        member.role,
                        StorageRole::NativeState | StorageRole::EdgePhase | StorageRole::External
                    ) || member.flags & 8 != 0
                        || (entry.flags & 1 != 0 && member.flags & 1 != 0)
                    {
                        return Err(RejectionReason::InvalidConstructionRecord.into());
                    }
                    Ok(())
                })?;
        lookups::validate_partial_type(
            tables,
            selected,
            entry.type_id,
            entry.partial_kind,
            entry.partial_index,
            budget,
        )?;
        budget.push(
            &mut names,
            tables.strings.entries[entry.name_idx as usize].as_str(),
        )?;
    }
    budget.sort_by(&mut names, |a, b, budget| budget.compare_names(a, b))?;
    for pair in names.windows(2) {
        if budget.compare_names(pair[0], pair[1])?.is_eq() {
            return Err(RejectionReason::InvalidConstructionRecord.into());
        }
    }
    Ok(())
}
