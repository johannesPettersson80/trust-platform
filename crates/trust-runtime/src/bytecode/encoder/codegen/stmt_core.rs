impl<'a> BytecodeEncoder<'a> {
    pub(super) fn emit_pou_body(
        &mut self,
        ctx: &mut CodegenContext,
        pou_id: u32,
        body: &[crate::program_model::Stmt],
    ) -> Result<(Vec<u8>, Vec<DebugEntry>), BytecodeError> {
        let mut code = Vec::new();
        let mut debug_entries = Vec::new();
        for stmt in body {
            self.emit_stmt(ctx, pou_id, stmt, &mut code, &mut debug_entries)?;
        }
        Ok((code, debug_entries))
    }

    fn emit_stmt(
        &mut self,
        ctx: &mut CodegenContext,
        pou_id: u32,
        stmt: &crate::program_model::Stmt,
        code: &mut Vec<u8>,
        debug_entries: &mut Vec<DebugEntry>,
    ) -> Result<(), BytecodeError> {
        let offset = to_u32(code.len(), "debug code offset")?;
        if let (Some(location), Some(sources)) = (stmt.location(), self.sources) {
            let source = sources
                .get(location.file_id as usize)
                .ok_or_else(|| BytecodeError::InvalidSection("debug source missing".into()))?;
            let (line, column) = crate::debug::location_to_line_col(source, location);
            let line = line.saturating_add(1);
            let column = column.saturating_add(1);
            let file_idx = self.file_path_index(location.file_id)?;
            debug_entries.push(DebugEntry {
                pou_id,
                code_offset: offset,
                file_idx,
                line,
                column,
                kind: 0,
            });
        }
        let emitted = match stmt {
            crate::program_model::Stmt::Assign { target, value, .. } => {
                self.emit_assign(ctx, target, value, code)?
            }
            crate::program_model::Stmt::AssignAttempt { target, value, .. } => {
                self.emit_assign(ctx, target, value, code)?
            }
            crate::program_model::Stmt::Expr { expr, .. } => {
                if !self.emit_expr(ctx, expr, code)? {
                    false
                } else {
                    code.push(0x12); // POP (statement value is discarded)
                    true
                }
            }
            crate::program_model::Stmt::Return { expr, .. } => {
                self.emit_return_stmt(ctx, expr.as_ref(), code)?
            }
            crate::program_model::Stmt::If {
                condition,
                then_block,
                else_if,
                else_block,
                ..
            } => self.emit_if_stmt(
                ctx,
                pou_id,
                condition,
                then_block,
                else_if,
                else_block,
                code,
                debug_entries,
            )?,
            crate::program_model::Stmt::Case {
                selector,
                branches,
                else_block,
                ..
            } => self.emit_case_stmt(
                ctx,
                pou_id,
                selector,
                branches,
                else_block,
                code,
                debug_entries,
            )?,
            crate::program_model::Stmt::While {
                condition, body, ..
            } => self.emit_while_stmt(ctx, pou_id, condition, body, code, debug_entries)?,
            crate::program_model::Stmt::Repeat { body, until, .. } => {
                self.emit_repeat_stmt(ctx, pou_id, body, until, code, debug_entries)?
            }
            crate::program_model::Stmt::For {
                control,
                start,
                end,
                step,
                body,
                ..
            } => self.emit_for_stmt(
                ctx,
                pou_id,
                control,
                start,
                end,
                step,
                body,
                code,
                debug_entries,
            )?,
            crate::program_model::Stmt::Label { stmt, .. } => {
                if let Some(stmt) = stmt.as_deref() {
                    self.emit_stmt(ctx, pou_id, stmt, code, debug_entries)?;
                    true
                } else {
                    false
                }
            }
            crate::program_model::Stmt::Exit { .. } => self.emit_loop_jump(ctx, code, true)?,
            crate::program_model::Stmt::Continue { .. } => {
                self.emit_loop_jump(ctx, code, false)?
            }
            _ => false,
        };

        if !emitted {
            if stmt_contains_c1_required_call(stmt) {
                return Err(BytecodeError::InvalidSection(
                    format!("unsupported C1 CALL_NATIVE lowering path: {:?}", stmt).into(),
                ));
            }
            if stmt_contains_c5_required_construct(stmt) {
                return Err(BytecodeError::InvalidSection(
                    "unsupported C5 edge-case lowering path".into(),
                ));
            }
            code.push(0x00);
        }
        Ok(())
    }

    fn emit_block(
        &mut self,
        ctx: &mut CodegenContext,
        pou_id: u32,
        block: &[crate::program_model::Stmt],
        code: &mut Vec<u8>,
        debug_entries: &mut Vec<DebugEntry>,
    ) -> Result<(), BytecodeError> {
        for stmt in block {
            self.emit_stmt(ctx, pou_id, stmt, code, debug_entries)?;
        }
        Ok(())
    }

    fn emit_loop_jump(
        &mut self,
        ctx: &mut CodegenContext,
        code: &mut Vec<u8>,
        exit: bool,
    ) -> Result<bool, BytecodeError> {
        let jump = self.emit_jump_placeholder(code, 0x02);
        let recorded = if exit {
            ctx.record_exit_jump(jump)
        } else {
            ctx.record_continue_jump(jump)
        };
        if recorded {
            Ok(true)
        } else {
            Err(BytecodeError::InvalidSection(
                if exit {
                    "EXIT lowering requires active loop"
                } else {
                    "CONTINUE lowering requires active loop"
                }
                .into(),
            ))
        }
    }

    fn emit_return_stmt(
        &mut self,
        ctx: &mut CodegenContext,
        expr: Option<&crate::program_model::Expr>,
        code: &mut Vec<u8>,
    ) -> Result<bool, BytecodeError> {
        if let Some(expr) = expr {
            let Some(return_name) = ctx.return_name.clone() else {
                return Ok(false);
            };
            if !self.emit_assign(
                ctx,
                &crate::program_model::LValue::Name(return_name),
                expr,
                code,
            )? {
                return Ok(false);
            }
        }
        code.push(0x06);
        Ok(true)
    }

}

