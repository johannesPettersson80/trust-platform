//! Preserve declaration groups when selecting lifecycle action order.

use super::*;
use crate::bytecode::{
    InitializationOnce, InitializationPhase, InitializationStage, InitializationTarget,
    InitializationTrigger, InitializerBodyKind, InitializerEntry, PouIndex, PouKind, StorageOwner,
    StorageRole,
};

impl BytecodeEncoder<'_> {
    pub(in crate::bytecode::encoder) fn collect_declaration_actions(
        &mut self,
        index: &PouIndex,
    ) -> Result<(), BytecodeError> {
        let lookup = ActionIndex::new(index, &self.construction);
        let mut groups: IndexMap<(u8, Option<u32>, u8), Vec<DeclarationPlan>> = IndexMap::new();
        for plan in self.construction.plans.clone() {
            let decl = &self.construction.layout.entries[plan.declaration as usize];
            let physical_owner = decl
                .owner_pou_id
                .and_then(|id| lookup.pous.get(&id).copied())
                .map(|pou| {
                    if pou.kind == PouKind::Method {
                        pou.owner_pou_id.unwrap_or(pou.id)
                    } else {
                        pou.id
                    }
                });
            let owner = match decl.owner {
                StorageOwner::Instance => physical_owner,
                StorageOwner::Global => None,
                StorageOwner::Frame => decl.owner_pou_id,
            };
            let group = match decl.role {
                StorageRole::Parameter => 0,
                StorageRole::Static => 2,
                StorageRole::EdgePhase => 3,
                _ => 1,
            };
            groups
                .entry((decl.owner as u8, owner, group))
                .or_default()
                .push(plan);
        }
        for ((owner, _, _), plans) in groups {
            let sequential = owner == StorageOwner::Frame as u8;
            let mut deferred = Vec::new();
            for plan in &plans {
                self.add_declaration_action(
                    &lookup,
                    plan,
                    InitializationStage::Default,
                    InitializationTrigger::Ordinary,
                )?;
                if plan.expression.is_some() {
                    let global_fb = owner == StorageOwner::Global as u8
                        && crate::harness::function_block_type_name(
                            plan.type_id,
                            self.runtime.registry(),
                        )
                        .is_some();
                    if sequential || global_fb {
                        self.add_declaration_action(
                            &lookup,
                            plan,
                            InitializationStage::Explicit,
                            InitializationTrigger::Ordinary,
                        )?;
                    } else {
                        deferred.push(plan);
                    }
                }
            }
            for plan in deferred {
                self.add_declaration_action(
                    &lookup,
                    plan,
                    InitializationStage::Explicit,
                    InitializationTrigger::Ordinary,
                )?;
            }
        }
        for plan in self.construction.plans.clone() {
            let declaration = &self.construction.layout.entries[plan.declaration as usize];
            if declaration.owner == StorageOwner::Global && declaration.role == StorageRole::Static
            {
                self.add_declaration_action(
                    &lookup,
                    &plan,
                    InitializationStage::Default,
                    InitializationTrigger::AfterRestart,
                )?;
                if plan.expression.is_some() {
                    self.add_declaration_action(
                        &lookup,
                        &plan,
                        InitializationStage::Explicit,
                        InitializationTrigger::AfterRestart,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn add_declaration_action(
        &mut self,
        lookup: &ActionIndex<'_>,
        plan: &DeclarationPlan,
        stage: InitializationStage,
        trigger: InitializationTrigger,
    ) -> Result<(), BytecodeError> {
        let declaration = self.construction.layout.entries[plan.declaration as usize].clone();
        let owner = declaration
            .owner_pou_id
            .and_then(|id| lookup.pous.get(&id).copied());
        let phase = match (declaration.owner, declaration.role) {
            (_, StorageRole::Static) => InitializationPhase::Static,
            (StorageOwner::Frame, StorageRole::Return) => InitializationPhase::Return,
            (StorageOwner::Frame, StorageRole::Parameter) => InitializationPhase::Parameter,
            (StorageOwner::Frame, _) => InitializationPhase::Frame,
            (StorageOwner::Instance, _) => InitializationPhase::Instance,
            (StorageOwner::Global, _) => InitializationPhase::Resource,
        };
        let once = match (declaration.owner, declaration.role) {
            (StorageOwner::Global, StorageRole::Static) => InitializationOnce::Module,
            (StorageOwner::Instance, StorageRole::Static) => InitializationOnce::Instance,
            _ => InitializationOnce::None,
        };
        let visible_local_count = match phase {
            InitializationPhase::Frame | InitializationPhase::Parameter => declaration.slot,
            InitializationPhase::Static if trigger == InitializationTrigger::AfterRestart => owner
                .map_or(0, |pou| {
                    u32::from(pou.return_type_id.is_some()) + pou.params.len() as u32
                }),
            _ => 0,
        };
        let visible_static_count = if declaration.role == StorageRole::Static
            && trigger == InitializationTrigger::Ordinary
        {
            0
        } else {
            lookup
                .static_positions
                .get(&declaration.owner_pou_id)
                .map_or(0, |positions| {
                    if declaration.role == StorageRole::Static {
                        positions.partition_point(|position| *position < plan.declaration as usize)
                            as u32
                    } else {
                        positions.len() as u32
                    }
                })
        };
        let id = self.reserve_initializer_result(
            plan.type_id,
            InitializerEntry {
                declaration_idx: Some(plan.declaration),
                owner_pou_id: declaration.owner_pou_id,
                result_ref_idx: 0,
                code_offset: 0,
                code_length: 0,
                visible_local_count,
                visible_static_count,
                phase,
                once,
                stage,
                trigger,
                target_idx: None,
                partial_kind: 0,
                target_kind: InitializationTarget::Declaration,
                target_reserved: [0; 2],
                partial_index: 0,
                context_initializer_idx: None,
                recipe_type_id: None,
                recipe_member_idx: None,
                body_kind: InitializerBodyKind::Action,
                recipe_reserved: [0; 3],
            },
        )?;
        let mut context = declaration
            .owner_pou_id
            .and_then(|owner| self.construction.contexts.get(&owner))
            .cloned()
            .unwrap_or_default();
        // Source-free bodies resolve the current instance dynamically; a reserved
        // concrete root must not bypass the construction visibility frontier.
        context.instance_id = None;
        if declaration.role == StorageRole::Static && trigger == InitializationTrigger::Ordinary {
            context.locals.clear();
            context.static_refs.clear();
            context.static_self_fields.clear();
        }
        if declaration.owner == StorageOwner::Instance {
            let template = owner.map(|pou| {
                if pou.kind == PouKind::Method {
                    pou.owner_pou_id.unwrap_or(pou.id)
                } else {
                    pou.id
                }
            });
            if let Some(fields) = template
                .and_then(|template| self.construction.templates.template_fields.get(&template))
            {
                let limit = if stage == InitializationStage::Default {
                    declaration.slot
                } else {
                    template
                        .and_then(|owner| lookup.group_ends.get(&(owner, declaration.role as u8)))
                        .copied()
                        .unwrap_or(declaration.slot + 1)
                };
                let unavailable: std::collections::HashSet<_> = fields
                    .iter()
                    .map(|id| &self.construction.layout.entries[*id as usize])
                    .filter(|field| field.slot >= limit)
                    .map(|field| normalize_name(&self.strings.entries[field.name_idx as usize]))
                    .collect();
                let mut inherited = std::collections::HashSet::new();
                let mut parent = template.and_then(|template| {
                    self.construction
                        .templates
                        .template_parents
                        .get(&template)
                        .copied()
                });
                for _ in 0..64 {
                    let Some(id) = parent else {
                        break;
                    };
                    for field in self
                        .construction
                        .templates
                        .template_fields
                        .get(&id)
                        .into_iter()
                        .flatten()
                    {
                        let field = &self.construction.layout.entries[*field as usize];
                        inherited.insert(normalize_name(
                            &self.strings.entries[field.name_idx as usize],
                        ));
                    }
                    parent = self
                        .construction
                        .templates
                        .template_parents
                        .get(&id)
                        .copied();
                }
                context.self_fields.retain(|_, name| {
                    let name = normalize_name(name);
                    !unavailable.contains(&name) || inherited.contains(&name)
                });
            }
        }
        self.construction
            .bodies
            .initializer_contexts
            .insert(id, context);
        let intrinsic_only = stage == InitializationStage::Default
            && (declaration.role == StorageRole::Return
                || (plan.expression.is_some()
                    && (declaration.owner == StorageOwner::Frame
                        || trigger == InitializationTrigger::AfterRestart)
                    && crate::harness::function_block_type_name(
                        plan.type_id,
                        self.runtime.registry(),
                    )
                    .is_none()));
        if declaration.role != StorageRole::EdgePhase && !intrinsic_only {
            self.collect_default_recipes(id, plan.type_id)?;
            self.construction
                .bodies
                .pending_bodies
                .push(PendingInitializerBody {
                    id,
                    type_id: plan.type_id,
                    expression: if stage == InitializationStage::Explicit {
                        plan.expression.clone()
                    } else {
                        None
                    },
                    coerce_opcode: crate::bytecode::opcodes::APPLY_INIT_VALUE,
                });
        }
        Ok(())
    }
}

/// Index immutable owner/signature and declaration order once for all actions.
struct ActionIndex<'a> {
    pous: HashMap<u32, &'a crate::bytecode::PouEntry>,
    static_positions: HashMap<Option<u32>, Vec<usize>>,
    group_ends: HashMap<(u32, u8), u32>,
}

impl<'a> ActionIndex<'a> {
    fn new(index: &'a PouIndex, model: &ConstructionModel) -> Self {
        let pous = index.entries.iter().map(|pou| (pou.id, pou)).collect();
        let mut static_positions: HashMap<_, Vec<_>> = HashMap::new();
        for (position, declaration) in model.layout.entries.iter().enumerate() {
            if declaration.role == StorageRole::Static {
                static_positions
                    .entry(declaration.owner_pou_id)
                    .or_default()
                    .push(position);
            }
        }
        let mut group_ends: HashMap<_, u32> = HashMap::new();
        for (owner, fields) in &model.templates.template_fields {
            for field in fields {
                let declaration = &model.layout.entries[*field as usize];
                let end = group_ends
                    .entry((*owner, declaration.role as u8))
                    .or_default();
                *end = (*end).max(declaration.slot + 1);
            }
        }
        Self {
            pous,
            static_positions,
            group_ends,
        }
    }
}
