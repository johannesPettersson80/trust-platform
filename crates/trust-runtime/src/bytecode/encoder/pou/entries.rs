#[derive(Clone, Copy)]
pub(super) enum ParamDefaultPolicy {
    CallLocal,
    InstanceBacked,
}

impl<'a> BytecodeEncoder<'a> {
    fn call_local_implicit_default_is_portable(
        &self,
        type_id: TypeId,
        depth: u8,
    ) -> Result<bool, BytecodeError> {
        if depth > crate::bytecode::BYTECODE_MAX_CONST_NESTING {
            return Err(BytecodeError::InvalidSection(
                "parameter default type recursion overflow".into(),
            ));
        }
        let ty = self
            .runtime
            .registry()
            .get(type_id)
            .ok_or_else(|| BytecodeError::InvalidSection("unknown parameter type".into()))?;
        match ty {
            Type::Alias { target, .. } => {
                self.call_local_implicit_default_is_portable(*target, depth + 1)
            }
            Type::Interface { .. } | Type::FunctionBlock { .. } | Type::Class { .. } => Ok(false),
            _ => Ok(true),
        }
    }

    fn pou_entry_program(
        &mut self,
        program: &crate::task::ProgramDef,
        id: u32,
    ) -> Result<PouEntry, BytecodeError> {
        let name_idx = self.strings.intern(program.name.clone());
        Ok(PouEntry {
            id,
            name_idx,
            kind: PouKind::Program,
            code_offset: 0,
            code_length: 0,
            local_ref_start: 0,
            local_ref_count: 0,
            return_type_id: None,
            owner_pou_id: None,
            params: Vec::new(),
            class_meta: None,
        })
    }

    fn pou_entry_function(
        &mut self,
        func: &FunctionDef,
        id: u32,
    ) -> Result<PouEntry, BytecodeError> {
        let name_idx = self.strings.intern(func.name.clone());
        let return_type_id = Some(self.type_index(func.return_type)?);
        let params = self.encode_params(&func.params, ParamDefaultPolicy::CallLocal)?;
        Ok(PouEntry {
            id,
            name_idx,
            kind: PouKind::Function,
            code_offset: 0,
            code_length: 0,
            local_ref_start: 0,
            local_ref_count: 0,
            return_type_id,
            owner_pou_id: None,
            params,
            class_meta: None,
        })
    }

    fn pou_entry_function_block(
        &mut self,
        fb: &FunctionBlockDef,
        id: u32,
        emit_params: bool,
    ) -> Result<PouEntry, BytecodeError> {
        let name_idx = self.strings.intern(fb.name.clone());
        let params = if emit_params {
            self.encode_params(&fb.params, ParamDefaultPolicy::InstanceBacked)?
        } else {
            Vec::new()
        };
        let class_meta = Some(self.class_meta(fb, None)?);
        Ok(PouEntry {
            id,
            name_idx,
            kind: PouKind::FunctionBlock,
            code_offset: 0,
            code_length: 0,
            local_ref_start: 0,
            local_ref_count: 0,
            return_type_id: None,
            owner_pou_id: None,
            params,
            class_meta,
        })
    }

    fn pou_entry_class(&mut self, class: &ClassDef, id: u32) -> Result<PouEntry, BytecodeError> {
        let name_idx = self.strings.intern(class.name.clone());
        let class_meta = Some(self.class_meta(class, None)?);
        Ok(PouEntry {
            id,
            name_idx,
            kind: PouKind::Class,
            code_offset: 0,
            code_length: 0,
            local_ref_start: 0,
            local_ref_count: 0,
            return_type_id: None,
            owner_pou_id: None,
            params: Vec::new(),
            class_meta,
        })
    }

    fn pou_entry_method(
        &mut self,
        method: &MethodDef,
        owner_id: u32,
        id: u32,
    ) -> Result<PouEntry, BytecodeError> {
        let name_idx = self.strings.intern(method.name.clone());
        let params = self.encode_params(&method.params, ParamDefaultPolicy::CallLocal)?;
        let return_type_id = method
            .return_type
            .map(|type_id| self.type_index(type_id))
            .transpose()?;
        Ok(PouEntry {
            id,
            name_idx,
            kind: PouKind::Method,
            code_offset: 0,
            code_length: 0,
            local_ref_start: 0,
            local_ref_count: 0,
            return_type_id,
            owner_pou_id: Some(owner_id),
            params,
            class_meta: None,
        })
    }

    pub(super) fn encode_params(
        &mut self,
        params: &[Param],
        default_policy: ParamDefaultPolicy,
    ) -> Result<Vec<ParamEntry>, BytecodeError> {
        let mut out = Vec::with_capacity(params.len());
        for param in params {
            let name_idx = self.strings.intern(param.name.clone());
            let type_id = self.type_index(param.type_id)?;
            let direction = match param.direction {
                ParamDirection::In => 0,
                ParamDirection::Out => 1,
                ParamDirection::InOut => 2,
            };
            let execution_control_default =
                (param.direction == ParamDirection::In
                    && param.name.eq_ignore_ascii_case("EN"))
                    || (param.direction == ParamDirection::Out
                        && param.name.eq_ignore_ascii_case("ENO"));
            let default_const_idx = if execution_control_default {
                Some(self.const_index_for(&crate::value::Value::Bool(true))?)
            } else {
                match (default_policy, &param.default, param.direction) {
                    (
                        ParamDefaultPolicy::CallLocal,
                        Some(expr),
                        ParamDirection::In | ParamDirection::Out,
                    ) => {
                        let value = crate::harness::initializer::evaluate_initializer(
                            self.runtime.storage(),
                            self.runtime.registry(),
                            self.runtime.initializer_catalog(),
                            &self.runtime.profile(),
                            None,
                            self.runtime.stdlib(),
                            expr,
                            param.type_id,
                        )
                        .map_err(|error| {
                            BytecodeError::InvalidSection(error.to_string().into())
                        })?;
                        Some(self.const_index_for_type(&value, param.type_id)?)
                    }
                    (
                        ParamDefaultPolicy::CallLocal,
                        None,
                        ParamDirection::In | ParamDirection::Out,
                    ) if self.call_local_implicit_default_is_portable(param.type_id, 0)? => {
                        let value = crate::harness::initializer::default_value_for_type_id(
                            self.runtime.storage(),
                            self.runtime.registry(),
                            self.runtime.initializer_catalog(),
                            &self.runtime.profile(),
                            None,
                            self.runtime.stdlib(),
                            param.type_id,
                        )
                        .map_err(|error| BytecodeError::InvalidSection(error.to_string().into()))?;
                        Some(self.const_index_for_type(&value, param.type_id)?)
                    }
                    _ => None,
                }
            };
            out.push(ParamEntry {
                name_idx,
                type_id,
                direction,
                default_const_idx,
            });
        }
        Ok(out)
    }
}
