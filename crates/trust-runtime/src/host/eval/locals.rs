pub(crate) fn static_storage_value_ref(
    ctx: &EvalContext<'_>,
    name: &SmolStr,
) -> Option<crate::value::ValueRef> {
    let (instance_id, owner) = static_storage_binding(ctx, name)?;
    let key = static_storage_name(&owner, name);
    match instance_id {
        Some(instance_id) => ctx.storage.ref_for_instance_recursive(instance_id, key.as_ref()),
        None => ctx.storage.ref_for_global(key.as_ref()),
    }
}

pub(crate) fn init_locals(
    ctx: &mut EvalContext<'_>,
    locals: &[VarDef],
) -> Result<(), RuntimeError> {
    for local in locals {
        if local.external {
            continue;
        }
        if local.static_storage {
            ensure_static_storage(ctx, local)?;
            continue;
        }
        if let Some(fb_name) = function_block_type_name(local.type_id, ctx.registry) {
            let function_blocks = ctx.function_blocks.ok_or(RuntimeError::TypeMismatch)?;
            let functions = ctx.functions.ok_or(RuntimeError::TypeMismatch)?;
            let stdlib = ctx.stdlib.ok_or(RuntimeError::TypeMismatch)?;
            let classes = ctx.classes.ok_or(RuntimeError::TypeMismatch)?;
            let key = SmolStr::new(fb_name.to_ascii_uppercase());
            let fb = function_blocks
                .get(&key)
                .ok_or(RuntimeError::UndefinedFunctionBlock(fb_name))?;
            let instance_id = create_fb_instance(
                ctx.storage,
                ctx.registry,
                &ctx.profile,
                classes,
                function_blocks,
                functions,
                stdlib,
                ctx.initializer_catalog
                    .unwrap_or(&crate::program_model::InitializerCatalog::default()),
                fb,
            )?;
            if let Some(expr) = &local.initializer {
                crate::instance::apply_fb_instance_initializer(
                    ctx.storage,
                    ctx.registry,
                    &ctx.profile,
                    stdlib,
                    ctx.initializer_catalog
                        .unwrap_or(&crate::program_model::InitializerCatalog::default()),
                    ctx.current_instance,
                    instance_id,
                    fb,
                    expr,
                )?;
            }
            ctx.storage
                .set_local(local.name.clone(), Value::Instance(instance_id));
            continue;
        }
        if let Some(class_name) = class_type_name(local.type_id, ctx.registry) {
            let function_blocks = ctx.function_blocks.ok_or(RuntimeError::TypeMismatch)?;
            let functions = ctx.functions.ok_or(RuntimeError::TypeMismatch)?;
            let stdlib = ctx.stdlib.ok_or(RuntimeError::TypeMismatch)?;
            let classes = ctx.classes.ok_or(RuntimeError::TypeMismatch)?;
            let key = SmolStr::new(class_name.to_ascii_uppercase());
            let class_def = classes.get(&key).ok_or(RuntimeError::TypeMismatch)?;
            let instance_id = create_class_instance(
                ctx.storage,
                ctx.registry,
                &ctx.profile,
                classes,
                function_blocks,
                functions,
                stdlib,
                ctx.initializer_catalog
                    .unwrap_or(&crate::program_model::InitializerCatalog::default()),
                class_def,
            )?;
            ctx.storage
                .set_local(local.name.clone(), Value::Instance(instance_id));
            continue;
        }
        let value = if let Some(expr) = &local.initializer {
            crate::harness::initializer::evaluate_initializer(
                ctx.storage,
                ctx.registry,
                ctx.initializer_catalog
                    .unwrap_or(&crate::program_model::InitializerCatalog::default()),
                &ctx.profile,
                ctx.current_instance,
                ctx.stdlib.ok_or(RuntimeError::TypeMismatch)?,
                expr,
                local.type_id,
            )?
        } else {
            crate::harness::initializer::default_value_for_type_id(
                ctx.storage,
                ctx.registry,
                ctx.initializer_catalog
                    .unwrap_or(&crate::program_model::InitializerCatalog::default()),
                &ctx.profile,
                ctx.current_instance,
                ctx.stdlib.ok_or(RuntimeError::TypeMismatch)?,
                local.type_id,
            )
            .map_err(|err| {
                let owner = current_init_owner(ctx);
                init_failed_display(&owner, &local.name, err)
            })?
        };
        ctx.storage.set_local(local.name.clone(), value);
    }
    Ok(())
}

