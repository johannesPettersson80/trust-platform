#[allow(clippy::too_many_arguments)]
pub(super) fn collect_instance_bindings(
    registry: &TypeRegistry,
    storage: &VariableStorage,
    function_blocks: &IndexMap<SmolStr, FunctionBlockDef>,
    instance_id: InstanceId,
    instance_name: &SmolStr,
    wildcards: &mut Vec<WildcardRequirement>,
    visited: &mut std::collections::HashSet<InstanceId>,
    out: &mut Vec<InstanceBinding>,
) -> Result<(), CompileError> {
    if !visited.insert(instance_id) {
        return Ok(());
    }
    let instance = storage
        .get_instance(instance_id)
        .ok_or_else(|| CompileError::new("invalid function block instance"))?;
    let key = SmolStr::new(instance.type_name.to_ascii_uppercase());
    let Some(fb) = function_blocks.get(&key) else {
        return Ok(());
    };

    for param in &fb.params {
        let Some(address) = &param.address else {
            let reference = storage
                .ref_for_instance(instance_id, param.name.as_ref())
                .ok_or_else(|| CompileError::new("invalid function block parameter"))?;
            let full_name = join_instance_path(instance_name, &param.name);
            collect_direct_field_bindings(
                registry,
                &reference,
                param.type_id,
                &full_name,
                wildcards,
                out,
            )?;
            continue;
        };
        let reference = storage
            .ref_for_instance(instance_id, param.name.as_ref())
            .ok_or_else(|| CompileError::new("invalid function block parameter"))?;
        let full_name = join_instance_path(instance_name, &param.name);
        if address.wildcard {
            wildcards.push(WildcardRequirement {
                name: full_name,
                reference,
                area: address.area,
            });
        } else {
            out.push(InstanceBinding {
                reference,
                type_id: param.type_id,
                address: address.clone(),
                display_name: full_name,
            });
        }
    }

    for var in &fb.vars {
        let Some(address) = &var.address else {
            let reference = storage
                .ref_for_instance(instance_id, var.name.as_ref())
                .ok_or_else(|| CompileError::new("invalid function block variable"))?;
            let full_name = join_instance_path(instance_name, &var.name);
            collect_direct_field_bindings(
                registry,
                &reference,
                var.type_id,
                &full_name,
                wildcards,
                out,
            )?;
            continue;
        };
        let reference = storage
            .ref_for_instance(instance_id, var.name.as_ref())
            .ok_or_else(|| CompileError::new("invalid function block variable"))?;
        let full_name = join_instance_path(instance_name, &var.name);
        if address.wildcard {
            wildcards.push(WildcardRequirement {
                name: full_name,
                reference,
                area: address.area,
            });
        } else {
            out.push(InstanceBinding {
                reference,
                type_id: var.type_id,
                address: address.clone(),
                display_name: full_name,
            });
        }
    }

    for (name, value) in instance.variables.iter() {
        let Value::Instance(nested_id) = value else {
            continue;
        };
        let nested_name = join_instance_path(instance_name, name);
        collect_instance_bindings(
            registry,
            storage,
            function_blocks,
            *nested_id,
            &nested_name,
            wildcards,
            visited,
            out,
        )?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn collect_program_instance_bindings(
    registry: &TypeRegistry,
    storage: &VariableStorage,
    function_blocks: &IndexMap<SmolStr, FunctionBlockDef>,
    program: &ProgramDef,
    instance_id: InstanceId,
    instance_name: &SmolStr,
    wildcards: &mut Vec<WildcardRequirement>,
    visited: &mut std::collections::HashSet<InstanceId>,
    out: &mut Vec<InstanceBinding>,
) -> Result<(), CompileError> {
    for var in &program.vars {
        let reference = storage
            .ref_for_instance(instance_id, var.name.as_ref())
            .ok_or_else(|| CompileError::new("invalid program variable reference"))?;
        let full_name = join_instance_path(instance_name, &var.name);
        if let Some(address) = &var.address {
            if address.wildcard {
                wildcards.push(WildcardRequirement {
                    name: full_name,
                    reference,
                    area: address.area,
                });
            } else {
                out.push(InstanceBinding {
                    reference,
                    type_id: var.type_id,
                    address: address.clone(),
                    display_name: full_name,
                });
            }
            continue;
        }
        collect_direct_field_bindings(
            registry,
            &reference,
            var.type_id,
            &full_name,
            wildcards,
            out,
        )?;
    }

    let instance = storage
        .get_instance(instance_id)
        .ok_or_else(|| CompileError::new("invalid program instance"))?;
    for (name, value) in instance.variables.iter() {
        let Value::Instance(nested_id) = value else {
            continue;
        };
        let nested_name = join_instance_path(instance_name, name);
        collect_instance_bindings(
            registry,
            storage,
            function_blocks,
            *nested_id,
            &nested_name,
            wildcards,
            visited,
            out,
        )?;
    }

    Ok(())
}
