impl<'a> BytecodeEncoder<'a> {
    fn emit_reference_attempt_assign(
        &mut self,
        ctx: &CodegenContext,
        target: &crate::program_model::LValue,
        value: &crate::program_model::Expr,
        target_type: trust_hir::TypeId,
        code: &mut Vec<u8>,
    ) -> Result<bool, BytecodeError> {
        let start_len = code.len();
        if !self.emit_expr(ctx, value, code)? {
            code.truncate(start_len);
            return Ok(false);
        }

        let target_type_idx = self.type_index(target_type)?;
        code.push(0x64); // REFERENCE_ATTEMPT
        code.extend_from_slice(&target_type_idx.to_le_bytes());

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
            self.emit_store_ref(&reference, code)?;
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
}
