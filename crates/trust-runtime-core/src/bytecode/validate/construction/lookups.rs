//! Once-built declaration indexes for construction and target-path validation.

use super::*;
use crate::bytecode::{RefEntry, RefSegment};

pub(super) struct DeclarationIndex<'a> {
    slots: Vec<(u32, u32, usize)>,
    instances: Vec<(u32, u32)>,
    fields: Vec<(u8, u32, &'a str, u32, Option<usize>)>,
    globals: Vec<(u8, u32, u32, usize)>,
    statics: Vec<(u32, usize)>,
}

impl<'a> DeclarationIndex<'a> {
    pub(super) fn new(
        tables: &ValidationContext<'a>,
        layout: &'a StorageLayout,
        roots: &ConstructionRoots,
        budget: &mut ValidationBudget,
    ) -> Result<Self, BytecodeError> {
        let mut result = Self {
            slots: Vec::new(),
            instances: Vec::new(),
            fields: Vec::new(),
            globals: Vec::new(),
            statics: Vec::new(),
        };
        for root in &roots.entries {
            budget.work(1)?;
            if let Some(instance) = root.instance_owner_id {
                budget.push(
                    &mut result.instances,
                    (
                        instance,
                        root.template_pou_id
                            .ok_or(RejectionReason::InvalidConstructionRecord)?,
                    ),
                )?;
            }
        }
        budget.sort_by(&mut result.instances, |a, b, _| Ok(a.cmp(b)))?;
        for pair in result.instances.windows(2) {
            budget.work(1)?;
            if pair[0].0 == pair[1].0 {
                return Err(RejectionReason::InvalidConstructionRecord.into());
            }
        }
        for (id, declaration) in layout.entries.iter().enumerate() {
            budget.work(1)?;
            if declaration.role == StorageRole::Static {
                let owner = declaration
                    .owner_pou_id
                    .ok_or(RejectionReason::InvalidConstructionRecord)?;
                budget.push(&mut result.statics, (owner, id))?;
            }
            if declaration.role == StorageRole::External {
                continue;
            }
            if declaration.owner == StorageOwner::Instance {
                let mut owner = declaration
                    .owner_pou_id
                    .ok_or(RejectionReason::InvalidConstructionRecord)?;
                let pou = tables
                    .pou(owner, budget)?
                    .ok_or(RejectionReason::InvalidConstructionRecord)?;
                if pou.kind == PouKind::Method {
                    owner = pou
                        .owner_pou_id
                        .ok_or(RejectionReason::InvalidConstructionRecord)?;
                }
                budget.push(&mut result.slots, (owner, declaration.slot, id))?;
                if let Some(ty) = declaration.type_id {
                    budget.push(
                        &mut result.fields,
                        (
                            1,
                            owner,
                            tables.strings.entries[declaration.name_idx as usize].as_str(),
                            ty,
                            Some(id),
                        ),
                    )?;
                }
            } else if declaration.owner == StorageOwner::Global {
                let reference = declaration
                    .ref_idx
                    .and_then(|at| tables.ref_table.entries.get(at as usize))
                    .ok_or(RejectionReason::InvalidConstructionRecord)?;
                budget.push(
                    &mut result.globals,
                    (
                        reference.location as u8,
                        reference.owner_id,
                        reference.offset,
                        id,
                    ),
                )?;
            }
        }
        for (id, entry) in tables.types.entries.iter().enumerate() {
            budget.work(1)?;
            if let TypeData::Struct { fields } | TypeData::Union { fields } = &entry.data {
                for field in fields {
                    budget.work(1)?;
                    budget.push(
                        &mut result.fields,
                        (
                            0,
                            id as u32,
                            tables.strings.entries[field.name_idx as usize].as_str(),
                            field.type_id,
                            None,
                        ),
                    )?;
                }
            }
        }
        budget.sort_by(&mut result.statics, |a, b, _| Ok(a.cmp(b)))?;
        budget.sort_by(&mut result.slots, |a, b, _| Ok(a.cmp(b)))?;
        budget.sort_by(&mut result.globals, |a, b, _| Ok(a.cmp(b)))?;
        budget.sort_by(&mut result.fields, |a, b, budget| {
            Ok((a.0, a.1)
                .cmp(&(b.0, b.1))
                .then(budget.compare_names(a.2, b.2)?))
        })?;
        Ok(result)
    }

