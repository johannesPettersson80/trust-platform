fn import_project_model_to_sources(
    root: roxmltree::Node<'_, '_>,
    sources_root: &Path,
    seen_files: &mut HashSet<PathBuf>,
    warnings: &mut Vec<String>,
    unsupported_nodes: &mut Vec<String>,
    unsupported_diagnostics: &mut Vec<PlcopenUnsupportedDiagnostic>,
    loss_warnings: &mut usize,
) -> anyhow::Result<ImportProjectModelStats> {
    let mut stats = ImportProjectModelStats::default();
    let mut configurations = Vec::new();

    for instances in root
        .children()
        .filter(|child| is_element_named_ci(*child, "instances"))
    {
        let mut discovered = false;
        for holder in instances
            .children()
            .filter(|child| is_element_named_ci(*child, "configurations"))
        {
            for configuration in holder
                .children()
                .filter(|child| is_element_named_ci(*child, "configuration"))
            {
                configurations.push(parse_configuration_model(configuration));
                discovered = true;
            }
        }
        if !discovered {
            for configuration in instances
                .children()
                .filter(|child| is_element_named_ci(*child, "configuration"))
            {
                configurations.push(parse_configuration_model(configuration));
                discovered = true;
            }
        }
        if !discovered {
            let direct_resources = instances
                .children()
                .filter(|child| is_element_named_ci(*child, "resource"))
                .collect::<Vec<_>>();
            if !direct_resources.is_empty() {
                let mut synthetic = ConfigurationDecl {
                    name: "ImportedConfiguration".to_string(),
                    tasks: Vec::new(),
                    programs: Vec::new(),
                    resources: Vec::new(),
                };
                for resource in direct_resources {
                    synthetic.resources.push(parse_resource_model(resource));
                }
                configurations.push(synthetic);
                discovered = true;
            }
        }
        if !discovered {
            unsupported_nodes.push("instances".to_string());
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO501",
                "warning",
                "instances",
                "PLCopen instances section is present but does not contain importable configuration/resource nodes",
                None,
                "Provide <configuration> entries under <instances>/<configurations> or direct <instances>",
            ));
            *loss_warnings += 1;
        }
    }

    stats.discovered_configurations = configurations.len();
    if configurations.is_empty() {
        return Ok(stats);
    }

    let mut used_configuration_names = HashSet::new();
    for (index, mut configuration) in configurations.into_iter().enumerate() {
        let default_name = format!("ImportedConfiguration{}", index + 1);
        let mut configuration_name = sanitize_st_identifier(&configuration.name, &default_name);
        if configuration_name != configuration.name {
            warnings.push(format!(
                "normalized configuration name '{}' -> '{}'",
                configuration.name, configuration_name
            ));
        }
        configuration_name = unique_identifier(configuration_name, &mut used_configuration_names);
        configuration.name = configuration_name;

        normalize_configuration_model(
            &mut configuration,
            warnings,
            unsupported_diagnostics,
            loss_warnings,
        );

        let source_text = render_configuration_source(&configuration);
        let path = unique_source_path(
            sources_root,
            &format!("plcopen_configuration_{}", configuration.name),
            seen_files,
        );
        write_text_file_with_parents(&path, &source_text).with_context(|| {
            format!(
                "failed to write imported configuration '{}'",
                path.display()
            )
        })?;
        stats.written_sources.push(path);
        stats.imported_configurations += 1;
        stats.imported_resources += configuration.resources.len();
        stats.imported_tasks += configuration.tasks.len();
        stats.imported_program_instances += configuration.programs.len();
        for resource in &configuration.resources {
            stats.imported_tasks += resource.tasks.len();
            stats.imported_program_instances += resource.programs.len();
        }
    }

    if stats.imported_configurations > 0 {
        warnings.push(format!(
            "imported {} PLCopen configuration(s), {} resource(s), {} task(s), {} program instance(s)",
            stats.imported_configurations,
            stats.imported_resources,
            stats.imported_tasks,
            stats.imported_program_instances
        ));
    }

    Ok(stats)
}

fn parse_configuration_model(node: roxmltree::Node<'_, '_>) -> ConfigurationDecl {
    let mut tasks = Vec::new();
    let mut programs = Vec::new();
    let mut resources = Vec::new();

    for child in node.children().filter(|child| child.is_element()) {
        if is_element_named_ci(child, "task") {
            if let Some(task) = parse_task_model(child) {
                tasks.push(task);
            }
        } else if let Some(program) = parse_program_instance_model(child, None) {
            programs.push(program);
        } else if is_element_named_ci(child, "resource") {
            resources.push(parse_resource_model(child));
        } else if is_element_named_ci(child, "resources") {
            for resource in child
                .children()
                .filter(|entry| is_element_named_ci(*entry, "resource"))
            {
                resources.push(parse_resource_model(resource));
            }
        }
    }

    ConfigurationDecl {
        name: attribute_ci_any(&node, &["name", "configurationName"])
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "ImportedConfiguration".to_string()),
        tasks,
        programs,
        resources,
    }
}

