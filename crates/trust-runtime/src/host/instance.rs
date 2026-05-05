//! Function block and class instance management.

#![allow(missing_docs)]

use indexmap::IndexMap;
use smol_str::SmolStr;
use trust_hir::symbols::ParamDirection;
use trust_hir::types::TypeRegistry;
use trust_hir::Type;

use crate::error::RuntimeError;
use crate::memory::{InstanceId, VariableStorage};
use crate::program_model::{
    ClassDef, Expr, FunctionBlockBase, FunctionBlockDef, FunctionDef, InitializerCatalog, Param,
    VarDef,
};
use crate::stdlib::StandardLibrary;
use crate::task::ProgramDef;
use crate::value::{DateTimeProfile, Value};

/// Create and initialize a function block instance.
#[allow(clippy::too_many_arguments)]
pub fn create_fb_instance(
    storage: &mut VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    classes: &IndexMap<SmolStr, ClassDef>,
    function_blocks: &IndexMap<SmolStr, FunctionBlockDef>,
    functions: &IndexMap<SmolStr, FunctionDef>,
    stdlib: &StandardLibrary,
    initializer_catalog: &InitializerCatalog,
    fb: &FunctionBlockDef,
) -> Result<InstanceId, RuntimeError> {
    let parent_id = if let Some(base) = &fb.base {
        match base {
            FunctionBlockBase::FunctionBlock(base_name) => {
                let key = SmolStr::new(base_name.to_ascii_uppercase());
                let base_def = function_blocks
                    .get(&key)
                    .ok_or(RuntimeError::TypeMismatch)?;
                Some(create_fb_instance(
                    storage,
                    registry,
                    profile,
                    classes,
                    function_blocks,
                    functions,
                    stdlib,
                    initializer_catalog,
                    base_def,
                )?)
            }
            FunctionBlockBase::Class(base_name) => {
                let key = SmolStr::new(base_name.to_ascii_uppercase());
                let base_def = classes.get(&key).ok_or(RuntimeError::TypeMismatch)?;
                Some(create_class_instance(
                    storage,
                    registry,
                    profile,
                    classes,
                    function_blocks,
                    functions,
                    stdlib,
                    initializer_catalog,
                    base_def,
                )?)
            }
        }
    } else {
        None
    };

    let instance_id = storage.create_instance(fb.name.clone());
    if let Some(parent_id) = parent_id {
        if let Some(instance) = storage.get_instance_mut(instance_id) {
            instance.parent = Some(parent_id);
        }
    }

    init_param_defaults(
        storage,
        registry,
        profile,
        stdlib,
        initializer_catalog,
        instance_id,
        &fb.name,
        &fb.params,
    )?;
    init_var_defaults(
        storage,
        registry,
        profile,
        classes,
        function_blocks,
        functions,
        stdlib,
        initializer_catalog,
        instance_id,
        &fb.name,
        &fb.vars,
    )?;
    init_method_static_defaults(
        storage,
        registry,
        profile,
        classes,
        function_blocks,
        functions,
        stdlib,
        initializer_catalog,
        instance_id,
        &fb.name,
        &fb.methods,
    )?;

    Ok(instance_id)
}

/// Create and initialize a program instance.
#[allow(clippy::too_many_arguments)]
pub fn create_program_instance(
    storage: &mut VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    classes: &IndexMap<SmolStr, ClassDef>,
    function_blocks: &IndexMap<SmolStr, FunctionBlockDef>,
    functions: &IndexMap<SmolStr, FunctionDef>,
    stdlib: &StandardLibrary,
    initializer_catalog: &InitializerCatalog,
    program: &ProgramDef,
) -> Result<InstanceId, RuntimeError> {
    let instance_id = storage.create_instance(program.name.clone());
    init_var_defaults(
        storage,
        registry,
        profile,
        classes,
        function_blocks,
        functions,
        stdlib,
        initializer_catalog,
        instance_id,
        &program.name,
        &program.vars,
    )?;
    Ok(instance_id)
}

