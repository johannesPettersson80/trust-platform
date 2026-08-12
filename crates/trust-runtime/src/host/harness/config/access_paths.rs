pub(super) fn access_path_display(path: &AccessPath) -> SmolStr {
    match path {
        AccessPath::Direct { text, .. } => text.clone(),
        AccessPath::Parts(parts) => {
            let mut out = String::new();
            for part in parts {
                match part {
                    AccessPart::Name(name) => {
                        if !out.is_empty() {
                            out.push('.');
                        }
                        out.push_str(name);
                    }
                    AccessPart::Index(indices) => {
                        out.push('[');
                        for (idx, index) in indices.iter().enumerate() {
                            if idx > 0 {
                                out.push_str(", ");
                            }
                            out.push_str(&index.to_string());
                        }
                        out.push(']');
                    }
                    AccessPart::Partial(partial) => {
                        let (prefix, index) = match partial {
                            crate::value::PartialAccess::Bit(index) => ("X", *index),
                            crate::value::PartialAccess::Byte(index) => ("B", *index),
                            crate::value::PartialAccess::Word(index) => ("W", *index),
                            crate::value::PartialAccess::DWord(index) => ("D", *index),
                        };
                        out.push_str(".%");
                        out.push_str(prefix);
                        out.push_str(&index.to_string());
                    }
                }
            }
            SmolStr::new(out)
        }
    }
}

pub(super) fn resolve_access_path(
    runtime: &Runtime,
    path: &AccessPath,
) -> Result<ResolvedAccess, CompileError> {
    match path {
        AccessPath::Direct { address, .. } => Ok(ResolvedAccess::Direct(address.clone())),
        AccessPath::Parts(parts) => resolve_access_parts(runtime, parts),
    }
}

#[derive(Clone, Copy)]
enum AccessTargetPolicy {
    Writable,
    ReadOnly(&'static str),
    Forbidden(&'static str),
}

fn access_target_policy(runtime: &Runtime, path: &AccessPath) -> AccessTargetPolicy {
    let AccessPath::Parts(parts) = path else {
        return AccessTargetPolicy::Writable;
    };
    let mut names = parts.iter().filter_map(|part| match part {
        AccessPart::Name(name) => Some(name),
        AccessPart::Index(_) | AccessPart::Partial(_) => None,
    });
    let Some(program_name) = names.next() else {
        return AccessTargetPolicy::Writable;
    };
    let Some(variable_name) = names.next() else {
        return AccessTargetPolicy::Writable;
    };
    let Some(program) = runtime
        .programs()
        .values()
        .find(|program| program.name.eq_ignore_ascii_case(program_name.as_str()))
    else {
        return AccessTargetPolicy::Writable;
    };

    if program
        .temps
        .iter()
        .any(|variable| variable.name.eq_ignore_ascii_case(variable_name.as_str()))
    {
        return AccessTargetPolicy::Forbidden("VAR_TEMP");
    }
    let Some(variable) = program
        .vars
        .iter()
        .find(|variable| variable.name.eq_ignore_ascii_case(variable_name.as_str()))
    else {
        return AccessTargetPolicy::Writable;
    };
    if variable.external {
        AccessTargetPolicy::Forbidden("VAR_EXTERNAL")
    } else if variable.in_out {
        AccessTargetPolicy::Forbidden("VAR_IN_OUT")
    } else if variable.constant {
        AccessTargetPolicy::ReadOnly("CONSTANT")
    } else {
        AccessTargetPolicy::Writable
    }
}

fn resolve_access_parts(
    runtime: &Runtime,
    parts: &[AccessPart],
) -> Result<ResolvedAccess, CompileError> {
    let name_positions: Vec<(usize, &SmolStr)> = parts
        .iter()
        .enumerate()
        .filter_map(|(idx, part)| match part {
            AccessPart::Name(name) => Some((idx, name)),
            _ => None,
        })
        .collect();

    for (pos, name) in name_positions {
        let mut value_ref = if let Some(reference) = runtime.storage().ref_for_global(name.as_ref())
        {
            reference
        } else {
            let mut matched = None;
            for program in runtime.programs().values() {
                let Some(Value::Instance(id)) = runtime.storage().get_global(program.name.as_ref())
                else {
                    continue;
                };
                let Some(reference) = runtime.storage().ref_for_instance(*id, name.as_ref()) else {
                    continue;
                };
                if matched.is_some() {
                    matched = None;
                    break;
                }
                matched = Some(reference);
            }
            let Some(reference) = matched else {
                continue;
            };
            reference
        };
        let mut current_value = runtime
            .storage()
            .read_by_ref(value_ref.clone())
            .cloned()
            .ok_or_else(|| CompileError::new("invalid access path reference"))?;
        let mut partial = None;

        for part in &parts[pos + 1..] {
            match part {
                AccessPart::Index(indices) => {
                    value_ref
                        .path
                        .push(crate::value::RefSegment::Index(
                            crate::value::ref_indices_from_iter(indices.iter().copied()),
                        ));
                    current_value = runtime
                        .storage()
                        .read_by_ref(value_ref.clone())
                        .cloned()
                        .ok_or_else(|| CompileError::new("invalid access path index"))?;
                }
                AccessPart::Name(field) => {
                    if let Value::Instance(id) = current_value {
                        value_ref = runtime
                            .storage()
                            .ref_for_instance(id, field.as_ref())
                            .ok_or_else(|| {
                                CompileError::new(
                                    "invalid access path instance field; VAR_TEMP and VAR_EXTERNAL cannot be exposed",
                                )
                            })?;
                        current_value = runtime
                            .storage()
                            .read_by_ref(value_ref.clone())
                            .cloned()
                            .ok_or_else(|| CompileError::new("invalid access path"))?;
                    } else {
                        value_ref
                            .path
                            .push(crate::value::RefSegment::Field(field.clone()));
                        current_value =
                            runtime
                                .storage()
                                .read_by_ref(value_ref.clone())
                                .cloned()
                                .ok_or_else(|| CompileError::new("invalid access path field"))?;
                    }
                }
                AccessPart::Partial(access) => {
                    partial = Some(*access);
                    break;
                }
            }
        }

        return Ok(ResolvedAccess::Variable {
            reference: value_ref,
            partial,
        });
    }

    Err(CompileError::new("unresolved access path"))
}
