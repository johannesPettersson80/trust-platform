use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use indexmap::IndexMap;
use smol_str::SmolStr;
use trust_hir::{Type, TypeId};

use crate::error::RuntimeError;
use crate::helper_eval::eval_storage_expr_with_stdlib;
use crate::instance::{create_class_instance, create_fb_instance};
use crate::memory::InstanceId;
use crate::program_model::{method_static_storage_owner, static_storage_name, Param, VarDef};
use crate::value::{default_value_for_type_id, Value};

use super::frames::VmFrame;
use super::VmModule;

pub(super) fn initialize_declared_locals(
    runtime: &mut crate::Runtime,
    module: &VmModule,
    frame: &mut VmFrame,
) -> Result<(), RuntimeError> {
    let Some(plan) = runtime
        .vm_local_init_plan_cache
        .plan_for(runtime, module, frame.pou_id)
    else {
        return Ok(());
    };
    if frame.locals.is_empty() {
        return Ok(());
    }
    if can_initialize_directly(plan.as_ref()) {
        return initialize_declared_locals_direct(runtime, plan.as_ref(), frame);
    }

    let temp_frame_id = if let Some(instance_id) = frame.runtime_instance {
        runtime
            .storage_mut()
            .push_frame_with_instance(plan.frame_owner().clone(), instance_id)
    } else {
        runtime.storage_mut().push_frame(plan.frame_owner().clone())
    };

    let result = initialize_declared_locals_inner(runtime, plan.as_ref(), frame);
    runtime.storage_mut().remove_frame(temp_frame_id);
    result
}

fn initialize_declared_locals_inner(
    runtime: &mut crate::Runtime,
    plan: &VmPouInitPlan,
    frame: &mut VmFrame,
) -> Result<(), RuntimeError> {
    let profile = runtime.profile();
    let current_instance = frame.runtime_instance;
    let mut slot = 0usize;

    if let Some((return_name, return_type)) = plan.return_slot() {
        let value = frame
            .locals
            .get(slot)
            .cloned()
            .filter(|value| !matches!(value, Value::Null))
            .unwrap_or_else(|| {
                default_value_for_type_id(return_type, runtime.registry(), &profile)
                    .unwrap_or(Value::Null)
            });
        runtime.storage_mut().set_local(return_name.clone(), value);
        slot = slot.saturating_add(1);
    }

    for param in plan.params() {
        let value = frame.locals.get(slot).cloned().unwrap_or_else(|| {
            default_value_for_type_id(param.type_id, runtime.registry(), &profile)
                .unwrap_or(Value::Null)
        });
        runtime.storage_mut().set_local(param.name.clone(), value);
        slot = slot.saturating_add(1);
    }

    for local in plan.static_locals() {
        ensure_static_local(runtime, plan, current_instance, local)?;
    }

    for local in plan.locals() {
        let value = initialize_var_value(runtime, current_instance, local)?;
        runtime.storage_mut().set_local(local.name.clone(), value);
    }

    slot = 0usize;
    if let Some((return_name, _)) = plan.return_slot() {
        if let Some(value) = runtime.storage().get_local(return_name.as_str()).cloned() {
            if let Some(slot_ref) = frame.locals.get_mut(slot) {
                *slot_ref = value;
            }
        }
        slot = slot.saturating_add(1);
    }

    for param in plan.params() {
        if let Some(value) = runtime.storage().get_local(param.name.as_str()).cloned() {
            if let Some(slot_ref) = frame.locals.get_mut(slot) {
                *slot_ref = value;
            }
        }
        slot = slot.saturating_add(1);
    }

    for local in plan.locals() {
        if let Some(value) = runtime.storage().get_local(local.name.as_str()).cloned() {
            if let Some(slot_ref) = frame.locals.get_mut(slot) {
                *slot_ref = value;
            }
        }
        slot = slot.saturating_add(1);
    }

    Ok(())
}

