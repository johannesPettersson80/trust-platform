fn import_data_types_to_sources(
    root: roxmltree::Node<'_, '_>,
    sources_root: &Path,
    seen_files: &mut HashSet<PathBuf>,
    warnings: &mut Vec<String>,
    unsupported_nodes: &mut Vec<String>,
    unsupported_diagnostics: &mut Vec<PlcopenUnsupportedDiagnostic>,
    loss_warnings: &mut usize,
) -> anyhow::Result<Option<(PathBuf, usize)>> {
    let mut declarations = Vec::new();
    let mut imported_count = 0usize;
    let mut seen_names = BTreeSet::new();

    for data_type in root
        .descendants()
        .filter(|node| is_element_named_ci(*node, "dataType"))
        .filter(|node| {
            node.ancestors()
                .any(|ancestor| is_element_named_ci(ancestor, "dataTypes"))
        })
    {
        let Some(name) = attribute_ci(data_type, "name")
            .or_else(|| {
                data_type
                    .children()
                    .find(|child| is_element_named_ci(*child, "name"))
                    .and_then(extract_text_content)
            })
            .map(|raw| raw.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            unsupported_nodes.push("types/dataTypes/unnamed".to_string());
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO401",
                "warning",
                "types/dataTypes/dataType",
                "dataType entry skipped because required name attribute is missing",
                None,
                "Provide a non-empty dataType name before import",
            ));
            *loss_warnings += 1;
            continue;
        };

        let name_key = name.to_ascii_lowercase();
        if !seen_names.insert(name_key) {
            unsupported_nodes.push(format!("types/dataTypes/{name}"));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO403",
                "warning",
                format!("types/dataTypes/{name}"),
                format!("dataType '{}' skipped because the name is duplicated", name),
                None,
                "Rename duplicate dataType entries to unique names before import",
            ));
            *loss_warnings += 1;
            continue;
        }

        let Some(type_expr) = parse_data_type_expression(data_type) else {
            unsupported_nodes.push(format!("types/dataTypes/{name}"));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO402",
                "warning",
                format!("types/dataTypes/{name}"),
                format!(
                    "dataType '{}' uses an unsupported or missing baseType representation",
                    name
                ),
                None,
                "Supported baseType subset: elementary, derived, array, struct, enum, subrange",
            ));
            *loss_warnings += 1;
            continue;
        };

        declarations.push(format_data_type_declaration(&name, &type_expr));
        imported_count += 1;
    }

    if imported_count == 0 {
        return Ok(None);
    }

    let mut source = String::from("TYPE\n");
    for declaration in declarations {
        source.push_str(&declaration);
        source.push('\n');
    }
    source.push_str("END_TYPE\n");

    let path = unique_source_path(sources_root, GENERATED_DATA_TYPES_SOURCE_PREFIX, seen_files);
    write_text_file_with_parents(&path, &source)?;

    warnings.push(format!(
        "imported {} PLCopen dataType declaration(s) into {}",
        imported_count,
        path.display()
    ));
    Ok(Some((path, imported_count)))
}

