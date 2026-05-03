pub(super) fn apply_program_retain_overrides(
    program_defs: &mut IndexMap<SmolStr, ProgramDef>,
    programs: &[ProgramInstanceConfig],
    using: &[SmolStr],
) -> Result<(), CompileError> {
    let mut retain_by_type: std::collections::HashMap<SmolStr, crate::RetainPolicy> =
        std::collections::HashMap::new();
    for program in programs {
        let Some(policy) = program.retain else {
            continue;
        };
        let type_name = super::resolve_program_type_name(program_defs, &program.type_name, using)?;
        if let Some(existing) = retain_by_type.insert(type_name.clone(), policy) {
            if existing != policy {
                return Err(CompileError::new(
                    "conflicting RETAIN/NON_RETAIN qualifiers for program type",
                ));
            }
        }
    }

    for (type_name, policy) in retain_by_type {
        let key = SmolStr::new(type_name.to_ascii_uppercase());
        let Some(program) = program_defs.get_mut(&key) else {
            continue;
        };
        for var in &mut program.vars {
            if matches!(var.retain, crate::RetainPolicy::Unspecified) {
                var.retain = policy;
            }
        }
    }
    Ok(())
}

pub(super) fn register_program_instances(
    runtime: &mut Runtime,
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
    let mut seen_types = std::collections::HashSet::new();
    for program in programs {
        let instance_key = program.name.to_ascii_uppercase();
        if !seen_instances.insert(instance_key.clone()) {
            return Err(CompileError::new(format!(
                "duplicate PROGRAM instance name '{}'",
                program.name
            )));
        }
        let type_name = super::resolve_program_type_name(program_defs, &program.type_name, using)?;
        let type_key = type_name.to_ascii_uppercase();
        if !seen_types.insert(type_key.clone()) {
            return Err(CompileError::new(
                "multiple instances of the same PROGRAM type are not supported yet",
            ));
        }
        let def_key = SmolStr::new(type_key);
        let def = program_defs
            .get(&def_key)
            .ok_or_else(|| CompileError::new("unknown PROGRAM type"))?;
        let mut instance = def.clone();
        instance.name = program.name.clone();
        runtime
            .register_program(instance.clone())
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
    runtime: &Runtime,
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