fn initialize_declared_locals_direct(
    runtime: &mut crate::Runtime,
    plan: &VmPouInitPlan,
    frame: &mut VmFrame,
) -> Result<(), RuntimeError> {
    let profile = runtime.profile();
    let mut slot = 0usize;

    if let Some((_, return_type)) = plan.return_slot() {
        if matches!(frame.locals.get(slot), Some(Value::Null) | None) {
            if let Some(slot_ref) = frame.locals.get_mut(slot) {
                *slot_ref = default_value_for_type_id(return_type, runtime.registry(), &profile)
                    .unwrap_or(Value::Null);
            }
        }
        slot = slot.saturating_add(1);
    }

    slot = slot.saturating_add(plan.params().len());

    for local in plan.locals() {
        if !local.external {
            if let Some(slot_ref) = frame.locals.get_mut(slot) {
                *slot_ref = initialize_var_value(runtime, frame.runtime_instance, local)?;
            }
        }
        slot = slot.saturating_add(1);
    }

    Ok(())
}

fn ensure_static_local(
    runtime: &mut crate::Runtime,
    plan: &VmPouInitPlan,
    current_instance: Option<InstanceId>,
    local: &VarDef,
) -> Result<(), RuntimeError> {
    let profile = runtime.profile();
    let static_owner = plan.static_owner();
    let key = static_storage_name(&static_owner, &local.name);

    if let Some(instance_id) = current_instance {
        if runtime
            .storage()
            .get_instance_var(instance_id, key.as_str())
            .is_some()
        {
            return Ok(());
        }
        let value = initialize_var_value(runtime, current_instance, local)?;
        runtime
            .storage_mut()
            .set_instance_var(instance_id, key, value);
        return Ok(());
    }

    if runtime.storage().get_global(key.as_str()).is_some() {
        return Ok(());
    }
    let value = if let Some(expr) = &local.initializer {
        let value = eval_storage_expr_with_stdlib(
            runtime.storage(),
            runtime.registry(),
            &profile,
            current_instance,
            Some(runtime.stdlib()),
            expr,
        )?;
        crate::harness::coerce_initializer_value_to_type(
            value,
            local.type_id,
            runtime.registry(),
            &profile,
        )
        .map_err(|_| RuntimeError::TypeMismatch)?
    } else {
        default_value_for_type_id(local.type_id, runtime.registry(), &profile)
            .unwrap_or(Value::Null)
    };
    runtime.storage_mut().set_global(key, value);
    Ok(())
}

fn initialize_var_value(
    runtime: &mut crate::Runtime,
    current_instance: Option<InstanceId>,
    local: &VarDef,
) -> Result<Value, RuntimeError> {
    let profile = runtime.profile();
    if let Some(fb_name) = function_block_type_name(local.type_id, runtime.registry()) {
        if local.initializer.is_some() {
            return Err(RuntimeError::TypeMismatch);
        }
        let fb_key = SmolStr::new(fb_name.to_ascii_uppercase());
        let fb = runtime
            .function_blocks()
            .get(&fb_key)
            .cloned()
            .ok_or_else(|| RuntimeError::UndefinedFunctionBlock(fb_name.clone()))?;
        let (storage, registry, classes, function_blocks, functions, stdlib) =
            runtime.instance_init_context();
        let instance_id = create_fb_instance(
            storage,
            registry,
            &profile,
            classes,
            function_blocks,
            functions,
            stdlib,
            &fb,
        )?;
        return Ok(Value::Instance(instance_id));
    }
    if let Some(class_name) = class_type_name(local.type_id, runtime.registry()) {
        if local.initializer.is_some() {
            return Err(RuntimeError::TypeMismatch);
        }
        let class_key = SmolStr::new(class_name.to_ascii_uppercase());
        let class_def = runtime
            .classes()
            .get(&class_key)
            .cloned()
            .ok_or(RuntimeError::TypeMismatch)?;
        let (storage, registry, classes, function_blocks, functions, stdlib) =
            runtime.instance_init_context();
        let instance_id = create_class_instance(
            storage,
            registry,
            &profile,
            classes,
            function_blocks,
            functions,
            stdlib,
            &class_def,
        )?;
        return Ok(Value::Instance(instance_id));
    }

    if let Some(expr) = &local.initializer {
        let value = eval_storage_expr_with_stdlib(
            runtime.storage(),
            runtime.registry(),
            &profile,
            current_instance,
            Some(runtime.stdlib()),
            expr,
        )?;
        return crate::harness::coerce_initializer_value_to_type(
            value,
            local.type_id,
            runtime.registry(),
            &profile,
        )
        .map_err(|_| RuntimeError::TypeMismatch);
    }

    Ok(
        default_value_for_type_id(local.type_id, runtime.registry(), &profile)
            .unwrap_or(Value::Null),
    )
}

