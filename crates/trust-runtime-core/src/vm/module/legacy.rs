//! Explicit legacy construction for hosted register-tier adapters and fixtures.
use super::*;

impl VmModule {
    /// Construct legacy execution metadata for a hosted adapter or instruction fixture.
    ///
    /// This deliberately does not validate instruction bodies: register-tier verifier
    /// tests need malformed bodies. It cannot construct STBC 2.0 metadata or a
    /// `PreparedModule`, and cannot bypass source-free admission.
    pub fn legacy(
        version: crate::bytecode::BytecodeVersion,
        code: Vec<u8>,
        strings: Vec<SmolStr>,
        types: TypeTable,
        refs: Vec<VmRef>,
        consts: Vec<Value>,
    ) -> Result<Self, RuntimeError> {
        if version.major != 1 {
            return Err(invalid_bytecode("legacy adapter requires STBC 1.x"));
        }
        let native_symbol_specs = strings
            .iter()
            .map(symbols::preparse_native_symbol_spec)
            .collect();
        Ok(Self {
            version,
            code,
            strings,
            types,
            refs,
            consts,
            native_symbol_specs,
            pou_by_id: HashMap::new(),
            program_ids: HashMap::new(),
            function_ids: HashMap::new(),
            function_block_ids: HashMap::new(),
            class_ids: HashMap::new(),
            parent_pou_ids: HashMap::new(),
            interface_type_ids_by_pou: HashMap::new(),
            pou_params: HashMap::new(),
            pou_has_return_slot: HashSet::new(),
            method_table_by_owner: HashMap::new(),
            ref_types: HashMap::new(),
            debug_map: debug_map::VmDebugMap::default(),
            instruction_budget: super::super::VM_MAX_EXECUTED_INSTRUCTIONS,
        })
    }

    /// Associate one legacy POU identity with its kind, signature and code range.
    pub fn define_legacy_pou(
        &mut self,
        id: u32,
        kind: PouKind,
        entry: VmPouEntry,
        params: Vec<VmParamMeta>,
        has_return: bool,
    ) {
        self.assert_legacy();
        let key = SmolStr::new(entry.name.to_ascii_uppercase());
        match kind {
            PouKind::Program => {
                self.program_ids.insert(key, id);
            }
            PouKind::Function => {
                self.function_ids.insert(key, id);
            }
            PouKind::FunctionBlock => {
                self.function_block_ids.insert(key, id);
            }
            PouKind::Class => {
                self.class_ids.insert(key, id);
            }
            _ => {}
        }
        self.pou_by_id.insert(id, entry);
        self.pou_params.insert(id, params);
        if has_return {
            self.pou_has_return_slot.insert(id);
        } else {
            self.pou_has_return_slot.remove(&id);
        }
        symbols::resolve_native_symbol_specs(&mut self.native_symbol_specs, &self.function_ids);
    }

    /// Replace the type table of a legacy instruction fixture.
    pub fn set_legacy_types(&mut self, types: TypeTable) {
        self.assert_legacy();
        self.types = types;
    }

    /// Append a legacy reference and its optional declared type as one association.
    pub fn append_legacy_reference(&mut self, reference: VmRef, ty: Option<u32>) {
        self.assert_legacy();
        let index = self.refs.len() as u32;
        self.refs.push(reference);
        if let Some(ty) = ty {
            self.ref_types.insert(index, ty);
        }
    }

    /// Append a symbol while updating its cached native-call descriptor.
    pub fn append_legacy_string(&mut self, text: SmolStr) {
        self.assert_legacy();
        self.native_symbol_specs
            .push(symbols::preparse_native_symbol_spec(&text));
        self.strings.push(text);
        symbols::resolve_native_symbol_specs(&mut self.native_symbol_specs, &self.function_ids);
    }

    /// Replace one legacy POU body; old code remains unused so other ranges stay stable.
    /// Deliberately malformed bodies are permitted for register-verifier fixtures.
    pub fn replace_legacy_pou_body(&mut self, id: u32, code: &[u8]) -> Result<(), RuntimeError> {
        self.assert_legacy();
        let entry = self
            .pou_by_id
            .get_mut(&id)
            .ok_or_else(|| invalid_bytecode("missing legacy POU"))?;
        entry.code_start = self.code.len();
        self.code.extend_from_slice(code);
        entry.code_end = self.code.len();
        Ok(())
    }

    /// Associate a legacy instruction address with a debugger source location.
    pub fn set_legacy_source_location(
        &mut self,
        pou: u32,
        pc: u32,
        source: debug_map::VmSourceLocation,
    ) {
        self.assert_legacy();
        self.debug_map.source_by_pc.insert((pou, pc), source);
    }

    fn assert_legacy(&self) {
        assert_eq!(
            self.version.major, 1,
            "legacy adapter cannot alter source-free metadata"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_construction_cannot_forge_source_free_metadata() {
        let result = VmModule::legacy(
            crate::bytecode::BytecodeVersion::SOURCE_FREE,
            Vec::new(),
            Vec::new(),
            TypeTable::default(),
            Vec::new(),
            Vec::new(),
        );
        assert!(result.is_err());
    }
}
