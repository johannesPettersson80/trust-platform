use super::*;
use crate::bytecode::{InitializationTrigger, InitializerEntry};

mod configuration;
mod recipes;

pub(super) fn validate_initializers(
    tables: &ValidationContext<'_>,
    construction: &ConstructionTables<'_>,
    bodies: &[u8],
    instruction_count: &mut usize,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let (layout, initializers) = (construction.layout, construction.initializers);
    recipes::validate_unique(initializers, budget)?;
    let mut ranges = Vec::new();
    let mut declarations = Vec::new();
    let mut stages: Vec<[u8; 2]> = Vec::new();
    budget.reserve(&mut stages, layout.entries.len())?;
    budget.work(layout.entries.len())?;
    stages.resize(layout.entries.len(), [0u8; 2]);
    for pou in &tables.index.entries {
        budget.work(1)?;
        if pou.code_length != 0 {
            budget.push(
                &mut ranges,
                (
                    pou.code_offset,
                    pou.code_offset
                        .checked_add(pou.code_length)
                        .ok_or(RejectionReason::InitializerCodeRange)?,
                ),
            )?;
        }
    }
    for (id, entry) in initializers.entries.iter().enumerate() {
        budget.work(1)?;
        let declaration = entry
            .declaration_idx
            .map(|id| {
                layout
                    .entries
                    .get(id as usize)
                    .ok_or(RejectionReason::InvalidInitializerRecord)
            })
            .transpose()?;
        if entry.target_reserved != [0; 2] {
            return Err(RejectionReason::InvalidInitializerRecord.into());
        }
        let owner = entry
            .owner_pou_id
            .map(|owner| tables.pou(owner, budget))
            .transpose()?
            .flatten();
        if entry.owner_pou_id.is_some() && owner.is_none() {
            return Err(RejectionReason::InvalidInitializerRecord.into());
        }
        let expected_type =
            validate_descriptor(tables, construction, declaration, owner, id, entry, budget)?;
        if declaration.is_some() && entry.phase != InitializationPhase::Configuration {
            let declaration_id = entry
                .declaration_idx
                .ok_or(RejectionReason::InvalidInitializerRecord)?;
            let stage = &mut stages[declaration_id as usize][entry.trigger as usize];
            match entry.stage {
                InitializationStage::Default if *stage == 0 => {
                    *stage = 1;
                    budget.push(&mut declarations, (declaration_id, entry.trigger as u8))?;
                }
                InitializationStage::Explicit if *stage == 1 => *stage = 2,
                _ => return Err(RejectionReason::InvalidInitializerRecord.into()),
            }
        }
        validate_result(tables, entry, id, expected_type, budget)?;
        let end = entry
            .code_offset
            .checked_add(entry.code_length)
            .ok_or(RejectionReason::InitializerCodeRange)?;
        if end as usize > bodies.len() {
            return Err(RejectionReason::InitializerCodeRange.into());
        }
        if entry.code_length != 0 {
            budget.push(&mut ranges, (entry.code_offset, end))?;
        }
        validate_body(
            tables,
            construction,
            entry,
            id,
            &bodies[entry.code_offset as usize..end as usize],
            instruction_count,
            budget,
        )?;
    }
    validate_coverage(tables, construction, &mut ranges, &mut declarations, budget)
}

