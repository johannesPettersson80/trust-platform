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
        let writable = match access_target_policy(runtime, &decl.path) {
            AccessTargetPolicy::Writable => decl.writable,
            AccessTargetPolicy::ReadOnly(_) => false,
            AccessTargetPolicy::Forbidden(section) => {
                return Err(CompileError::new(format!(
                    "VAR_ACCESS cannot expose {section} target"
                )))
            }
        };
        let resolved = resolve_access_path(runtime, &decl.path)?;
        match resolved {
            ResolvedAccess::Variable { reference, partial } => {
                runtime
                    .access_map_mut()
                    .bind_with_mode(decl.name.clone(), reference, partial, writable);
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
