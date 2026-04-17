pub(crate) fn lower_configuration(
    syntax: &SyntaxNode,
    registry: &mut trust_hir::types::TypeRegistry,
    inputs: &mut LoweringInputs<'_>,
) -> Result<Option<ConfigModel>, CompileError> {
    let configs: Vec<SyntaxNode> = syntax
        .descendants()
        .filter(|child| child.kind() == SyntaxKind::Configuration)
        .collect();
    if configs.is_empty() {
        return Ok(None);
    }
    if configs.len() > 1 {
        return Err(CompileError::new(
            "multiple CONFIGURATION declarations not supported",
        ));
    }
    let config = configs[0].clone();
    let using = collect_using_directives(&config);
    let mut ctx = inputs.context(registry, using);
    let mut globals = Vec::new();
    let mut tasks = Vec::new();
    let mut programs = Vec::new();
    let mut access = Vec::new();
    let mut config_inits = Vec::new();

    for child in config.children() {
        match child.kind() {
            SyntaxKind::VarBlock => globals.extend(lower_global_var_block(&child, &mut ctx)?),
            SyntaxKind::TaskConfig => tasks.push(lower_task_config(&child, &mut ctx)?),
            SyntaxKind::ProgramConfig => programs.push(lower_program_config(&child, &mut ctx)?),
            SyntaxKind::VarAccessBlock => {
                let result = lower_var_access_block(&child, &mut ctx)?;
                globals.extend(result.globals);
                access.extend(result.access);
            }
            SyntaxKind::VarConfigBlock => {
                config_inits.extend(lower_var_config_block(&child, &mut ctx)?);
            }
            SyntaxKind::Resource => {
                let resource = child;
                for res_child in resource.children() {
                    match res_child.kind() {
                        SyntaxKind::VarBlock => {
                            globals.extend(lower_global_var_block(&res_child, &mut ctx)?)
                        }
                        SyntaxKind::TaskConfig => {
                            tasks.push(lower_task_config(&res_child, &mut ctx)?)
                        }
                        SyntaxKind::ProgramConfig => {
                            programs.push(lower_program_config(&res_child, &mut ctx)?)
                        }
                        SyntaxKind::VarAccessBlock => {
                            let result = lower_var_access_block(&res_child, &mut ctx)?;
                            globals.extend(result.globals);
                            access.extend(result.access);
                        }
                        SyntaxKind::VarConfigBlock => {
                            config_inits.extend(lower_var_config_block(&res_child, &mut ctx)?);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    Ok(Some(ConfigModel {
        globals,
        tasks,
        programs,
        using: ctx.using.clone(),
        access,
        config_inits,
    }))
}
