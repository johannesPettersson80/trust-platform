impl<'a> BytecodeEncoder<'a> {
    fn emit_expr(
        &mut self,
        ctx: &CodegenContext,
        expr: &crate::program_model::Expr,
        code: &mut Vec<u8>,
    ) -> Result<bool, BytecodeError> {
        let start_len = code.len();
        let result = match expr {
            crate::program_model::Expr::Literal(value) => {
                if matches!(value, Value::Null) {
                    code.push(0x25); // LOAD_NULL
                    return Ok(true);
                }
                let const_idx = match self.const_index_for(value) {
                    Ok(idx) => idx,
                    Err(_) => {
                        code.truncate(start_len);
                        return Ok(false);
                    }
                };
                code.push(0x10);
                code.extend_from_slice(&const_idx.to_le_bytes());
                Ok(true)
            }
            crate::program_model::Expr::ArrayInitializer(_) => Ok(false),
            crate::program_model::Expr::StructInitializer(_) => Ok(false),
            crate::program_model::Expr::SizeOf(target) => self.emit_sizeof_expr(ctx, target, code),
            crate::program_model::Expr::Name(name) => {
                if let Some(reference) = ctx.local_ref(name) {
                    let ref_idx = self.ref_index_for(reference)?;
                    code.push(0x20);
                    code.extend_from_slice(&ref_idx.to_le_bytes());
                    return Ok(true);
                }
                if self.emit_dynamic_load_name(ctx, name, code)? {
                    return Ok(true);
                }
                let reference = match self.resolve_name_ref(ctx, name)? {
                    Some(reference) => reference,
                    None => {
                        code.truncate(start_len);
                        return Ok(false);
                    }
                };
                let ref_idx = self.ref_index_for(&reference)?;
                code.push(0x20);
                code.extend_from_slice(&ref_idx.to_le_bytes());
                Ok(true)
            }
            crate::program_model::Expr::This => {
                code.push(0x23); // LOAD_SELF
                Ok(true)
            }
            crate::program_model::Expr::Super => {
                code.push(0x24); // LOAD_SUPER
                Ok(true)
            }
            crate::program_model::Expr::Field { target, field } => {
                if let Some(qualified) = qualified_field_expr_name(expr) {
                    if let Some(reference) = self.resolve_name_ref(ctx, &qualified)? {
                        let ref_idx = self.ref_index_for(&reference)?;
                        code.push(0x20);
                        code.extend_from_slice(&ref_idx.to_le_bytes());
                        return Ok(true);
                    }
                }
                if let crate::program_model::Expr::Name(base) = target.as_ref() {
                    if let Some(access) = crate::value::parse_partial_access(field.as_str()) {
                        if self.emit_partial_read_for_name(ctx, base, access, code)? {
                            return Ok(true);
                        }
                    }
                    if ctx.local_ref(base).is_some() {
                        if !self.emit_ref_for_name(ctx, base, code)? {
                            code.truncate(start_len);
                            return Ok(false);
                        }
                        let field_idx = self.strings.intern(field.clone());
                        code.push(0x30);
                        code.extend_from_slice(&field_idx.to_le_bytes());
                        code.push(0x32);
                        return Ok(true);
                    }
                    if self.emit_dynamic_load_field(ctx, base, field, code)? {
                        return Ok(true);
                    }
                    code.truncate(start_len);
                    let reference = match self.resolve_lvalue_ref(
                        ctx,
                        &crate::program_model::LValue::Field {
                            target: Box::new(crate::program_model::LValue::Name(base.clone())),
                            field: field.clone(),
                        },
                    )? {
                        Some(reference) => reference,
                        None => {
                            code.truncate(start_len);
                            return Ok(false);
                        }
                    };
                    let ref_idx = self.ref_index_for(&reference)?;
                    code.push(0x20);
                    code.extend_from_slice(&ref_idx.to_le_bytes());
                    Ok(true)
                } else if matches!(target.as_ref(), crate::program_model::Expr::This) {
                    if self.emit_dynamic_load_name(ctx, field, code)? {
                        Ok(true)
                    } else {
                        Ok(false)
                    }
                } else if !self.emit_ref_expr(ctx, target, code)? {
                    Ok(false)
                } else {
                    if let Some(access) = crate::value::parse_partial_access(field.as_str()) {
                        code.push(0x32);
                        self.emit_partial_read(access, code);
                    } else {
                        let field_idx = self.strings.intern(field.clone());
                        code.push(0x30);
                        code.extend_from_slice(&field_idx.to_le_bytes());
                        code.push(0x32);
                    }
                    Ok(true)
                }
            }
            crate::program_model::Expr::Index { target, indices } => {
                if let crate::program_model::Expr::Name(base) = target.as_ref() {
                    if self.emit_dynamic_load_index(ctx, base, indices, code)? {
                        return Ok(true);
                    }
                    code.truncate(start_len);
                    if let Some(reference) = self.resolve_lvalue_ref(
                        ctx,
                        &crate::program_model::LValue::Index {
                            target: Box::new(crate::program_model::LValue::Name(base.clone())),
                            indices: indices.clone(),
                        },
                    )? {
                        let ref_idx = self.ref_index_for(&reference)?;
                        code.push(0x20);
                        code.extend_from_slice(&ref_idx.to_le_bytes());
                        return Ok(true);
                    }
                    code.truncate(start_len);
                    if !self.emit_ref_for_name(ctx, base, code)? {
                        code.truncate(start_len);
                        return Ok(false);
                    }
                    for index in indices {
                        if !self.emit_expr(ctx, index, code)? {
                            code.truncate(start_len);
                            return Ok(false);
                        }
                        code.push(0x31);
                    }
                    code.push(0x32);
                    Ok(true)
                } else if !self.emit_ref_expr(ctx, target, code)? {
                    Ok(false)
                } else {
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
            crate::program_model::Expr::Ref(target) => self.emit_ref_lvalue(ctx, target, code),
            crate::program_model::Expr::Deref(expr) => {
                if !self.emit_expr(ctx, expr, code)? {
                    code.truncate(start_len);
                    return Ok(false);
                }
                code.push(0x32);
                Ok(true)
            }
            crate::program_model::Expr::Unary { op, expr } => {
                use crate::program_model::UnaryOp;
                if !self.emit_expr(ctx, expr, code)? {
                    code.truncate(start_len);
                    return Ok(false);
                }
                match op {
                    UnaryOp::Neg => code.push(0x45),
                    UnaryOp::Not => code.push(0x49),
                    UnaryOp::Pos => {}
                }
                Ok(true)
            }
            crate::program_model::Expr::Binary { op, left, right } => {
                use crate::program_model::BinaryOp;
                if matches!(op, BinaryOp::AndThen | BinaryOp::OrElse) {
                    if !self.emit_expr(ctx, left, code)? {
                        code.truncate(start_len);
                        return Ok(false);
                    }
                    code.push(0x11); // DUP: preserve the left value as the branch result.
                    let short_circuit = self.emit_jump_placeholder(
                        code,
                        if matches!(op, BinaryOp::AndThen) {
                            0x04 // JUMP_IF_FALSE
                        } else {
                            0x03 // JUMP_IF_TRUE
                        },
                    );
                    if !self.emit_expr(ctx, right, code)? {
                        code.truncate(start_len);
                        return Ok(false);
                    }
                    code.push(if matches!(op, BinaryOp::AndThen) {
                        0x46 // AND
                    } else {
                        0x47 // OR
                    });
                    let end = code.len();
                    self.patch_jump(code, short_circuit, end)?;
                    return Ok(true);
                }
                let opcode = match op {
                    BinaryOp::Add => 0x40,
                    BinaryOp::Sub => 0x41,
                    BinaryOp::Mul => 0x42,
                    BinaryOp::Div => 0x43,
                    BinaryOp::Mod => 0x44,
                    BinaryOp::Pow => 0x4C,
                    BinaryOp::And => 0x46,
                    BinaryOp::Or => 0x47,
                    BinaryOp::AndThen | BinaryOp::OrElse => unreachable!(),
                    BinaryOp::Xor => 0x48,
                    BinaryOp::Eq => 0x50,
                    BinaryOp::Ne => 0x51,
                    BinaryOp::Lt => 0x52,
                    BinaryOp::Le => 0x53,
                    BinaryOp::Gt => 0x54,
                    BinaryOp::Ge => 0x55,
                };
                if !self.emit_expr(ctx, left, code)? {
                    code.truncate(start_len);
                    return Ok(false);
                }
                if !self.emit_expr(ctx, right, code)? {
                    code.truncate(start_len);
                    return Ok(false);
                }
                code.push(opcode);
                Ok(true)
            }
            crate::program_model::Expr::Call { target, args } => {
                self.emit_call_expr(ctx, target, args, code)
            }
        };
        match result {
            Ok(true) => Ok(true),
            Ok(false) => {
                code.truncate(start_len);
                Ok(false)
            }
            Err(err) => {
                code.truncate(start_len);
                Err(err)
            }
        }
    }

    fn emit_sizeof_expr(
        &mut self,
        _ctx: &CodegenContext,
        target: &crate::program_model::SizeOfTarget,
        code: &mut Vec<u8>,
    ) -> Result<bool, BytecodeError> {
        match target {
            crate::program_model::SizeOfTarget::Type(type_id) => {
                let type_idx = self.type_index(*type_id)?;
                code.push(0x60); // SIZEOF_TYPE
                code.extend_from_slice(&type_idx.to_le_bytes());
                Ok(true)
            }
        }
    }

    fn emit_ref_lvalue(
        &mut self,
        ctx: &CodegenContext,
        target: &crate::program_model::LValue,
        code: &mut Vec<u8>,
    ) -> Result<bool, BytecodeError> {
        if let Some(reference) = self.resolve_lvalue_ref(ctx, target)? {
            let ref_idx = self.ref_index_for(&reference)?;
            code.push(0x22);
            code.extend_from_slice(&ref_idx.to_le_bytes());
            return Ok(true);
        }
        self.emit_dynamic_ref_for_lvalue(ctx, target, code)
    }

    fn emit_ref_expr(
        &mut self,
        ctx: &CodegenContext,
        expr: &crate::program_model::Expr,
        code: &mut Vec<u8>,
    ) -> Result<bool, BytecodeError> {
        match expr {
            crate::program_model::Expr::Name(name) => self.emit_ref_for_name(ctx, name, code),
            crate::program_model::Expr::Field { target, field } => {
                if let Some(qualified) = qualified_field_expr_name(expr) {
                    if let Some(reference) = self.resolve_name_ref(ctx, &qualified)? {
                        let ref_idx = self.ref_index_for(&reference)?;
                        code.push(0x22);
                        code.extend_from_slice(&ref_idx.to_le_bytes());
                        return Ok(true);
                    }
                }
                if matches!(target.as_ref(), crate::program_model::Expr::This) {
                    return self.emit_self_field_ref(ctx, field, code);
                }
                if !self.emit_ref_expr(ctx, target, code)? {
                    return Ok(false);
                }
                let field_idx = self.strings.intern(field.clone());
                code.push(0x30);
                code.extend_from_slice(&field_idx.to_le_bytes());
                Ok(true)
            }
            crate::program_model::Expr::Index { target, indices } => {
                if !self.emit_ref_expr(ctx, target, code)? {
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
            crate::program_model::Expr::Ref(target) => self.emit_ref_lvalue(ctx, target, code),
            crate::program_model::Expr::Deref(expr) => self.emit_expr(ctx, expr, code),
            _ => Ok(false),
        }
    }

    fn emit_ref_builtin_call(
        &mut self,
        ctx: &CodegenContext,
        args: &[crate::program_model::CallArg],
        code: &mut Vec<u8>,
    ) -> Result<bool, BytecodeError> {
        if args.len() != 1 {
            return Err(BytecodeError::InvalidSection(
                "REF lowering expects exactly one argument".into(),
            ));
        }
        match &args[0].value {
            crate::program_model::ArgValue::Target(target) => {
                if self.emit_ref_lvalue(ctx, target, code)? {
                    Ok(true)
                } else {
                    Err(BytecodeError::InvalidSection(
                        "unsupported REF lowering argument target".into(),
                    ))
                }
            }
            crate::program_model::ArgValue::Expr(expr) => {
                if self.emit_ref_expr(ctx, expr, code)? {
                    Ok(true)
                } else {
                    Err(BytecodeError::InvalidSection(
                        "unsupported REF lowering argument expression".into(),
                    ))
                }
            }
        }
    }

    fn intern_native_call_symbol(
        &mut self,
        target_name: &SmolStr,
        arg_tokens: &[SmolStr],
    ) -> u32 {
        let mut symbol = target_name.as_str().to_owned();
        for token in arg_tokens {
            symbol.push('|');
            symbol.push_str(token.as_str());
        }
        self.strings.intern(SmolStr::new(symbol))
    }
}

fn qualified_field_expr_name(expr: &crate::program_model::Expr) -> Option<SmolStr> {
    match expr {
        crate::program_model::Expr::Name(name) => Some(name.clone()),
        crate::program_model::Expr::Field { target, field } => {
            let prefix = qualified_field_expr_name(target)?;
            Some(SmolStr::new(format!("{prefix}.{field}")))
        }
        _ => None,
    }
}