/// Create and initialize a class instance (including inherited base classes).
#[allow(clippy::too_many_arguments)]
pub fn create_class_instance(
    storage: &mut VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    classes: &IndexMap<SmolStr, ClassDef>,
    function_blocks: &IndexMap<SmolStr, FunctionBlockDef>,
    functions: &IndexMap<SmolStr, FunctionDef>,
    stdlib: &StandardLibrary,
    initializer_catalog: &InitializerCatalog,
    class_def: &ClassDef,
) -> Result<InstanceId, RuntimeError> {
    let parent_id = if let Some(base) = &class_def.base {
        let key = SmolStr::new(base.to_ascii_uppercase());
        let base_def = classes.get(&key).ok_or(RuntimeError::TypeMismatch)?;
        Some(create_class_instance(
            storage,
            registry,
            profile,
            classes,
            function_blocks,
            functions,
            stdlib,
            initializer_catalog,
            base_def,
        )?)
    } else {
        None
    };

    let instance_id = storage.create_instance(class_def.name.clone());
    if let Some(parent_id) = parent_id {
        if let Some(instance) = storage.get_instance_mut(instance_id) {
            instance.parent = Some(parent_id);
        }
    }

    init_var_defaults(
        storage,
        registry,
        profile,
        classes,
        function_blocks,
        functions,
        stdlib,
        initializer_catalog,
        instance_id,
        &class_def.name,
        &class_def.vars,
    )?;
    init_method_static_defaults(
        storage,
        registry,
        profile,
        classes,
        function_blocks,
        functions,
        stdlib,
        initializer_catalog,
        instance_id,
        &class_def.name,
        &class_def.methods,
    )?;

    Ok(instance_id)
}

