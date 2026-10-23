//! FB overrides evaluate and assign one member at a time, unlike value aggregates.

use super::*;
use crate::bytecode::InitializerEntry;
use crate::value::Value;
use trust_hir::Type;

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
        if let Expr::ArrayInitializer(entries) = expression
            && let Some((element, dimensions)) = self.instance_array_shape(body.type_id)
        {
            return self.emit_instance_array_overrides(
                context,
                body,
                entry,
                element,
                &dimensions,
                entries,
                code,
            );
        }
        let Some(owner) =
            crate::harness::function_block_type_name(body.type_id, self.runtime.registry())
        else {
            return Ok(false);
        };
        let Expr::StructInitializer(fields) = expression else {
            return Err(invalid("function-block initializer must name members"));
        };
        self.emit_member_overrides(context, body, entry, &owner, fields, &[], code)?;
        Ok(true)
    }

    /// The initializer a named array type of function-block instances declares (IEC 61131-3
    /// Ed.3 Table 11 feature 5b: `TYPE TONS : ARRAY[1..50] OF TON := [50(PT := T#100ms)]`). It
    /// sets members of the instances that each variable of the type owns, so a declaration of
    /// the type without its own initializer lowers it as its own, and it is no value recipe.
    pub(super) fn instance_array_type_default(&self, type_id: TypeId) -> Option<Expr> {
        self.instance_array_shape(type_id)?;
        let registry = self.runtime.registry();
        let catalog = self.runtime.initializer_catalog();
        let mut current = type_id;
        for _ in 0..crate::bytecode::BYTECODE_MAX_CONST_NESTING {
            if let Some(id) = catalog.type_default(current) {
                return catalog.initializer(id).cloned();
            }
            let Type::Alias { target, .. } = registry.get(current)? else {
                return None;
            };
            current = *target;
        }
        None
    }

    /// The element type and dimensions of an array of function-block instances.
    pub(super) fn instance_array_shape(
        &self,
        type_id: TypeId,
    ) -> Option<(TypeId, Vec<(i64, i64)>)> {
        let registry = self.runtime.registry();
        let mut current = type_id;
        for _ in 0..crate::bytecode::BYTECODE_MAX_CONST_NESTING {
            match registry.get(current)? {
                Type::Alias { target, .. } => current = *target,
                Type::Array {
                    element,
                    dimensions,
                } => {
                    return crate::harness::function_block_type_name(*element, registry)
                        .map(|_| (*element, dimensions.clone()));
                }
                _ => return None,
            }
        }
        None
    }

    /// An instance array's initializer (IEC 61131-3 Ed.3 Table 11 feature 5): each element's
    /// member initializers in row-major order, `n((...))` repeating a group; elements past the
    /// list keep their declared initial values and initializers past the last element are
    /// ignored.
    #[allow(clippy::too_many_arguments)]
    fn emit_instance_array_overrides(
        &mut self,
        context: &CodegenContext,
        body: &PendingInitializerBody,
        entry: &InitializerEntry,
        element: TypeId,
        dimensions: &[(i64, i64)],
        entries: &[Expr],
        code: &mut Vec<u8>,
    ) -> Result<bool, BytecodeError> {
        let owner = crate::harness::function_block_type_name(element, self.runtime.registry())
            .ok_or_else(|| invalid("instance array element is not a function block"))?;
        let mut lengths = Vec::with_capacity(dimensions.len());
        for (lower, upper) in dimensions {
            let length = upper
                .checked_sub(*lower)
                .and_then(|n| n.checked_add(1))
                .and_then(|n| usize::try_from(n).ok())
                .ok_or_else(|| invalid("invalid instance array bound"))?;
            lengths.push(length);
        }
        let total = lengths
            .iter()
            .try_fold(1usize, |count, length| count.checked_mul(*length))
            .ok_or_else(|| invalid("instance array size overflow"))?;
        let mut elements: Vec<&Expr> = Vec::new();
        for item in entries {
            match crate::helper_eval::array_repeat_group(item)
                .map_err(|_| invalid("invalid instance array repetition"))?
            {
                Some((count, args)) => {
                    for _ in 0..count {
                        for arg in args {
                            let crate::program_model::ArgValue::Expr(value) = &arg.value else {
                                return Err(invalid("invalid instance array repetition"));
                            };
                            elements.push(value);
                            if elements.len() >= total {
                                break;
                            }
                        }
                        if elements.len() >= total {
                            break;
                        }
                    }
                }
                None => elements.push(item),
            }
            if elements.len() >= total {
                break;
            }
        }
        for (offset, value) in elements.into_iter().take(total).enumerate() {
            let Expr::StructInitializer(fields) = value else {
                return Err(invalid(
                    "an instance array element initializer must name members",
                ));
            };
            let mut rest = offset;
            let mut indices = vec![0i64; dimensions.len()];
            for (at, length) in lengths.iter().enumerate().rev() {
                let position = i64::try_from(rest % length)
                    .map_err(|_| invalid("instance array index overflow"))?;
                indices[at] = dimensions[at]
                    .0
                    .checked_add(position)
                    .ok_or_else(|| invalid("instance array index overflow"))?;
                rest /= length;
            }
            self.emit_member_overrides(context, body, entry, &owner, fields, &indices, code)?;
        }
        Ok(true)
    }

    /// Assigns each named member of one function-block instance: the initializer result itself,
    /// or the element at `indices` of an instance array.
    #[allow(clippy::too_many_arguments)]
    fn emit_member_overrides(
        &mut self,
        context: &CodegenContext,
        body: &PendingInitializerBody,
        entry: &InitializerEntry,
        owner: &SmolStr,
        fields: &[(SmolStr, Expr)],
        indices: &[i64],
        code: &mut Vec<u8>,
    ) -> Result<(), BytecodeError> {
        let mut seen = std::collections::HashSet::new();
        for (name, expression) in fields {
            if !seen.insert(normalize_name(name)) {
                return Err(invalid("duplicate function-block initializer member"));
            }
            let definition = self
                .runtime
                .function_blocks()
                .get(&normalize_name(owner))
                .ok_or_else(|| invalid("function-block initializer owner missing"))?;
            let (name, ty) = crate::instance::fb_initializer_target(
                self.runtime.function_blocks(),
                definition,
                name,
            )
            .map_err(|_| invalid("invalid function-block initializer member"))?;
            self.collect_default_recipes(entry.context_initializer_idx.unwrap_or(body.id), ty)?;
            self.emit_address_ref(&initializers::initializer_result_reference(body.id), code)?;
            for index in indices {
                if !self.emit_expr(
                    context,
                    &Expr::Literal(Value::DInt(
                        i32::try_from(*index)
                            .map_err(|_| invalid("instance array index overflow"))?,
                    )),
                    code,
                )? {
                    return Err(invalid("instance array index cannot be lowered"));
                }
                code.push(0x31);
            }
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
        Ok(())
    }
    pub(super) fn seed_member(
        &self,
        owner: &SmolStr,
        member: &SmolStr,
    ) -> Result<(SmolStr, TypeId), BytecodeError> {
        let mut owner = owner.clone();
        for _ in 0..crate::bytecode::BYTECODE_MAX_CONST_NESTING {
            if let Some(fb) = self.runtime.function_blocks().get(&normalize_name(&owner)) {
                if let Ok(field) = crate::instance::fb_initializer_target(
                    self.runtime.function_blocks(),
                    fb,
                    member,
                ) {
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