fn can_initialize_directly(plan: &VmPouInitPlan) -> bool {
    if !plan.static_locals().is_empty() {
        return false;
    }
    plan.locals()
        .iter()
        .filter(|local| !local.external)
        .all(|local| !local.static_storage && local.initializer.is_none())
}

#[derive(Debug, Clone)]
enum VmPouInitPlan {
    Program {
        frame_owner: SmolStr,
        locals: Vec<VarDef>,
    },
    Function {
        frame_owner: SmolStr,
        params: Vec<Param>,
        locals: Vec<VarDef>,
        static_locals: Vec<VarDef>,
        return_slot: (SmolStr, TypeId),
    },
    FunctionBlock {
        frame_owner: SmolStr,
        locals: Vec<VarDef>,
    },
    Method {
        owner: SmolStr,
        frame_owner: SmolStr,
        params: Vec<Param>,
        locals: Vec<VarDef>,
        static_locals: Vec<VarDef>,
        return_slot: Option<(SmolStr, TypeId)>,
    },
}

impl VmPouInitPlan {
    fn frame_owner(&self) -> &SmolStr {
        match self {
            Self::Program { frame_owner, .. }
            | Self::Function { frame_owner, .. }
            | Self::FunctionBlock { frame_owner, .. }
            | Self::Method { frame_owner, .. } => frame_owner,
        }
    }

    fn static_owner(&self) -> SmolStr {
        match self {
            Self::Program { frame_owner, .. }
            | Self::Function { frame_owner, .. }
            | Self::FunctionBlock { frame_owner, .. } => frame_owner.clone(),
            Self::Method {
                owner, frame_owner, ..
            } => method_static_storage_owner(owner, frame_owner),
        }
    }

    fn return_slot(&self) -> Option<(&SmolStr, TypeId)> {
        match self {
            Self::Function { return_slot, .. } => Some((&return_slot.0, return_slot.1)),
            Self::Method { return_slot, .. } => return_slot.as_ref().map(|(name, ty)| (name, *ty)),
            Self::Program { .. } | Self::FunctionBlock { .. } => None,
        }
    }

    fn params(&self) -> &[Param] {
        match self {
            Self::Program { .. } => &[],
            Self::Function { params, .. } => params.as_slice(),
            // Function block bytecode bodies read parameters via instance fields.
            // Only function/method bodies materialize parameters into local slots.
            Self::FunctionBlock { .. } => &[],
            Self::Method { params, .. } => params.as_slice(),
        }
    }

    fn locals(&self) -> &[VarDef] {
        match self {
            Self::Program { locals, .. }
            | Self::Function { locals, .. }
            | Self::FunctionBlock { locals, .. }
            | Self::Method { locals, .. } => locals.as_slice(),
        }
    }

    fn static_locals(&self) -> &[VarDef] {
        match self {
            Self::Function { static_locals, .. } | Self::Method { static_locals, .. } => {
                static_locals.as_slice()
            }
            Self::Program { .. } | Self::FunctionBlock { .. } => &[],
        }
    }
}

#[derive(Debug, Default)]
struct VmLocalInitPlanCacheData {
    module_ptr: Option<usize>,
    entries: HashMap<u32, Arc<VmPouInitPlan>>,
}

#[derive(Debug, Default)]
pub(in crate::runtime) struct VmLocalInitPlanCacheState {
    data: RefCell<VmLocalInitPlanCacheData>,
}

impl VmLocalInitPlanCacheState {
    pub(in crate::runtime) fn invalidate_all(&self) {
        let mut data = self.data.borrow_mut();
        data.module_ptr = None;
        data.entries.clear();
    }

