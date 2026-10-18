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
            _ => {
                return Err(BytecodeError::MissingSection(
                    smol_str::SmolStr::new_static("STRING_TABLE"),
                ))
            }
        };
        let debug_strings = match self.section(SectionId::DebugStringTable) {
            Some(SectionData::DebugStringTable(table)) => Some(table),
            _ => None,
        };
        let types = match self.section(SectionId::TypeTable) {
            Some(SectionData::TypeTable(table)) => table,
            _ => {
                return Err(BytecodeError::MissingSection(
                    smol_str::SmolStr::new_static("TYPE_TABLE"),
                ))
            }
        };
        let const_pool = match self.section(SectionId::ConstPool) {
            Some(SectionData::ConstPool(pool)) => pool,
            _ => {
                return Err(BytecodeError::MissingSection(
                    smol_str::SmolStr::new_static("CONST_POOL"),
                ))
            }
        };
        let ref_table = match self.section(SectionId::RefTable) {
            Some(SectionData::RefTable(table)) => table,
            _ => {
                return Err(BytecodeError::MissingSection(
                    smol_str::SmolStr::new_static("REF_TABLE"),
                ))
            }
        };
        let pou_index = match self.section(SectionId::PouIndex) {
            Some(SectionData::PouIndex(index)) => index,
            _ => {
                return Err(BytecodeError::MissingSection(
                    smol_str::SmolStr::new_static("POU_INDEX"),
                ))
            }
        };
        let pou_bodies = match self.section(SectionId::PouBodies) {
            Some(SectionData::PouBodies(bodies)) => bodies,
            _ => {
                return Err(BytecodeError::MissingSection(
                    smol_str::SmolStr::new_static("POU_BODIES"),
                ))
            }
        };
        let var_meta = match self.section(SectionId::VarMeta) {
            Some(SectionData::VarMeta(meta)) => Some(meta),
            _ => None,
        };
        let resource_meta = match self.section(SectionId::ResourceMeta) {
            Some(SectionData::ResourceMeta(meta)) => meta,
            _ => {
                return Err(BytecodeError::MissingSection(
                    smol_str::SmolStr::new_static("RESOURCE_META"),
                ))
            }
        };
        let io_map = match self.section(SectionId::IoMap) {
            Some(SectionData::IoMap(map)) => map,
            _ => {
                return Err(BytecodeError::MissingSection(
                    smol_str::SmolStr::new_static("IO_MAP"),
                ))
            }
        };

        budget.work(pou_index.entries.len())?;
        validate_declared_resource_limits(ref_table, pou_index)?;
        validate_type_table(
            strings,
            types,
            self.version,
            self.storage_abi()?,
            &mut budget,
        )?;
        for entry in &types.entries {
            budget.work(1)?;
            if self.version.is_source_free()
                && let TypeData::Primitive {
                    prim_id,
                    max_length,
                } = entry.data
                && (!matches!(prim_id, 1..=27 | 0x0100)
                    || (!matches!(prim_id, 24 | 25) && max_length != 0))
            {
                return Err(RejectionReason::InvalidConstructionRecord.into());
            }
            if let TypeData::Primitive {
                prim_id: 0x0100,
                max_length,
            } = entry.data
                && (!self.version.is_source_free() || max_length != 0)
            {
                return Err(BytecodeError::section_static(
                    "generic native state requires source-free STBC",
                ));
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
        if self.version.is_source_free() {
            let aliases = match self.section(SectionId::AccessBindings) {
                Some(SectionData::AccessBindings(aliases)) => aliases,
                _ => {
                    return Err(BytecodeError::MissingSection(
                        smol_str::SmolStr::new_static("ACCESS_BINDINGS"),
                    ))
                }
            };
            tables.index_declarations(construction::sections(self)?.0, aliases, &mut budget)?;
            tables.initializers = match self.section(SectionId::Initializers) {
                Some(SectionData::Initializers(index)) => Some(index),
                _ => {
                    return Err(BytecodeError::MissingSection(
                        smol_str::SmolStr::new_static("INITIALIZERS"),
                    ))
                }
            };
        }
        let mut instruction_count = 0;
        budget.temporary(|budget| {
            validate_pou_index(&tables, pou_bodies, &mut instruction_count, budget)
        })?;
        let construction_layout = if self.version.is_source_free() {
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
                return Err(BytecodeError::MissingSection(
                    smol_str::SmolStr::new_static("DEBUG_STRING_TABLE"),
                ));
            }
            let file_strings = debug_strings.unwrap_or(strings);
            validate_debug_map(file_strings, &tables, debug_map, &mut budget)?;
        }
        budget.temporary(|budget| {
            validate_line_table_section(self, &tables, pou_bodies, debug_strings, budget)
        })?;
        if self.version.is_source_free() {
            construction::validate_construction(
                self,
                &tables,
                pou_bodies,
                &mut instruction_count,
                &mut budget,
            )?;
        }
        // The static call depth sizes construction's per-call reservations (MEM-09). It is
        // computed last so it sees only a container that already passed every other check.
        budget.stats.call_depth =
            module_call_depth(&tables, construction_layout, pou_bodies, &mut budget)?;
        Ok(budget.stats)
    }
}

