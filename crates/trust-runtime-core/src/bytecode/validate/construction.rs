use super::*;
use crate::bytecode::{
    ConstructionRoots, InitializationOnce, InitializationPhase, InitializationStage,
    InitializationTarget, InitializerIndex, StorageDeclaration, StorageLayout, StorageOwner,
    StorageRole, BYTECODE_MAX_CONSTRUCTION_NODES, BYTECODE_MAX_CONSTRUCTION_RECORDS,
};

mod declarations;
mod demand;
use declarations::validate_declarations;
mod aliases;
mod edges;
mod initializers;
mod io;
mod lookups;
mod roots;

pub(super) fn sections<'a>(
    module: &BytecodeModuleView<'a>,
) -> Result<
    (
        &'a StorageLayout,
        &'a ConstructionRoots,
        &'a InitializerIndex,
    ),
    BytecodeError,
> {
    let layout = match module.section(SectionId::StorageLayout) {
        Some(SectionData::StorageLayout(v)) => v,
        _ => {
            return Err(BytecodeError::MissingSection(
                smol_str::SmolStr::new_static("STORAGE_LAYOUT"),
            ))
        }
    };
    let roots = match module.section(SectionId::ConstructionRoots) {
        Some(SectionData::ConstructionRoots(v)) => v,
        _ => {
            return Err(BytecodeError::MissingSection(
                smol_str::SmolStr::new_static("CONSTRUCTION_ROOTS"),
            ))
        }
    };
    let initializers = match module.section(SectionId::Initializers) {
        Some(SectionData::Initializers(v)) => v,
        _ => {
            return Err(BytecodeError::MissingSection(
                smol_str::SmolStr::new_static("INITIALIZERS"),
            ))
        }
    };
    if [
        layout.entries.len(),
        roots.entries.len(),
        initializers.entries.len(),
    ]
    .iter()
    .any(|count| *count > BYTECODE_MAX_CONSTRUCTION_RECORDS)
    {
        return Err(RejectionReason::InvalidConstructionRecord.into());
    }
    Ok((layout, roots, initializers))
}

struct ConstructionTables<'a> {
    resource: &'a crate::bytecode::ResourceEntry,
    layout: &'a StorageLayout,
    roots: &'a ConstructionRoots,
    initializers: &'a InitializerIndex,
    paths: lookups::DeclarationIndex<'a>,
}

pub(super) fn validate_construction(
    module: &BytecodeModuleView<'_>,
    tables: &ValidationContext<'_>,
    bodies: &[u8],
    instruction_count: &mut usize,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let (layout, roots, initializers) = sections(module)?;
    budget.temporary(|budget| validate_declarations(tables, layout, budget))?;
    budget.temporary(|budget| edges::validate_edges(tables, layout, budget))?;
    budget.temporary(|budget| {
        let resource = match module.section(SectionId::ResourceMeta) {
            Some(SectionData::ResourceMeta(meta)) if meta.resources.len() == 1 => {
                &meta.resources[0]
            }
            _ => return Err(RejectionReason::InvalidConstructionRecord.into()),
        };
        let construction = ConstructionTables {
            resource,
            layout,
            roots,
            initializers,
            paths: lookups::DeclarationIndex::new(tables, layout, roots, budget)?,
        };
        budget.temporary(|budget| roots::validate_roots(tables, &construction, budget))?;
        budget.temporary(|budget| io::validate_bindings(module, tables, &construction, budget))?;
        budget
            .temporary(|budget| aliases::validate_aliases(module, tables, &construction, budget))?;
        budget.temporary(|budget| {
            initializers::validate_initializers(
                tables,
                &construction,
                bodies,
                instruction_count,
                budget,
            )
        })?;
        Ok(())
    })?;
    Ok(())
}

/// Compute portable logical construction demand for an authoring candidate.
/// This is not semantic validation or profile admission; producers must validate afterward.
pub fn construction_demands(
    module: BytecodeModuleView<'_>,
    limits: crate::bytecode::ValidationLimits,
) -> Result<Vec<u32>, BytecodeError> {
    let (layout, _, _) = sections(&module)?;
    let strings = match module.section(SectionId::StringTable) {
        Some(SectionData::StringTable(v)) => v,
        _ => return Err(RejectionReason::IncompleteConstructionMetadata.into()),
    };
    let types = match module.section(SectionId::TypeTable) {
        Some(SectionData::TypeTable(v)) => v,
        _ => return Err(RejectionReason::IncompleteConstructionMetadata.into()),
    };
    let index = match module.section(SectionId::PouIndex) {
        Some(SectionData::PouIndex(v)) => v,
        _ => return Err(RejectionReason::IncompleteConstructionMetadata.into()),
    };
    let const_pool = match module.section(SectionId::ConstPool) {
        Some(SectionData::ConstPool(v)) => v,
        _ => return Err(RejectionReason::IncompleteConstructionMetadata.into()),
    };
    let ref_table = match module.section(SectionId::RefTable) {
        Some(SectionData::RefTable(v)) => v,
        _ => return Err(RejectionReason::IncompleteConstructionMetadata.into()),
    };
    let mut budget = ValidationBudget::new(limits);
    let tables = ValidationContext::new(
        strings,
        index,
        types,
        const_pool,
        ref_table,
        None,
        &mut budget,
    )?;
    demand::declaration_demands(&tables, layout, &mut budget)
}