#[allow(clippy::too_many_arguments)]
fn import_global_var_lists_to_sources(
    root: roxmltree::Node<'_, '_>,
    sources_root: &Path,
    seen_files: &mut HashSet<PathBuf>,
    warnings: &mut Vec<String>,
    unsupported_nodes: &mut Vec<String>,
    unsupported_diagnostics: &mut Vec<PlcopenUnsupportedDiagnostic>,
    loss_warnings: &mut usize,
    project_structure_map: &CodesysProjectStructureMap,
    imported_folder_paths: &mut HashSet<String>,
    global_var_mode: PlcopenImportGlobalVarMode,
) -> anyhow::Result<ImportGlobalVarStats> {
    let mut stats = ImportGlobalVarStats::default();

    for global_vars in root
        .descendants()
        .filter(|node| is_element_named_ci(*node, "globalVars"))
    {
        stats.discovered_global_var_lists += 1;

        let default_name = format!("GlobalVars{}", stats.discovered_global_var_lists);
        let name = attribute_ci(global_vars, "name")
            .or_else(|| {
                global_vars
                    .children()
                    .find(|child| is_element_named_ci(*child, "name"))
                    .and_then(extract_text_content)
            })
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or(default_name.clone());

        let global_source = extract_codesys_global_vars_plaintext(global_vars)
            .or_else(|| synthesize_global_vars_source(global_vars, warnings))
            .map(normalize_body_text);
        let Some(global_source) = global_source else {
            warnings.push(format!(
                "skipping globalVars '{}': missing plaintext/variable declarations",
                name
            ));
            unsupported_nodes.push(format!("addData/globalVars/{name}"));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO601",
                "warning",
                format!("addData/globalVars/{name}"),
                "globalVars entry skipped because no importable declarations were found",
                None,
                "Provide interface-as-plaintext or variable/type entries in the globalVars node",
            ));
            *loss_warnings += 1;
            continue;
        };

        let variables = parse_global_var_entries_from_st_block(&global_source);
        if variables.is_empty() {
            warnings.push(format!(
                "skipping globalVars '{}': no parseable VAR_GLOBAL declarations found",
                name
            ));
            unsupported_nodes.push(format!("addData/globalVars/{name}"));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO602",
                "warning",
                format!("addData/globalVars/{name}"),
                "globalVars plaintext was present but no declarations could be parsed",
                None,
                "Ensure declarations follow 'name : TYPE [:= value];' syntax inside VAR_GLOBAL",
            ));
            *loss_warnings += 1;
            continue;
        }

        let list_identifier = sanitize_st_identifier(&name, "GlobalVars");
        if !list_identifier.eq_ignore_ascii_case(&name) {
            warnings.push(format!(
                "normalized globalVars identifier '{}' -> '{}'",
                name, list_identifier
            ));
        }

        let var_global_header_suffix = extract_var_global_header_suffix(&global_source);
        let global_prefix = extract_leading_global_prefix(&global_source);
        let is_qualified_only = codesys_global_vars_is_qualified_only(global_vars, &global_source);

        let (rendered_source, qualified_external) = match (global_var_mode, is_qualified_only) {
            (PlcopenImportGlobalVarMode::StrictIecAdapter, true) => {
                let type_name = format!("{list_identifier}_TYPE");
                let configuration_name = format!("{list_identifier}_Globals");

                let mut rendered = String::new();
                rendered.push_str("TYPE\n");
                rendered.push_str(&format!("{type_name} : STRUCT\n"));
                for variable in &variables {
                    rendered.push_str("    ");
                    rendered.push_str(&format_global_var_declaration(variable));
                    rendered.push('\n');
                }
                rendered.push_str("END_STRUCT\n");
                rendered.push_str("END_TYPE\n\n");
                rendered.push_str(&format!("CONFIGURATION {configuration_name}\n"));
                rendered.push_str(&render_var_global_header(&var_global_header_suffix));
                rendered.push('\n');
                rendered.push_str(&format!("    {list_identifier} : {type_name};\n"));
                rendered.push_str("END_VAR\n");
                rendered.push_str("END_CONFIGURATION\n");

                warnings.push(format!(
                    "mapped qualified_only globalVars '{}' to strict-IEC configuration/type wrapper",
                    name
                ));
                (
                    rendered,
                    Some(QualifiedGlobalListExternalDecl {
                        list_name: list_identifier.clone(),
                        type_name,
                    }),
                )
            }
            (PlcopenImportGlobalVarMode::StrictIecAdapter, false) => {
                let configuration_name = format!("{list_identifier}_Globals");
                let mut rendered = String::new();
                rendered.push_str(&format!("CONFIGURATION {configuration_name}\n"));
                rendered.push_str(&render_var_global_header(&var_global_header_suffix));
                rendered.push('\n');
                for variable in &variables {
                    rendered.push_str("    ");
                    rendered.push_str(&format_global_var_declaration(variable));
                    rendered.push('\n');
                }
                rendered.push_str("END_VAR\n");
                rendered.push_str("END_CONFIGURATION\n");
                (rendered, None)
            }
            (_, true) => {
                let mut rendered = String::new();
                if !global_prefix.is_empty() {
                    rendered.push_str(&global_prefix);
                    if !global_prefix.ends_with('\n') {
                        rendered.push('\n');
                    }
                }
                rendered.push_str(&format!("NAMESPACE {list_identifier}\n"));
                rendered.push_str(&render_var_global_header(&var_global_header_suffix));
                rendered.push('\n');
                for variable in &variables {
                    rendered.push_str("    ");
                    rendered.push_str(&format_global_var_declaration(variable));
                    rendered.push('\n');
                }
                rendered.push_str("END_VAR\n");
                rendered.push_str("END_NAMESPACE\n");

                warnings.push(format!(
                    "mapped qualified_only globalVars '{}' to namespaced vendor-style GVL for native qualified access",
                    name
                ));
                (rendered, None)
            }
            (_, false) => {
                let mut rendered = String::new();
                if !global_prefix.is_empty() {
                    rendered.push_str(&global_prefix);
                    if !global_prefix.ends_with('\n') {
                        rendered.push('\n');
                    }
                }
                rendered.push_str(&render_var_global_header(&var_global_header_suffix));
                rendered.push('\n');
                for variable in &variables {
                    rendered.push_str("    ");
                    rendered.push_str(&format_global_var_declaration(variable));
                    rendered.push('\n');
                }
                rendered.push_str("END_VAR\n");
                (rendered, None)
            }
        };

        let folder_segments =
            resolve_codesys_folder_segments_for_node(global_vars, &name, project_structure_map);
        let candidate =
            unique_source_path_with_segments(sources_root, &folder_segments, &name, seen_files);
        write_text_file_with_parents(&candidate, &rendered_source)?;
        track_imported_folder_path(&candidate, sources_root, imported_folder_paths);

        if let Some(external_decl) = qualified_external {
            stats.qualified_list_externals.push(external_decl);
        }
        stats.imported_global_var_lists += 1;
        warnings.push(format!(
            "imported globalVars '{}' into {}",
            name,
            candidate.display()
        ));
        stats.written_sources.push(candidate);
    }

    Ok(stats)
}

