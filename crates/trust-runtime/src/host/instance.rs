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

    let instance_id = storage.try_create_instance(fb.name.clone())?;
    if let Some(parent_id) = parent_id
        && let Some(instance) = storage.get_instance_mut(instance_id)
    {
        instance.parent = Some(parent_id);
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
    let instance_id = storage.try_create_instance(program.name.clone())?;
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

    let instance_id = storage.try_create_instance(class_def.name.clone())?;
    if let Some(parent_id) = parent_id
        && let Some(instance) = storage.get_instance_mut(instance_id)
    {
        instance.parent = Some(parent_id);
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

/// The value of a variable whose type is an array of function block or class instances: a new instance for every element,
/// in element order (IEC 61131-3 Ed.3 Table 11 feature 5). `None` for every other type.
#[allow(clippy::too_many_arguments)]
pub(crate) fn create_instance_array(
    storage: &mut VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    classes: &IndexMap<SmolStr, ClassDef>,
    function_blocks: &IndexMap<SmolStr, FunctionBlockDef>,
    functions: &IndexMap<SmolStr, FunctionDef>,
    stdlib: &StandardLibrary,
    initializer_catalog: &InitializerCatalog,
    type_id: trust_hir::TypeId,
) -> Result<Option<Value>, RuntimeError> {
    let Some((element, dimensions)) = instance_array_shape(type_id, registry) else {
        return Ok(None);
    };
    let count = dimensions.iter().try_fold(1usize, |count, (lower, upper)| {
        let width = upper.checked_sub(*lower)?.checked_add(1)?;
        count.checked_mul(usize::try_from(width).ok().filter(|width| *width > 0)?)
    });
    let count = count.ok_or(RuntimeError::TypeMismatch)?;
    let mut elements = Vec::with_capacity(count);
    for _ in 0..count {
        let value = if let Some(fb_name) = function_block_type_name(element, registry) {
            let key = SmolStr::new(fb_name.to_ascii_uppercase());
            let fb = function_blocks
                .get(&key)
                .ok_or_else(|| RuntimeError::UndefinedFunctionBlock(fb_name.clone()))?;
            Value::Instance(create_fb_instance(
                storage,
                registry,
                profile,
                classes,
                function_blocks,
                functions,
                stdlib,
                initializer_catalog,
                fb,
            )?)
        } else if let Some(class_name) = class_type_name(element, registry) {
            let key = SmolStr::new(class_name.to_ascii_uppercase());
            let class_def = classes.get(&key).ok_or(RuntimeError::TypeMismatch)?;
            Value::Instance(create_class_instance(
                storage,
                registry,
                profile,
                classes,
                function_blocks,
                functions,
                stdlib,
                initializer_catalog,
                class_def,
            )?)
        } else {
            create_instance_array(
                storage,
                registry,
                profile,
                classes,
                function_blocks,
                functions,
                stdlib,
                initializer_catalog,
                element,
            )?
            .ok_or(RuntimeError::TypeMismatch)?
        };
        elements.push(value);
    }
    let array = Value::Array(Box::new(crate::value::ArrayValue::from_canonical_parts(
        elements, dimensions,
    )));
    // An array type declared with an initializer gives it to every variable of the type (Table 11 feature 5b).
    if let Some(expr) = type_default_initializer(type_id, registry, initializer_catalog) {
        apply_instance_array_initializer(
            storage,
            registry,
            profile,
            stdlib,
            initializer_catalog,
            function_blocks,
            None,
            &array,
            expr,
        )?;
    }
    Ok(Some(array))
}

/// The initializer a declared type gives its variables, found on the type or on the alias chain it names.
fn type_default_initializer<'c>(
    type_id: trust_hir::TypeId,
    registry: &TypeRegistry,
    initializer_catalog: &'c InitializerCatalog,
) -> Option<&'c Expr> {
    let mut current = type_id;
    for _ in 0..64 {
        if let Some(id) = initializer_catalog.type_default(current) {
            return initializer_catalog.initializer(id);
        }
        match registry.get(current)? {
            Type::Alias { target, .. } => current = *target,
            _ => return None,
        }
    }
    None
}

