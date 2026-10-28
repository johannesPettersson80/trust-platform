fn lower_task_config(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<crate::task::TaskConfig, CompileError> {
    let name_node = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)
        .ok_or_else(|| CompileError::new("missing task name"))?;
    let name = SmolStr::new(node_text(&name_node));

    let mut interval = Duration::ZERO;
    let mut single = None;
    let mut priority: u32 = 0;

    if let Some(init) = node
        .children()
        .find(|child| child.kind() == SyntaxKind::TaskInit)
    {
        let mut current_key: Option<String> = None;
        let mut seen_keys = std::collections::HashSet::new();
        for child in init.children() {
            match child.kind() {
                SyntaxKind::Name => {
                    let key = node_text(&child);
                    let canonical = key.to_ascii_uppercase();
                    if !matches!(canonical.as_str(), "INTERVAL" | "SINGLE" | "PRIORITY") {
                        return Err(CompileError::new(format!(
                            "unknown TASK initializer field '{key}'"
                        )));
                    }
                    if !seen_keys.insert(canonical) {
                        return Err(CompileError::new(format!(
                            "duplicate TASK initializer field '{key}'"
                        )));
                    }
                    current_key = Some(key);
                }
                _ if is_expression_kind(child.kind()) => {
                    if let Some(key) = current_key.take() {
                        match key.to_ascii_uppercase().as_str() {
                            "INTERVAL" => {
                                if child.text().to_string().contains('-') {
                                    return Err(CompileError::new(
                                        "TASK INTERVAL must be non-negative",
                                    ));
                                }
                                interval = const_duration_from_node(&child, ctx).map_err(|error| {
                                    CompileError::new(format!(
                                        "invalid TASK INTERVAL: {error}"
                                    ))
                                })?;
                            }
                            "SINGLE" => {
                                let name = extract_name_from_expr(&child).ok_or_else(|| {
                                    CompileError::new(
                                        "TASK SINGLE requires a named BOOL storage variable",
                                    )
                                })?;
                                single = Some(name);
                            }
                            "PRIORITY" => {
                                let value = const_int_from_node(&child, ctx).map_err(|error| {
                                    CompileError::new(format!(
                                        "TASK PRIORITY is outside the supported u32 range: {error}"
                                    ))
                                })?;
                                priority = u32::try_from(value).map_err(|_| {
                                    CompileError::new(
                                        "TASK PRIORITY must be non-negative and fit u32",
                                    )
                                })?;
                            }
                            _ => unreachable!(),
                        }
                    }
                }
                _ => {}
            }
        }
    }

    Ok(crate::task::TaskConfig {
        name,
        interval,
        single,
        priority,
        programs: Vec::new(),
        fb_instances: Vec::new(),
    })
}

