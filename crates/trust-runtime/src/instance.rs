//! Function block and class instance management.

#![allow(missing_docs)]

use indexmap::IndexMap;
use smol_str::SmolStr;
use trust_hir::types::TypeRegistry;
use trust_hir::Type;

use crate::error::RuntimeError;
use crate::eval::{
    eval_expr, ClassDef, EvalContext, FunctionBlockBase, FunctionBlockDef, FunctionDef, Param,
    VarDef,
};
use crate::memory::{InstanceId, VariableStorage};
use crate::stdlib::StandardLibrary;
use crate::task::ProgramDef;
use crate::value::{default_value_for_type_id, DateTimeProfile, Duration, Value};

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

    init_param_defaults(storage, registry, profile, instance_id, &fb.params);
    init_var_defaults(
        storage,
        registry,
        profile,
        classes,
        function_blocks,
        functions,
        stdlib,
        instance_id,
        &fb.vars,
        &fb.using,
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
        instance_id,
        &program.vars,
        &program.using,
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
        instance_id,
        &class_def.vars,
        &class_def.using,
    )?;

    Ok(instance_id)
}

fn init_param_defaults(
    storage: &mut VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    instance_id: InstanceId,
    params: &[Param],
) {
    for param in params {
        let value =
            default_value_for_type_id(param.type_id, registry, profile).unwrap_or(Value::Null);
        storage.set_instance_var(instance_id, param.name.clone(), value);
    }
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
    instance_id: InstanceId,
    vars: &[VarDef],
    using: &[SmolStr],
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
                class_def,
            )?;
            storage.set_instance_var(instance_id, var.name.clone(), Value::Instance(nested_id));
            continue;
        }
        if var.external {
            continue;
        }
        let value =
            default_value_for_type_id(var.type_id, registry, profile).unwrap_or(Value::Null);
        storage.set_instance_var(instance_id, var.name.clone(), value);
    }
    let mut ctx = EvalContext {
        storage,
        registry,
        profile: *profile,
        now: Duration::ZERO,
        debug: None,
        call_depth: 0,
        functions: Some(functions),
        stdlib: Some(stdlib),
        function_blocks: Some(function_blocks),
        classes: Some(classes),
        using: Some(using),
        access: None,
        current_instance: Some(instance_id),
        return_name: None,
        loop_depth: 0,
        pause_requested: false,
        execution_deadline: None,
    };
    for var in vars {
        if function_block_type_name(var.type_id, registry).is_some() {
            if var.initializer.is_some() {
                return Err(RuntimeError::TypeMismatch);
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
        let value = eval_expr(&mut ctx, expr)?;
        let value = crate::harness::coerce_value_to_type(value, var.type_id)
            .map_err(|_| RuntimeError::TypeMismatch)?;
        ctx.storage
            .set_instance_var(instance_id, var.name.clone(), value);
    }
    Ok(())
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