#[allow(clippy::too_many_arguments)]
fn init_param_defaults(
    storage: &mut VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    stdlib: &StandardLibrary,
    initializer_catalog: &InitializerCatalog,
    instance_id: InstanceId,
    owner: &SmolStr,
    params: &[Param],
) -> Result<(), RuntimeError> {
    for param in params {
        let value = crate::harness::initializer::default_value_for_type_id(
            storage,
            registry,
            initializer_catalog,
            profile,
            Some(instance_id),
            stdlib,
            param.type_id,
        )
        .map_err(|err| init_failed(owner, &param.name, err))?;
        storage.set_instance_var(instance_id, param.name.clone(), value);
    }

    for param in params {
        let Some(expr) = &param.default else {
            continue;
        };
        let value = crate::harness::initializer::evaluate_initializer(
            storage,
            registry,
            initializer_catalog,
            profile,
            Some(instance_id),
            stdlib,
            expr,
            param.type_id,
        )?;
        storage.set_instance_var(instance_id, param.name.clone(), value);
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn init_var_defaults(
    storage: &mut VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    classes: &IndexMap<SmolStr, ClassDef>,
    function_blocks: &IndexMap<SmolStr, FunctionBlockDef>,
    functions: &IndexMap<SmolStr, FunctionDef>,
    stdlib: &StandardLibrary,
    initializer_catalog: &InitializerCatalog,
    instance_id: InstanceId,
    owner: &SmolStr,
    vars: &[VarDef],
) -> Result<(), RuntimeError> {
    for var in vars {
        if let Some(fb_name) = function_block_type_name(var.type_id, registry) {
            let key = SmolStr::new(fb_name.to_ascii_uppercase());
            let fb = function_blocks
                .get(&key)
                .ok_or_else(|| RuntimeError::UndefinedFunctionBlock(fb_name.clone()))?;
            let nested_id = create_fb_instance(
                storage,
                registry,
                profile,
                classes,
                function_blocks,
                functions,
                stdlib,
                initializer_catalog,
                fb,
            )?;
            storage.set_instance_var(instance_id, var.name.clone(), Value::Instance(nested_id));
            continue;
        }
        if let Some(class_name) = class_type_name(var.type_id, registry) {
            let key = SmolStr::new(class_name.to_ascii_uppercase());
            let class_def = classes.get(&key).ok_or(RuntimeError::TypeMismatch)?;
            let nested_id = create_class_instance(
                storage,
                registry,
                profile,
                classes,
                function_blocks,
                functions,
                stdlib,
                initializer_catalog,
                class_def,
            )?;
            storage.set_instance_var(instance_id, var.name.clone(), Value::Instance(nested_id));
            continue;
        }
        if var.external {
            continue;
        }
        let value = crate::harness::initializer::default_value_for_type_id(
            storage,
            registry,
            initializer_catalog,
            profile,
            Some(instance_id),
            stdlib,
            var.type_id,
        )
        .map_err(|err| init_failed(owner, &var.name, err))?;
        storage.set_instance_var(instance_id, var.name.clone(), value);
    }
    for var in vars {
        if function_block_type_name(var.type_id, registry).is_some() {
            if let Some(expr) = &var.initializer {
                let Some(Value::Instance(nested_id)) = storage
                    .get_instance_var(instance_id, var.name.as_str())
                    .cloned()
                else {
                    return Err(RuntimeError::TypeMismatch);
                };
                let fb_name = function_block_type_name(var.type_id, registry)
                    .ok_or(RuntimeError::TypeMismatch)?;
                let fb_key = SmolStr::new(fb_name.to_ascii_uppercase());
                let fb = function_blocks
                    .get(&fb_key)
                    .ok_or_else(|| RuntimeError::UndefinedFunctionBlock(fb_name.clone()))?;
                apply_fb_instance_initializer(
                    storage,
                    registry,
                    profile,
                    stdlib,
                    initializer_catalog,
                    Some(instance_id),
                    nested_id,
                    fb,
                    expr,
                )?;
            }
            continue;
        }
        if class_type_name(var.type_id, registry).is_some() {
            if var.initializer.is_some() {
                return Err(RuntimeError::TypeMismatch);
            }
            continue;
        }
        let Some(expr) = &var.initializer else {
            continue;
        };
        if var.external {
            continue;
        }
        let value = crate::harness::initializer::evaluate_initializer(
            storage,
            registry,
            initializer_catalog,
            profile,
            Some(instance_id),
            stdlib,
            expr,
            var.type_id,
        )?;
        storage.set_instance_var(instance_id, var.name.clone(), value);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn init_method_static_defaults(
    storage: &mut VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    classes: &IndexMap<SmolStr, ClassDef>,
    function_blocks: &IndexMap<SmolStr, FunctionBlockDef>,
    functions: &IndexMap<SmolStr, FunctionDef>,
    stdlib: &StandardLibrary,
    initializer_catalog: &InitializerCatalog,
    instance_id: InstanceId,
    owner: &SmolStr,
    methods: &[crate::program_model::MethodDef],
) -> Result<(), RuntimeError> {
    for method in methods {
        let method_owner = crate::program_model::method_static_storage_owner(owner, &method.name);
        for local in &method.static_locals {
            let key = crate::program_model::static_storage_name(&method_owner, &local.name);
            if let Some(fb_name) = function_block_type_name(local.type_id, registry) {
                let fb_key = SmolStr::new(fb_name.to_ascii_uppercase());
                let fb = function_blocks
                    .get(&fb_key)
                    .ok_or_else(|| RuntimeError::UndefinedFunctionBlock(fb_name.clone()))?;
                let nested_id = create_fb_instance(
                    storage,
                    registry,
                    profile,
                    classes,
                    function_blocks,
                    functions,
                    stdlib,
                    initializer_catalog,
                    fb,
                )?;
                storage.set_instance_var(instance_id, key, Value::Instance(nested_id));
                continue;
            }
            if let Some(class_name) = class_type_name(local.type_id, registry) {
                if local.initializer.is_some() {
                    return Err(RuntimeError::TypeMismatch);
                }
                let class_key = SmolStr::new(class_name.to_ascii_uppercase());
                let class_def = classes.get(&class_key).ok_or(RuntimeError::TypeMismatch)?;
                let nested_id = create_class_instance(
                    storage,
                    registry,
                    profile,
                    classes,
                    function_blocks,
                    functions,
                    stdlib,
                    initializer_catalog,
                    class_def,
                )?;
                storage.set_instance_var(instance_id, key, Value::Instance(nested_id));
                continue;
            }
            let value = crate::harness::initializer::default_value_for_type_id(
                storage,
                registry,
                initializer_catalog,
                profile,
                Some(instance_id),
                stdlib,
                local.type_id,
            )
            .map_err(|err| init_failed(owner, &local.name, err))?;
            storage.set_instance_var(instance_id, key, value);
        }
    }

    for method in methods {
        let method_owner = crate::program_model::method_static_storage_owner(owner, &method.name);
        for local in &method.static_locals {
            if function_block_type_name(local.type_id, registry).is_some() {
                if let Some(expr) = &local.initializer {
                    let key = crate::program_model::static_storage_name(&method_owner, &local.name);
                    let Some(Value::Instance(nested_id)) =
                        storage.get_instance_var(instance_id, key.as_str()).cloned()
                    else {
                        return Err(RuntimeError::TypeMismatch);
                    };
                    let fb_name = function_block_type_name(local.type_id, registry)
                        .ok_or(RuntimeError::TypeMismatch)?;
                    let fb_key = SmolStr::new(fb_name.to_ascii_uppercase());
                    let fb = function_blocks
                        .get(&fb_key)
                        .ok_or_else(|| RuntimeError::UndefinedFunctionBlock(fb_name.clone()))?;
                    apply_fb_instance_initializer(
                        storage,
                        registry,
                        profile,
                        stdlib,
                        initializer_catalog,
                        Some(instance_id),
                        nested_id,
                        fb,
                        expr,
                    )?;
                }
                continue;
            }
            if class_type_name(local.type_id, registry).is_some() {
                continue;
            }
            let Some(expr) = &local.initializer else {
                continue;
            };
            let value = crate::harness::initializer::evaluate_initializer(
                storage,
                registry,
                initializer_catalog,
                profile,
                Some(instance_id),
                stdlib,
                expr,
                local.type_id,
            )?;
            let key = crate::program_model::static_storage_name(&method_owner, &local.name);
            storage.set_instance_var(instance_id, key, value);
        }
    }

    Ok(())
}

fn init_failed(owner: &SmolStr, variable: &SmolStr, error: RuntimeError) -> RuntimeError {
    RuntimeError::InitFailed {
        owner: owner.clone(),
        variable: variable.clone(),
        error: SmolStr::new(error.to_string()),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_fb_instance_initializer(
    storage: &mut VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    stdlib: &StandardLibrary,
    initializer_catalog: &InitializerCatalog,
    parent_instance_id: Option<InstanceId>,
    target_instance_id: InstanceId,
    fb: &FunctionBlockDef,
    expr: &Expr,
) -> Result<(), RuntimeError> {
    let Expr::StructInitializer(fields) = expr else {
        return Err(RuntimeError::TypeMismatch);
    };

    let mut seen = Vec::<SmolStr>::new();
    for (name, value_expr) in fields {
        if seen
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(name.as_str()))
        {
            return Err(RuntimeError::TypeMismatch);
        }
        seen.push(name.clone());

        let (canonical_name, type_id) = fb_initializer_target(fb, name)?;
        let value = crate::harness::initializer::evaluate_initializer(
            storage,
            registry,
            initializer_catalog,
            profile,
            parent_instance_id,
            stdlib,
            value_expr,
            type_id,
        )?;
        let Some(reference) =
            storage.ref_for_instance_recursive(target_instance_id, canonical_name.as_str())
        else {
            return Err(RuntimeError::TypeMismatch);
        };
        if !storage.write_by_ref(reference, value) {
            return Err(RuntimeError::TypeMismatch);
        }
    }
    Ok(())
}

pub(crate) fn fb_initializer_target(
    fb: &FunctionBlockDef,
    name: &SmolStr,
) -> Result<(SmolStr, trust_hir::TypeId), RuntimeError> {
    if let Some(param) = fb
        .params
        .iter()
        .find(|param| param.name.eq_ignore_ascii_case(name.as_str()))
    {
        if param.direction == ParamDirection::InOut {
            return Err(RuntimeError::TypeMismatch);
        }
        return Ok((param.name.clone(), param.type_id));
    }
    if let Some(var) = fb
        .vars
        .iter()
        .find(|var| var.name.eq_ignore_ascii_case(name.as_str()))
    {
        if var.external {
            return Err(RuntimeError::TypeMismatch);
        }
        return Ok((var.name.clone(), var.type_id));
    }
    if fb
        .temps
        .iter()
        .any(|var| var.name.eq_ignore_ascii_case(name.as_str()))
    {
        return Err(RuntimeError::TypeMismatch);
    }
    Err(RuntimeError::TypeMismatch)
}

fn class_type_name(type_id: trust_hir::TypeId, registry: &TypeRegistry) -> Option<SmolStr> {
    let ty = registry.get(type_id)?;
    match ty {
        Type::Class { name } => Some(name.clone()),
        Type::Alias { target, .. } => class_type_name(*target, registry),
        _ => None,
    }
}

fn function_block_type_name(
    type_id: trust_hir::TypeId,
    registry: &TypeRegistry,
) -> Option<SmolStr> {
    let ty = registry.get(type_id)?;
    match ty {
        Type::FunctionBlock { name } => Some(name.clone()),
        Type::Alias { target, .. } => function_block_type_name(*target, registry),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::create_fb_instance;
    use crate::program_model::{expr::Expr, FunctionBlockDef, Param};
    use crate::stdlib::StandardLibrary;
    use crate::value::{DateTimeProfile, Value};
    use indexmap::IndexMap;
    use smol_str::SmolStr;
    use trust_hir::symbols::ParamDirection;
    use trust_hir::types::TypeRegistry;
    use trust_hir::TypeId;

    #[test]
    fn create_fb_instance_honors_declared_var_input_initializer() {
        let registry = TypeRegistry::new();
        let mut storage = crate::memory::VariableStorage::new();
        let fb = FunctionBlockDef {
            name: "Adjust".into(),
            base: None,
            params: vec![Param {
                name: "inc".into(),
                type_id: TypeId::INT,
                direction: ParamDirection::In,
                address: None,
                default: Some(Expr::Literal(Value::Int(5))),
            }],
            vars: Vec::new(),
            temps: Vec::new(),
            using: Vec::new(),
            methods: Vec::new(),
            body: Vec::new(),
        };
        let function_blocks: IndexMap<SmolStr, FunctionBlockDef> = IndexMap::new();
        let functions: IndexMap<SmolStr, crate::program_model::FunctionDef> = IndexMap::new();
        let classes: IndexMap<SmolStr, crate::program_model::ClassDef> = IndexMap::new();

        let instance_id = create_fb_instance(
            &mut storage,
            &registry,
            &DateTimeProfile::default(),
            &classes,
            &function_blocks,
            &functions,
            &StandardLibrary::new(),
            &crate::program_model::InitializerCatalog::default(),
            &fb,
        )
        .expect("create fb instance");

        assert_eq!(
            storage.get_instance_var(instance_id, "inc"),
            Some(&Value::Int(5))
        );
    }
}
