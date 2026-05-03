pub(super) fn apply_config_inits(
    runtime: &mut Runtime,
    config_inits: &[ConfigInit],
    wildcards: &mut Vec<WildcardRequirement>,
) -> Result<(), CompileError> {
    if config_inits.is_empty() {
        return Ok(());
    }
    let registry = runtime.registry().clone();
    let profile = runtime.profile();
    let stdlib = runtime.stdlib().clone();
    let initializer_catalog = runtime.initializer_catalog().clone();

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

        let value = crate::harness::initializer::evaluate_initializer(
            runtime.storage(),
            &registry,
            &initializer_catalog,
            &profile,
            None,
            &stdlib,
            expr,
            init.type_id,
        )
        .map_err(|err| CompileError::new(format!("VAR_CONFIG initializer error: {err}")))?;

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
