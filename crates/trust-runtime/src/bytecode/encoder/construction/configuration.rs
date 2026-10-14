//! Configuration aliases, task bindings and startup overrides share structural path resolution.

use super::*;
use crate::bytecode::{
    AccessBindingEntry, InitializationOnce, InitializationPhase, InitializationStage,
    InitializationTarget, InitializerBodyKind, InitializerEntry,
};
use crate::harness::{AccessPart, AccessPath};
use crate::value::PartialAccess;

impl BytecodeEncoder<'_> {
    pub(in crate::bytecode::encoder) fn collect_source_configuration(
        &mut self,
    ) -> Result<(), BytecodeError> {
        let input = self
            .authoring
            .ok_or_else(|| invalid("source input missing"))?;
        if let Some(config) = &input.configuration {
            self.construction.bindings.configuration_name = config.configuration_name.clone();
            self.construction.bindings.resource_name = config.resource_name.clone();
        }
        let roots = self.source_program_roots();
        self.collect_source_tasks(input, &roots)?;
        self.collect_source_aliases(input, &roots)?;
        self.collect_config_initializers(input, &roots)?;
        if !self.construction.wildcards.is_empty() {
            return Err(invalid("unresolved wildcard I/O bindings"));
        }
        Ok(())
    }
    fn collect_source_tasks(
        &mut self,
        input: &crate::harness::LoweredApplication,
        roots: &[ValueRef],
    ) -> Result<(), BytecodeError> {
        let config = input
            .configuration
            .as_ref()
            .ok_or_else(|| invalid("configuration missing"))?;
        self.construction.tasks = config.tasks.clone();
        for program in &config.programs {
            for binding in &program.fb_tasks {
                let AccessPath::Parts(parts) = &binding.path else {
                    return Err(invalid("direct addresses are not valid FB task bindings"));
                };
                let mut full = vec![AccessPart::Name(program.name.clone())];
                full.extend(parts.iter().cloned());
                let target = self.construction.bindings.resolve_parts(
                    &full,
                    roots,
                    self.runtime.registry(),
                )?;
                if target.partial.is_some()
                    || target
                        .type_id
                        .and_then(|ty| {
                            crate::harness::function_block_type_name(ty, self.runtime.registry())
                        })
                        .is_none()
                {
                    return Err(invalid(
                        "FB task binding must reference a function block instance",
                    ));
                }
                let task = self
                    .construction
                    .tasks
                    .iter_mut()
                    .find(|task| task.name.eq_ignore_ascii_case(&binding.task))
                    .ok_or_else(|| invalid("unknown task in FB binding"))?;
                task.fb_instances.push(target.reference);
            }
        }
        for (task_name, parts) in &input.builder.manual_task_bindings {
            let target =
                self.construction
                    .bindings
                    .resolve_parts(parts, roots, self.runtime.registry())?;
            if target.partial.is_some()
                || target
                    .type_id
                    .and_then(|ty| {
                        crate::harness::function_block_type_name(ty, self.runtime.registry())
                    })
                    .is_none()
            {
                return Err(invalid(
                    "FB task binding must reference a function block instance",
                ));
            }
            let task = self
                .construction
                .tasks
                .iter_mut()
                .find(|task| task.name.eq_ignore_ascii_case(task_name))
                .ok_or_else(|| invalid("unknown task in builder binding"))?;
            task.fb_instances.push(target.reference);
        }
        Ok(())
    }

    fn collect_source_aliases(
        &mut self,
        input: &crate::harness::LoweredApplication,
        roots: &[ValueRef],
    ) -> Result<(), BytecodeError> {
        let config = input
            .configuration
            .as_ref()
            .ok_or_else(|| invalid("configuration missing"))?;
        for alias in config.access.iter().chain(&input.program_access) {
            if let crate::harness::AccessTargetPolicy::Forbidden(section) =
                source_access_target_policy(input, &alias.path)
            {
                return Err(invalid(&format!("VAR_ACCESS cannot expose {section}")));
            }
            if self.construction.bindings.global(&alias.name).is_some() {
                return Err(invalid("access alias conflicts with global"));
            }
            let AccessPath::Parts(parts) = &alias.path else {
                return Err(invalid(
                    "VAR_ACCESS direct addresses must be declared as globals",
                ));
            };
            let target =
                self.construction
                    .bindings
                    .resolve_parts(parts, roots, self.runtime.registry())?;
            let declaration_id = self.declaration_for_binding(&target.reference)?;
            let declaration = &self.construction.layout.entries[declaration_id as usize];
            if declaration.flags & 8 != 0 {
                return Err(invalid("VAR_ACCESS cannot expose VAR_IN_OUT"));
            }
            let writable = alias.writable && declaration.flags & 1 == 0;
            let value_type = target
                .type_id
                .ok_or_else(|| invalid("access alias value type missing"))?;
            self.construction
                .bindings
                .types
                .insert(target.reference.clone(), value_type);
            let selected_type = partial_type(target.partial).unwrap_or(value_type);
            let ref_idx = self.ref_index_for(&target.reference)?;
            let type_id = self.type_index(selected_type)?;
            let name_idx = self.strings.intern(alias.name.clone());
            let (partial_kind, partial_index) = partial_fields(target.partial);
            self.construction.access.entries.push(AccessBindingEntry {
                name_idx,
                type_id,
                ref_idx,
                partial_kind,
                flags: u8::from(writable),
                reserved: 0,
                partial_index,
            });
            self.construction
                .aliases
                .insert(normalize_name(&alias.name), target);
        }
        Ok(())
    }

    fn collect_config_initializers(
        &mut self,
        input: &crate::harness::LoweredApplication,
        roots: &[ValueRef],
    ) -> Result<(), BytecodeError> {
        let config = input
            .configuration
            .as_ref()
            .ok_or_else(|| invalid("configuration missing"))?;
        for init in &config.config_inits {
            match source_access_target_policy(input, &init.path) {
                crate::harness::AccessTargetPolicy::Writable => {}
                crate::harness::AccessTargetPolicy::ReadOnly(reason)
                | crate::harness::AccessTargetPolicy::Forbidden(reason) => {
                    return Err(invalid(&format!(
                        "VAR_CONFIG cannot initialize this variable section ({reason})"
                    )));
                }
            }
            let (declaration_idx, target_idx, target_kind, partial) = match &init.path {
                AccessPath::Direct { address, text } => {
                    if address.wildcard || init.address.is_some() {
                        return Err(invalid("invalid direct configuration target"));
                    }
                    if init.initializer.is_some()
                        && let Some(end) = address
                            .flat_byte_end()
                            .map_err(|_| invalid("direct image extent overflow"))?
                    {
                        if end as usize > crate::io::PROCESS_IMAGE_AREA_LIMIT {
                            return Err(invalid("direct image limit exceeded"));
                        }
                        let area = match address.area {
                            crate::memory::IoArea::Input => 0,
                            crate::memory::IoArea::Output => 1,
                            crate::memory::IoArea::Memory => 2,
                        };
                        self.construction.direct_image_extents[area] =
                            self.construction.direct_image_extents[area].max(end);
                    }
                    (
                        None,
                        self.strings.intern(text.clone()),
                        InitializationTarget::DirectIo,
                        None,
                    )
                }
                AccessPath::Parts(parts) => {
                    let target = self.construction.bindings.resolve_parts(
                        parts,
                        roots,
                        self.runtime.registry(),
                    )?;
                    let declaration = self.declaration_for_binding(&target.reference)?;
                    if let Some(address) = &init.address {
                        if address.wildcard || target.partial.is_some() {
                            return Err(invalid("invalid configuration AT binding"));
                        }
                        if let Some(at) = self
                            .construction
                            .wildcards
                            .iter()
                            .position(|requirement| requirement.reference == target.reference)
                        {
                            if self.construction.wildcards[at].area != address.area {
                                return Err(invalid("VAR_CONFIG address area mismatch"));
                            }
                            self.construction.wildcards.remove(at);
                        }
                        self.bind_source_declaration(
                            target.reference.clone(),
                            init.type_id,
                            Some(address),
                            init.display_name.as_ref().unwrap_or(&"@config".into()),
                        )?;
                    }
                    let ty = target
                        .type_id
                        .ok_or_else(|| invalid("configuration target type missing"))?;
                    self.construction
                        .bindings
                        .types
                        .insert(target.reference.clone(), ty);
                    (
                        Some(declaration),
                        self.ref_index_for(&target.reference)?,
                        InitializationTarget::Reference,
                        target.partial,
                    )
                }
            };
            let Some(expression) = &init.initializer else {
                continue;
            };
            let (partial_kind, partial_index) = partial_fields(partial);
            let id = self.reserve_initializer_result(
                init.type_id,
                InitializerEntry {
                    declaration_idx,
                    owner_pou_id: None,
                    result_ref_idx: 0,
                    code_offset: 0,
                    code_length: 0,
                    visible_local_count: 0,
                    visible_static_count: 0,
                    phase: InitializationPhase::Configuration,
                    once: InitializationOnce::None,
                    stage: InitializationStage::Explicit,
                    trigger: crate::bytecode::InitializationTrigger::Ordinary,
                    target_idx: Some(target_idx),
                    partial_kind,
                    target_kind,
                    target_reserved: [0; 2],
                    partial_index,
                    context_initializer_idx: None,
                    recipe_type_id: None,
                    recipe_member_idx: None,
                    body_kind: InitializerBodyKind::Action,
                    recipe_reserved: [0; 3],
                },
            )?;
            self.collect_default_recipes(id, init.type_id)?;
            self.construction
                .bodies
                .pending_bodies
                .push(PendingInitializerBody {
                    id,
                    type_id: init.type_id,
                    expression: Some(expression.clone()),
                    coerce_opcode: crate::bytecode::opcodes::APPLY_INIT_VALUE,
                });
        }
        Ok(())
    }
}

