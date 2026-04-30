impl<'a> BytecodeEncoder<'a> {
    pub(super) fn build_pou_index_and_bodies(
        &mut self,
    ) -> Result<(PouIndex, Vec<u8>, Vec<DebugEntry>), BytecodeError> {
        if let (Some(sources), Some(paths)) = (self.sources, self.paths) {
            if sources.len() != paths.len() {
                return Err(BytecodeError::InvalidSection(
                    "debug paths length mismatch".into(),
                ));
            }
        }

        let mut entries = Vec::new();
        let mut bodies = Vec::new();
        let mut debug_entries = Vec::new();
        let mut offset: usize = 0;

        for program in self.runtime.programs().values() {
            let id = self
                .pou_ids
                .program_id(&program.name)
                .ok_or_else(|| BytecodeError::InvalidSection("program id missing".into()))?;
            let instance_id = match self.runtime.storage().get_global(program.name.as_ref()) {
                Some(Value::Instance(id)) => Some(*id),
                _ => None,
            };
            let LocalScope {
                locals,
                local_ref_start,
                local_ref_count,
                for_temp_pairs,
            } = self.local_scope_for_body(None, &[], &program.temps, &program.body)?;
            let mut ctx = CodegenContext::new(
                instance_id,
                None,
                program.using.clone(),
                locals,
                HashMap::new(),
                HashMap::new(),
                for_temp_pairs,
            );
            let (code, local_debug) = self.emit_pou_body(&mut ctx, id, &program.body)?;
            let entry = self.pou_entry_program(program, id)?;
            append_emitted_pou(
                entry,
                local_ref_start,
                local_ref_count,
                code,
                local_debug,
                &mut EmittedPouBuffers {
                    entries: &mut entries,
                    bodies: &mut bodies,
                    debug_entries: &mut debug_entries,
                    offset: &mut offset,
                },
            )?;
        }

        for fb in self.runtime.function_blocks().values() {
            let id = self
                .pou_ids
                .function_block_id(&fb.name)
                .ok_or_else(|| BytecodeError::InvalidSection("function block id missing".into()))?;
            let LocalScope {
                locals,
                local_ref_start,
                local_ref_count,
                for_temp_pairs,
            } = self.local_scope_for_body(None, &[], &fb.temps, &fb.body)?;
            let self_fields = self.self_fields_for_owner(&fb.name)?;
            let mut ctx = CodegenContext::new(
                None,
                None,
                fb.using.clone(),
                locals,
                HashMap::new(),
                self_fields,
                for_temp_pairs,
            );
            let (code, local_debug) = self.emit_pou_body(&mut ctx, id, &fb.body)?;
            let entry = self.pou_entry_function_block(fb, id, !self.is_stdlib_fb(&fb.name))?;
            append_emitted_pou(
                entry,
                local_ref_start,
                local_ref_count,
                code,
                local_debug,
                &mut EmittedPouBuffers {
                    entries: &mut entries,
                    bodies: &mut bodies,
                    debug_entries: &mut debug_entries,
                    offset: &mut offset,
                },
            )?;
        }

        for func in self.runtime.functions().values() {
            let id = self
                .pou_ids
                .function_id(&func.name)
                .ok_or_else(|| BytecodeError::InvalidSection("function id missing".into()))?;
            let LocalScope {
                locals,
                local_ref_start,
                local_ref_count,
                for_temp_pairs,
            } = self.local_scope_for_body(Some(&func.name), &func.params, &func.locals, &func.body)?;
            let static_refs = self.static_refs_for_function(func)?;
            let mut ctx = CodegenContext::new(
                None,
                Some(func.name.clone()),
                func.using.clone(),
                locals,
                static_refs,
                HashMap::new(),
                for_temp_pairs,
            );
            let (code, local_debug) = self.emit_pou_body(&mut ctx, id, &func.body)?;
            let entry = self.pou_entry_function(func, id)?;
            append_emitted_pou(
                entry,
                local_ref_start,
                local_ref_count,
                code,
                local_debug,
                &mut EmittedPouBuffers {
                    entries: &mut entries,
                    bodies: &mut bodies,
                    debug_entries: &mut debug_entries,
                    offset: &mut offset,
                },
            )?;
        }

        for class in self.runtime.classes().values() {
            let id = self
                .pou_ids
                .class_id(&class.name)
                .ok_or_else(|| BytecodeError::InvalidSection("class id missing".into()))?;
            let mut entry = self.pou_entry_class(class, id)?;
            entry.code_offset = to_u32(offset, "POU code offset")?;
            entry.code_length = 0;
            entries.push(entry);
        }

        for (_owner, fb) in self.runtime.function_blocks().iter() {
            let owner_id = self
                .pou_ids
                .function_block_id(&fb.name)
                .ok_or_else(|| BytecodeError::InvalidSection("method owner missing".into()))?;
            for method in &fb.methods {
                self.emit_method_entry(
                    &fb.name,
                    method,
                    owner_id,
                    &mut EmittedPouBuffers {
                        entries: &mut entries,
                        bodies: &mut bodies,
                        debug_entries: &mut debug_entries,
                        offset: &mut offset,
                    },
                )?;
            }
        }

        for (_owner, class) in self.runtime.classes().iter() {
            let owner_id = self
                .pou_ids
                .class_id(&class.name)
                .ok_or_else(|| BytecodeError::InvalidSection("method owner missing".into()))?;
            for method in &class.methods {
                self.emit_method_entry(
                    &class.name,
                    method,
                    owner_id,
                    &mut EmittedPouBuffers {
                        entries: &mut entries,
                        bodies: &mut bodies,
                        debug_entries: &mut debug_entries,
                        offset: &mut offset,
                    },
                )?;
            }
        }

        Ok((PouIndex { entries }, bodies, debug_entries))
    }

    fn emit_method_entry(
        &mut self,
        owner: &SmolStr,
        method: &MethodDef,
        owner_id: u32,
        emitted: &mut EmittedPouBuffers<'_>,
    ) -> Result<(), BytecodeError> {
        let id = self
            .pou_ids
            .method_id(owner, &method.name)
            .ok_or_else(|| BytecodeError::InvalidSection("method id missing".into()))?;
        let LocalScope {
            locals,
            local_ref_start,
            local_ref_count,
            for_temp_pairs,
        } = self.local_scope_for_body(
            method.return_type.as_ref().map(|_| &method.name),
            &method.params,
            &method.locals,
            &method.body,
        )?;
        let mut self_fields = self.self_fields_for_owner(owner)?;
        for local in &method.static_locals {
            let owner = crate::program_model::method_static_storage_owner(owner, &method.name);
            let hidden = crate::program_model::static_storage_name(&owner, &local.name);
            let key = super::util::normalize_name(&local.name);
            self_fields.insert(key, hidden);
        }
        let mut ctx = CodegenContext::new(
            None,
            method.return_type.as_ref().map(|_| method.name.clone()),
            method.using.clone(),
            locals,
            HashMap::new(),
            self_fields,
            for_temp_pairs,
        );
        let (code, local_debug) = self.emit_pou_body(&mut ctx, id, &method.body)?;
        let entry = self.pou_entry_method(method, owner_id, id)?;
        append_emitted_pou(
            entry,
            local_ref_start,
            local_ref_count,
            code,
            local_debug,
            emitted,
        )
    }

    fn static_refs_for_function(
        &self,
        function: &FunctionDef,
    ) -> Result<HashMap<SmolStr, crate::value::ValueRef>, BytecodeError> {
        let mut refs = HashMap::new();
        for local in &function.static_locals {
            let hidden = crate::program_model::static_storage_name(&function.name, &local.name);
            let reference = self
                .runtime
                .storage()
                .ref_for_global(hidden.as_ref())
                .ok_or_else(|| {
                    BytecodeError::InvalidSection(
                        format!("missing function VAR_STAT backing storage for '{}'", local.name)
                            .into(),
                    )
                })?;
            refs.insert(super::util::normalize_name(&local.name), reference);
        }
        Ok(refs)
    }
}

