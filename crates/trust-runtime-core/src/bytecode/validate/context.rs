use super::*;

/// Module-wide borrowed tables and indexes. Equal keys retain original wire order.
pub(super) struct ValidationContext<'a> {
    pub(super) strings: &'a StringTable,
    pub(super) index: &'a PouIndex,
    pub(super) types: &'a TypeTable,
    pub(super) const_pool: &'a ConstPool,
    pub(super) ref_table: &'a RefTable,
    pub(super) var_meta: Option<&'a VarMeta>,
    pub(super) initializers: Option<&'a crate::bytecode::InitializerIndex>,
    declaration_names: Vec<(Option<u32>, &'a str, u8, u32)>,
    pou_ids: Vec<(u32, usize)>,
    pou_names: Vec<(u8, &'a str, usize)>,
    var_names: Vec<(u8, &'a str, usize)>,
    var_refs: Vec<(u32, usize)>,
    pub(super) local_ranges: Vec<(u32, u32, usize)>,
}

impl<'a> ValidationContext<'a> {
    pub(super) fn index_declarations(
        &mut self,
        layout: &'a crate::bytecode::StorageLayout,
        aliases: &'a crate::bytecode::AccessBindings,
        budget: &mut ValidationBudget,
    ) -> Result<(), BytecodeError> {
        use crate::bytecode::{StorageOwner, StorageRole};
        for declaration in &layout.entries {
            budget.work(1)?;
            if matches!(
                declaration.role,
                StorageRole::External
                    | StorageRole::Scratch
                    | StorageRole::NativeState
                    | StorageRole::EdgePhase
            ) {
                continue;
            }
            let Some(ty) = declaration.type_id else {
                continue;
            };
            ensure_string_index(self.strings, declaration.name_idx)?;
            ensure_type_index(self.types, ty)?;
            let static_name = declaration.role == StorageRole::Static;
            let name = if static_name {
                declaration
                    .source_name_idx
                    .ok_or(RejectionReason::InvalidConstructionRecord)?
            } else {
                declaration.name_idx
            };
            ensure_string_index(self.strings, name)?;
            let owner = if declaration.owner == StorageOwner::Global && !static_name {
                None
            } else {
                declaration.owner_pou_id
            };
            let precedence = if declaration.owner == StorageOwner::Frame {
                0
            } else if static_name {
                1
            } else {
                2
            };
            budget.push(
                &mut self.declaration_names,
                (
                    owner,
                    self.strings.entries[name as usize].as_str(),
                    precedence,
                    ty,
                ),
            )?;
        }
        for alias in &aliases.entries {
            budget.work(1)?;
            ensure_string_index(self.strings, alias.name_idx)?;
            ensure_type_index(self.types, alias.type_id)?;
            budget.push(
                &mut self.declaration_names,
                (
                    None,
                    self.strings.entries[alias.name_idx as usize].as_str(),
                    3,
                    alias.type_id,
                ),
            )?;
        }
        budget.sort_by(&mut self.declaration_names, &mut |a, b, budget| {
            Ok(a.0
                .cmp(&b.0)
                .then(budget.compare_names(a.1, b.1)?)
                .then(a.2.cmp(&b.2)))
        })
    }

    pub(super) fn declared_call_type(
        &self,
        pou: &PouEntry,
        name: &str,
        budget: &mut ValidationBudget,
    ) -> Result<Option<u32>, BytecodeError> {
        let mut owner = Some(pou.id);
        for _ in 0..=crate::bytecode::BYTECODE_MAX_CONST_NESTING {
            let at = budget.lower_bound(&self.declaration_names, |entry, budget| {
                Ok(entry
                    .0
                    .cmp(&owner)
                    .then(budget.compare_names(entry.1, name)?)
                    .is_lt())
            })?;
            if let Some(entry) = self.declaration_names.get(at) {
                if entry.0 == owner && budget.compare_names(entry.1, name)?.is_eq() {
                    return Ok(Some(entry.3));
                }
            }
            let Some(id) = owner else {
                return Ok(None);
            };
            let current = self
                .pou(id, budget)?
                .ok_or(RejectionReason::InvalidConstructionRecord)?;
            owner = if current.kind == PouKind::Method {
                current.owner_pou_id
            } else {
                current
                    .class_meta
                    .as_ref()
                    .and_then(|meta| meta.parent_pou_id)
            };
        }
        Err(RejectionReason::TypeReferenceRecursionOverflow.into())
    }

    pub(super) fn function_block_type(
        &self,
        mut ty: u32,
        budget: &mut ValidationBudget,
    ) -> Result<Option<u32>, BytecodeError> {
        for _ in 0..=crate::bytecode::BYTECODE_MAX_CONST_NESTING {
            budget.work(1)?;
            let entry = self
                .types
                .entries
                .get(ty as usize)
                .ok_or(BytecodeError::InvalidIndex {
                    kind: "type".into(),
                    index: ty,
                })?;
            match entry.data {
                TypeData::Alias { target_type_id } => ty = target_type_id,
                TypeData::Pou { pou_id } if entry.kind == TypeKind::FunctionBlock => {
                    return Ok(Some(pou_id))
                }
                _ => return Ok(None),
            }
        }
        Err(RejectionReason::TypeReferenceRecursionOverflow.into())
    }

    pub(super) fn new(
        strings: &'a StringTable,
        index: &'a PouIndex,
        types: &'a TypeTable,
        const_pool: &'a ConstPool,
        ref_table: &'a RefTable,
        var_meta: Option<&'a VarMeta>,
        budget: &mut ValidationBudget,
    ) -> Result<Self, BytecodeError> {
        let mut tables = Self {
            strings,
            index,
            types,
            const_pool,
            ref_table,
            var_meta,
            initializers: None,
            declaration_names: Vec::new(),
            pou_ids: Vec::new(),
            pou_names: Vec::new(),
            var_names: Vec::new(),
            var_refs: Vec::new(),
            local_ranges: Vec::new(),
        };
        for (position, pou) in index.entries.iter().enumerate() {
            budget.work(1)?;
            budget.push(&mut tables.pou_ids, (pou.id, position))?;
            if let Some(name) = strings.entries.get(pou.name_idx as usize) {
                budget.push(
                    &mut tables.pou_names,
                    (pou.kind as u8, name.as_str(), position),
                )?;
            }
            let end = pou
                .local_ref_start
                .checked_add(pou.local_ref_count)
                .ok_or(RejectionReason::PouLocalRefRangeOverflow)?;
            if end as usize > ref_table.entries.len() {
                return Err(RejectionReason::PouLocalRefRangeOutOfBounds.into());
            }
            if pou.local_ref_count != 0 {
                budget.push(
                    &mut tables.local_ranges,
                    (pou.local_ref_start, end, position),
                )?;
            }
        }
        if let Some(meta) = var_meta {
            for (position, entry) in meta.entries.iter().enumerate() {
                budget.work(1)?;
                budget.push(&mut tables.var_refs, (entry.ref_idx, position))?;
                if let Some(name) = strings.entries.get(entry.name_idx as usize) {
                    budget.push(&mut tables.var_names, (0, name.as_str(), position))?;
                }
            }
        }
        budget.sort_by(&mut tables.pou_ids, &mut |a, b, _| Ok(a.cmp(b)))?;
        budget.sort_by(&mut tables.var_refs, &mut |a, b, _| Ok(a.cmp(b)))?;
        budget.sort_by(&mut tables.local_ranges, &mut |a, b, _| Ok(a.cmp(b)))?;
        for names in [&mut tables.pou_names, &mut tables.var_names] {
            budget.sort_by(names, &mut |a, b, budget| {
                let kind = a.0.cmp(&b.0);
                if !kind.is_eq() {
                    return Ok(kind);
                }
                Ok(budget.compare_names(a.1, b.1)?.then(a.2.cmp(&b.2)))
            })?;
        }
        Ok(tables)
    }

    pub(super) fn pou_position(
        &self,
        id: u32,
        budget: &mut ValidationBudget,
    ) -> Result<Option<usize>, BytecodeError> {
        let at = budget.lower_bound(&self.pou_ids, |entry, _| Ok(entry.0 < id))?;
        Ok(self
            .pou_ids
            .get(at)
            .filter(|entry| entry.0 == id)
            .map(|entry| entry.1))
    }

    pub(super) fn pou(
        &self,
        id: u32,
        budget: &mut ValidationBudget,
    ) -> Result<Option<&'a PouEntry>, BytecodeError> {
        Ok(self
            .pou_position(id, budget)?
            .map(|at| &self.index.entries[at]))
    }

    fn named(
        names: &[(u8, &str, usize)],
        kind: u8,
        name: &str,
        budget: &mut ValidationBudget,
    ) -> Result<Option<usize>, BytecodeError> {
        let at = budget.lower_bound(names, |entry, budget| {
            Ok(entry.0 < kind || (entry.0 == kind && budget.compare_names(entry.1, name)?.is_lt()))
        })?;
        if let Some(entry) = names.get(at) {
            if entry.0 == kind && budget.compare_names(entry.1, name)?.is_eq() {
                return Ok(Some(entry.2));
            }
        }
        Ok(None)
    }

    pub(super) fn named_pou(
        &self,
        kind: PouKind,
        name: &str,
        budget: &mut ValidationBudget,
    ) -> Result<Option<u32>, BytecodeError> {
        Ok(Self::named(&self.pou_names, kind as u8, name, budget)?
            .map(|at| self.index.entries[at].id))
    }

    pub(super) fn named_var(
        &self,
        name: &str,
        budget: &mut ValidationBudget,
    ) -> Result<Option<usize>, BytecodeError> {
        Self::named(&self.var_names, 0, name, budget)
    }

    pub(super) fn first_var_ref(
        &self,
        id: u32,
        budget: &mut ValidationBudget,
    ) -> Result<Option<usize>, BytecodeError> {
        let at = budget.lower_bound(&self.var_refs, |entry, _| Ok(entry.0 < id))?;
        Ok(self
            .var_refs
            .get(at)
            .filter(|entry| entry.0 == id)
            .map(|entry| entry.1))
    }

    pub(super) fn ref_type(
        &self,
        id: u32,
        budget: &mut ValidationBudget,
    ) -> Result<Option<u32>, BytecodeError> {
        let at = budget.lower_bound(&self.var_refs, |entry, _| Ok(entry.0 <= id))?;
        Ok(at
            .checked_sub(1)
            .and_then(|at| self.var_refs.get(at))
            .filter(|entry| entry.0 == id)
            .and_then(|entry| self.var_meta.map(|meta| meta.entries[entry.1].type_id)))
    }

    // Called after partition validation has proved disjoint intervals.
    pub(super) fn local_owner(
        &self,
        id: u32,
        budget: &mut ValidationBudget,
    ) -> Result<Option<&'a PouEntry>, BytecodeError> {
        let at = budget.lower_bound(&self.local_ranges, |entry, _| Ok(entry.0 <= id))?;
        Ok(at
            .checked_sub(1)
            .and_then(|at| self.local_ranges.get(at))
            .filter(|entry| id < entry.1)
            .map(|entry| &self.index.entries[entry.2]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::VarMetaEntry;
    use alloc::vec;

    #[test]
    fn name_indexes_preserve_first_match_and_ref_types_preserve_last_match() {
        let strings = StringTable {
            entries: vec![
                "Main".into(),
                "motor".into(),
                "MAIN.MOTOR".into(),
                "MOTOR".into(),
            ],
        };
        let index = PouIndex {
            entries: vec![PouEntry {
                id: 7,
                name_idx: 0,
                kind: PouKind::Program,
                code_offset: 0,
                code_length: 0,
                local_ref_start: 0,
                local_ref_count: 0,
                return_type_id: None,
                owner_pou_id: None,
                params: vec![],
                class_meta: None,
            }],
        };
        let types = TypeTable::default();
        let pool = ConstPool::default();
        let refs = RefTable::default();
        let meta = VarMeta {
            entries: (1..=3)
                .map(|name_idx| VarMetaEntry {
                    name_idx,
                    type_id: name_idx,
                    ref_idx: 9,
                    retain: 0,
                    init_const_idx: None,
                })
                .collect(),
        };
        let mut budget = ValidationBudget::new(ValidationLimits::default());
        let tables = ValidationContext::new(
            &strings,
            &index,
            &types,
            &pool,
            &refs,
            Some(&meta),
            &mut budget,
        )
        .unwrap();
        assert_eq!(
            tables
                .named_pou(PouKind::Program, "main", &mut budget)
                .unwrap(),
            Some(7)
        );
        assert_eq!(tables.named_var("Motor", &mut budget).unwrap(), Some(0));
        assert_eq!(
            tables.named_var("main.motor", &mut budget).unwrap(),
            Some(1)
        );
        assert_eq!(tables.first_var_ref(9, &mut budget).unwrap(), Some(0));
        assert_eq!(tables.ref_type(9, &mut budget).unwrap(), Some(3));
        assert_eq!(tables.ref_type(8, &mut budget).unwrap(), None);
    }
}
