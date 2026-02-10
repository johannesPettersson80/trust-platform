use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::eval::{eval_expr, EvalContext};
use crate::instance::{create_class_instance, create_fb_instance};
use crate::task::ProgramDef;
use crate::value::{default_value_for_type_id, Value};
use crate::Runtime;

use super::io::{
    bind_value_ref_to_address, collect_direct_field_bindings, collect_instance_bindings,
    collect_program_instance_bindings,
};
use super::{
    AccessDecl, AccessPart, AccessPath, CompileError, ConfigInit, GlobalInit,
    ProgramInstanceConfig, ResolvedAccess, WildcardRequirement,
};

pub(super) fn access_path_display(path: &AccessPath) -> SmolStr {
    match path {
        AccessPath::Direct { text, .. } => text.clone(),
        AccessPath::Parts(parts) => {
            let mut out = String::new();
            for part in parts {
                match part {
                    AccessPart::Name(name) => {
                        if !out.is_empty() {
                            out.push('.');
                        }
                        out.push_str(name);
                    }
                    AccessPart::Index(indices) => {
                        out.push('[');
                        for (idx, index) in indices.iter().enumerate() {
                            if idx > 0 {
                                out.push_str(", ");
                            }
                            out.push_str(&index.to_string());
                        }
                        out.push(']');
                    }
                    AccessPart::Partial(partial) => {
                        let (prefix, index) = match partial {
                            crate::value::PartialAccess::Bit(index) => ("X", *index),
                            crate::value::PartialAccess::Byte(index) => ("B", *index),
                            crate::value::PartialAccess::Word(index) => ("W", *index),
                            crate::value::PartialAccess::DWord(index) => ("D", *index),
                        };
                        out.push_str(".%");
                        out.push_str(prefix);
                        out.push_str(&index.to_string());
                    }
                }
            }
            SmolStr::new(out)
        }
    }
}

pub(super) fn resolve_access_path(
    runtime: &Runtime,
    path: &AccessPath,
) -> Result<ResolvedAccess, CompileError> {
    match path {
        AccessPath::Direct { address, .. } => Ok(ResolvedAccess::Direct(address.clone())),
        AccessPath::Parts(parts) => resolve_access_parts(runtime, parts),
    }
}

fn resolve_access_parts(
    runtime: &Runtime,
    parts: &[AccessPart],
) -> Result<ResolvedAccess, CompileError> {
    let name_positions: Vec<(usize, &SmolStr)> = parts
        .iter()
        .enumerate()
        .filter_map(|(idx, part)| match part {
            AccessPart::Name(name) => Some((idx, name)),
            _ => None,
        })
        .collect();

    for (pos, name) in name_positions {
        let mut value_ref = if let Some(reference) = runtime.storage().ref_for_global(name.as_ref())
        {
            reference
        } else {
            let mut matched = None;
            for program in runtime.programs().values() {
                let Some(Value::Instance(id)) = runtime.storage().get_global(program.name.as_ref())
                else {
                    continue;
                };
                let Some(reference) = runtime.storage().ref_for_instance(*id, name.as_ref()) else {
                    continue;
                };
                if matched.is_some() {
                    matched = None;
                    break;
                }
                matched = Some(reference);
            }
            let Some(reference) = matched else {
                continue;
            };
            reference
        };
        let mut current_value = runtime
            .storage()
            .read_by_ref(value_ref.clone())
            .cloned()
            .ok_or_else(|| CompileError::new("invalid access path reference"))?;
        let mut partial = None;

        for part in &parts[pos + 1..] {
            match part {
                AccessPart::Index(indices) => {
                    value_ref
                        .path
                        .push(crate::value::RefSegment::Index(indices.clone()));
                    current_value = runtime
                        .storage()
                        .read_by_ref(value_ref.clone())
                        .cloned()
                        .ok_or_else(|| CompileError::new("invalid access path index"))?;
                }
                AccessPart::Name(field) => {
                    if let Value::Instance(id) = current_value {
                        value_ref = runtime
                            .storage()
                            .ref_for_instance(id, field.as_ref())
                            .ok_or_else(|| {
                                CompileError::new("invalid access path instance field")
                            })?;
                        current_value = runtime
                            .storage()
                            .read_by_ref(value_ref.clone())
                            .cloned()
                            .ok_or_else(|| CompileError::new("invalid access path"))?;
                    } else {
                        value_ref
                            .path
                            .push(crate::value::RefSegment::Field(field.clone()));
                        current_value =
                            runtime
                                .storage()
                                .read_by_ref(value_ref.clone())
                                .cloned()
                                .ok_or_else(|| CompileError::new("invalid access path field"))?;
                    }
                }
                AccessPart::Partial(access) => {
                    partial = Some(*access);
                    break;
                }
            }
        }

        return Ok(ResolvedAccess::Variable {
            reference: value_ref,
            partial,
        });
    }

    Err(CompileError::new("unresolved access path"))
}

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