fn parse_resource_model(node: roxmltree::Node<'_, '_>) -> ResourceDecl {
    let mut tasks = Vec::new();
    let mut programs = Vec::new();

    for child in node.children().filter(|child| child.is_element()) {
        if is_element_named_ci(child, "task") {
            if let Some(task) = parse_task_model(child) {
                let task_name = task.name.clone();
                tasks.push(task);
                for nested in child.children().filter(|entry| entry.is_element()) {
                    if let Some(program) = parse_program_instance_model(nested, Some(&task_name)) {
                        programs.push(program);
                    }
                }
            }
        } else if let Some(program) = parse_program_instance_model(child, None) {
            programs.push(program);
        }
    }

    ResourceDecl {
        name: attribute_ci_any(&node, &["name", "resourceName"])
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "ImportedResource".to_string()),
        target: attribute_ci_any(&node, &["target", "type", "on"])
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "CPU".to_string()),
        tasks,
        programs,
    }
}

fn parse_task_model(node: roxmltree::Node<'_, '_>) -> Option<TaskDecl> {
    let name = attribute_ci_any(&node, &["name", "taskName"])
        .or_else(|| {
            node.children()
                .find(|child| is_element_named_ci(*child, "name"))
                .and_then(extract_text_content)
        })?
        .trim()
        .to_string();
    if name.is_empty() {
        return None;
    }

    let interval = attribute_ci_any(&node, &["interval", "cycle", "cycleTime", "period"])
        .or_else(|| {
            node.children()
                .find(|child| is_element_named_ci(*child, "interval"))
                .and_then(|entry| {
                    attribute_ci_any(&entry, &["value"]).or_else(|| extract_text_content(entry))
                })
        })
        .map(|value| normalize_task_interval_literal(&value));

    let single = attribute_ci_any(&node, &["single", "event", "trigger"]);
    let priority = attribute_ci_any(&node, &["priority"]);

    Some(TaskDecl {
        name,
        interval,
        single,
        priority,
    })
}

fn parse_program_instance_model(
    node: roxmltree::Node<'_, '_>,
    inherited_task_name: Option<&str>,
) -> Option<ProgramBindingDecl> {
    let node_name = node.tag_name().name();
    if !node_name.eq_ignore_ascii_case("program")
        && !node_name.eq_ignore_ascii_case("pouInstance")
        && !node_name.eq_ignore_ascii_case("programInstance")
        && !node_name.eq_ignore_ascii_case("instance")
    {
        return None;
    }

    let instance_name = attribute_ci_any(&node, &["name", "instanceName", "programName"])
        .or_else(|| {
            node.children()
                .find(|child| is_element_named_ci(*child, "name"))
                .and_then(extract_text_content)
        })?
        .trim()
        .to_string();
    if instance_name.is_empty() {
        return None;
    }

    let type_name = attribute_ci_any(&node, &["typeName", "type", "pouName", "programType"])
        .or_else(|| {
            node.children()
                .find(|child| is_element_named_ci(*child, "type"))
                .and_then(|entry| {
                    attribute_ci_any(&entry, &["name"]).or_else(|| extract_text_content(entry))
                })
        })?
        .trim()
        .to_string();
    if type_name.is_empty() {
        return None;
    }

    let task_name = attribute_ci_any(&node, &["task", "taskName", "withTask"])
        .or_else(|| inherited_task_name.map(ToOwned::to_owned))
        .filter(|value| !value.trim().is_empty());

    Some(ProgramBindingDecl {
        instance_name,
        task_name,
        type_name,
    })
}