    /// Number of static declarations in the owner's permitted declaration prefix.
    pub(super) fn static_count(
        &self,
        owner: Option<u32>,
        before: Option<usize>,
        budget: &mut ValidationBudget,
    ) -> Result<u32, BytecodeError> {
        let Some(owner) = owner else {
            return Ok(0);
        };
        let start = budget.lower_bound(&self.statics, |entry, _| Ok(entry.0 < owner))?;
        let end = budget.lower_bound(&self.statics, |entry, _| {
            Ok(entry.0 < owner || (entry.0 == owner && before.is_none_or(|limit| entry.1 < limit)))
        })?;
        u32::try_from(end - start).map_err(|_| RejectionReason::InitializerVisibility.into())
    }

    pub(super) fn instance_template(
        &self,
        instance: u32,
        budget: &mut ValidationBudget,
    ) -> Result<Option<u32>, BytecodeError> {
        Ok(budget
            .search_by(&self.instances, |entry, _| Ok(entry.0.cmp(&instance)))?
            .ok()
            .map(|at| self.instances[at].1))
    }

    pub(super) fn declaration_for_reference(
        &self,
        reference: &RefEntry,
        budget: &mut ValidationBudget,
    ) -> Result<Option<usize>, BytecodeError> {
        match reference.location {
            RefLocation::Instance => match self.instance_template(reference.owner_id, budget)? {
                Some(template) => self.instance_slot(template, reference.offset, budget),
                None => Ok(None),
            },
            RefLocation::Global | RefLocation::Retain => self.global(reference, budget),
            _ => Ok(None),
        }
    }

    pub(super) fn global(
        &self,
        reference: &RefEntry,
        budget: &mut ValidationBudget,
    ) -> Result<Option<usize>, BytecodeError> {
        let key = (
            reference.location as u8,
            reference.owner_id,
            reference.offset,
        );
        Ok(budget
            .search_by(&self.globals, |entry, _| {
                Ok((entry.0, entry.1, entry.2).cmp(&key))
            })?
            .ok()
            .map(|at| self.globals[at].3))
    }

    pub(super) fn instance_slot(
        &self,
        pou: u32,
        slot: u32,
        budget: &mut ValidationBudget,
    ) -> Result<Option<usize>, BytecodeError> {
        Ok(budget
            .search_by(&self.slots, |entry, _| {
                Ok((entry.0, entry.1).cmp(&(pou, slot)))
            })?
            .ok()
            .map(|at| self.slots[at].2))
    }

    pub(super) fn path_type(
        &self,
        tables: &ValidationContext<'_>,
        ty: u32,
        segments: &[RefSegment],
        budget: &mut ValidationBudget,
    ) -> Result<u32, BytecodeError> {
        self.select_path(tables, ty, segments, budget, |_| Ok(()))
    }