fn extract_var_global_header_suffix(source: &str) -> String {
    for line in source.lines() {
        let trimmed = line.trim();
        let upper = trimmed.to_ascii_uppercase();
        if upper.starts_with("VAR_GLOBAL") {
            return trimmed["VAR_GLOBAL".len()..].trim().to_string();
        }
    }
    String::new()
}

fn extract_leading_global_prefix(source: &str) -> String {
    let mut prefix = Vec::new();
    for line in source.lines() {
        if line.trim().to_ascii_uppercase().starts_with("VAR_GLOBAL") {
            break;
        }
        prefix.push(line);
    }
    let prefix = prefix.join("\n");
    prefix.trim_end().to_string()
}

fn render_var_global_header(suffix: &str) -> String {
    if suffix.trim().is_empty() {
        "VAR_GLOBAL".to_string()
    } else {
        format!("VAR_GLOBAL {}", suffix.trim())
    }
}

fn format_global_var_declaration(variable: &GlobalVarVariableDecl) -> String {
    let mut declaration = format!("{} : {}", variable.name.trim(), variable.type_expr.trim());
    if let Some(initial_value) = variable.initial_value.as_deref() {
        let init = initial_value.trim();
        if !init.is_empty() {
            declaration.push_str(" := ");
            declaration.push_str(init);
        }
    }
    declaration.push(';');
    declaration
}

fn codesys_global_vars_is_qualified_only(node: roxmltree::Node<'_, '_>, source: &str) -> bool {
    let lowered_source = source.to_ascii_lowercase();
    if lowered_source.contains("attribute 'qualified_only'")
        || lowered_source.contains("attribute \"qualified_only\"")
    {
        return true;
    }

    for attribute in node
        .descendants()
        .filter(|entry| is_element_named_ci(*entry, "Attribute"))
    {
        let Some(name) = attribute_ci_any(&attribute, &["Name", "name"])
            .map(|value| value.trim().to_ascii_lowercase())
        else {
            continue;
        };
        if name != "qualified_only" {
            continue;
        }
        let value = attribute_ci_any(&attribute, &["Value", "value"]).unwrap_or_default();
        let normalized = value.trim().to_ascii_lowercase();
        if normalized.is_empty() || normalized == "1" || normalized == "true" {
            return true;
        }
    }

    false
}

fn extract_codesys_global_vars_plaintext(node: roxmltree::Node<'_, '_>) -> Option<String> {
    for data in node
        .descendants()
        .filter(|entry| is_element_named_ci(*entry, "data"))
    {
        let Some(name) = attribute_ci(data, "name") else {
            continue;
        };
        if !name.to_ascii_lowercase().contains("interfaceasplaintext")
            && !name.eq_ignore_ascii_case(CODESYS_INTERFACE_PLAINTEXT_DATA_NAME)
        {
            continue;
        }
        if let Some(text) = extract_text_content(data) {
            if !text.trim().is_empty() {
                return Some(text);
            }
        }
    }
    None
}

fn synthesize_global_vars_source(
    node: roxmltree::Node<'_, '_>,
    warnings: &mut Vec<String>,
) -> Option<String> {
    let mut out = String::new();
    for attribute in node
        .descendants()
        .filter(|entry| is_element_named_ci(*entry, "Attribute"))
    {
        let Some(name) = attribute_ci_any(&attribute, &["Name", "name"])
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if let Some(value) = attribute_ci_any(&attribute, &["Value", "value"]) {
            if value.trim().is_empty() {
                out.push_str(&format!("{{attribute '{}'}}\n", name));
            } else {
                out.push_str(&format!("{{attribute '{}': '{}'}}\n", name, value.trim()));
            }
        } else {
            out.push_str(&format!("{{attribute '{}'}}\n", name));
        }
    }

    let declarations = parse_interface_var_declarations(node);
    if declarations.is_empty() {
        return None;
    }
    warnings.push(
        "synthesized globalVars text from variable entries (interface-as-plaintext missing)"
            .to_string(),
    );
    out.push_str("VAR_GLOBAL\n");
    for declaration in declarations {
        out.push_str(&declaration);
        out.push('\n');
    }
    out.push_str("END_VAR\n");
    Some(out)
}
