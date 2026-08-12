use crate::bytecode::{TypeData, TypeKind};
use crate::value::Value;

use super::dispatch_refs::dynamic_ref_type;
use super::errors::VmTrap;
use super::frames::VmFrame;
use super::VmModule;

pub(super) fn apply_reference_attempt(
    runtime: &crate::runtime::Runtime,
    module: &VmModule,
    frame: &VmFrame,
    value: Value,
    target_type_idx: u32,
) -> Result<Value, VmTrap> {
    let target_type_idx = resolved_type_idx(module, target_type_idx)
        .ok_or_else(|| VmTrap::Runtime(super::invalid_bytecode("invalid attempt target type")))?;
    let target = module
        .types
        .entries
        .get(target_type_idx as usize)
        .ok_or_else(|| VmTrap::Runtime(super::invalid_bytecode("invalid attempt target type")))?;

    match (&target.kind, &target.data) {
        (TypeKind::Reference, TypeData::Reference { target_type_id }) => {
            apply_typed_reference_attempt(module, frame, value, *target_type_id)
        }
        (TypeKind::Interface, TypeData::Interface { .. }) => {
            apply_interface_attempt(runtime, module, value, target_type_idx)
        }
        _ => Err(VmTrap::Runtime(super::invalid_bytecode(
            "REFERENCE_ATTEMPT target is not reference-like",
        ))),
    }
}

fn apply_typed_reference_attempt(
    module: &VmModule,
    frame: &VmFrame,
    value: Value,
    target_value_type_idx: u32,
) -> Result<Value, VmTrap> {
    let Value::Reference(reference) = value else {
        return Ok(Value::Reference(None));
    };
    let Some(reference) = reference else {
        return Ok(Value::Reference(None));
    };
    let Some(actual_type_idx) = dynamic_ref_type(module, frame, &reference)? else {
        return Ok(Value::Reference(None));
    };
    if runtime_type_is_compatible(module, actual_type_idx, target_value_type_idx) {
        Ok(Value::Reference(Some(reference)))
    } else {
        Ok(Value::Reference(None))
    }
}

fn apply_interface_attempt(
    runtime: &crate::runtime::Runtime,
    module: &VmModule,
    value: Value,
    target_interface_type_idx: u32,
) -> Result<Value, VmTrap> {
    let Value::Instance(instance_id) = value else {
        return Ok(Value::Null);
    };
    let Some(instance) = runtime.storage.get_instance(instance_id) else {
        return Ok(Value::Null);
    };
    let key = instance.type_name.to_ascii_uppercase();
    let Some(pou_id) = module
        .class_ids
        .get(key.as_str())
        .or_else(|| module.function_block_ids.get(key.as_str()))
        .copied()
    else {
        return Ok(Value::Null);
    };
    if pou_implements_interface(module, pou_id, target_interface_type_idx) {
        Ok(Value::Instance(instance_id))
    } else {
        Ok(Value::Null)
    }
}

fn runtime_type_is_compatible(module: &VmModule, actual: u32, target: u32) -> bool {
    let Some(actual) = resolved_type_idx(module, actual) else {
        return false;
    };
    let Some(target) = resolved_type_idx(module, target) else {
        return false;
    };
    if actual == target {
        return true;
    }

    let Some(target_entry) = module.types.entries.get(target as usize) else {
        return false;
    };
    match target_entry.kind {
        TypeKind::Class | TypeKind::FunctionBlock => {
            let Some(actual_pou) = type_pou_id(module, actual) else {
                return false;
            };
            let Some(target_pou) = type_pou_id(module, target) else {
                return false;
            };
            pou_is_same_or_derived(module, actual_pou, target_pou)
        }
        TypeKind::Interface => type_pou_id(module, actual)
            .is_some_and(|pou_id| pou_implements_interface(module, pou_id, target)),
        _ => false,
    }
}

fn resolved_type_idx(module: &VmModule, mut type_idx: u32) -> Option<u32> {
    for _ in 0..module.types.entries.len().saturating_add(1) {
        let entry = module.types.entries.get(type_idx as usize)?;
        if let TypeData::Alias { target_type_id } = &entry.data {
            type_idx = *target_type_id;
        } else {
            return Some(type_idx);
        }
    }
    None
}

fn type_pou_id(module: &VmModule, type_idx: u32) -> Option<u32> {
    let type_idx = resolved_type_idx(module, type_idx)?;
    match &module.types.entries.get(type_idx as usize)?.data {
        TypeData::Pou { pou_id } => Some(*pou_id),
        _ => None,
    }
}

fn pou_is_same_or_derived(module: &VmModule, mut actual: u32, target: u32) -> bool {
    for _ in 0..module.pou_by_id.len().saturating_add(1) {
        if actual == target {
            return true;
        }
        let Some(parent) = module.parent_pou_ids.get(&actual).copied() else {
            return false;
        };
        actual = parent;
    }
    false
}

fn pou_implements_interface(module: &VmModule, mut pou_id: u32, interface_type_idx: u32) -> bool {
    for _ in 0..module.pou_by_id.len().saturating_add(1) {
        if module
            .interface_type_ids_by_pou
            .get(&pou_id)
            .is_some_and(|interfaces| interfaces.contains(&interface_type_idx))
        {
            return true;
        }
        let Some(parent) = module.parent_pou_ids.get(&pou_id).copied() else {
            return false;
        };
        pou_id = parent;
    }
    false
}