    pub(super) fn select_path(
        &self,
        tables: &ValidationContext<'_>,
        mut ty: u32,
        segments: &[RefSegment],
        budget: &mut ValidationBudget,
        mut member: impl FnMut(usize) -> Result<(), BytecodeError>,
    ) -> Result<u32, BytecodeError> {
        for segment in segments {
            budget.work(1)?;
            ty = resolve_alias(tables, ty, budget)?;
            let entry = tables
                .types
                .entries
                .get(ty as usize)
                .ok_or(RejectionReason::InvalidConstructionRecord)?;
            match (segment, &entry.data) {
                (RefSegment::Index(indices), TypeData::Array { elem_type_id, dims }) => {
                    if indices.len() != dims.len() {
                        return Err(RejectionReason::InvalidConstructionRecord.into());
                    }
                    for (index, (low, high)) in indices.iter().zip(dims) {
                        budget.work(1)?;
                        if index < low || index > high {
                            return Err(RejectionReason::InvalidConstructionRecord.into());
                        }
                    }
                    ty = *elem_type_id;
                }
                (RefSegment::Field { name_idx }, data) => {
                    let name = tables
                        .strings
                        .entries
                        .get(*name_idx as usize)
                        .ok_or(RejectionReason::InvalidConstructionRecord)?
                        .as_str();
                    let (kind, mut owner) = match data {
                        TypeData::Struct { .. } | TypeData::Union { .. } => (0, ty),
                        TypeData::Pou { pou_id } => (1, *pou_id),
                        _ => return Err(RejectionReason::InvalidConstructionRecord.into()),
                    };
                    let mut found = None;
                    for _ in 0..64 {
                        budget.work(1)?;
                        if let Ok(at) = budget.search_by(&self.fields, |entry, budget| {
                            Ok((entry.0, entry.1)
                                .cmp(&(kind, owner))
                                .then(budget.compare_names(entry.2, name)?))
                        })? {
                            found = Some(at);
                            break;
                        }
                        if kind == 0 {
                            break;
                        }
                        let Some(parent) = tables
                            .pou(owner, budget)?
                            .and_then(|pou| pou.class_meta.as_ref())
                            .and_then(|meta| meta.parent_pou_id)
                        else {
                            break;
                        };
                        owner = parent;
                    }
                    let at = found.ok_or(RejectionReason::InvalidConstructionRecord)?;
                    if let Some(declaration) = self.fields[at].4 {
                        member(declaration)?;
                    }
                    ty = self.fields[at].3;
                }
                _ => return Err(RejectionReason::InvalidConstructionRecord.into()),
            }
        }
        Ok(ty)
    }
}

pub(super) fn resolve_alias(
    tables: &ValidationContext<'_>,
    mut ty: u32,
    budget: &mut ValidationBudget,
) -> Result<u32, BytecodeError> {
    for _ in 0..64 {
        budget.work(1)?;
        match tables
            .types
            .entries
            .get(ty as usize)
            .map(|entry| &entry.data)
        {
            Some(TypeData::Alias { target_type_id }) => ty = *target_type_id,
            Some(_) => return Ok(ty),
            None => return Err(RejectionReason::InvalidConstructionRecord.into()),
        }
    }
    Err(RejectionReason::InvalidConstructionRecord.into())
}

pub(super) fn primitive(
    tables: &ValidationContext<'_>,
    ty: u32,
    budget: &mut ValidationBudget,
) -> Result<u16, BytecodeError> {
    let ty = resolve_alias(tables, ty, budget)?;
    match &tables.types.entries[ty as usize].data {
        TypeData::Primitive { prim_id, .. } => Ok(*prim_id),
        _ => Err(RejectionReason::InvalidConstructionRecord.into()),
    }
}

pub(super) fn validate_partial_type(
    tables: &ValidationContext<'_>,
    target: u32,
    result: u32,
    kind: u8,
    index: u32,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    if kind == 0 {
        if index != 0 || target != result {
            return Err(RejectionReason::InvalidConstructionRecord.into());
        }
        return Ok(());
    }
    let selection = crate::bytecode::PartialAccessSpec::from_raw(kind)
        .ok_or(RejectionReason::InvalidConstructionRecord)?;
    let bits = match primitive(tables, target, budget)? {
        2 => 8,
        3 => 16,
        4 => 32,
        5 => 64,
        _ => return Err(RejectionReason::InvalidConstructionRecord.into()),
    };
    let end = index
        .checked_add(1)
        .and_then(|parts| parts.checked_mul(selection.width))
        .ok_or(RejectionReason::InvalidConstructionRecord)?;
    if end > bits || primitive(tables, result, budget)? != selection.result_primitive {
        return Err(RejectionReason::InvalidConstructionRecord.into());
    }
    Ok(())
}