fn partial_fields(partial: Option<PartialAccess>) -> (u8, u32) {
    partial.map_or((0, 0), |value| {
        let (spec, index) = crate::bytecode::PartialAccessSpec::from_access(value);
        (spec.kind, index)
    })
}
fn partial_type(partial: Option<PartialAccess>) -> Option<TypeId> {
    let (spec, _) = crate::bytecode::PartialAccessSpec::from_access(partial?);
    match spec.result_primitive {
        1 => Some(TypeId::BOOL),
        2 => Some(TypeId::BYTE),
        3 => Some(TypeId::WORD),
        4 => Some(TypeId::DWORD),
        _ => None,
    }
}

fn source_access_target_policy(
    input: &crate::harness::LoweredApplication,
    path: &AccessPath,
) -> crate::harness::AccessTargetPolicy {
    let AccessPath::Parts(parts) = path else {
        return crate::harness::AccessTargetPolicy::Writable;
    };
    let mut names = parts.iter().filter_map(|part| match part {
        AccessPart::Name(name) => Some(name),
        _ => None,
    });
    let (Some(root), Some(member)) = (names.next(), names.next()) else {
        return crate::harness::AccessTargetPolicy::Writable;
    };
    let program = input
        .configuration
        .as_ref()
        .and_then(|config| {
            config
                .programs
                .iter()
                .find(|entry| entry.name.eq_ignore_ascii_case(root))
        })
        .and_then(|entry| {
            input
                .program_defs
                .values()
                .find(|program| program.name.eq_ignore_ascii_case(&entry.type_name))
        })
        .or_else(|| {
            input
                .program_defs
                .values()
                .find(|program| program.name.eq_ignore_ascii_case(root))
        });
    program.map_or(crate::harness::AccessTargetPolicy::Writable, |program| {
        if input
            .program_externals
            .get(program.name.to_ascii_uppercase().as_str())
            .is_some_and(|names| names.iter().any(|name| name.eq_ignore_ascii_case(member)))
        {
            crate::harness::AccessTargetPolicy::Forbidden("VAR_EXTERNAL")
        } else {
            crate::harness::program_access_target_policy(program, member)
        }
    })
}
