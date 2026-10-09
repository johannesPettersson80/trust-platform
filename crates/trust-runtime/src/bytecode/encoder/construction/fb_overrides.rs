//! FB overrides evaluate and assign one member at a time, unlike value aggregates.

use super::*;
use crate::bytecode::InitializerEntry;

impl BytecodeEncoder<'_> {
    pub(super) fn emit_fb_override(
        &mut self,
        context: &CodegenContext,
        body: &PendingInitializerBody,
        entry: &InitializerEntry,
        code: &mut Vec<u8>,
    ) -> Result<bool, BytecodeError> {
        let Some(expression) = &body.expression else {
            return Ok(false);
        };
        let Some(name) =
            crate::harness::function_block_type_name(body.type_id, self.runtime.registry())
        else {
            return Ok(false);
        };
        let Expr::StructInitializer(fields) = expression else {
            return Err(invalid("function-block initializer must name members"));
        };
        let definition = self
            .runtime
            .function_blocks()
            .get(&normalize_name(&name))
            .cloned()
            .ok_or_else(|| invalid("function-block initializer owner missing"))?;
        let mut seen = std::collections::HashSet::new();
        for (name, expression) in fields {
            if !seen.insert(normalize_name(name)) {
                return Err(invalid("duplicate function-block initializer member"));
            }
            let (name, ty) = crate::instance::fb_initializer_target(&definition, name)
                .map_err(|_| invalid("invalid function-block initializer member"))?;
            self.collect_default_recipes(entry.context_initializer_idx.unwrap_or(body.id), ty)?;
            self.emit_address_ref(&initializers::initializer_result_reference(body.id), code)?;
            code.push(0x30);
            code.extend_from_slice(&self.strings.intern(name).to_le_bytes());
            if !self.emit_expr(context, expression, code)? {
                return Err(invalid(
                    "function-block initializer expression cannot be lowered",
                ));
            }
            code.push(crate::bytecode::opcodes::APPLY_INIT_VALUE);
            code.extend_from_slice(&self.type_index(ty)?.to_le_bytes());
            code.push(0x33);
        }
        Ok(true)
    }
}
