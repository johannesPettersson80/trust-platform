impl<'a> BytecodeEncoder<'a> {
    fn emit_assign(
        &mut self,
        ctx: &CodegenContext,
        target: &crate::program_model::LValue,
        value: &crate::program_model::Expr,
        code: &mut Vec<u8>,
    ) -> Result<bool, BytecodeError> {
        if let Some(emitted) = self.emit_partial_assign(ctx, target, value, code)? {
            return Ok(emitted);
        }
        if let Some(emitted) = self.emit_dynamic_assign(ctx, target, value, code)? {
            return Ok(emitted);
        }
        let start_len = code.len();
        if !self.emit_expr(ctx, value, code)? {
            code.truncate(start_len);
            return Ok(false);
        }
        if self.lvalue_root_is_local_field(ctx, target) {
            if !self.emit_dynamic_ref_for_lvalue(ctx, target, code)? {
                code.truncate(start_len);
                return Ok(false);
            }
            code.push(0x13); // SWAP
            code.push(0x33); // STORE
            return Ok(true);
        }
        if let Some(reference) = self.resolve_lvalue_ref(ctx, target)? {
            let ref_idx = self.ref_index_for(&reference)?;
            code.push(0x21);
            code.extend_from_slice(&ref_idx.to_le_bytes());
            return Ok(true);
        }
        if !self.emit_dynamic_ref_for_lvalue(ctx, target, code)? {
            code.truncate(start_len);
            return Ok(false);
        }
        code.push(0x13); // SWAP
        code.push(0x33); // STORE
        Ok(true)
    }

    fn emit_partial_assign(
        &mut self,
        ctx: &CodegenContext,
        target: &crate::program_model::LValue,
        value: &crate::program_model::Expr,
        code: &mut Vec<u8>,
    ) -> Result<Option<bool>, BytecodeError> {
        let crate::program_model::LValue::Field { target, field } = target else {
            return Ok(None);
        };
        let crate::program_model::LValue::Name(name) = target.as_ref() else {
            return Ok(None);
        };
        let Some(partial) = crate::value::parse_partial_access(field.as_str()) else {
            return Ok(None);
        };

        let start_len = code.len();
        let Some(reference) = self.resolve_name_ref(ctx, name)? else {
            code.truncate(start_len);
            return Ok(Some(false));
        };
        self.emit_load_ref(&reference, code)?;
        if !self.emit_expr(ctx, value, code)? {
            code.truncate(start_len);
            return Ok(Some(false));
        }
        self.emit_partial_write(partial, code);
        self.emit_store_ref(&reference, code)?;
        Ok(Some(true))
    }

    fn emit_dynamic_assign(
        &mut self,
        ctx: &CodegenContext,
        target: &crate::program_model::LValue,
        value: &crate::program_model::Expr,
        code: &mut Vec<u8>,
    ) -> Result<Option<bool>, BytecodeError> {
        if !self.lvalue_is_self_field(ctx, target) {
            return Ok(None);
        }
        let start_len = code.len();
        if !self.emit_expr(ctx, value, code)? {
            code.truncate(start_len);
            return Ok(Some(false));
        }
        if !self.emit_dynamic_ref_for_lvalue(ctx, target, code)? {
            code.truncate(start_len);
            return Ok(Some(false));
        }
        code.push(0x13); // SWAP
        code.push(0x33); // STORE
        Ok(Some(true))
    }

    fn emit_dynamic_ref_for_lvalue(
        &mut self,
        ctx: &CodegenContext,
        target: &crate::program_model::LValue,
        code: &mut Vec<u8>,
    ) -> Result<bool, BytecodeError> {
        use crate::program_model::LValue;
        match target {
            LValue::Name(name) => self.emit_ref_for_name(ctx, name, code),
            LValue::Field { target, field } => {
                if !self.emit_dynamic_ref_for_lvalue(ctx, target, code)? {
                    return Ok(false);
                }
                let field_idx = self.strings.intern(field.clone());
                code.push(0x30);
                code.extend_from_slice(&field_idx.to_le_bytes());
                Ok(true)
            }
            LValue::Index { target, indices } => {
                if !self.emit_dynamic_ref_for_lvalue(ctx, target, code)? {
                    return Ok(false);
                }
                for index in indices {
                    if !self.emit_expr(ctx, index, code)? {
                        return Ok(false);
                    }
                    code.push(0x31);
                }
                Ok(true)
            }
            LValue::Deref(expr) => self.emit_expr(ctx, expr, code),
        }
    }

    fn lvalue_is_self_field(
        &self,
        ctx: &CodegenContext,
        target: &crate::program_model::LValue,
    ) -> bool {
        let Some(name) = target.root_name() else {
            return false;
        };
        ctx.self_field_name(name).is_some() && ctx.local_ref(name).is_none()
    }

    fn lvalue_root_is_local_field(
        &self,
        ctx: &CodegenContext,
        target: &crate::program_model::LValue,
    ) -> bool {
        let Some(name) = target.root_name() else {
            return false;
        };
        ctx.local_ref(name).is_some() && lvalue_contains_field(target)
    }

    fn emit_self_field_ref(
        &mut self,
        ctx: &CodegenContext,
        name: &SmolStr,
        code: &mut Vec<u8>,
    ) -> Result<bool, BytecodeError> {
        let Some(field_name) = ctx.self_field_name(name) else {
            return Ok(false);
        };
        code.push(0x23);
        let name_idx = self.strings.intern(field_name.clone());
        code.push(0x30);
        code.extend_from_slice(&name_idx.to_le_bytes());
        Ok(true)
    }

    fn emit_load_access(
        &mut self,
        access: &AccessKind,
        code: &mut Vec<u8>,
    ) -> Result<(), BytecodeError> {
        match access {
            AccessKind::Static(reference) => self.emit_load_ref(reference, code),
            AccessKind::SelfField(field) => {
                code.push(0x23);
                let name_idx = self.strings.intern(field.clone());
                code.push(0x30);
                code.extend_from_slice(&name_idx.to_le_bytes());
                code.push(0x32);
                Ok(())
            }
        }
    }

    fn emit_store_access(
        &mut self,
        access: &AccessKind,
        code: &mut Vec<u8>,
    ) -> Result<(), BytecodeError> {
        match access {
            AccessKind::Static(reference) => self.emit_store_ref(reference, code),
            AccessKind::SelfField(field) => {
                code.push(0x23);
                let name_idx = self.strings.intern(field.clone());
                code.push(0x30);
                code.extend_from_slice(&name_idx.to_le_bytes());
                code.push(0x13);
                code.push(0x33);
                Ok(())
            }
        }
    }

    fn emit_dynamic_load_name(
        &mut self,
        ctx: &CodegenContext,
        name: &SmolStr,
        code: &mut Vec<u8>,
    ) -> Result<bool, BytecodeError> {
        if !self.emit_self_field_ref(ctx, name, code)? {
            return Ok(false);
        }
        code.push(0x32);
        Ok(true)
    }

    fn emit_ref_for_name(
        &mut self,
        ctx: &CodegenContext,
        name: &SmolStr,
        code: &mut Vec<u8>,
    ) -> Result<bool, BytecodeError> {
        if ctx.local_ref(name).is_none() && self.emit_self_field_ref(ctx, name, code)? {
            return Ok(true);
        }
        let reference = match self.resolve_name_ref(ctx, name)? {
            Some(reference) => reference,
            None => return Ok(false),
        };
        let ref_idx = self.ref_index_for(&reference)?;
        code.push(0x22);
        code.extend_from_slice(&ref_idx.to_le_bytes());
        Ok(true)
    }

    fn emit_dynamic_load_field(
        &mut self,
        ctx: &CodegenContext,
        base: &SmolStr,
        field: &SmolStr,
        code: &mut Vec<u8>,
    ) -> Result<bool, BytecodeError> {
        if !self.emit_self_field_ref(ctx, base, code)? {
            return Ok(false);
        }
        let field_idx = self.strings.intern(field.clone());
        code.push(0x30);
        code.extend_from_slice(&field_idx.to_le_bytes());
        code.push(0x32);
        Ok(true)
    }

    fn emit_partial_read_for_name(
        &mut self,
        ctx: &CodegenContext,
        name: &SmolStr,
        access: crate::value::PartialAccess,
        code: &mut Vec<u8>,
    ) -> Result<bool, BytecodeError> {
        let Some(reference) = self.resolve_name_ref(ctx, name)? else {
            return Ok(false);
        };
        self.emit_load_ref(&reference, code)?;
        self.emit_partial_read(access, code);
        Ok(true)
    }

    fn emit_partial_read(
        &self,
        access: crate::value::PartialAccess,
        code: &mut Vec<u8>,
    ) {
        code.push(0x62); // PARTIAL_READ
        code.extend_from_slice(&Self::partial_access_operand(access).to_le_bytes());
    }

    fn emit_partial_write(
        &self,
        access: crate::value::PartialAccess,
        code: &mut Vec<u8>,
    ) {
        code.push(0x63); // PARTIAL_WRITE
        code.extend_from_slice(&Self::partial_access_operand(access).to_le_bytes());
    }

    fn partial_access_operand(access: crate::value::PartialAccess) -> u32 {
        match access {
            crate::value::PartialAccess::Bit(index) => u32::from(index),
            crate::value::PartialAccess::Byte(index) => 0x0100 | u32::from(index),
            crate::value::PartialAccess::Word(index) => 0x0200 | u32::from(index),
            crate::value::PartialAccess::DWord(index) => 0x0300 | u32::from(index),
        }
    }

    fn emit_dynamic_load_index(
        &mut self,
        ctx: &CodegenContext,
        base: &SmolStr,
        indices: &[crate::program_model::Expr],
        code: &mut Vec<u8>,
    ) -> Result<bool, BytecodeError> {
        if !self.emit_self_field_ref(ctx, base, code)? {
            return Ok(false);
        }
        for index in indices {
            if !self.emit_expr(ctx, index, code)? {
                return Ok(false);
            }
            code.push(0x31);
        }
        code.push(0x32);
        Ok(true)
    }
}

fn lvalue_contains_field(target: &crate::program_model::LValue) -> bool {
    match target {
        crate::program_model::LValue::Field { .. } => true,
        crate::program_model::LValue::Index { target, .. } => lvalue_contains_field(target),
        crate::program_model::LValue::Name(_) | crate::program_model::LValue::Deref(_) => false,
    }
}