fn validate_visibility(
    tables: &ValidationContext<'_>,
    entry: &InitializerEntry,
    id: usize,
    instruction: &Instruction,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    budget.work(1)?;
    if instruction.opcode == 0x09
        && instruction.operand(0) != crate::bytecode::NATIVE_CALL_KIND_STDLIB
    {
        return Err(RejectionReason::InvalidInitializerRecord.into());
    }
    if instruction.opcode == 0x05 {
        return Err(RejectionReason::LegacyCall.into());
    }
    if matches!(instruction.opcode, 0x23 | 0x24) {
        let owner = entry
            .owner_pou_id
            .map(|id| tables.pou(id, budget))
            .transpose()?
            .flatten();
        if owner.is_none_or(|pou| {
            !matches!(
                pou.kind,
                PouKind::Program | PouKind::FunctionBlock | PouKind::Class | PouKind::Method
            )
        }) {
            return Err(RejectionReason::InitializerVisibility.into());
        }
    }
    if matches!(instruction.opcode, 0x20..=0x22) {
        let reference = tables
            .ref_table
            .entries
            .get(instruction.operand(0) as usize)
            .ok_or(BytecodeError::InvalidIndex {
                kind: "ref".into(),
                index: instruction.operand(0),
            })?;
        if instruction.opcode == 0x21 && reference.location != RefLocation::InitializerResult {
            return Err(RejectionReason::InitializerReferenceScope.into());
        }
        match reference.location {
            RefLocation::InitializerResult if reference.owner_id as usize != id => {
                return Err(RejectionReason::InitializerReferenceScope.into())
            }
            RefLocation::Local if reference.offset >= entry.visible_local_count => {
                return Err(RejectionReason::InitializerVisibility.into())
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_declaration_plan(
    declaration: &StorageDeclaration,
    owner: Option<&PouEntry>,
    entry: &InitializerEntry,
) -> Result<(), BytecodeError> {
    if entry.owner_pou_id != declaration.owner_pou_id
        || matches!(
            declaration.role,
            StorageRole::ProgramRoot
                | StorageRole::External
                | StorageRole::Scratch
                | StorageRole::NativeState
        )
        || entry.target_kind != InitializationTarget::Declaration
        || entry.target_idx.is_some()
        || entry.partial_kind != 0
        || entry.partial_index != 0
    {
        return Err(RejectionReason::InvalidInitializerRecord.into());
    }
    if declaration.role == StorageRole::EdgePhase
        && (entry.code_length != 0 || entry.stage != InitializationStage::Default)
    {
        return Err(RejectionReason::InvalidInitializerRecord.into());
    }
    let phase = match (declaration.owner, declaration.role) {
        (_, StorageRole::Static) => InitializationPhase::Static,
        (StorageOwner::Frame, StorageRole::Return) => InitializationPhase::Return,
        (StorageOwner::Frame, StorageRole::Parameter) => InitializationPhase::Parameter,
        (StorageOwner::Frame, _) => InitializationPhase::Frame,
        (StorageOwner::Instance, _) => InitializationPhase::Instance,
        (StorageOwner::Global, _) => InitializationPhase::Resource,
    };
    let once = match (declaration.role, declaration.owner) {
        (StorageRole::Static, StorageOwner::Global) => InitializationOnce::Module,
        (StorageRole::Static, StorageOwner::Instance) => InitializationOnce::Instance,
        _ => InitializationOnce::None,
    };
    if entry.phase != phase || entry.once != once {
        return Err(RejectionReason::InvalidInitializerRecord.into());
    }
    if entry.trigger == InitializationTrigger::AfterRestart
        && (declaration.owner != StorageOwner::Global || declaration.role != StorageRole::Static)
    {
        return Err(RejectionReason::InvalidInitializerRecord.into());
    }
    let visible = match phase {
        InitializationPhase::Frame | InitializationPhase::Parameter => declaration.slot,
        InitializationPhase::Static if entry.trigger == InitializationTrigger::AfterRestart => {
            owner.map_or(0, |pou| {
                u32::from(pou.return_type_id.is_some()) + pou.params.len() as u32
            })
        }
        _ => 0,
    };
    if phase == InitializationPhase::Static
        && entry.trigger == InitializationTrigger::Ordinary
        && entry.visible_static_count != 0
    {
        return Err(RejectionReason::InitializerVisibility.into());
    }
    if entry.visible_local_count != visible {
        return Err(RejectionReason::InitializerVisibility.into());
    }
    Ok(())
}

fn validate_body(
    tables: &ValidationContext<'_>,
    construction: &ConstructionTables<'_>,
    entry: &InitializerEntry,
    id: usize,
    code: &[u8],
    instruction_count: &mut usize,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let declaration = entry
        .declaration_idx
        .and_then(|id| construction.layout.entries.get(id as usize));
    let owner = entry
        .owner_pou_id
        .map(|id| tables.pou(id, budget))
        .transpose()?
        .flatten();
    budget.temporary(|budget| {
        let instructions =
            decode_instructions(code, instruction_count, budget, |instruction, budget| {
                validate_visibility(tables, entry, id, instruction, budget)?;
                validate_instruction_operands(tables, owner, instruction, budget)
            })?;
        let result_is_local = entry.phase != InitializationPhase::Configuration
            && declaration.is_some_and(|value| {
                value.owner == StorageOwner::Frame && value.role == StorageRole::Variable
            });
        budget.temporary(|budget| {
            validate_initializer_reference_escape(tables, &instructions, result_is_local, budget)
        })?;
        budget.temporary(|budget| {
            validate_initializer_stack_shape(
                tables,
                &instructions,
                code.len(),
                result_is_local,
                budget,
            )
        })?;
        budget.temporary(|budget| validate_const_compat(tables, &instructions, budget))?;
        budget.temporary(|budget| {
            validate_param_direction_calls(tables, owner, &instructions, budget)
        })?;
        Ok(())
    })
}

fn validate_descriptor(
    tables: &ValidationContext<'_>,
    construction: &ConstructionTables<'_>,
    declaration: Option<&StorageDeclaration>,
    owner: Option<&PouEntry>,
    id: usize,
    entry: &InitializerEntry,
    budget: &mut ValidationBudget,
) -> Result<u32, BytecodeError> {
    let recipe_type = recipes::validate_identity(
        tables,
        construction.initializers,
        construction.layout,
        id,
        entry,
        budget,
    )?;
    let expected_type = if let Some(type_id) = recipe_type {
        type_id
    } else if entry.phase == InitializationPhase::Configuration {
        configuration::validate_action(tables, construction, declaration, entry, budget)?
    } else if entry.phase == InitializationPhase::ValueDefault {
        if declaration.is_some()
            || entry.owner_pou_id.is_some()
            || entry.visible_local_count != 0
            || entry.visible_static_count != 0
            || entry.once != InitializationOnce::None
            || entry.stage != InitializationStage::Default
            || entry.target_kind != InitializationTarget::Declaration
            || entry.target_idx.is_some()
            || entry.partial_kind != 0
            || entry.partial_index != 0
            || entry.trigger != InitializationTrigger::Ordinary
        {
            return Err(RejectionReason::InvalidInitializerRecord.into());
        }
        tables
            .ref_type(entry.result_ref_idx, budget)?
            .ok_or(RejectionReason::InvalidInitializerRecord)?
    } else {
        let declaration = declaration.ok_or(RejectionReason::InvalidInitializerRecord)?;
        validate_declaration_plan(declaration, owner, entry)?;
        let expected_statics = if declaration.role == StorageRole::Static
            && entry.trigger == InitializationTrigger::Ordinary
        {
            0
        } else {
            let before = if declaration.role == StorageRole::Static {
                entry.declaration_idx.map(|id| id as usize)
            } else {
                None
            };
            construction
                .paths
                .static_count(declaration.owner_pou_id, before, budget)?
        };
        if entry.visible_static_count != expected_statics {
            return Err(RejectionReason::InitializerVisibility.into());
        }
        declaration
            .type_id
            .ok_or(RejectionReason::InvalidInitializerRecord)?
    };
    Ok(expected_type)
}

fn validate_result(
    tables: &ValidationContext<'_>,
    entry: &InitializerEntry,
    id: usize,
    expected_type: u32,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let result = tables
        .ref_table
        .entries
        .get(entry.result_ref_idx as usize)
        .ok_or(RejectionReason::InvalidInitializerRecord)?;
    if result.location != RefLocation::InitializerResult
        || result.owner_id as usize != id
        || result.offset != 0
        || !result.segments.is_empty()
        || tables.ref_type(entry.result_ref_idx, budget)? != Some(expected_type)
    {
        return Err(RejectionReason::InvalidInitializerRecord.into());
    }
    let meta = tables
        .first_var_ref(entry.result_ref_idx, budget)?
        .and_then(|at| tables.var_meta.and_then(|meta| meta.entries.get(at)))
        .ok_or(RejectionReason::InvalidInitializerRecord)?;
    if meta.retain != 0 || meta.init_const_idx.is_some() {
        return Err(RejectionReason::InvalidInitializerRecord.into());
    }
    Ok(())
}

fn validate_coverage(
    tables: &ValidationContext<'_>,
    construction: &ConstructionTables<'_>,
    ranges: &mut [(u32, u32)],
    declarations: &mut [(u32, u8)],
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let (layout, initializers, paths) = (
        construction.layout,
        construction.initializers,
        &construction.paths,
    );
    budget.sort_by(ranges, &mut |a, b, _| Ok(a.cmp(b)))?;
    for pair in ranges.windows(2) {
        budget.work(1)?;
        if pair[1].0 < pair[0].1 {
            return Err(RejectionReason::InitializerCodeRange.into());
        }
    }
    budget.sort_by(declarations, &mut |a, b, _| Ok(a.cmp(b)))?;
    for pair in declarations.windows(2) {
        budget.work(1)?;
        if pair[0] == pair[1] {
            return Err(RejectionReason::InvalidInitializerRecord.into());
        }
    }
    for (id, declaration) in layout.entries.iter().enumerate() {
        budget.work(1)?;
        let requires_plan = !matches!(
            declaration.role,
            StorageRole::ProgramRoot
                | StorageRole::External
                | StorageRole::Scratch
                | StorageRole::NativeState
        );
        if requires_plan
            && budget
                .search_by(declarations, |value, _| Ok(value.cmp(&(id as u32, 0))))?
                .is_err()
        {
            return Err(RejectionReason::IncompleteConstructionMetadata.into());
        }
        if declaration.owner == StorageOwner::Global
            && declaration.role == StorageRole::Static
            && budget
                .search_by(declarations, |value, _| Ok(value.cmp(&(id as u32, 1))))?
                .is_err()
        {
            return Err(RejectionReason::IncompleteConstructionMetadata.into());
        }
    }
    for (id, reference) in tables.ref_table.entries.iter().enumerate() {
        budget.work(1)?;
        if reference.location != RefLocation::InitializerResult {
            continue;
        }
        let owner = initializers
            .entries
            .get(reference.owner_id as usize)
            .ok_or(RejectionReason::InitializerReferenceScope)?;
        if reference.offset != 0 {
            return Err(RejectionReason::InitializerReferenceScope.into());
        }
        let base_type = tables
            .ref_type(owner.result_ref_idx, budget)?
            .ok_or(RejectionReason::InvalidInitializerRecord)?;
        let selected_type = paths.path_type(tables, base_type, &reference.segments, budget)?;
        if tables.ref_type(id as u32, budget)? != Some(selected_type) {
            return Err(RejectionReason::InvalidInitializerRecord.into());
        }
    }
    Ok(())
}