pub(crate) fn init_locals_in_frame(
    ctx: &mut EvalContext<'_>,
    locals: &[VarDef],
) -> Result<(), RuntimeError> {
    for local in locals {
        if local.external {
            continue;
        }
        if local.static_storage {
            ensure_static_storage(ctx, local)?;
            continue;
        }
        if let Some(fb_name) = function_block_type_name(local.type_id, ctx.registry) {
            let function_blocks = ctx.function_blocks.ok_or(RuntimeError::TypeMismatch)?;
            let functions = ctx.functions.ok_or(RuntimeError::TypeMismatch)?;
            let stdlib = ctx.stdlib.ok_or(RuntimeError::TypeMismatch)?;
            let classes = ctx.classes.ok_or(RuntimeError::TypeMismatch)?;
            let key = SmolStr::new(fb_name.to_ascii_uppercase());
            let fb = function_blocks
                .get(&key)
                .ok_or(RuntimeError::UndefinedFunctionBlock(fb_name))?;
            let instance_id = create_fb_instance(
                ctx.storage,
                ctx.registry,
                &ctx.profile,
                classes,
                function_blocks,
                functions,
                stdlib,
                ctx.initializer_catalog
                    .unwrap_or(&crate::program_model::InitializerCatalog::default()),
                fb,
            )?;
            if let Some(expr) = &local.initializer {
                crate::instance::apply_fb_instance_initializer(
                    ctx.storage,
                    ctx.registry,
                    &ctx.profile,
                    stdlib,
                    ctx.initializer_catalog
                        .unwrap_or(&crate::program_model::InitializerCatalog::default()),
                    ctx.current_instance,
                    instance_id,
                    fb,
                    expr,
                )?;
            }
            ctx.storage
                .set_local(local.name.clone(), Value::Instance(instance_id));
            continue;
        }
        if let Some(class_name) = class_type_name(local.type_id, ctx.registry) {
            let function_blocks = ctx.function_blocks.ok_or(RuntimeError::TypeMismatch)?;
            let functions = ctx.functions.ok_or(RuntimeError::TypeMismatch)?;
            let stdlib = ctx.stdlib.ok_or(RuntimeError::TypeMismatch)?;
            let classes = ctx.classes.ok_or(RuntimeError::TypeMismatch)?;
            let key = SmolStr::new(class_name.to_ascii_uppercase());
            let class_def = classes.get(&key).ok_or(RuntimeError::TypeMismatch)?;
            let instance_id = create_class_instance(
                ctx.storage,
                ctx.registry,
                &ctx.profile,
                classes,
                function_blocks,
                functions,
                stdlib,
                ctx.initializer_catalog
                    .unwrap_or(&crate::program_model::InitializerCatalog::default()),
                class_def,
            )?;
            ctx.storage
                .set_local(local.name.clone(), Value::Instance(instance_id));
            continue;
        }
        let value = if let Some(expr) = &local.initializer {
            crate::harness::initializer::evaluate_initializer(
                ctx.storage,
                ctx.registry,
                ctx.initializer_catalog
                    .unwrap_or(&crate::program_model::InitializerCatalog::default()),
                &ctx.profile,
                ctx.current_instance,
                ctx.stdlib.ok_or(RuntimeError::TypeMismatch)?,
                expr,
                local.type_id,
            )?
        } else {
            crate::harness::initializer::default_value_for_type_id(
                ctx.storage,
                ctx.registry,
                ctx.initializer_catalog
                    .unwrap_or(&crate::program_model::InitializerCatalog::default()),
                &ctx.profile,
                ctx.current_instance,
                ctx.stdlib.ok_or(RuntimeError::TypeMismatch)?,
                local.type_id,
            )
            .map_err(|err| {
                let owner = current_init_owner(ctx);
                init_failed_display(&owner, &local.name, err)
            })?
        };
        ctx.storage.set_local(local.name.clone(), value);
    }
    Ok(())
}

fn ensure_static_storage(ctx: &mut EvalContext<'_>, local: &VarDef) -> Result<(), RuntimeError> {
    let (instance_id, owner) =
        static_storage_binding(ctx, &local.name).ok_or(RuntimeError::TypeMismatch)?;
    let key = static_storage_name(&owner, &local.name);

    if let Some(instance_id) = instance_id {
        if ctx.storage.get_instance_var(instance_id, key.as_ref()).is_none() {
            let value = if let Some(expr) = &local.initializer {
                crate::harness::initializer::evaluate_initializer(
                    ctx.storage,
                    ctx.registry,
                    ctx.initializer_catalog
                        .unwrap_or(&crate::program_model::InitializerCatalog::default()),
                    &ctx.profile,
                    ctx.current_instance,
                    ctx.stdlib.ok_or(RuntimeError::TypeMismatch)?,
                    expr,
                    local.type_id,
                )?
            } else {
                crate::harness::initializer::default_value_for_type_id(
                    ctx.storage,
                    ctx.registry,
                    ctx.initializer_catalog
                        .unwrap_or(&crate::program_model::InitializerCatalog::default()),
                    &ctx.profile,
                    ctx.current_instance,
                    ctx.stdlib.ok_or(RuntimeError::TypeMismatch)?,
                    local.type_id,
                )
                .map_err(|err| init_failed_display(&owner, &local.name, err))?
            };
            ctx.storage.set_instance_var(instance_id, key, value);
        }
        return Ok(());
    }

    if ctx.storage.get_global(key.as_ref()).is_none() {
        let value = if let Some(expr) = &local.initializer {
            crate::harness::initializer::evaluate_initializer(
                ctx.storage,
                ctx.registry,
                ctx.initializer_catalog
                    .unwrap_or(&crate::program_model::InitializerCatalog::default()),
                &ctx.profile,
                ctx.current_instance,
                ctx.stdlib.ok_or(RuntimeError::TypeMismatch)?,
                expr,
                local.type_id,
            )?
        } else {
            crate::harness::initializer::default_value_for_type_id(
                ctx.storage,
                ctx.registry,
                ctx.initializer_catalog
                    .unwrap_or(&crate::program_model::InitializerCatalog::default()),
                &ctx.profile,
                ctx.current_instance,
                ctx.stdlib.ok_or(RuntimeError::TypeMismatch)?,
                local.type_id,
            )
            .map_err(|err| init_failed_display(&owner, &local.name, err))?
        };
        ctx.storage.set_global(key, value);
    }
    Ok(())
}