/// Pairs every instance of an instance array with its initializer, in element order: the array initializer's entries with
/// every repetition n(...) expanded, recursing into nested arrays (IEC 61131-3 Ed.3 Table 11 feature 5b). Elements after the
/// last entry keep their declared initial values; entries after the last element are ignored, as for every array
/// initializer (docs/specs/02-data-types.md §3.4).
pub(crate) fn for_each_element_initializer(
    array: &Value,
    expr: &Expr,
    apply: &mut dyn FnMut(InstanceId, &Expr) -> Result<(), RuntimeError>,
) -> Result<(), RuntimeError> {
    let (Value::Array(array), Expr::ArrayInitializer(entries)) = (array, expr) else {
        return Err(RuntimeError::TypeMismatch);
    };
    let elements = array.elements();
    let mut next = 0usize;
    for entry in entries {
        let (count, values) = match crate::helper_eval::array_repeat_group(entry)? {
            Some((count, args)) => {
                let values = args
                    .iter()
                    .map(|arg| match &arg.value {
                        crate::program_model::ArgValue::Expr(value) => Ok(value),
                        crate::program_model::ArgValue::Target(_) => {
                            Err(RuntimeError::TypeMismatch)
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                (count, values)
            }
            None => (1, vec![entry]),
        };
        for _ in 0..count {
            for value in &values {
                let Some(element) = elements.get(next) else {
                    return Ok(());
                };
                next += 1;
                match element {
                    Value::Instance(id) => apply(*id, value)?,
                    Value::Array(_) => for_each_element_initializer(element, value, apply)?,
                    _ => return Err(RuntimeError::TypeMismatch),
                }
            }
        }
    }
    Ok(())
}

/// Applies an instance array's initializer: each function block element receives its aggregate initializer. Class
/// elements, like class instances, accept no initializer.
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_instance_array_initializer(
    storage: &mut VariableStorage,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
    stdlib: &StandardLibrary,
    initializer_catalog: &InitializerCatalog,
    function_blocks: &IndexMap<SmolStr, FunctionBlockDef>,
    parent_instance_id: Option<InstanceId>,
    array: &Value,
    expr: &Expr,
) -> Result<(), RuntimeError> {
    for_each_element_initializer(array, expr, &mut |instance_id, value| {
        let type_name = storage
            .get_instance(instance_id)
            .ok_or(RuntimeError::NullReference)?
            .type_name
            .clone();
        let fb = function_blocks
            .get(&SmolStr::new(type_name.to_ascii_uppercase()))
            .ok_or(RuntimeError::TypeMismatch)?;
        apply_fb_instance_initializer(
            storage,
            registry,
            profile,
            stdlib,
            initializer_catalog,
            function_blocks,
            parent_instance_id,
            instance_id,
            fb,
            value,
        )
    })
}

/// Whether a variable of `type_id` is an array of function block or class instances.
pub(crate) fn is_instance_array(type_id: trust_hir::TypeId, registry: &TypeRegistry) -> bool {
    instance_array_shape(type_id, registry).is_some()
}

/// The element type and dimensions of an array whose elements are function block or class instances, directly or through
/// further arrays.
fn instance_array_shape(
    type_id: trust_hir::TypeId,
    registry: &TypeRegistry,
) -> Option<(trust_hir::TypeId, Vec<(i64, i64)>)> {
    match registry.get(type_id)? {
        Type::Alias { target, .. } => instance_array_shape(*target, registry),
        Type::Array {
            element,
            dimensions,
        } if function_block_type_name(*element, registry).is_some()
            || class_type_name(*element, registry).is_some()
            || instance_array_shape(*element, registry).is_some() =>
        {
            Some((*element, dimensions.clone()))
        }
        _ => None,
    }
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
        if let Some(value) = create_instance_array(
            storage,
            registry,
            profile,
            classes,
            function_blocks,
            functions,
            stdlib,
            initializer_catalog,
            var.type_id,
        )
        .map_err(|err| init_failed(owner, &var.name, err))?
        {
            storage.set_instance_var(instance_id, var.name.clone(), value);
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
                    function_blocks,
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
        if is_instance_array(var.type_id, registry) {
            let array = storage
                .get_instance_var(instance_id, var.name.as_str())
                .cloned()
                .ok_or(RuntimeError::TypeMismatch)?;
            apply_instance_array_initializer(
                storage,
                registry,
                profile,
                stdlib,
                initializer_catalog,
                function_blocks,
                Some(instance_id),
                &array,
                expr,
            )
            .map_err(|err| init_failed(owner, &var.name, err))?;
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
            if let Some(value) = create_instance_array(
                storage,
                registry,
                profile,
                classes,
                function_blocks,
                functions,
                stdlib,
                initializer_catalog,
                local.type_id,
            )
            .map_err(|err| init_failed(owner, &local.name, err))?
            {
                storage.set_instance_var(instance_id, key, value);
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
                        function_blocks,
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
            if is_instance_array(local.type_id, registry) {
                let key = crate::program_model::static_storage_name(&method_owner, &local.name);
                let array = storage
                    .get_instance_var(instance_id, key.as_str())
                    .cloned()
                    .ok_or(RuntimeError::TypeMismatch)?;
                apply_instance_array_initializer(
                    storage,
                    registry,
                    profile,
                    stdlib,
                    initializer_catalog,
                    function_blocks,
                    Some(instance_id),
                    &array,
                    expr,
                )
                .map_err(|err| init_failed(owner, &local.name, err))?;
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
    function_blocks: &IndexMap<SmolStr, FunctionBlockDef>,
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

        let (canonical_name, type_id) = fb_initializer_target(function_blocks, fb, name)?;
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

/// The member an instance initializer names and its type: an input, an output or a variable of the
/// block or of a block it extends (IEC 61131-3 Ed.3 §6.6.5.3: a derived block has its base's
/// variables), so `P102 : Plant.FB_Pump := (StartTimeout := T#10s)` sets an inherited setting.
pub(crate) fn fb_initializer_target(
    function_blocks: &IndexMap<SmolStr, FunctionBlockDef>,
    fb: &FunctionBlockDef,
    name: &SmolStr,
) -> Result<(SmolStr, trust_hir::TypeId), RuntimeError> {
    let mut current = Some(fb);
    let mut depth = 0;
    while let Some(block) = current {
        if let Some(target) = own_initializer_target(block, name)? {
            return Ok(target);
        }
        depth += 1;
        if depth > 32 {
            break;
        }
        current = match &block.base {
            Some(FunctionBlockBase::FunctionBlock(base)) => {
                function_blocks.get(&SmolStr::new(base.to_ascii_uppercase()))
            }
            _ => None,
        };
    }
    Err(RuntimeError::TypeMismatch)
}

/// The member as one block declares it, `None` when that block does not declare it.
fn own_initializer_target(
    fb: &FunctionBlockDef,
    name: &SmolStr,
) -> Result<Option<(SmolStr, trust_hir::TypeId)>, RuntimeError> {
    if let Some(param) = fb
        .params
        .iter()
        .find(|param| param.name.eq_ignore_ascii_case(name.as_str()))
    {
        if param.direction == ParamDirection::InOut {
            return Err(RuntimeError::TypeMismatch);
        }
        return Ok(Some((param.name.clone(), param.type_id)));
    }
    if let Some(var) = fb
        .vars
        .iter()
        .find(|var| var.name.eq_ignore_ascii_case(name.as_str()))
    {
        if var.external {
            return Err(RuntimeError::TypeMismatch);
        }
        return Ok(Some((var.name.clone(), var.type_id)));
    }
    if fb
        .temps
        .iter()
        .any(|var| var.name.eq_ignore_ascii_case(name.as_str()))
    {
        return Err(RuntimeError::TypeMismatch);
    }
    Ok(None)
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
    use super::{create_class_instance, create_fb_instance, create_program_instance};
    use crate::error::RuntimeError;
    use crate::program_model::{expr::Expr, ClassDef, FunctionBlockDef, Param};
    use crate::stdlib::StandardLibrary;
    use crate::task::ProgramDef;
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
            interfaces: Vec::new(),
            params: vec![Param {
                name: "inc".into(),
                type_id: TypeId::INT,
                direction: ParamDirection::In,
                address: None,
                default: Some(Expr::Literal(Value::Int(5))),
                retain: crate::RetainPolicy::Unspecified,
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

    #[test]
    fn hosted_creation_at_instance_identity_limit_fails_and_keeps_existing_instances() {
        let registry = TypeRegistry::new();
        let profile = DateTimeProfile::default();
        let stdlib = StandardLibrary::new();
        let catalog = crate::program_model::InitializerCatalog::default();
        let classes: IndexMap<SmolStr, ClassDef> = IndexMap::new();
        let function_blocks: IndexMap<SmolStr, FunctionBlockDef> = IndexMap::new();
        let functions: IndexMap<SmolStr, crate::program_model::FunctionDef> = IndexMap::new();
        let program = ProgramDef {
            name: "Main".into(),
            vars: Vec::new(),
            temps: Vec::new(),
            using: Vec::new(),
            body: Vec::new(),
        };
        let class = ClassDef {
            name: "Counter".into(),
            base: None,
            interfaces: Vec::new(),
            vars: Vec::new(),
            using: Vec::new(),
            methods: Vec::new(),
        };
        let fb = FunctionBlockDef {
            name: "Adjust".into(),
            base: None,
            interfaces: Vec::new(),
            params: Vec::new(),
            vars: Vec::new(),
            temps: Vec::new(),
            using: Vec::new(),
            methods: Vec::new(),
            body: Vec::new(),
        };
        let mut storage = crate::memory::VariableStorage::new();
        let existing = storage
            .try_create_instance("EXISTING")
            .expect("first instance");
        assert!(storage.set_instance_var(existing, "KEEP", Value::Int(7)));
        storage.set_next_instance_id_for_test(u32::MAX);

        let program_error = create_program_instance(
            &mut storage,
            &registry,
            &profile,
            &classes,
            &function_blocks,
            &functions,
            &stdlib,
            &catalog,
            &program,
        )
        .expect_err("program identity space is exhausted");
        let class_error = create_class_instance(
            &mut storage,
            &registry,
            &profile,
            &classes,
            &function_blocks,
            &functions,
            &stdlib,
            &catalog,
            &class,
        )
        .expect_err("class identity space is exhausted");
        let fb_error = create_fb_instance(
            &mut storage,
            &registry,
            &profile,
            &classes,
            &function_blocks,
            &functions,
            &stdlib,
            &catalog,
            &fb,
        )
        .expect_err("function block identity space is exhausted");

        assert_eq!(program_error, RuntimeError::Overflow);
        assert_eq!(class_error, RuntimeError::Overflow);
        assert_eq!(fb_error, RuntimeError::Overflow);
        assert_eq!(storage.instances().len(), 1);
        assert_eq!(
            storage.get_instance_var(existing, "KEEP"),
            Some(&Value::Int(7))
        );
    }
}
