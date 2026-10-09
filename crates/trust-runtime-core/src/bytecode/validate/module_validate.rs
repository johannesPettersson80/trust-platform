use super::*;

impl BytecodeModuleView<'_> {
    /// Validate the full container with hosted default analysis limits.
    pub fn validate(&self) -> Result<(), BytecodeError> {
        self.validate_with_limits(ValidationLimits::default())
            .map(|_| ())
    }

    /// Validate with caller-selected analysis budgets, returning logical storage/work accounting.
    pub fn validate_with_limits(
        &self,
        limits: ValidationLimits,
    ) -> Result<ValidationStats, BytecodeError> {
        let mut budget = ValidationBudget::new(limits);
        validate_container_records(self, &mut budget)?;
        let strings = match self.section(SectionId::StringTable) {
            Some(SectionData::StringTable(table)) => table,
            _ => return Err(BytecodeError::MissingSection("STRING_TABLE".into())),
        };
        let debug_strings = match self.section(SectionId::DebugStringTable) {
            Some(SectionData::DebugStringTable(table)) => Some(table),
            _ => None,
        };
        let types = match self.section(SectionId::TypeTable) {
            Some(SectionData::TypeTable(table)) => table,
            _ => return Err(BytecodeError::MissingSection("TYPE_TABLE".into())),
        };
        let const_pool = match self.section(SectionId::ConstPool) {
            Some(SectionData::ConstPool(pool)) => pool,
            _ => return Err(BytecodeError::MissingSection("CONST_POOL".into())),
        };
        let ref_table = match self.section(SectionId::RefTable) {
            Some(SectionData::RefTable(table)) => table,
            _ => return Err(BytecodeError::MissingSection("REF_TABLE".into())),
        };
        let pou_index = match self.section(SectionId::PouIndex) {
            Some(SectionData::PouIndex(index)) => index,
            _ => return Err(BytecodeError::MissingSection("POU_INDEX".into())),
        };
        let pou_bodies = match self.section(SectionId::PouBodies) {
            Some(SectionData::PouBodies(bodies)) => bodies,
            _ => return Err(BytecodeError::MissingSection("POU_BODIES".into())),
        };
        let var_meta = match self.section(SectionId::VarMeta) {
            Some(SectionData::VarMeta(meta)) => Some(meta),
            _ => None,
        };
        let resource_meta = match self.section(SectionId::ResourceMeta) {
            Some(SectionData::ResourceMeta(meta)) => meta,
            _ => return Err(BytecodeError::MissingSection("RESOURCE_META".into())),
        };
        let io_map = match self.section(SectionId::IoMap) {
            Some(SectionData::IoMap(map)) => map,
            _ => return Err(BytecodeError::MissingSection("IO_MAP".into())),
        };

        budget.work(pou_index.entries.len())?;
        validate_declared_resource_limits(ref_table, pou_index)?;
        validate_type_table(strings, types, &mut budget)?;
        for entry in &types.entries {
            budget.work(1)?;
            if self.version.major == 2 {
                if let TypeData::Primitive {
                    prim_id,
                    max_length,
                } = entry.data
                {
                    if !matches!(prim_id, 1..=27 | 0x0100)
                        || (!matches!(prim_id, 24 | 25) && max_length != 0)
                    {
                        return Err(RejectionReason::InvalidConstructionRecord.into());
                    }
                }
            }
            if let TypeData::Primitive {
                prim_id: 0x0100,
                max_length,
            } = entry.data
            {
                if self.version.major != 2 || max_length != 0 {
                    return Err(BytecodeError::InvalidSection(
                        "generic native state requires STBC 2.0".into(),
                    ));
                }
            }
        }
        validate_const_pool(types, const_pool, &mut budget)?;
        validate_ref_table(strings, ref_table, &mut budget)?;
        if self.version.major == 1
            && ref_table
                .entries
                .iter()
                .any(|r| r.location == RefLocation::InitializerResult)
        {
            return Err(RejectionReason::InvalidRefLocation.into());
        }
        let mut tables = ValidationContext::new(
            strings,
            pou_index,
            types,
            const_pool,
            ref_table,
            var_meta,
            &mut budget,
        )?;
        if self.version.major == 2 {
            let aliases = match self.section(SectionId::AccessBindings) {
                Some(SectionData::AccessBindings(aliases)) => aliases,
                _ => return Err(BytecodeError::MissingSection("ACCESS_BINDINGS".into())),
            };
            tables.index_declarations(construction::sections(self)?.0, aliases, &mut budget)?;
            tables.initializers = match self.section(SectionId::Initializers) {
                Some(SectionData::Initializers(index)) => Some(index),
                _ => return Err(BytecodeError::MissingSection("INITIALIZERS".into())),
            };
        }
        let mut instruction_count = 0;
        budget.temporary(|budget| {
            validate_pou_index(&tables, pou_bodies, &mut instruction_count, budget)
        })?;
        let construction_layout = if self.version.major == 2 {
            Some(construction::sections(self)?.0)
        } else {
            None
        };
        budget.temporary(|budget| {
            validate_resource_meta(&tables, resource_meta, construction_layout, budget)
        })?;
        validate_io_map(strings, types, ref_table, io_map, &mut budget)?;
        if let Some(meta) = var_meta {
            budget.temporary(|budget| validate_var_meta(&tables, meta, budget))?;
        }
        if let Some(SectionData::RetainInit(retain)) = self.section(SectionId::RetainInit) {
            validate_retain_init(const_pool, ref_table, retain, &mut budget)?;
        }
        if let Some(SectionData::DebugMap(debug_map)) = self.section(SectionId::DebugMap) {
            if self.version.uses_extended_layout() && debug_strings.is_none() {
                return Err(BytecodeError::MissingSection("DEBUG_STRING_TABLE".into()));
            }
            let file_strings = debug_strings.unwrap_or(strings);
            validate_debug_map(file_strings, &tables, debug_map, &mut budget)?;
        }
        if self.version.major == 2 {
            construction::validate_construction(
                self,
                &tables,
                pou_bodies,
                &mut instruction_count,
                &mut budget,
            )?;
        }
        Ok(budget.stats)
    }
}

