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
        if self
            .authoring
            .is_some_and(|input| input.builder.manual_seed_mode)
            && matches!(
                expression,
                Expr::StructInitializer(_) | Expr::ArrayInitializer(_)
            )
        {
            let start = code.len();
            self.emit_seed_override(
                context,
                body.type_id,
                expression,
                initializers::initializer_result_reference(body.id),
                code,
                0,
            )?;
            if code.len() == start {
                code.push(0x00);
            } // Empty seed preserves staged defaults.
            return Ok(true);
        }
        let Some(owner) =
            crate::harness::function_block_type_name(body.type_id, self.runtime.registry())
        else {
            return Ok(false);
        };
        let Expr::StructInitializer(fields) = expression else {
            return Err(invalid("function-block initializer must name members"));
        };
        let mut seen = std::collections::HashSet::new();
        for (name, expression) in fields {
            if !seen.insert(normalize_name(name)) {
                return Err(invalid("duplicate function-block initializer member"));
            }
            let definition = self
                .runtime
                .function_blocks()
                .get(&normalize_name(&owner))
                .ok_or_else(|| invalid("function-block initializer owner missing"))?;
            let (name, ty) = crate::instance::fb_initializer_target(definition, name)
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
    pub(super) fn seed_member(
        &self,
        owner: &SmolStr,
        member: &SmolStr,
    ) -> Result<(SmolStr, TypeId), BytecodeError> {
        let mut owner = owner.clone();
        for _ in 0..crate::bytecode::BYTECODE_MAX_CONST_NESTING {
            if let Some(fb) = self.runtime.function_blocks().get(&normalize_name(&owner)) {
                if let Ok(field) = crate::instance::fb_initializer_target(fb, member) {
                    return Ok(field);
                }
                if let Some(field) = seed_method_static(&fb.name, &fb.methods, member) {
                    return Ok(field);
                }
                owner = match &fb.base {
                    Some(
                        crate::program_model::FunctionBlockBase::FunctionBlock(name)
                        | crate::program_model::FunctionBlockBase::Class(name),
                    ) => name.clone(),
                    None => return Err(invalid("undeclared builder seed member")),
                };
            } else if let Some(class) = self.runtime.classes().get(&normalize_name(&owner)) {
                if let Some(field) = class
                    .vars
                    .iter()
                    .find(|field| !field.external && field.name.eq_ignore_ascii_case(member))
                {
                    return Ok((field.name.clone(), field.type_id));
                }
                if let Some(field) = seed_method_static(&class.name, &class.methods, member) {
                    return Ok(field);
                }
                owner = class
                    .base
                    .clone()
                    .ok_or_else(|| invalid("undeclared builder seed member"))?;
            } else {
                return Err(invalid("builder seed type missing"));
            }
        }
        Err(invalid("builder seed inheritance depth exceeded"))
    }
}

fn seed_method_static(
    owner: &SmolStr,
    methods: &[crate::program_model::MethodDef],
    name: &SmolStr,
) -> Option<(SmolStr, TypeId)> {
    for method in methods {
        let owner = crate::program_model::method_static_storage_owner(owner, &method.name);
        for local in &method.static_locals {
            let storage = crate::program_model::static_storage_name(&owner, &local.name);
            if storage.eq_ignore_ascii_case(name) {
                return Some((storage, local.type_id));
            }
        }
    }
    None
}