fn stmt_contains_c1_required_call(stmt: &crate::program_model::Stmt) -> bool {
    use crate::program_model::Stmt;
    match stmt {
        Stmt::Assign { value, .. } => expr_contains_call(value),
        Stmt::Expr { expr, .. } => expr_contains_call(expr),
        Stmt::If {
            condition,
            then_block,
            else_if,
            else_block,
            ..
        } => {
            expr_contains_call(condition)
                || then_block.iter().any(stmt_contains_c1_required_call)
                || else_if
                    .iter()
                    .any(|(expr, block)| {
                        expr_contains_call(expr)
                            || block.iter().any(stmt_contains_c1_required_call)
                    })
                || else_block.iter().any(stmt_contains_c1_required_call)
        }
        Stmt::Case {
            selector,
            branches,
            else_block,
            ..
        } => {
            expr_contains_call(selector)
                || branches.iter().any(|(_, block)| {
                    block.iter().any(stmt_contains_c1_required_call)
                })
                || else_block.iter().any(stmt_contains_c1_required_call)
        }
        Stmt::While {
            condition, body, ..
        } => {
            expr_contains_call(condition) || body.iter().any(stmt_contains_c1_required_call)
        }
        Stmt::Repeat { body, until, .. } => {
            body.iter().any(stmt_contains_c1_required_call) || expr_contains_call(until)
        }
        Stmt::For {
            start,
            end,
            step,
            body,
            ..
        } => {
            expr_contains_call(start)
                || expr_contains_call(end)
                || expr_contains_call(step)
                || body.iter().any(stmt_contains_c1_required_call)
        }
        Stmt::Label { stmt, .. } => stmt
            .as_deref()
            .map(stmt_contains_c1_required_call)
            .unwrap_or(false),
        Stmt::AssignAttempt { value, .. } | Stmt::Return { expr: Some(value), .. } => {
            expr_contains_call(value)
        }
        Stmt::Return { expr: None, .. }
        | Stmt::Jmp { .. }
        | Stmt::Exit { .. }
        | Stmt::Continue { .. } => false,
    }
}

fn expr_contains_call(expr: &crate::program_model::Expr) -> bool {
    use crate::program_model::Expr;
    match expr {
        Expr::Call { .. } => true,
        Expr::ArrayInitializer(elements) => elements.iter().any(expr_contains_call),
        Expr::StructInitializer(fields) => fields.iter().any(|(_, value)| expr_contains_call(value)),
        Expr::Unary { expr, .. } => expr_contains_call(expr),
        Expr::Binary { left, right, .. } => expr_contains_call(left) || expr_contains_call(right),
        Expr::Index { target, indices } => {
            expr_contains_call(target) || indices.iter().any(expr_contains_call)
        }
        Expr::Field { target, .. } => expr_contains_call(target),
        Expr::Ref(lvalue) => lvalue_contains_call(lvalue),
        Expr::Deref(expr) => expr_contains_call(expr),
        Expr::SizeOf(crate::program_model::SizeOfTarget::Type(_))
        | Expr::Literal(_)
        | Expr::This
        | Expr::Super
        | Expr::Name(_) => false,
    }
}

