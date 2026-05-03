pub(crate) fn resolve_program_type_name(
    program_defs: &IndexMap<SmolStr, ProgramDef>,
    type_name: &SmolStr,
    using: &[SmolStr],
) -> Result<SmolStr, CompileError> {
    let direct_key = SmolStr::new(type_name.to_ascii_uppercase());
    if let Some(def) = program_defs.get(&direct_key) {
        return Ok(def.name.clone());
    }
    if !type_name.contains('.') {
        for namespace in using {
            let qualified = format!("{namespace}.{type_name}");
            let key = SmolStr::new(qualified.to_ascii_uppercase());
            if let Some(def) = program_defs.get(&key) {
                return Ok(def.name.clone());
            }
        }
    }
    Err(CompileError::new(format!(
        "unknown PROGRAM type '{}'",
        type_name
    )))
}
