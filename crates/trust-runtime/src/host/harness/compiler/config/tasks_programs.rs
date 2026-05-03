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
        for child in init.children() {
            match child.kind() {
                SyntaxKind::Name => {
                    current_key = Some(node_text(&child));
                }
                _ if is_expression_kind(child.kind()) => {
                    if let Some(key) = current_key.take() {
                        match key.to_ascii_uppercase().as_str() {
                            "INTERVAL" => {
                                interval = const_duration_from_node(&child, ctx)?;
                            }
                            "SINGLE" => {
                                let name = extract_name_from_expr(&child).ok_or_else(|| {
                                    CompileError::new("invalid SINGLE expression")
                                })?;
                                single = Some(name);
                            }
                            "PRIORITY" => {
                                let value = const_int_from_node(&child, ctx)?;
                                priority = u32::try_from(value).map_err(|_| {
                                    CompileError::new("TASK PRIORITY must be non-negative")
                                })?;
                            }
                            _ => {}
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

    if let Some(list) = node
        .children()
        .find(|child| child.kind() == SyntaxKind::ProgramConfigList)
    {
        fb_tasks = lower_program_config_list(&list, ctx)?;
    }

    Ok(ProgramInstanceConfig {
        name: instance,
        type_name,
        task,
        retain,
        fb_tasks,
    })
}

fn lower_program_config_list(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Vec<FbTaskBinding>, CompileError> {
    let mut bindings = Vec::new();
    for elem in node
        .children()
        .filter(|child| child.kind() == SyntaxKind::ProgramConfigElem)
    {
        let mut seen_with = false;
        let mut task_name: Option<SmolStr> = None;
        for element in elem.children_with_tokens() {
            if let Some(token) = element.as_token() {
                if token.kind() == SyntaxKind::KwWith {
                    seen_with = true;
                }
                continue;
            }
            let Some(child) = element.as_node() else {
                continue;
            };
            if child.kind() == SyntaxKind::Name && seen_with {
                task_name = Some(SmolStr::new(node_text(child)));
                break;
            }
        }

        if let Some(task) = task_name {
            let path_node = elem
                .children()
                .find(|child| child.kind() == SyntaxKind::AccessPath)
                .ok_or_else(|| CompileError::new("missing access path for FB task"))?;
            let path = parse_access_path(&path_node, ctx)?;
            bindings.push(FbTaskBinding { path, task });
        }
    }
    Ok(bindings)
}