fn normalize_configuration_model(
    configuration: &mut ConfigurationDecl,
    warnings: &mut Vec<String>,
    unsupported_diagnostics: &mut Vec<PlcopenUnsupportedDiagnostic>,
    loss_warnings: &mut usize,
) {
    let mut used_resource_names = HashSet::new();
    let mut used_task_names = HashSet::new();
    let mut used_program_names = HashSet::new();

    configuration.name = sanitize_st_identifier(&configuration.name, "ImportedConfiguration");
    for task in &mut configuration.tasks {
        let original = task.name.clone();
        let mut normalized = sanitize_st_identifier(&task.name, "Task");
        normalized = unique_identifier(normalized, &mut used_task_names);
        if normalized != original {
            warnings.push(format!(
                "normalized task name '{}' -> '{}' in configuration '{}'",
                original, normalized, configuration.name
            ));
        }
        task.name = normalized;
    }
    for program in &mut configuration.programs {
        let original = program.instance_name.clone();
        let mut normalized = sanitize_st_identifier(&program.instance_name, "Program");
        normalized = unique_identifier(normalized, &mut used_program_names);
        if normalized != original {
            warnings.push(format!(
                "normalized program instance name '{}' -> '{}' in configuration '{}'",
                original, normalized, configuration.name
            ));
        }
        program.instance_name = normalized;
        program.type_name = sanitize_st_identifier(&program.type_name, "MainProgram");
        if let Some(task_name) = &program.task_name {
            let normalized_task = sanitize_st_identifier(task_name, "Task");
            if used_task_names.contains(&normalized_task.to_ascii_lowercase()) {
                program.task_name = Some(normalized_task);
            } else if let Some(first) = configuration.tasks.first() {
                program.task_name = Some(first.name.clone());
            }
        }
    }

    for resource in &mut configuration.resources {
        let original = resource.name.clone();
        let mut normalized = sanitize_st_identifier(&resource.name, "Resource");
        normalized = unique_identifier(normalized, &mut used_resource_names);
        if normalized != original {
            warnings.push(format!(
                "normalized resource name '{}' -> '{}' in configuration '{}'",
                original, normalized, configuration.name
            ));
        }
        resource.name = normalized;
        resource.target = sanitize_st_identifier(&resource.target, "CPU");

        let mut local_task_names = HashSet::new();
        let mut local_program_names = HashSet::new();
        for task in &mut resource.tasks {
            let original = task.name.clone();
            let mut task_name = sanitize_st_identifier(&task.name, "Task");
            task_name = unique_identifier(task_name, &mut local_task_names);
            if task_name != original {
                warnings.push(format!(
                    "normalized task name '{}' -> '{}' in resource '{}'",
                    original, task_name, resource.name
                ));
            }
            task.name = task_name;
        }
        for program in &mut resource.programs {
            let original = program.instance_name.clone();
            let mut program_name = sanitize_st_identifier(&program.instance_name, "Program");
            program_name = unique_identifier(program_name, &mut local_program_names);
            if program_name != original {
                warnings.push(format!(
                    "normalized program instance name '{}' -> '{}' in resource '{}'",
                    original, program_name, resource.name
                ));
            }
            program.instance_name = program_name;
            program.type_name = sanitize_st_identifier(&program.type_name, "MainProgram");
            if let Some(task_name) = &program.task_name {
                let task_name = sanitize_st_identifier(task_name, "Task");
                program.task_name = Some(task_name);
            }
        }

        if !resource.programs.is_empty() && resource.tasks.is_empty() {
            let auto_task_name = unique_identifier("AutoTask".to_string(), &mut local_task_names);
            resource.tasks.push(TaskDecl {
                name: auto_task_name.clone(),
                interval: Some("T#100ms".to_string()),
                single: None,
                priority: Some("1".to_string()),
            });
            for program in &mut resource.programs {
                if program.task_name.is_none() {
                    program.task_name = Some(auto_task_name.clone());
                }
            }
            warnings.push(format!(
                "resource '{}' had PROGRAM instances without TASK declarations; generated TASK '{}'",
                resource.name, auto_task_name
            ));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO506",
                "info",
                format!("instances/resource/{}", resource.name),
                "Generated deterministic fallback TASK for resource PROGRAM bindings",
                None,
                "Review generated configuration task timing and priority",
            ));
        }
    }

    if !configuration.programs.is_empty()
        && configuration.tasks.is_empty()
        && configuration.resources.is_empty()
    {
        let auto_task_name = unique_identifier("AutoTask".to_string(), &mut used_task_names);
        configuration.tasks.push(TaskDecl {
            name: auto_task_name.clone(),
            interval: Some("T#100ms".to_string()),
            single: None,
            priority: Some("1".to_string()),
        });
        for program in &mut configuration.programs {
            if program.task_name.is_none() {
                program.task_name = Some(auto_task_name.clone());
            }
        }
        warnings.push(format!(
            "configuration '{}' had PROGRAM instances without TASK declarations; generated TASK '{}'",
            configuration.name, auto_task_name
        ));
        unsupported_diagnostics.push(unsupported_diagnostic(
            "PLCO507",
            "info",
            format!("instances/configuration/{}", configuration.name),
            "Generated deterministic fallback TASK for configuration-level PROGRAM bindings",
            None,
            "Review generated configuration task timing and priority",
        ));
    }

    if configuration.tasks.is_empty()
        && configuration.programs.is_empty()
        && configuration.resources.is_empty()
    {
        *loss_warnings += 1;
        unsupported_diagnostics.push(unsupported_diagnostic(
            "PLCO508",
            "warning",
            format!("instances/configuration/{}", configuration.name),
            "Configuration is empty after import normalization",
            None,
            "Add TASK/PROGRAM/RESOURCE entries to preserve runtime scheduling intent",
        ));
    }
}

fn render_configuration_source(configuration: &ConfigurationDecl) -> String {
    let mut out = String::new();
    out.push_str(&format!("CONFIGURATION {}\n", configuration.name));
    for task in &configuration.tasks {
        out.push_str(&format!("{}\n", format_task_declaration(task)));
    }
    for program in &configuration.programs {
        out.push_str(&format!("{}\n", format_program_binding(program)));
    }
    for resource in &configuration.resources {
        out.push_str(&format!(
            "RESOURCE {} ON {}\n",
            resource.name, resource.target
        ));
        for task in &resource.tasks {
            out.push_str("    ");
            out.push_str(&format_task_declaration(task));
            out.push('\n');
        }
        for program in &resource.programs {
            out.push_str("    ");
            out.push_str(&format_program_binding(program));
            out.push('\n');
        }
        out.push_str("END_RESOURCE\n");
    }
    out.push_str("END_CONFIGURATION\n");
    out
}