pub(super) fn apply_globals(
    runtime: &mut Runtime,
    globals: &[GlobalInit],
) -> Result<Vec<WildcardRequirement>, CompileError> {
    let registry = runtime.registry().clone();
    let profile = runtime.profile();
    let functions = runtime.functions().clone();
    let stdlib = runtime.stdlib().clone();
    let function_blocks = runtime.function_blocks().clone();
    let classes = runtime.classes().clone();
    {
        let now = runtime.current_time();
        let mut ctx = EvalContext {
            storage: runtime.storage_mut(),
            registry: &registry,
            profile,
            now,
            debug: None,
            call_depth: 0,
            functions: Some(&functions),
            stdlib: Some(&stdlib),
            function_blocks: Some(&function_blocks),
            classes: Some(&classes),
            using: None,
            access: None,
            current_instance: None,
            return_name: None,
            loop_depth: 0,
            pause_requested: false,
            execution_deadline: None,
        };

        for init in globals {
            if let Some(fb_name) = super::function_block_type_name(init.type_id, &registry) {
                if init.initializer.is_some() {
                    return Err(CompileError::new(
                        "function block instances cannot have initializers",
                    ));
                }
                let key = SmolStr::new(fb_name.to_ascii_uppercase());
                let fb = function_blocks.get(&key).ok_or_else(|| {
                    CompileError::new(format!("unknown function block '{fb_name}'"))
                })?;
                let instance_id = create_fb_instance(
                    ctx.storage,
                    &registry,
                    &profile,
                    &classes,
                    &function_blocks,
                    &functions,
                    &stdlib,
                    fb,
                )
                .map_err(|err| CompileError::new(err.to_string()))?;
                ctx.storage
                    .set_global(init.name.clone(), Value::Instance(instance_id));
                continue;
            }
            if let Some(class_name) = super::class_type_name(init.type_id, &registry) {
                if init.initializer.is_some() {
                    return Err(CompileError::new(
                        "class instances cannot have initializers",
                    ));
                }
                let key = SmolStr::new(class_name.to_ascii_uppercase());
                let class_def = classes
                    .get(&key)
                    .ok_or_else(|| CompileError::new(format!("unknown class '{class_name}'")))?;
                let instance_id = create_class_instance(
                    ctx.storage,
                    &registry,
                    &profile,
                    &classes,
                    &function_blocks,
                    &functions,
                    &stdlib,
                    class_def,
                )
                .map_err(|err| CompileError::new(err.to_string()))?;
                ctx.storage
                    .set_global(init.name.clone(), Value::Instance(instance_id));
                continue;
            }
            if super::interface_type_name(init.type_id, &registry).is_some() {
                ctx.storage.set_global(init.name.clone(), Value::Null);
                continue;
            }
            let value = default_value_for_type_id(init.type_id, &registry, &profile)
                .map_err(|err| CompileError::new(format!("default value error: {err:?}")))?;
            ctx.storage.set_global(init.name.clone(), value);
        }

        for init in globals {
            if let Some(expr) = &init.initializer {
                if super::function_block_type_name(init.type_id, &registry).is_some()
                    || super::class_type_name(init.type_id, &registry).is_some()
                {
                    continue;
                }
                ctx.using = Some(&init.using);
                let value = eval_expr(&mut ctx, expr)
                    .map_err(|err| CompileError::new(format!("initializer error: {err}")))?;
                let value = super::coerce_value_to_type(value, init.type_id)?;
                ctx.storage.set_global(init.name.clone(), value);
            }
        }
    }

    let mut wildcards = Vec::new();
    let mut bindings = Vec::new();
    for init in globals {
        if let Some(address) = init.address.as_ref() {
            let parsed = crate::io::IoAddress::parse(address)
                .map_err(|err| CompileError::new(format!("invalid I/O address: {err}")))?;
            let reference = runtime
                .storage()
                .ref_for_global(init.name.as_ref())
                .ok_or_else(|| CompileError::new("failed to resolve global for I/O binding"))?;
            if parsed.wildcard {
                wildcards.push(WildcardRequirement {
                    name: init.name.clone(),
                    reference,
                    area: parsed.area,
                });
            } else {
                let io = runtime.io_mut();
                bind_value_ref_to_address(
                    io,
                    &registry,
                    reference,
                    init.type_id,
                    &parsed,
                    Some(init.name.clone()),
                )?;
            }
        } else {
            let reference = runtime
                .storage()
                .ref_for_global(init.name.as_ref())
                .ok_or_else(|| CompileError::new("failed to resolve global for I/O binding"))?;
            collect_direct_field_bindings(
                &registry,
                &reference,
                init.type_id,
                &init.name,
                &mut wildcards,
                &mut bindings,
            )?;
        }
        if let Some(fb_name) = super::function_block_type_name(init.type_id, &registry) {
            runtime.register_global_meta(
                init.name.clone(),
                init.type_id,
                init.retain,
                crate::GlobalInitValue::FunctionBlock { type_name: fb_name },
            );
            continue;
        }
        if let Some(class_name) = super::class_type_name(init.type_id, &registry) {
            runtime.register_global_meta(
                init.name.clone(),
                init.type_id,
                init.retain,
                crate::GlobalInitValue::Class {
                    type_name: class_name,
                },
            );
            continue;
        }
        let value = runtime
            .storage()
            .get_global(init.name.as_ref())
            .cloned()
            .unwrap_or(Value::Null);
        runtime.register_global_meta(
            init.name.clone(),
            init.type_id,
            init.retain,
            crate::GlobalInitValue::Value(value),
        );
    }

    let mut visited = std::collections::HashSet::new();
    for init in globals {
        if super::function_block_type_name(init.type_id, &registry).is_none() {
            continue;
        }
        let instance_id = match runtime.storage().get_global(init.name.as_ref()) {
            Some(Value::Instance(id)) => *id,
            _ => {
                return Err(CompileError::new(format!(
                    "failed to resolve function block instance '{}'",
                    init.name
                )))
            }
        };
        collect_instance_bindings(
            &registry,
            runtime.storage(),
            &function_blocks,
            instance_id,
            &init.name,
            &mut wildcards,
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

    Ok(wildcards)
}

pub(super) fn apply_config_inits(
    runtime: &mut Runtime,
    config_inits: &[ConfigInit],
    using: &[SmolStr],
    wildcards: &mut Vec<WildcardRequirement>,
) -> Result<(), CompileError> {
    if config_inits.is_empty() {
        return Ok(());
    }
    let registry = runtime.registry().clone();
    let profile = runtime.profile();
    let functions = runtime.functions().clone();
    let stdlib = runtime.stdlib().clone();
    let function_blocks = runtime.function_blocks().clone();
    let classes = runtime.classes().clone();

    for init in config_inits {
        let resolved = resolve_access_path(runtime, &init.path)?;

        if let Some(address) = &init.address {
            if address.wildcard {
                return Err(CompileError::new(
                    "VAR_CONFIG AT address must be fully specified",
                ));
            }
            match &resolved {
                ResolvedAccess::Variable { reference, partial } => {
                    if partial.is_some() {
                        return Err(CompileError::new(
                            "AT binding not allowed on partial access",
                        ));
                    }
                    if let Some(pos) = wildcards.iter().position(|req| req.reference == *reference)
                    {
                        let requirement = &wildcards[pos];
                        if requirement.area != address.area {
                            return Err(CompileError::new(format!(
                                "VAR_CONFIG address area mismatch for '{}'",
                                requirement.name
                            )));
                        }
                        wildcards.remove(pos);
                    }
                    let display_name = access_path_display(&init.path);
                    let io = runtime.io_mut();
                    bind_value_ref_to_address(
                        io,
                        &registry,
                        reference.clone(),
                        init.type_id,
                        address,
                        Some(display_name),
                    )?;
                }
                ResolvedAccess::Direct(_) => {
                    return Err(CompileError::new(
                        "VAR_CONFIG AT binding must target a variable",
                    ));
                }
            }
        }

        let Some(expr) = &init.initializer else {
            continue;
        };

        let value = {
            let now = runtime.current_time();
            let mut ctx = EvalContext {
                storage: runtime.storage_mut(),
                registry: &registry,
                profile,
                now,
                debug: None,
                call_depth: 0,
                functions: Some(&functions),
                stdlib: Some(&stdlib),
                function_blocks: Some(&function_blocks),
                classes: Some(&classes),
                using: Some(using),
                access: None,
                current_instance: None,
                return_name: None,
                loop_depth: 0,
                pause_requested: false,
                execution_deadline: None,
            };
            let value = eval_expr(&mut ctx, expr)
                .map_err(|err| CompileError::new(format!("VAR_CONFIG initializer error: {err}")))?;
            super::coerce_value_to_type(value, init.type_id)?
        };

        match resolved {
            ResolvedAccess::Variable { reference, partial } => {
                let storage = runtime.storage_mut();
                if let Some(access) = partial {
                    let current = storage
                        .read_by_ref(reference.clone())
                        .cloned()
                        .ok_or_else(|| CompileError::new("invalid VAR_CONFIG target"))?;
                    let updated = crate::value::write_partial_access(current, access, value)
                        .map_err(|_| CompileError::new("invalid VAR_CONFIG partial access"))?;
                    if !storage.write_by_ref(reference, updated) {
                        return Err(CompileError::new("invalid VAR_CONFIG target"));
                    }
                } else if !storage.write_by_ref(reference, value) {
                    return Err(CompileError::new("invalid VAR_CONFIG target"));
                }
            }
            ResolvedAccess::Direct(address) => {
                runtime
                    .io_mut()
                    .write(&address, value)
                    .map_err(|err| CompileError::new(err.to_string()))?;
            }
        }
    }
    Ok(())
}

pub(super) fn ensure_wildcards_resolved(
    wildcards: &[WildcardRequirement],
) -> Result<(), CompileError> {
    if wildcards.is_empty() {
        return Ok(());
    }
    let mut names: Vec<String> = wildcards.iter().map(|req| req.name.to_string()).collect();
    names.sort();
    names.dedup();
    let joined = names.join(", ");
    Err(CompileError::new(format!(
        "missing VAR_CONFIG address for wildcard variables: {joined}"
    )))
}

pub(super) fn register_access_bindings(
    runtime: &mut Runtime,
    access_decls: &[AccessDecl],
) -> Result<(), CompileError> {
    for decl in access_decls {
        let resolved = resolve_access_path(runtime, &decl.path)?;
        match resolved {
            ResolvedAccess::Variable { reference, partial } => {
                runtime
                    .access_map_mut()
                    .bind(decl.name.clone(), reference, partial);
            }
            ResolvedAccess::Direct(_) => {
                return Err(CompileError::new(
                    "VAR_ACCESS direct addresses must be declared as globals",
                ));
            }
        }
    }
    Ok(())
}
