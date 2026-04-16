impl<'a> BytecodeEncoder<'a> {
    fn emit_while_stmt(
        &mut self,
        ctx: &mut CodegenContext,
        pou_id: u32,
        condition: &crate::program_model::Expr,
        body: &[crate::program_model::Stmt],
        code: &mut Vec<u8>,
        debug_entries: &mut Vec<DebugEntry>,
    ) -> Result<bool, BytecodeError> {
        let code_start = code.len();
        let debug_start = debug_entries.len();
        if !expr_supported(condition) {
            code.truncate(code_start);
            debug_entries.truncate(debug_start);
            return Ok(false);
        }
        let loop_start = code.len();
        if !self.emit_expr(ctx, condition, code)? {
            code.truncate(code_start);
            debug_entries.truncate(debug_start);
            return Ok(false);
        }
        let jump_false = self.emit_jump_placeholder(code, 0x04);
        ctx.push_loop();
        if let Err(err) = self.emit_block(ctx, pou_id, body, code, debug_entries) {
            let _ = ctx.pop_loop();
            code.truncate(code_start);
            debug_entries.truncate(debug_start);
            return Err(err);
        }
        let loop_patches = ctx.pop_loop().unwrap_or_default();
        for jump in loop_patches.continue_jumps {
            self.patch_jump(code, jump, loop_start)?;
        }
        let jump_back = self.emit_jump_placeholder(code, 0x02);
        self.patch_jump(code, jump_back, loop_start)?;
        let loop_end = code.len();
        self.patch_jump(code, jump_false, loop_end)?;
        for jump in loop_patches.exit_jumps {
            self.patch_jump(code, jump, loop_end)?;
        }
        Ok(true)
    }

    fn emit_repeat_stmt(
        &mut self,
        ctx: &mut CodegenContext,
        pou_id: u32,
        body: &[crate::program_model::Stmt],
        until: &crate::program_model::Expr,
        code: &mut Vec<u8>,
        debug_entries: &mut Vec<DebugEntry>,
    ) -> Result<bool, BytecodeError> {
        let code_start = code.len();
        let debug_start = debug_entries.len();
        if !expr_supported(until) {
            code.truncate(code_start);
            debug_entries.truncate(debug_start);
            return Ok(false);
        }
        let loop_start = code.len();
        ctx.push_loop();
        if let Err(err) = self.emit_block(ctx, pou_id, body, code, debug_entries) {
            let _ = ctx.pop_loop();
            code.truncate(code_start);
            debug_entries.truncate(debug_start);
            return Err(err);
        }
        let loop_patches = ctx.pop_loop().unwrap_or_default();
        let continue_target = code.len();
        for jump in loop_patches.continue_jumps {
            self.patch_jump(code, jump, continue_target)?;
        }
        if !self.emit_expr(ctx, until, code)? {
            code.truncate(code_start);
            debug_entries.truncate(debug_start);
            return Ok(false);
        }
        let jump_false = self.emit_jump_placeholder(code, 0x04);
        self.patch_jump(code, jump_false, loop_start)?;
        let loop_end = code.len();
        for jump in loop_patches.exit_jumps {
            self.patch_jump(code, jump, loop_end)?;
        }
        Ok(true)
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_for_stmt(
        &mut self,
        ctx: &mut CodegenContext,
        pou_id: u32,
        control: &SmolStr,
        start: &crate::program_model::Expr,
        end: &crate::program_model::Expr,
        step: &crate::program_model::Expr,
        body: &[crate::program_model::Stmt],
        code: &mut Vec<u8>,
        debug_entries: &mut Vec<DebugEntry>,
    ) -> Result<bool, BytecodeError> {
        let code_start = code.len();
        let debug_start = debug_entries.len();
        if !expr_supported(start) || !expr_supported(end) || !expr_supported(step) {
            code.truncate(code_start);
            debug_entries.truncate(debug_start);
            return Ok(false);
        }
        let Some((end_temp, step_temp)) = ctx.next_for_temp_pair() else {
            code.truncate(code_start);
            debug_entries.truncate(debug_start);
            return Ok(false);
        };
        let control_access = match self
            .resolve_lvalue_ref(ctx, &crate::program_model::LValue::Name(control.clone()))?
        {
            Some(reference) => AccessKind::Static(reference),
            None => match ctx.self_field_name(control) {
                Some(field) => AccessKind::SelfField(field.clone()),
                None => return Ok(false),
            },
        };
        let end_ref = match self.resolve_name_ref(ctx, &end_temp)? {
            Some(reference) => reference,
            None => return Ok(false),
        };
        let step_ref = match self.resolve_name_ref(ctx, &step_temp)? {
            Some(reference) => reference,
            None => return Ok(false),
        };

        if !self.emit_expr(ctx, start, code)? {
            code.truncate(code_start);
            debug_entries.truncate(debug_start);
            return Ok(false);
        }
        self.emit_store_access(&control_access, code)?;
        if !self.emit_expr(ctx, end, code)? {
            code.truncate(code_start);
            debug_entries.truncate(debug_start);
            return Ok(false);
        }
        self.emit_store_ref(&end_ref, code)?;
        if !self.emit_expr(ctx, step, code)? {
            code.truncate(code_start);
            debug_entries.truncate(debug_start);
            return Ok(false);
        }
        self.emit_store_ref(&step_ref, code)?;

        self.emit_load_ref(&step_ref, code)?;
        if !self.emit_const_value(&Value::LInt(0), code)? {
            code.truncate(code_start);
            debug_entries.truncate(debug_start);
            return Ok(false);
        }
        code.push(0x50);
        let jump_step_ok = self.emit_jump_placeholder(code, 0x04);
        code.push(0x01);
        let after_fault = code.len();
        self.patch_jump(code, jump_step_ok, after_fault)?;

        let loop_start = code.len();
        self.emit_load_ref(&step_ref, code)?;
        if !self.emit_const_value(&Value::LInt(0), code)? {
            code.truncate(code_start);
            debug_entries.truncate(debug_start);
            return Ok(false);
        }
        code.push(0x55);
        let jump_true_pos = self.emit_jump_placeholder(code, 0x03);

        self.emit_load_access(&control_access, code)?;
        self.emit_load_ref(&end_ref, code)?;
        code.push(0x55);
        let jump_false_end_neg = self.emit_jump_placeholder(code, 0x04);
        let jump_to_body = self.emit_jump_placeholder(code, 0x02);

        let pos_check = code.len();
        self.patch_jump(code, jump_true_pos, pos_check)?;
        self.emit_load_access(&control_access, code)?;
        self.emit_load_ref(&end_ref, code)?;
        code.push(0x53);
        let jump_false_end_pos = self.emit_jump_placeholder(code, 0x04);

        let body_start = code.len();
        self.patch_jump(code, jump_to_body, body_start)?;
        ctx.push_loop();
        if let Err(err) = self.emit_block(ctx, pou_id, body, code, debug_entries) {
            let _ = ctx.pop_loop();
            code.truncate(code_start);
            debug_entries.truncate(debug_start);
            return Err(err);
        }
        let loop_patches = ctx.pop_loop().unwrap_or_default();
        let continue_target = code.len();
        for jump in loop_patches.continue_jumps {
            self.patch_jump(code, jump, continue_target)?;
        }
        self.emit_load_access(&control_access, code)?;
        self.emit_load_ref(&step_ref, code)?;
        code.push(0x40);
        self.emit_store_access(&control_access, code)?;
        let jump_back = self.emit_jump_placeholder(code, 0x02);
        self.patch_jump(code, jump_back, loop_start)?;

        let loop_end = code.len();
        self.patch_jump(code, jump_false_end_neg, loop_end)?;
        self.patch_jump(code, jump_false_end_pos, loop_end)?;
        for jump in loop_patches.exit_jumps {
            self.patch_jump(code, jump, loop_end)?;
        }
        Ok(true)
    }
}
