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
        let storage = runtime.storage_mut();

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
                    storage,
                    &registry,
                    &profile,
                    &classes,
                    &function_blocks,
                    &functions,
                    &stdlib,
                    fb,
                )
                .map_err(|err| CompileError::new(err.to_string()))?;
                storage.set_global(init.name.clone(), Value::Instance(instance_id));
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
                    storage,
                    &registry,
                    &profile,
                    &classes,
                    &function_blocks,
                    &functions,
                    &stdlib,
                    class_def,
                )
                .map_err(|err| CompileError::new(err.to_string()))?;
                storage.set_global(init.name.clone(), Value::Instance(instance_id));
                continue;
            }
            if super::interface_type_name(init.type_id, &registry).is_some() {
                storage.set_global(init.name.clone(), Value::Null);
                continue;
            }
            let value = default_value_for_type_id(init.type_id, &registry, &profile)
                .map_err(|err| CompileError::new(format!("default value error: {err:?}")))?;
            storage.set_global(init.name.clone(), value);
        }

        for init in globals {
            if let Some(expr) = &init.initializer {
                if super::function_block_type_name(init.type_id, &registry).is_some()
                    || super::class_type_name(init.type_id, &registry).is_some()
                {
                    continue;
                }
                let value = crate::helper_eval::eval_storage_expr(
                    storage,
                    &registry,
                    &profile,
                    None,
                    expr,
                )
                .map_err(|err| CompileError::new(format!("initializer error: {err}")))?;
                let value = super::coerce_value_to_type(value, init.type_id)?;
                storage.set_global(init.name.clone(), value);
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