fn static_storage_binding(
    ctx: &EvalContext<'_>,
    name: &SmolStr,
) -> Option<(Option<InstanceId>, SmolStr)> {
    let frame = ctx.storage.current_frame()?;
    if let Some(functions) = ctx.functions {
        let key = SmolStr::new(frame.owner.to_ascii_uppercase());
        if let Some(function) = functions.get(&key) {
            if contains_var_named(&function.static_locals, name) {
                return Some((None, function.name.clone()));
            }
        }
    }

    let instance_id = frame.instance_id.or(ctx.current_instance)?;
    let owner = method_static_owner_label(ctx, instance_id, &frame.owner, name)?;
    Some((Some(instance_id), owner))
}

fn contains_var_named(vars: &[VarDef], name: &SmolStr) -> bool {
    vars.iter()
        .any(|var| var.name.eq_ignore_ascii_case(name.as_str()))
}

fn method_static_owner_label(
    ctx: &EvalContext<'_>,
    instance_id: InstanceId,
    method_name: &SmolStr,
    local_name: &SmolStr,
) -> Option<SmolStr> {
    let instance = ctx.storage.get_instance(instance_id)?;
    let key = SmolStr::new(instance.type_name.to_ascii_uppercase());

    if let Some(function_blocks) = ctx.function_blocks {
        if let Some(function_block) = function_blocks.get(&key) {
            let classes = ctx.classes?;
            return method_static_owner_label_in_fb(
                function_blocks,
                classes,
                function_block,
                method_name,
                local_name,
            );
        }
    }

    let classes = ctx.classes?;
    let class_def = classes.get(&key)?;
    method_static_owner_label_in_class(classes, class_def, method_name, local_name)
}

fn method_static_owner_label_in_fb(
    function_blocks: &IndexMap<SmolStr, FunctionBlockDef>,
    classes: &IndexMap<SmolStr, ClassDef>,
    function_block: &FunctionBlockDef,
    method_name: &SmolStr,
    local_name: &SmolStr,
) -> Option<SmolStr> {
    let mut current = Some(function_block);
    while let Some(def) = current {
        if let Some(method) = def
            .methods
            .iter()
            .find(|method| method.name.eq_ignore_ascii_case(method_name.as_str()))
        {
            return contains_var_named(&method.static_locals, local_name)
                .then(|| method_static_storage_owner(&def.name, &method.name));
        }
        let Some(base) = &def.base else {
            break;
        };
        match base {
            FunctionBlockBase::FunctionBlock(base_name) => {
                let key = SmolStr::new(base_name.to_ascii_uppercase());
                current = function_blocks.get(&key);
            }
            FunctionBlockBase::Class(base_name) => {
                let key = SmolStr::new(base_name.to_ascii_uppercase());
                let class_def = classes.get(&key)?;
                return method_static_owner_label_in_class(
                    classes,
                    class_def,
                    method_name,
                    local_name,
                );
            }
        }
    }
    None
}

fn method_static_owner_label_in_class(
    classes: &IndexMap<SmolStr, ClassDef>,
    class_def: &ClassDef,
    method_name: &SmolStr,
    local_name: &SmolStr,
) -> Option<SmolStr> {
    let mut current = class_def;
    loop {
        if let Some(method) = current
            .methods
            .iter()
            .find(|method| method.name.eq_ignore_ascii_case(method_name.as_str()))
        {
            return contains_var_named(&method.static_locals, local_name)
                .then(|| method_static_storage_owner(&current.name, &method.name));
        }
        let Some(base) = &current.base else {
            break;
        };
        let key = SmolStr::new(base.to_ascii_uppercase());
        let next = classes.get(&key)?;
        current = next;
    }
    None
}

fn function_block_type_name(type_id: TypeId, registry: &TypeRegistry) -> Option<SmolStr> {
    let ty = registry.get(type_id)?;
    match ty {
        trust_hir::Type::FunctionBlock { name } => Some(name.clone()),
        trust_hir::Type::Alias { target, .. } => function_block_type_name(*target, registry),
        _ => None,
    }
}

fn class_type_name(type_id: TypeId, registry: &TypeRegistry) -> Option<SmolStr> {
    let ty = registry.get(type_id)?;
    match ty {
        trust_hir::Type::Class { name } => Some(name.clone()),
        trust_hir::Type::Alias { target, .. } => class_type_name(*target, registry),
        _ => None,
    }
}
