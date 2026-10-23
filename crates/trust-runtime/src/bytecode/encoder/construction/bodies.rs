//! Initializer bodies share the ordinary expression encoder and a private result binding.

use super::*;
use crate::bytecode::{InitializerBodyKind, BYTECODE_MAX_CONTAINER_BYTES};

impl BytecodeEncoder<'_> {
    pub(in crate::bytecode::encoder) fn emit_initializer_bodies(
        &mut self,
        bodies: &mut Vec<u8>,
    ) -> Result<(), BytecodeError> {
        let mut defaults = 0;
        let mut next_body = 0;
        loop {
            while defaults < self.construction.recipes.recipe_types.len() {
                let (id, ty) = self.construction.recipes.recipe_types[defaults];
                defaults += 1;
                self.collect_default_recipes(id, ty)?;
                self.construction
                    .bodies
                    .pending_bodies
                    .push(PendingInitializerBody {
                        id,
                        type_id: ty,
                        expression: None,
                        coerce_opcode: crate::bytecode::opcodes::DEFAULT_TYPED,
                    });
            }
            if next_body == self.construction.bodies.pending_bodies.len() {
                break;
            }
            let body = self.construction.bodies.pending_bodies[next_body].clone();
            next_body += 1;
            let entry = self.construction.initializers.entries[body.id as usize].clone();
            let root = if entry.body_kind == InitializerBodyKind::Action {
                body.id
            } else {
                entry.context_initializer_idx.unwrap_or(body.id)
            };
            let mut context = self
                .construction
                .bodies
                .initializer_contexts
                .get(&root)
                .cloned()
                .unwrap_or_default();
            context.initializer = true;
            context.visible_locals = Some(entry.visible_local_count as usize);
            context.visible_statics = Some(entry.visible_static_count as usize);
            if entry.body_kind != InitializerBodyKind::Action {
                context.locals.clear();
                context.static_refs.clear();
                context.static_self_fields.clear();
            }
            let mut code = Vec::new();
            if !self.emit_fb_override(&context, &body, &entry, &mut code)? {
                let ty = self.type_index(body.type_id)?;
                if let Some(expression) = &body.expression {
                    if !self.emit_expr(&context, expression, &mut code)? {
                        return Err(BytecodeError::InvalidSection(
                            "initializer expression cannot be lowered".into(),
                        ));
                    }
                    code.push(body.coerce_opcode);
                } else {
                    code.push(crate::bytecode::opcodes::DEFAULT_TYPED);
                }
                code.extend_from_slice(&ty.to_le_bytes());
                self.emit_store_ref(
                    &initializers::initializer_result_reference(body.id),
                    &mut code,
                )?;
            }
            if bodies
                .len()
                .checked_add(code.len())
                .is_none_or(|size| size > BYTECODE_MAX_CONTAINER_BYTES)
            {
                return Err(BytecodeError::InvalidSection(
                    "initializer body size limit exceeded".into(),
                ));
            }
            let offset = u32::try_from(bodies.len()).map_err(|_| {
                BytecodeError::InvalidSection("initializer body offset overflow".into())
            })?;
            let length = u32::try_from(code.len()).map_err(|_| {
                BytecodeError::InvalidSection("initializer body length overflow".into())
            })?;
            let record = &mut self.construction.initializers.entries[body.id as usize];
            record.code_offset = offset;
            record.code_length = length;
            bodies.extend_from_slice(&code);
        }
        Ok(())
    }
}
