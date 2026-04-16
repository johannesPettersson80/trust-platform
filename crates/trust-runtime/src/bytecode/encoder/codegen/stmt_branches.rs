impl<'a> BytecodeEncoder<'a> {
    #[allow(clippy::too_many_arguments)]
    fn emit_if_stmt(
        &mut self,
        ctx: &mut CodegenContext,
        pou_id: u32,
        condition: &crate::program_model::Expr,
        then_block: &[crate::program_model::Stmt],
        else_if: &[(crate::program_model::Expr, Vec<crate::program_model::Stmt>)],
        else_block: &[crate::program_model::Stmt],
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
        for (cond, _) in else_if {
            if !expr_supported(cond) {
                code.truncate(code_start);
                debug_entries.truncate(debug_start);
                return Ok(false);
            }
        }
        if !self.emit_expr(ctx, condition, code)? {
            code.truncate(code_start);
            debug_entries.truncate(debug_start);
            return Ok(false);
        }
        let mut end_jumps = Vec::new();
        let mut jump_false = self.emit_jump_placeholder(code, 0x04);
        if let Err(err) = self.emit_block(ctx, pou_id, then_block, code, debug_entries) {
            code.truncate(code_start);
            debug_entries.truncate(debug_start);
            return Err(err);
        }
        if !else_if.is_empty() || !else_block.is_empty() {
            end_jumps.push(self.emit_jump_placeholder(code, 0x02));
        }
        let mut next_start = code.len();
        self.patch_jump(code, jump_false, next_start)?;
        for (cond, block) in else_if {
            if !self.emit_expr(ctx, cond, code)? {
                code.truncate(code_start);
                debug_entries.truncate(debug_start);
                return Ok(false);
            }
            jump_false = self.emit_jump_placeholder(code, 0x04);
            if let Err(err) = self.emit_block(ctx, pou_id, block, code, debug_entries) {
                code.truncate(code_start);
                debug_entries.truncate(debug_start);
                return Err(err);
            }
            end_jumps.push(self.emit_jump_placeholder(code, 0x02));
            next_start = code.len();
            self.patch_jump(code, jump_false, next_start)?;
        }
        if let Err(err) = self.emit_block(ctx, pou_id, else_block, code, debug_entries) {
            code.truncate(code_start);
            debug_entries.truncate(debug_start);
            return Err(err);
        }
        let end = code.len();
        for jump in end_jumps {
            self.patch_jump(code, jump, end)?;
        }
        Ok(true)
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_case_stmt(
        &mut self,
        ctx: &mut CodegenContext,
        pou_id: u32,
        selector: &crate::program_model::Expr,
        branches: &[(
            Vec<crate::program_model::CaseLabel>,
            Vec<crate::program_model::Stmt>,
        )],
        else_block: &[crate::program_model::Stmt],
        code: &mut Vec<u8>,
        debug_entries: &mut Vec<DebugEntry>,
    ) -> Result<bool, BytecodeError> {
        let code_start = code.len();
        let debug_start = debug_entries.len();
        if !expr_supported(selector) {
            code.truncate(code_start);
            debug_entries.truncate(debug_start);
            return Ok(false);
        }
        if !self.emit_expr(ctx, selector, code)? {
            code.truncate(code_start);
            debug_entries.truncate(debug_start);
            return Ok(false);
        }
        let mut end_jumps = Vec::new();
        for (labels, block) in branches {
            let mut label_jumps = Vec::new();
            for label in labels {
                match label {
                    crate::program_model::CaseLabel::Single(value) => {
                        code.push(0x11);
                        if !self.emit_const_value(value, code)? {
                            code.truncate(code_start);
                            debug_entries.truncate(debug_start);
                            return Ok(false);
                        }
                        code.push(0x50);
                        label_jumps.push(self.emit_jump_placeholder(code, 0x03));
                    }
                    crate::program_model::CaseLabel::Range(lower, upper) => {
                        code.push(0x11);
                        if !self.emit_const_value(&Value::LInt(*lower), code)? {
                            code.truncate(code_start);
                            debug_entries.truncate(debug_start);
                            return Ok(false);
                        }
                        code.push(0x55);
                        let skip_range = self.emit_jump_placeholder(code, 0x04);
                        code.push(0x11);
                        if !self.emit_const_value(&Value::LInt(*upper), code)? {
                            code.truncate(code_start);
                            debug_entries.truncate(debug_start);
                            return Ok(false);
                        }
                        code.push(0x53);
                        label_jumps.push(self.emit_jump_placeholder(code, 0x03));
                        let after_range = code.len();
                        self.patch_jump(code, skip_range, after_range)?;
                    }
                }
            }
            let skip_branch = self.emit_jump_placeholder(code, 0x02);
            let branch_start = code.len();
            for jump in label_jumps {
                self.patch_jump(code, jump, branch_start)?;
            }
            code.push(0x12);
            if let Err(err) = self.emit_block(ctx, pou_id, block, code, debug_entries) {
                code.truncate(code_start);
                debug_entries.truncate(debug_start);
                return Err(err);
            }
            end_jumps.push(self.emit_jump_placeholder(code, 0x02));
            let next_check = code.len();
            self.patch_jump(code, skip_branch, next_check)?;
        }
        code.push(0x12);
        if let Err(err) = self.emit_block(ctx, pou_id, else_block, code, debug_entries) {
            code.truncate(code_start);
            debug_entries.truncate(debug_start);
            return Err(err);
        }
        let end = code.len();
        for jump in end_jumps {
            self.patch_jump(code, jump, end)?;
        }
        Ok(true)
    }
}