fn lvalue_contains_call(lvalue: &crate::program_model::LValue) -> bool {
    use crate::program_model::LValue;
    match lvalue {
        LValue::Name(_) => false,
        LValue::Field { target, .. } => lvalue_contains_call(target),
        LValue::Index { target, indices } => {
            lvalue_contains_call(target) || indices.iter().any(expr_contains_call)
        }
        LValue::Deref(expr) => expr_contains_call(expr),
    }
}

fn stmt_contains_c5_required_construct(stmt: &crate::program_model::Stmt) -> bool {
    use crate::program_model::Stmt;
    match stmt {
        Stmt::Assign { value, .. } => expr_contains_sizeof(value),
        Stmt::AssignAttempt { .. }
        | Stmt::Jmp { .. }
        | Stmt::Exit { .. }
        | Stmt::Continue { .. } => true,
        Stmt::Return { expr, .. } => expr.as_ref().is_some_and(expr_contains_sizeof),
        Stmt::Expr { expr, .. } => expr_contains_sizeof(expr),
        Stmt::If {
            condition,
            then_block,
            else_if,
            else_block,
            ..
        } => {
            expr_contains_sizeof(condition)
                || then_block.iter().any(stmt_contains_c5_required_construct)
                || else_if.iter().any(|(expr, block)| {
                    expr_contains_sizeof(expr)
                        || block.iter().any(stmt_contains_c5_required_construct)
                })
                || else_block.iter().any(stmt_contains_c5_required_construct)
        }
        Stmt::Case {
            selector,
            branches,
            else_block,
            ..
        } => {
            expr_contains_sizeof(selector)
                || branches
                    .iter()
                    .any(|(_, block)| block.iter().any(stmt_contains_c5_required_construct))
                || else_block.iter().any(stmt_contains_c5_required_construct)
        }
        Stmt::While {
            condition, body, ..
        } => {
            expr_contains_sizeof(condition) || body.iter().any(stmt_contains_c5_required_construct)
        }
        Stmt::Repeat { body, until, .. } => {
            body.iter().any(stmt_contains_c5_required_construct) || expr_contains_sizeof(until)
        }
        Stmt::For {
            start,
            end,
            step,
            body,
            ..
        } => {
            expr_contains_sizeof(start)
                || expr_contains_sizeof(end)
                || expr_contains_sizeof(step)
                || body.iter().any(stmt_contains_c5_required_construct)
        }
        Stmt::Label { stmt, .. } => stmt
            .as_deref()
            .map(stmt_contains_c5_required_construct)
            .unwrap_or(false),
    }
}

fn expr_contains_sizeof(expr: &crate::program_model::Expr) -> bool {
    use crate::program_model::Expr;
    match expr {
        Expr::SizeOf(_) => true,
        Expr::ArrayInitializer(elements) => elements.iter().any(expr_contains_sizeof),
        Expr::StructInitializer(fields) => {
            fields.iter().any(|(_, value)| expr_contains_sizeof(value))
        }
        Expr::Unary { expr, .. } | Expr::Deref(expr) => expr_contains_sizeof(expr),
        Expr::Binary { left, right, .. } => {
            expr_contains_sizeof(left) || expr_contains_sizeof(right)
        }
        Expr::Index { target, indices } => {
            expr_contains_sizeof(target) || indices.iter().any(expr_contains_sizeof)
        }
        Expr::Field { target, .. } => expr_contains_sizeof(target),
        Expr::Call { target, args } => {
            expr_contains_sizeof(target)
                || args.iter().any(|arg| match &arg.value {
                    crate::program_model::ArgValue::Expr(expr) => expr_contains_sizeof(expr),
                    crate::program_model::ArgValue::Target(target) => lvalue_contains_sizeof(target),
                })
        }
        Expr::Ref(lvalue) => lvalue_contains_sizeof(lvalue),
        Expr::Literal(_) | Expr::This | Expr::Super | Expr::Name(_) => false,
    }
}

fn lvalue_contains_sizeof(lvalue: &crate::program_model::LValue) -> bool {
    use crate::program_model::LValue;
    match lvalue {
        LValue::Name(_) => false,
        LValue::Field { target, .. } => lvalue_contains_sizeof(target),
        LValue::Index { target, indices } => {
            lvalue_contains_sizeof(target) || indices.iter().any(expr_contains_sizeof)
        }
        LValue::Deref(expr) => expr_contains_sizeof(expr),
    }
}