    fn plan_for(
        &self,
        runtime: &crate::Runtime,
        module: &VmModule,
        pou_id: u32,
    ) -> Option<Arc<VmPouInitPlan>> {
        let module_ptr = module as *const VmModule as usize;
        let mut data = self.data.borrow_mut();
        if data.module_ptr != Some(module_ptr) {
            data.module_ptr = Some(module_ptr);
            data.entries.clear();
        }
        if let Some(plan) = data.entries.get(&pou_id).cloned() {
            return Some(plan);
        }
        let plan = Arc::new(build_init_plan_for_pou(
            module,
            pou_id,
            runtime.programs(),
            runtime.functions(),
            runtime.function_blocks(),
            runtime.classes(),
        )?);
        data.entries.insert(pou_id, plan.clone());
        Some(plan)
    }
}

fn build_init_plan_for_pou(
    module: &VmModule,
    pou_id: u32,
    programs: &IndexMap<SmolStr, crate::task::ProgramDef>,
    functions: &IndexMap<SmolStr, crate::program_model::FunctionDef>,
    function_blocks: &IndexMap<SmolStr, crate::program_model::FunctionBlockDef>,
    classes: &IndexMap<SmolStr, crate::program_model::ClassDef>,
) -> Option<VmPouInitPlan> {
    let pou = module.pou(pou_id)?;
    let key = SmolStr::new(pou.name.to_ascii_uppercase());

    if module.program_ids.get(&key).copied() == Some(pou_id) {
        return programs.get(&key).map(|program| VmPouInitPlan::Program {
            frame_owner: program.name.clone(),
            locals: program.temps.clone(),
        });
    }
    if module.function_ids.get(&key).copied() == Some(pou_id) {
        return functions.get(&key).map(|function| VmPouInitPlan::Function {
            frame_owner: function.name.clone(),
            params: function.params.clone(),
            locals: function.locals.clone(),
            static_locals: function.static_locals.clone(),
            return_slot: (function.name.clone(), function.return_type),
        });
    }
    if module.function_block_ids.get(&key).copied() == Some(pou_id) {
        return function_blocks
            .get(&key)
            .map(|function_block| VmPouInitPlan::FunctionBlock {
                frame_owner: function_block.name.clone(),
                locals: function_block.temps.clone(),
            });
    }

    for (owner_key, function_block) in function_blocks {
        let Some(owner_id) = module.function_block_ids.get(owner_key).copied() else {
            continue;
        };
        let Some(method_table) = module.method_table_by_owner.get(&owner_id) else {
            continue;
        };
        for method in &function_block.methods {
            let method_key = SmolStr::new(method.name.to_ascii_uppercase());
            if method_table.get(&method_key).copied() == Some(pou_id) {
                return Some(VmPouInitPlan::Method {
                    owner: function_block.name.clone(),
                    frame_owner: method.name.clone(),
                    params: method.params.clone(),
                    locals: method.locals.clone(),
                    static_locals: method.static_locals.clone(),
                    return_slot: method.return_type.map(|ty| (method.name.clone(), ty)),
                });
            }
        }
    }

    for (owner_key, class_def) in classes {
        let Some(owner_id) = module.class_ids.get(owner_key).copied() else {
            continue;
        };
        let Some(method_table) = module.method_table_by_owner.get(&owner_id) else {
            continue;
        };
        for method in &class_def.methods {
            let method_key = SmolStr::new(method.name.to_ascii_uppercase());
            if method_table.get(&method_key).copied() == Some(pou_id) {
                return Some(VmPouInitPlan::Method {
                    owner: class_def.name.clone(),
                    frame_owner: method.name.clone(),
                    params: method.params.clone(),
                    locals: method.locals.clone(),
                    static_locals: method.static_locals.clone(),
                    return_slot: method.return_type.map(|ty| (method.name.clone(), ty)),
                });
            }
        }
    }

    None
}

fn class_type_name(type_id: TypeId, registry: &trust_hir::types::TypeRegistry) -> Option<SmolStr> {
    let ty = registry.get(type_id)?;
    match ty {
        Type::Class { name } => Some(name.clone()),
        Type::Alias { target, .. } => class_type_name(*target, registry),
        _ => None,
    }
}

fn function_block_type_name(
    type_id: TypeId,
    registry: &trust_hir::types::TypeRegistry,
) -> Option<SmolStr> {
    let ty = registry.get(type_id)?;
    match ty {
        Type::FunctionBlock { name } => Some(name.clone()),
        Type::Alias { target, .. } => function_block_type_name(*target, registry),
        _ => None,
    }
}