fn lower_program_config(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<ProgramInstanceConfig, CompileError> {
    let mut retain = None;
    let mut instance = None;
    let mut task = None;
    let mut fb_tasks = Vec::new();
    let mut type_node = None;
    let mut seen_with = false;
    for element in node.children_with_tokens() {
        if let Some(child) = element.as_node() {
            match child.kind() {
                SyntaxKind::Name => {
                    let name = SmolStr::new(node_text(child));
                    if seen_with {
                        task = Some(name);
                        seen_with = false;
                    } else if instance.is_none() {
                        instance = Some(name);
                    }
                }
                SyntaxKind::QualifiedName | SyntaxKind::TypeRef => {
                    type_node = Some(child.clone());
                }
                _ => {}
            }
            continue;
        }

        let Some(token) = element.as_token() else {
            continue;
        };
        if token.kind().is_trivia() {
            continue;
        }
        match token.kind() {
            SyntaxKind::KwRetain => retain = Some(crate::RetainPolicy::Retain),
            SyntaxKind::KwNonRetain => retain = Some(crate::RetainPolicy::NonRetain),
            SyntaxKind::KwWith => seen_with = true,
            _ => {}
        }
    }

    let instance = instance.ok_or_else(|| CompileError::new("missing program instance name"))?;
    let type_node = type_node.ok_or_else(|| CompileError::new("missing program type"))?;
    let type_name = SmolStr::new(node_text(&type_node));

    let mut connections = Vec::new();
    if let Some(list) = node
        .children()
        .find(|child| child.kind() == SyntaxKind::ProgramConfigList)
    {
        (fb_tasks, connections) = lower_program_config_list(&list, ctx)?;
    }

    Ok(ProgramInstanceConfig {
        name: instance,
        type_name,
        task,
        retain,
        fb_tasks,
        connections,
    })
}

/// The function block task bindings (IEC 61131-3 Table 62 6b) and the connections of the
/// program's inputs and outputs (8a, 8b, 9a, 9b) of a program configuration's list.
fn lower_program_config_list(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<(Vec<FbTaskBinding>, Vec<ProgramConnection>), CompileError> {
    let mut bindings = Vec::new();
    let mut connections = Vec::new();
    for elem in node
        .children()
        .filter(|child| child.kind() == SyntaxKind::ProgramConfigElem)
    {
        let mut seen_with = false;
        let mut task_name: Option<SmolStr> = None;
        let mut output = None;
        for element in elem.children_with_tokens() {
            if let Some(token) = element.as_token() {
                match token.kind() {
                    SyntaxKind::KwWith => seen_with = true,
                    SyntaxKind::Assign => output = Some(false),
                    SyntaxKind::Arrow => output = Some(true),
                    _ => {}
                }
                continue;
            }
            let Some(child) = element.as_node() else {
                continue;
            };
            if child.kind() == SyntaxKind::Name && seen_with && task_name.is_none() {
                task_name = Some(SmolStr::new(node_text(child)));
            }
        }
        let Some(path_node) = elem
            .children()
            .find(|child| child.kind() == SyntaxKind::AccessPath)
        else {
            continue;
        };
        if let Some(task) = task_name {
            let path = parse_access_path(&path_node, ctx)?;
            bindings.push(FbTaskBinding { path, task });
        } else if let Some(output) = output {
            connections.push(lower_program_connection(&elem, &path_node, output, ctx)?);
        }
    }
    Ok((bindings, connections))
}

/// The connection of a program's input (`:=`) or output (`=>`) (IEC 61131-3 §6.8.2, Table 62 8a,
/// 8b, 9a, 9b): an input reads a constant, a global variable or a directly represented variable,
/// an output writes a global variable or a directly represented variable.
fn lower_program_connection(
    elem: &SyntaxNode,
    path_node: &SyntaxNode,
    output: bool,
    ctx: &mut LoweringContext<'_>,
) -> Result<ProgramConnection, CompileError> {
    let named = match parse_access_path(path_node, ctx)? {
        AccessPath::Parts(parts) => match parts.as_slice() {
            [AccessPart::Name(name)] => Some(name.clone()),
            _ => None,
        },
        AccessPath::Direct { .. } => None,
    };
    let parameter = named.ok_or_else(|| {
        CompileError::new(format!(
            "the program connection {} names no input or output of the program",
            path_node.text().to_string().trim()
        ))
    })?;
    let value = elem
        .children()
        .find(|child| child.kind() != SyntaxKind::AccessPath && is_expression_kind(child.kind()))
        .ok_or_else(|| {
            CompileError::new(format!("the connection of {parameter} connects nothing"))
        })?;
    let end = if let Some((address, text)) = connected_address(&value)? {
        ConnectionEnd::Address { address, text }
    } else if let Some(parts) = connected_variable(&value, ctx)? {
        ConnectionEnd::Global(parts)
    } else if output {
        return Err(CompileError::new(format!(
            "the output {parameter} is connected to {}: an output writes a global variable or a \
             directly represented variable (IEC 61131-3 Table 62 9a, 9b)",
            value.text().to_string().trim()
        )));
    } else {
        ConnectionEnd::Constant(lower_expr_with_context(&value, ctx, None)?)
    };
    Ok(ProgramConnection {
        parameter,
        output,
        end,
    })
}

/// The directly represented variable `node` names, `%IX1.1`, with its text; `None` for another
/// expression.
fn connected_address(node: &SyntaxNode) -> Result<Option<(IoAddress, SmolStr)>, CompileError> {
    if node.kind() != SyntaxKind::NameRef {
        return Ok(None);
    }
    let Some(token) = node
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
    else {
        return Ok(None);
    };
    if token.kind() != SyntaxKind::DirectAddress {
        return Ok(None);
    }
    let text = SmolStr::new(token.text());
    IoAddress::parse(&text)
        .map(|address| Some((address, text)))
        .map_err(|error| CompileError::new(error.to_string()))
}

/// The variable `node` names: a name, then members and constant indices; `None` for another
/// expression.
fn connected_variable(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Option<Vec<AccessPart>>, CompileError> {
    match node.kind() {
        SyntaxKind::NameRef => {
            let plain = node
                .children_with_tokens()
                .filter_map(|element| element.into_token())
                .filter(|token| !token.kind().is_trivia())
                .all(|token| token.kind() == SyntaxKind::Ident);
            Ok(plain
                .then(|| extract_name_from_expr(node))
                .flatten()
                .map(|name| vec![AccessPart::Name(name)]))
        }
        SyntaxKind::FieldExpr => {
            let mut children = node.children();
            let (Some(base), Some(member)) = (children.next(), children.next()) else {
                return Ok(None);
            };
            if member.kind() != SyntaxKind::Name {
                return Ok(None);
            }
            let Some(mut parts) = connected_variable(&base, ctx)? else {
                return Ok(None);
            };
            parts.push(AccessPart::Name(SmolStr::new(node_text(&member))));
            Ok(Some(parts))
        }
        SyntaxKind::IndexExpr => {
            let mut children = node.children();
            let Some(base) = children.next() else {
                return Ok(None);
            };
            let Some(mut parts) = connected_variable(&base, ctx)? else {
                return Ok(None);
            };
            let mut indices = Vec::new();
            for index in children {
                indices.push(const_int_from_node(&index, ctx)?);
            }
            parts.push(AccessPart::Index(indices));
            Ok(Some(parts))
        }
        _ => Ok(None),
    }
}