/// Check the LINE_TABLE section when the container carries one (spec 12 §6.13).
///
/// A line table resolves its labels in DEBUG_STRING_TABLE, so a container carrying
/// one requires that section; its statements are checked against POU_BODIES and
/// against the DEBUG_MAP statement entries.
fn validate_line_table_section(
    module: &BytecodeModuleView<'_>,
    tables: &ValidationContext<'_>,
    bodies: &[u8],
    debug_strings: Option<&StringTable>,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    let Some(line_table) = module.line_table() else {
        return Ok(());
    };
    let file_strings = debug_strings.ok_or_else(|| {
        BytecodeError::MissingSection(smol_str::SmolStr::new_static("DEBUG_STRING_TABLE"))
    })?;
    let debug_map = match module.section(SectionId::DebugMap) {
        Some(SectionData::DebugMap(map)) => Some(map),
        _ => None,
    };
    validate_line_table(tables, bodies, line_table, file_strings, debug_map, budget)
}

fn validate_container_records(
    module: &BytecodeModuleView<'_>,
    budget: &mut ValidationBudget,
) -> Result<(), BytecodeError> {
    module.storage_abi()?;
    u16::try_from(module.sections.len()).map_err(|_| {
        BytecodeError::InvalidHeader(smol_str::SmolStr::new_static("section count overflow"))
    })?;
    // A source-built module has not passed the decoder. Validate discriminants and extents here.
    let mut ids = 0u32;
    for section in module.sections {
        budget.work(1)?;
        if module.version.is_source_free() && section.flags != 0 {
            return Err(RejectionReason::ReservedConstructionSectionFlags.into());
        }
        let expected = match &section.data {
            SectionData::StringTable(_) => Some(SectionId::StringTable.as_raw()),
            SectionData::DebugStringTable(_) => Some(SectionId::DebugStringTable.as_raw()),
            SectionData::TypeTable(_) => Some(SectionId::TypeTable.as_raw()),
            SectionData::ConstPool(_) => Some(SectionId::ConstPool.as_raw()),
            SectionData::RefTable(_) => Some(SectionId::RefTable.as_raw()),
            SectionData::PouIndex(_) => Some(SectionId::PouIndex.as_raw()),
            SectionData::PouBodies(bytes) => {
                if bytes.len() > crate::bytecode::BYTECODE_MAX_CONTAINER_BYTES {
                    return Err(BytecodeError::from(RejectionReason::CodePositionOverflow));
                }
                Some(SectionId::PouBodies.as_raw())
            }
            SectionData::ResourceMeta(_) => Some(SectionId::ResourceMeta.as_raw()),
            SectionData::IoMap(_) => Some(SectionId::IoMap.as_raw()),
            SectionData::DebugMap(_) => Some(SectionId::DebugMap.as_raw()),
            SectionData::VarMeta(_) => Some(SectionId::VarMeta.as_raw()),
            SectionData::RetainInit(_) => Some(SectionId::RetainInit.as_raw()),
            SectionData::LineTable(_) => Some(LINE_TABLE_SECTION_ID),
            SectionData::StorageLayout(_) => Some(SectionId::StorageLayout.as_raw()),
            SectionData::ConstructionRoots(_) => Some(SectionId::ConstructionRoots.as_raw()),
            SectionData::Initializers(_) => Some(SectionId::Initializers.as_raw()),
            SectionData::AccessBindings(_) => Some(SectionId::AccessBindings.as_raw()),
            SectionData::Raw(bytes) => {
                if bytes.len() > crate::bytecode::BYTECODE_MAX_CONTAINER_BYTES {
                    return Err(BytecodeError::InvalidHeader(smol_str::SmolStr::new_static(
                        "encoded container exceeds fixed resource limit",
                    )));
                }
                None
            }
        };
        // §5 standardizes 0x0001-0x000C in a 1.x container; §11.5 standardizes the
        // construction sections 0x000D-0x0010 and the optional LINE_TABLE 0x0011 for
        // the source-free versions. A section's payload must match its id.
        let known = if module.version.is_source_free() && section.id == LINE_TABLE_SECTION_ID {
            Some(LINE_TABLE_SECTION_ID)
        } else {
            SectionId::from_raw(section.id)
                .filter(|id| {
                    module.version.is_source_free()
                        || id.as_raw() < SectionId::StorageLayout.as_raw()
                })
                .map(SectionId::as_raw)
        };
        if expected != known {
            return Err(BytecodeError::from(RejectionReason::SectionPayloadMismatch));
        }
        if expected.is_some() {
            if ids & (1u32 << section.id) != 0 {
                return Err(BytecodeError::section_diagnostic(
                    SectionDiagnostic::DuplicateSection(section.id),
                ));
            }
            ids |= 1u32 << section.id;
        }
    }
    Ok(())
}