struct EmittedPouBuffers<'a> {
    entries: &'a mut Vec<PouEntry>,
    bodies: &'a mut Vec<u8>,
    debug_entries: &'a mut Vec<DebugEntry>,
    offset: &'a mut usize,
}

fn append_emitted_pou(
    mut entry: PouEntry,
    local_ref_start: u32,
    local_ref_count: u32,
    code: Vec<u8>,
    mut local_debug: Vec<DebugEntry>,
    emitted: &mut EmittedPouBuffers<'_>,
) -> Result<(), BytecodeError> {
    let code_offset = to_u32(*emitted.offset, "POU code offset")?;
    let code_length = to_u32(code.len(), "POU code length")?;
    update_debug_offsets(&mut local_debug, code_offset)?;

    entry.code_offset = code_offset;
    entry.code_length = code_length;
    entry.local_ref_start = local_ref_start;
    entry.local_ref_count = local_ref_count;
    emitted.entries.push(entry);

    emitted.debug_entries.extend(local_debug);
    emitted.bodies.extend_from_slice(&code);
    *emitted.offset = emitted
        .offset
        .checked_add(code.len())
        .ok_or_else(|| BytecodeError::InvalidSection("POU body overflow".into()))?;
    Ok(())
}

fn update_debug_offsets(entries: &mut [DebugEntry], code_offset: u32) -> Result<(), BytecodeError> {
    for entry in entries {
        entry.code_offset = entry
            .code_offset
            .checked_add(code_offset)
            .ok_or_else(|| BytecodeError::InvalidSection("debug code offset overflow".into()))?;
    }
    Ok(())
}
