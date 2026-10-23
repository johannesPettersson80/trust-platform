impl<'a> BytecodeEncoder<'a> {
    fn emit_jump_placeholder(&self, code: &mut Vec<u8>, opcode: u8) -> usize {
        let pos = code.len();
        code.push(opcode);
        code.extend_from_slice(&0i32.to_le_bytes());
        pos
    }

    fn patch_jump(
        &self,
        code: &mut [u8],
        jump_pos: usize,
        target: usize,
    ) -> Result<(), BytecodeError> {
        let base = jump_pos
            .checked_add(5)
            .ok_or_else(|| BytecodeError::InvalidSection("jump base overflow".into()))?;
        let delta = target as i64 - base as i64;
        if delta < i64::from(i32::MIN) || delta > i64::from(i32::MAX) {
            return Err(BytecodeError::InvalidSection("jump offset overflow".into()));
        }
        let offset = delta as i32;
        let bytes = offset.to_le_bytes();
        let range = jump_pos + 1..jump_pos + 5;
        code[range].copy_from_slice(&bytes);
        Ok(())
    }

    fn emit_load_ref(
        &mut self,
        reference: &ValueRef,
        code: &mut Vec<u8>,
    ) -> Result<(), BytecodeError> {
        let ref_idx = self.ref_index_for(reference)?;
        code.push(0x20);
        code.extend_from_slice(&ref_idx.to_le_bytes());
        Ok(())
    }

    pub(in crate::bytecode::encoder) fn emit_store_ref(
        &mut self,
        reference: &ValueRef,
        code: &mut Vec<u8>,
    ) -> Result<(), BytecodeError> {
        let ref_idx = self.ref_index_for(reference)?;
        code.push(0x21);
        code.extend_from_slice(&ref_idx.to_le_bytes());
        Ok(())
    }

    pub(in crate::bytecode::encoder) fn emit_address_ref(
        &mut self,
        reference: &ValueRef,
        code: &mut Vec<u8>,
    ) -> Result<(), BytecodeError> {
        let ref_idx = self.ref_index_for(reference)?;
        code.push(0x22);
        code.extend_from_slice(&ref_idx.to_le_bytes());
        Ok(())
    }

    fn emit_const_value(
        &mut self,
        value: &Value,
        code: &mut Vec<u8>,
    ) -> Result<bool, BytecodeError> {
        let const_idx = match self.const_index_for(value) {
            Ok(idx) => idx,
            Err(_) => return Ok(false),
        };
        code.push(0x10);
        code.extend_from_slice(&const_idx.to_le_bytes());
        Ok(true)
    }
}
