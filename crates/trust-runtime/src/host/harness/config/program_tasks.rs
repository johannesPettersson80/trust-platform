/// Gives each unqualified variable of a program instance the instance's `RETAIN` or `NON_RETAIN`
/// (IEC 61131-3 §6.5.6.2), but a function block or class instance, which follows its own type's
/// declarations. Each instance has its own qualifier, so that two instances of one program type
/// may differ.
fn apply_instance_retain(
    instance: &mut ProgramDef,
    policy: crate::RetainPolicy,
    registry: &trust_hir::types::TypeRegistry,
) {
    for var in &mut instance.vars {
        let holds_instance = crate::harness::function_block_type_name(var.type_id, registry)
            .is_some()
            || crate::harness::class_type_name(var.type_id, registry).is_some();
        if matches!(var.retain, crate::RetainPolicy::Unspecified) && !holds_instance {
            var.retain = policy;
        }
    }
}

pub(super) fn register_program_instances(
    runtime: &mut RuntimeBuilder,
    program_defs: &IndexMap<SmolStr, ProgramDef>,
    programs: &[ProgramInstanceConfig],
    using: &[SmolStr],
    wildcards: &mut Vec<WildcardRequirement>,
) -> Result<(), CompileError> {
    let registry = runtime.registry().clone();
    let function_blocks = runtime.function_blocks().clone();
    let mut bindings = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut seen_instances = std::collections::HashSet::new();
    for program in programs {
        let instance_key = program.name.to_ascii_uppercase();
        if !seen_instances.insert(instance_key.clone()) {
            return Err(CompileError::new(format!(
                "duplicate PROGRAM instance name '{}'",
                program.name
            )));
        }
        // A program type may have several instances, each with its own variables (IEC 61131-3
        // §6.8.2, Table 62 6a).
        let type_name = super::resolve_program_type_name(program_defs, &program.type_name, using)?;
        let def_key = SmolStr::new(type_name.to_ascii_uppercase());
        let def = program_defs
            .get(&def_key)
            .ok_or_else(|| CompileError::new("unknown PROGRAM type"))?;
        let mut instance = def.clone();
        instance.name = program.name.clone();
        if let Some(policy) = program.retain {
            apply_instance_retain(&mut instance, policy, &registry);
        }
        runtime
            .instantiate_program(instance.clone())
            .map_err(|err| CompileError::new(format!("PROGRAM init error: {err}")))?;
        let instance_id = match runtime.storage().get_global(program.name.as_ref()) {
            Some(Value::Instance(id)) => *id,
            _ => {
                return Err(CompileError::new(
                    "failed to resolve program instance storage",
                ))
            }
        };
        collect_program_instance_bindings(
            &registry,
            runtime.storage(),
            &function_blocks,
            &instance,
            instance_id,
            &program.name,
            wildcards,
            &mut visited,
            &mut bindings,
        )?;
    }
    if !bindings.is_empty() {
        let io = runtime.io_mut();
        for binding in bindings {
            bind_value_ref_to_address(
                io,
                &registry,
                binding.reference,
                binding.type_id,
                &binding.address,
                Some(binding.display_name),
            )?;
        }
    }
    Ok(())
}

pub(super) fn attach_programs_to_tasks(
    tasks: &mut [crate::task::TaskConfig],
    programs: &[ProgramInstanceConfig],
) -> Result<(), CompileError> {
    let mut task_map = std::collections::HashMap::new();
    for (idx, task) in tasks.iter().enumerate() {
        task_map.insert(task.name.to_ascii_uppercase(), idx);
    }
    for program in programs {
        if let Some(task_name) = &program.task {
            let key = task_name.to_ascii_uppercase();
            let Some(&idx) = task_map.get(&key) else {
                return Err(CompileError::new(format!(
                    "unknown TASK '{}' for program '{}'",
                    task_name, program.name
                )));
            };
            let task = &mut tasks[idx];
            task.programs.push(program.name.clone());
        }
    }
    Ok(())
}

pub(super) fn attach_fb_instances_to_tasks(
    runtime: &RuntimeBuilder,
    tasks: &mut [crate::task::TaskConfig],
    programs: &[ProgramInstanceConfig],
) -> Result<(), CompileError> {
    let mut task_map = std::collections::HashMap::new();
    for (idx, task) in tasks.iter().enumerate() {
        task_map.insert(task.name.to_ascii_uppercase(), idx);
    }

    for program in programs {
        for fb_task in &program.fb_tasks {
            let key = fb_task.task.to_ascii_uppercase();
            let Some(&idx) = task_map.get(&key) else {
                return Err(CompileError::new(format!(
                    "unknown TASK '{}' for FB task binding",
                    fb_task.task
                )));
            };
            let parts = match &fb_task.path {
                AccessPath::Direct { .. } => {
                    return Err(CompileError::new(
                        "direct addresses are not valid FB task bindings",
                    ))
                }
                AccessPath::Parts(parts) => parts.clone(),
            };
            let mut full_parts = Vec::with_capacity(parts.len() + 1);
            full_parts.push(AccessPart::Name(program.name.clone()));
            full_parts.extend(parts);
            let resolved = resolve_access_parts(runtime, &full_parts)?;
            let reference = match resolved {
                ResolvedAccess::Variable { reference, .. } => reference,
                ResolvedAccess::Direct(_) => {
                    return Err(CompileError::new(
                        "direct address cannot be used for FB task binding",
                    ))
                }
            };
            let value = runtime
                .storage()
                .read_by_ref(reference.clone())
                .cloned()
                .ok_or_else(|| CompileError::new("invalid FB task reference"))?;
            let instance_id = match value {
                Value::Instance(id) => id,
                _ => {
                    return Err(CompileError::new(
                        "FB task binding must reference a function block instance",
                    ))
                }
            };
            let instance = runtime
                .storage()
                .get_instance(instance_id)
                .ok_or_else(|| CompileError::new("invalid FB task instance"))?;
            let key = SmolStr::new(instance.type_name.to_ascii_uppercase());
            if runtime.function_blocks().get(&key).is_none() {
                return Err(CompileError::new(
                    "FB task binding must reference a function block instance",
                ));
            }
            let task = &mut tasks[idx];
            task.fb_instances.push(reference);
        }
    }
    Ok(())
}