fn validate_container_records(
    module: &BytecodeModuleView<'_>,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    if !module.version.is_supported() {
        return Err(BytecodeError::UnsupportedVersion {
            major: module.version.major,
            minor: module.version.minor,
        });
    }
    if module.version.major == 2 && module.flags != crate::bytecode::HEADER_FLAG_CRC32 {
        return Err(BytecodeError::InvalidHeader(
            "STBC 2.0 requires CRC32 and no reserved flags".into(),
        ));
    }
    u16::try_from(module.sections.len())
        .map_err(|_| BytecodeError::InvalidHeader("section count overflow".into()))?;
    // A source-built module has not passed the decoder. Validate discriminants and extents here.
    let mut ids = 0u32;
    for section in module.sections {
        budget.work(1)?;
        if module.version.major == 2 && section.flags != 0 {
            return Err(RejectionReason::ReservedConstructionSectionFlags.into());
        }
        let expected = match &section.data {
            SectionData::StringTable(_) => Some(SectionId::StringTable),
            SectionData::DebugStringTable(_) => Some(SectionId::DebugStringTable),
            SectionData::TypeTable(_) => Some(SectionId::TypeTable),
            SectionData::ConstPool(_) => Some(SectionId::ConstPool),
            SectionData::RefTable(_) => Some(SectionId::RefTable),
            SectionData::PouIndex(_) => Some(SectionId::PouIndex),
            SectionData::PouBodies(bytes) => {
                if bytes.len() > crate::bytecode::BYTECODE_MAX_CONTAINER_BYTES {
                    return Err(BytecodeError::from(RejectionReason::CodePositionOverflow));
                }
                Some(SectionId::PouBodies)
            }
            SectionData::ResourceMeta(_) => Some(SectionId::ResourceMeta),
            SectionData::IoMap(_) => Some(SectionId::IoMap),
            SectionData::DebugMap(_) => Some(SectionId::DebugMap),
            SectionData::VarMeta(_) => Some(SectionId::VarMeta),
            SectionData::RetainInit(_) => Some(SectionId::RetainInit),
            SectionData::StorageLayout(_) => Some(SectionId::StorageLayout),
            SectionData::ConstructionRoots(_) => Some(SectionId::ConstructionRoots),
            SectionData::Initializers(_) => Some(SectionId::Initializers),
            SectionData::AccessBindings(_) => Some(SectionId::AccessBindings),
            SectionData::Raw(bytes) => {
                if bytes.len() > crate::bytecode::BYTECODE_MAX_CONTAINER_BYTES {
                    return Err(BytecodeError::InvalidHeader(
                        "encoded container exceeds fixed resource limit".into(),
                    ));
                }
                None
            }
        };
        let known = SectionId::from_raw(section.id)
            .filter(|id| module.version.major == 2 || id.as_raw() < 0x000D);
        if expected.map(SectionId::as_raw) != known.map(SectionId::as_raw) {
            return Err(BytecodeError::from(RejectionReason::SectionPayloadMismatch));
        }
        if expected.is_some() {
            if ids & (1u32 << section.id) != 0 {
                return Err(BytecodeError::InvalidSection(
                    format!("duplicate standardized section id 0x{:04X}", section.id).into(),
                ));
            }
            ids |= 1u32 << section.id;
        }
    }
    Ok(())
}
