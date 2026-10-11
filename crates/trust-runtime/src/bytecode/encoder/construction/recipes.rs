//! Compile type/member expressions once per lexical initialization context.

use super::*;
use crate::bytecode::{
    InitializationOnce, InitializationPhase, InitializationStage, InitializationTarget,
    InitializerBodyKind, InitializerEntry,
};
use trust_hir::Type;

impl BytecodeEncoder<'_> {
    pub(super) fn collect_default_recipes(
        &mut self,
        context: u32,
        ty: TypeId,
    ) -> Result<(), BytecodeError> {
        let key = self.construction.initializers.entries[context as usize]
            .recipe_context_key(&self.construction.layout)
            .ok_or_else(|| invalid("recipe context declaration missing"))?;
        let canonical = *self
            .construction
            .recipes
            .recipe_contexts
            .entry(key)
            .or_insert(context);
        if canonical != context {
            self.construction.initializers.entries[context as usize].context_initializer_idx =
                Some(canonical);
        }
        let context = canonical;
        let mut pending = vec![(ty, 0usize)];
        let mut visited = std::collections::HashSet::new();
        while let Some((ty, depth)) = pending.pop() {
            if depth > 64 {
                return Err(invalid("default type nesting limit exceeded"));
            }
            if !visited.insert(ty) {
                continue;
            }
            let catalog = self.runtime.initializer_catalog();
            if let Some(id) = catalog.type_default(ty) {
                let expression = catalog
                    .initializer(id)
                    .cloned()
                    .ok_or_else(|| invalid("type default expression missing"))?;
                self.add_default_recipe(context, ty, None, ty, expression)?;
            }
            let definition = self
                .runtime
                .registry()
                .get(ty)
                .cloned()
                .ok_or_else(|| invalid("default type missing"))?;
            match definition {
                Type::Alias { target, .. } => pending.push((target, depth + 1)),
                Type::Array { element, .. } => pending.push((element, depth + 1)),
                Type::Struct { fields, .. } => {
                    for (member, field) in fields.into_iter().enumerate() {
                        if let Some(id) = field.default_initializer {
                            let expression = self
                                .runtime
                                .initializer_catalog()
                                .initializer(id)
                                .cloned()
                                .ok_or_else(|| invalid("member default expression missing"))?;
                            self.add_default_recipe(
                                context,
                                ty,
                                Some(member as u32),
                                field.type_id,
                                expression,
                            )?;
                        }
                        pending.push((field.type_id, depth + 1));
                    }
                }
                Type::Union { variants, .. } => {
                    for (member, variant) in variants.into_iter().enumerate() {
                        if let Some(id) = variant.default_initializer {
                            let expression = self
                                .runtime
                                .initializer_catalog()
                                .initializer(id)
                                .cloned()
                                .ok_or_else(|| invalid("variant default expression missing"))?;
                            self.add_default_recipe(
                                context,
                                ty,
                                Some(member as u32),
                                variant.type_id,
                                expression,
                            )?;
                        }
                        pending.push((variant.type_id, depth + 1));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn add_default_recipe(
        &mut self,
        context: u32,
        owner_type: TypeId,
        member: Option<u32>,
        result_type: TypeId,
        expression: Expr,
    ) -> Result<(), BytecodeError> {
        let key = (InitializerId(context), owner_type, member.map(MemberId));
        if self.construction.recipes.typed_recipes.contains_key(&key) {
            return Ok(());
        }
        let root = self
            .construction
            .initializers
            .entries
            .get(context as usize)
            .ok_or_else(|| invalid("recipe root missing"))?;
        let owner = root.owner_pou_id;
        let trigger = root.trigger;
        let wire_type = self.type_index(owner_type)?;
        let id = self.reserve_initializer_result(
            result_type,
            InitializerEntry {
                declaration_idx: None,
                owner_pou_id: owner,
                result_ref_idx: 0,
                code_offset: 0,
                code_length: 0,
                visible_local_count: 0,
                visible_static_count: 0,
                phase: InitializationPhase::ValueDefault,
                once: InitializationOnce::None,
                stage: InitializationStage::Default,
                trigger,
                target_idx: None,
                partial_kind: 0,
                target_kind: InitializationTarget::Declaration,
                target_reserved: [0; 2],
                partial_index: 0,
                context_initializer_idx: Some(context),
                recipe_type_id: Some(wire_type),
                recipe_member_idx: member,
                body_kind: if member.is_some() {
                    InitializerBodyKind::MemberDefault
                } else {
                    InitializerBodyKind::TypeDefault
                },
                recipe_reserved: [0; 3],
            },
        )?;
        self.construction
            .recipes
            .typed_recipes
            .insert(key, InitializerId(id));
        self.construction
            .bodies
            .pending_bodies
            .push(PendingInitializerBody {
                id,
                type_id: result_type,
                expression: Some(expression),
                coerce_opcode: if member.is_some() {
                    crate::bytecode::opcodes::APPLY_INIT_VALUE
                } else {
                    crate::bytecode::opcodes::COERCE_INIT_VALUE
                },
            });
        Ok(())
    }
}
