impl BytecodeModule {
    pub fn validate(&self) -> Result<(), BytecodeError> {
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

        validate_string_table(strings)?;
        if let Some(table) = debug_strings {
            validate_string_table(table)?;
        }
        validate_type_table(strings, types)?;
        validate_const_pool(strings, types, const_pool)?;
        validate_ref_table(strings, ref_table)?;
        validate_pou_index(
            strings, types, const_pool, ref_table, var_meta, pou_index, pou_bodies,
        )?;
        validate_resource_meta(strings, ref_table, pou_index, resource_meta)?;
        validate_io_map(strings, types, ref_table, io_map)?;
        if let Some(meta) = var_meta {
            validate_var_meta(strings, types, const_pool, ref_table, pou_index, meta)?;
        }
        if let Some(SectionData::RetainInit(retain)) = self.section(SectionId::RetainInit) {
            validate_retain_init(const_pool, ref_table, retain)?;
        }
        if let Some(SectionData::DebugMap(debug_map)) = self.section(SectionId::DebugMap) {
            if self.version.minor >= 1 && debug_strings.is_none() {
                return Err(BytecodeError::MissingSection("DEBUG_STRING_TABLE".into()));
            }
            let file_strings = debug_strings.unwrap_or(strings);
            validate_debug_map(file_strings, pou_index, debug_map)?;
        }
        Ok(())
    }
}
