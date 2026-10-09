use super::*;
use crate::bytecode::RefEntry;

pub(super) fn validate_roots(
    tables: &ValidationContext<'_>,
    construction: &ConstructionTables<'_>,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let (layout, roots, declarations) =
        (construction.layout, construction.roots, &construction.paths);
    let mut bindings = Vec::new();
    let mut total_nodes = 0u32;
    let mut root_declarations = Vec::new();
    for (position, root) in roots.entries.iter().enumerate() {
        budget.work(1)?;
        let decl = layout
            .entries
            .get(root.declaration_idx as usize)
            .ok_or(RejectionReason::InvalidConstructionRecord)?;
        if root.flags & !1 != 0
            || root
                .parent_root_idx
                .is_some_and(|parent| parent as usize >= position)
            || decl.owner == StorageOwner::Frame
        {
            return Err(RejectionReason::InvalidConstructionRecord.into());
        }
        if root.instance_owner_id.is_some() {
            let template = root
                .template_pou_id
                .ok_or(RejectionReason::InvalidConstructionRecord)?;
            let pou = tables
                .pou(template, budget)?
                .ok_or(RejectionReason::InvalidConstructionRecord)?;
            if !matches!(
                pou.kind,
                PouKind::Program | PouKind::FunctionBlock | PouKind::Class
            ) {
                return Err(RejectionReason::InvalidConstructionRecord.into());
            }
            if root.flags == 1 {
                let parent = root
                    .parent_root_idx
                    .and_then(|at| roots.entries.get(at as usize))
                    .ok_or(RejectionReason::InvalidConstructionRecord)?;
                let owner = tables
                    .pou(
                        parent
                            .template_pou_id
                            .ok_or(RejectionReason::InvalidConstructionRecord)?,
                        budget,
                    )?
                    .ok_or(RejectionReason::InvalidConstructionRecord)?;
                if root.binding_ref_idx.is_some()
                    || owner.class_meta.as_ref().and_then(|v| v.parent_pou_id) != Some(template)
                {
                    return Err(RejectionReason::InvalidConstructionRecord.into());
                }
            }
        } else if root.template_pou_id.is_some() || root.flags != 0 {
            return Err(RejectionReason::InvalidConstructionRecord.into());
        }
        if root.flags == 0 {
            let reference = tables
                .ref_table
                .entries
                .get(
                    root.binding_ref_idx
                        .ok_or(RejectionReason::InvalidConstructionRecord)?
                        as usize,
                )
                .ok_or(RejectionReason::InvalidConstructionRecord)?;
            if !matches!(
                reference.location,
                RefLocation::Global | RefLocation::Instance | RefLocation::Retain
            ) {
                return Err(RejectionReason::InvalidConstructionRecord.into());
            }
            if root.parent_root_idx.is_none() {
                if decl.owner != StorageOwner::Global || root.binding_ref_idx != decl.ref_idx {
                    return Err(RejectionReason::InvalidConstructionRecord.into());
                }
                total_nodes = total_nodes
                    .checked_add(decl.construction_nodes)
                    .filter(|total| *total <= BYTECODE_MAX_CONSTRUCTION_NODES)
                    .ok_or(RejectionReason::ConstructionDemandOverflow)?;
                budget.push(&mut root_declarations, root.declaration_idx)?;
            }
            budget.push(
                &mut bindings,
                root.binding_ref_idx
                    .ok_or(RejectionReason::InvalidConstructionRecord)?,
            )?;
            if decl.role == StorageRole::ProgramRoot && root.template_pou_id != decl.owner_pou_id {
                return Err(RejectionReason::InvalidConstructionRecord.into());
            }
        }
    }
    budget.sort_by(&mut bindings, |a, b, _| Ok(a.cmp(b)))?;
    budget.sort_by(&mut root_declarations, |a, b, _| Ok(a.cmp(b)))?;
    for pair in bindings.windows(2) {
        budget.work(1)?;
        if pair[0] == pair[1] {
            return Err(RejectionReason::InvalidConstructionRecord.into());
        }
    }
    for (id, declaration) in layout.entries.iter().enumerate() {
        budget.work(1)?;
        if declaration.owner == StorageOwner::Global
            && declaration.role != StorageRole::External
            && budget
                .search_by(&root_declarations, |v, _| Ok((*v as usize).cmp(&id)))?
                .is_err()
        {
            return Err(RejectionReason::IncompleteConstructionMetadata.into());
        }
    }
    for (id, reference) in tables.ref_table.entries.iter().enumerate() {
        budget.work(1)?;
        if matches!(
            reference.location,
            RefLocation::Global | RefLocation::Retain | RefLocation::Instance
        ) {
            let (_, ty) = binding_type(tables, layout, declarations, reference, budget)?;
            if let Some(recorded) = tables.ref_type(id as u32, budget)? {
                if ty != Some(recorded) {
                    return Err(RejectionReason::InvalidConstructionRecord.into());
                }
            }
        }
    }
    for root in &roots.entries {
        budget.work(1)?;
        if root.flags != 0 {
            continue;
        }
        let reference = &tables.ref_table.entries[root
            .binding_ref_idx
            .ok_or(RejectionReason::InvalidConstructionRecord)?
            as usize];
        let (declaration, ty) = binding_type(tables, layout, declarations, reference, budget)?;
        if declaration != root.declaration_idx as usize {
            return Err(RejectionReason::InvalidConstructionRecord.into());
        }
        let expected_template = if let Some(ty) = ty {
            let ty = super::lookups::resolve_alias(tables, ty, budget)?;
            match &tables.types.entries[ty as usize].data {
                TypeData::Pou { pou_id } => Some(*pou_id),
                _ => None,
            }
        } else {
            layout.entries[declaration].owner_pou_id
        };
        if root.template_pou_id != expected_template
            || root.instance_owner_id.is_some() != expected_template.is_some()
        {
            return Err(RejectionReason::InvalidConstructionRecord.into());
        }
        if let Some(parent) = root.parent_root_idx {
            let parent = &roots.entries[parent as usize];
            let inside_instance = reference.location == RefLocation::Instance
                && Some(reference.owner_id) == parent.instance_owner_id;
            let inside_aggregate = parent
                .binding_ref_idx
                .and_then(|id| tables.ref_table.entries.get(id as usize))
                .is_some_and(|base| {
                    reference.location == base.location
                        && reference.owner_id == base.owner_id
                        && reference.offset == base.offset
                        && reference.segments.len() > base.segments.len()
                        && reference.segments.starts_with(&base.segments)
                });
            if !inside_instance && !inside_aggregate {
                return Err(RejectionReason::InvalidConstructionRecord.into());
            }
        }
    }
    Ok(())
}

fn binding_type(
    tables: &ValidationContext<'_>,
    layout: &StorageLayout,
    declarations: &super::lookups::DeclarationIndex<'_>,
    reference: &RefEntry,
    budget: &mut ValidationBudget,
) -> Result<(usize, Option<u32>), BytecodeError> {
    let declaration = declarations
        .declaration_for_reference(reference, budget)?
        .ok_or(RejectionReason::IncompleteConstructionMetadata)?;
    let ty = match layout.entries[declaration].type_id {
        Some(ty) => Some(declarations.path_type(tables, ty, &reference.segments, budget)?),
        None if reference.segments.is_empty()
            && layout.entries[declaration].role == StorageRole::ProgramRoot =>
        {
            None
        }
        None => return Err(RejectionReason::InvalidConstructionRecord.into()),
    };
    Ok((declaration, ty))
}
