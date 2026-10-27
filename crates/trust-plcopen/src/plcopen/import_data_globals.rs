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

        let extends_base = match extract_data_type_extends_base(data_type) {
            Ok(base) => base,
            Err(()) => {
                unsupported_nodes.push(format!("types/dataTypes/{name}/datatypeinheritance"));
                unsupported_diagnostics.push(unsupported_diagnostic(
                    "PLCO404",
                    "warning",
                    format!("types/dataTypes/{name}/datatypeinheritance"),
                    format!(
                        "dataType '{}' carries a CODESYS inheritance marker without a projectable base type",
                        name
                    ),
                    None,
                    "Provide an <Extends> element naming a STRUCT dataType in the project",
                ));
                *loss_warnings += 1;
                continue;
            }
        };

        declarations.push(format_data_type_declaration(
            &name,
            extends_base.as_deref(),
            &type_expr,
        ));
        if extends_base.is_some() {
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO220",
                "info",
                format!("types/dataTypes/{name}/datatypeinheritance"),
                format!(
                    "dataType '{}' imported with CODESYS DUT inheritance (TYPE x EXTENDS y)",
                    name
                ),
                None,
                "Review inherited STRUCT fields for semantic parity with the source vendor model",
            ));
        }
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

/// CODESYS DUT inheritance: e!COCKPIT stores `TYPE x EXTENDS y` as a
/// `datatypeinheritance` addData block on the dataType. `Ok(None)` means no
/// marker was present; `Ok(Some(base))` a projectable base; `Err(())` a
/// marker whose base could not be projected (fail-closed, reported).
fn extract_data_type_extends_base(data_type: roxmltree::Node<'_, '_>) -> Result<Option<String>, ()> {
    let mut marker_found = false;
    for data in data_type
        .descendants()
        .filter(|node| is_element_named_ci(*node, "data"))
    {
        let Some(name) = attribute_ci(data, "name") else {
            continue;
        };
        if !name.to_ascii_lowercase().contains("datatypeinheritance") {
            continue;
        }
        marker_found = true;
        for inheritance in data
            .descendants()
            .filter(|node| is_element_named_ci(*node, "Inheritance"))
        {
            for extends in inheritance
                .children()
                .filter(|child| is_element_named_ci(*child, "Extends"))
            {
                if let Some(text) = extract_text_content(extends) {
                    let base = text.trim();
                    if is_valid_qualified_identifier(base) {
                        return Ok(Some(base.to_string()));
                    }
                    return Err(());
                }
            }
        }
    }
    if marker_found {
        Err(())
    } else {
        Ok(None)
    }
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
    let mut used_list_identifiers = HashSet::new();
    let mut imported_lists_by_name: HashMap<String, Vec<GlobalVarVariableDecl>> = HashMap::new();
    // e!COCKPIT exports container qualifiers per list: a same-named
    // `constant="true"` mirror re-declares members the primary list
    // carries as plain variables. When the mirror drops as a duplicate,
    // its members must still land in the mirror's `VAR_GLOBAL CONSTANT`
    // section, or they lose constant-ness (PLCOPEN_DECISIONS 2026-10-26).
    let constant_names_by_list = constant_var_names_by_list(root);

    for global_vars in root
        .descendants()
        .filter(|node| is_element_named_ci(*node, "globalVars"))
    {
        stats.discovered_global_var_lists += 1;

        let default_name = format!("GlobalVars{}", stats.discovered_global_var_lists);
        let name = global_vars_list_name(global_vars, &default_name);

        if global_vars_is_mixed_attrs_view(global_vars) {
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO223",
                "info",
                format!("addData/globalVars/{name}"),
                format!(
                    "globalVars '{}' skipped as an e!COCKPIT MixedAttrsVarList attributes-view copy",
                    name
                ),
                None,
                "The vendor attributes view duplicates the primary list; only the primary list is imported",
            ));
            continue;
        }

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

        // e!COCKPIT MixedAttrsVarList parity: identical duplicate
        // declarations are dropped; conflicting redefinitions reject the
        // list fail-closed.
        let (variables, duplicate_conflicts, dropped_duplicates) =
            dedupe_global_var_entries(variables);
        if !duplicate_conflicts.is_empty() {
            warnings.push(format!(
                "skipping globalVars '{}': conflicting duplicate declarations for {}",
                name,
                duplicate_conflicts.join(", ")
            ));
            unsupported_nodes.push(format!("addData/globalVars/{name}"));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO604",
                "error",
                format!("addData/globalVars/{name}"),
                "globalVars declaration list contains conflicting duplicate declarations",
                None,
                "Provide a VAR_GLOBAL declaration list with unique IEC identifiers and matching types",
            ));
            *loss_warnings += 1;
            continue;
        }
        if dropped_duplicates > 0 {
            warnings.push(format!(
                "dropped {dropped_duplicates} identical duplicate declaration(s) from globalVars '{}'",
                name
            ));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO605",
                "info",
                format!("addData/globalVars/{name}"),
                format!("dropped {dropped_duplicates} identical duplicate declaration(s)"),
                None,
                "Identical duplicate declarations are tolerated for e!COCKPIT MixedAttrsVarList parity",
            ));
        }

        if !global_var_source_is_well_formed(&global_source)
            || !structured_global_var_entries_are_well_formed(global_vars)
        {
            warnings.push(format!(
                "skipping globalVars '{}': declaration list is malformed or ambiguous",
                name
            ));
            unsupported_nodes.push(format!("addData/globalVars/{name}"));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO604",
                "error",
                format!("addData/globalVars/{name}"),
                "globalVars declaration list contains malformed or invalid ST identities",
                None,
                "Provide a complete VAR_GLOBAL declaration list with unique IEC identifiers",
            ));
            *loss_warnings += 1;
            continue;
        }

        // e!COCKPIT duplicate whole-list copies (for example a
        // `constant="true"` mirror of the primary list) must not multiply
        // into `_N.st` mirror files that redeclare every member.
        let list_key = name.to_ascii_lowercase();
        if let Some(existing) = imported_lists_by_name.get(&list_key)
            && list_is_identical_duplicate(&variables, existing)
        {
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO223",
                "info",
                format!("addData/globalVars/{name}"),
                format!(
                    "globalVars '{}' dropped as an identical duplicate of the imported list",
                    name
                ),
                None,
                "e!COCKPIT duplicate list copies are merged into the primary list",
            ));
            continue;
        }

        let mut list_identifier = sanitize_st_identifier(&name, "GlobalVars");
        list_identifier = unique_identifier(list_identifier, &mut used_list_identifiers);
        if !list_identifier.eq_ignore_ascii_case(&name) {
            warnings.push(format!(
                "normalized globalVars identifier '{}' -> '{}'",
                name, list_identifier
            ));
        }

        let mut var_global_header_suffix = extract_var_global_header_suffix(&global_source);
        if merge_global_var_container_qualifiers(global_vars, &mut var_global_header_suffix) {
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO224",
                "info",
                format!("addData/globalVars/{name}"),
                format!(
                    "globalVars '{}' container qualifiers carried into the VAR_GLOBAL declaration header",
                    name
                ),
                None,
                "Retention/constancy attributes on the vendor container are preserved in the imported declaration",
            ));
        }
        let self_is_constant = attribute_ci(global_vars, "constant")
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("true"));
        let mirrored_constant_names = constant_names_by_list
            .get(&name.to_ascii_lowercase())
            .map(|names| {
                variables
                    .iter()
                    .filter(|variable| {
                        names.contains(&variable.name.to_ascii_lowercase())
                    })
                    .map(|variable| variable.name.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !self_is_constant
            && !mirrored_constant_names.is_empty()
            && mirrored_constant_names.len() < variables.len()
        {
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO226",
                "info",
                format!("addData/globalVars/{name}"),
                format!(
                    "globalVars '{}': {} member(s) declared CONSTANT in a same-named vendor mirror list",
                    name,
                    mirrored_constant_names.len()
                ),
                None,
                "Members that a dropped constant mirror re-declares keep constant-ness in the imported list",
            ));
        }
        imported_lists_by_name.insert(list_key, variables.clone());
        let global_prefix = extract_leading_global_prefix(&global_source);
        let is_qualified_only = codesys_global_vars_is_qualified_only(global_vars, &global_source);
        let (plain_variables, constant_variables) =
            split_constant_variables(&variables, &mirrored_constant_names);
        let constant_header_suffix = if constant_variables.is_empty() {
            String::new()
        } else {
            let mut suffix = var_global_header_suffix.clone();
            if !suffix
                .split_whitespace()
                .any(|flag| flag.eq_ignore_ascii_case("CONSTANT"))
            {
                if suffix.trim().is_empty() {
                    suffix = "CONSTANT".to_string();
                } else {
                    suffix.push_str(" CONSTANT");
                }
            }
            suffix
        };

        let (rendered_source, qualified_external) = match (global_var_mode, is_qualified_only) {
            (PlcopenImportGlobalVarMode::StrictIecAdapter, true) => {
                let type_name = format!("{list_identifier}_TYPE");
                let configuration_name = format!("{list_identifier}_Globals");

                // The configuration-level instance rides the LAST
                // non-empty VAR_GLOBAL section (original shape: one block
                // holding the members plus the list instance).
                let instance_decl = GlobalVarVariableDecl {
                    name: list_identifier.clone(),
                    type_expr: type_name.clone(),
                    initial_value: None,
                };
                let mut plain_variables = plain_variables;
                let mut constant_variables = constant_variables;
                if !constant_variables.is_empty() {
                    constant_variables.push(&instance_decl);
                } else {
                    plain_variables.push(&instance_decl);
                }

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
                push_global_var_section(&mut rendered, &var_global_header_suffix, &plain_variables);
                push_global_var_section(
                    &mut rendered,
                    &constant_header_suffix,
                    &constant_variables,
                );
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
                push_global_var_section(&mut rendered, &var_global_header_suffix, &plain_variables);
                push_global_var_section(
                    &mut rendered,
                    &constant_header_suffix,
                    &constant_variables,
                );
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
                push_global_var_section(&mut rendered, &var_global_header_suffix, &plain_variables);
                push_global_var_section(
                    &mut rendered,
                    &constant_header_suffix,
                    &constant_variables,
                );
                rendered.push_str("END_NAMESPACE\n");

                warnings.push(format!(
                    "mapped qualified_only globalVars '{}' to namespaced vendor-style GVL for native qualified access",
                    name
                ));
                (rendered, None)
            }
            (_, false) => {
                // CODESYS allows qualified access to any GVL list;
                // qualified_only only restricts the unqualified form. Plain
                // lists therefore get the namespace wrap too, so qualified
                // references resolve (s4 ask #11 — the 1 335 false-E101
                // flood came from qualified refs to bare plain lists).
                // Caveat: unqualified access to these members now requires a
                // USING directive, which the importer does not synthesize.
                let mut rendered = String::new();
                if !global_prefix.is_empty() {
                    rendered.push_str(&global_prefix);
                    if !global_prefix.ends_with('\n') {
                        rendered.push('\n');
                    }
                }
                rendered.push_str(&format!("NAMESPACE {list_identifier}\n"));
                push_global_var_section(&mut rendered, &var_global_header_suffix, &plain_variables);
                push_global_var_section(
                    &mut rendered,
                    &constant_header_suffix,
                    &constant_variables,
                );
                rendered.push_str("END_NAMESPACE\n");

                warnings.push(format!(
                    "namespaced globalVars '{}' so qualified access resolves (unqualified access now requires a USING directive)",
                    name
                ));
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

fn global_vars_list_name(global_vars: roxmltree::Node<'_, '_>, default_name: &str) -> String {
    attribute_ci(global_vars, "name")
        .or_else(|| {
            global_vars
                .children()
                .find(|child| is_element_named_ci(*child, "name"))
                .and_then(extract_text_content)
        })
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default_name.to_string())
}

/// Collects the lowercased member names declared by every
/// `constant="true"` `globalVars` list, keyed by lowercased list name —
/// including lists that a later duplicate-drop will remove (their
/// constant-ness must survive on the primary list).
fn constant_var_names_by_list(
    root: roxmltree::Node<'_, '_>,
) -> HashMap<String, HashSet<String>> {
    let mut names_by_list: HashMap<String, HashSet<String>> = HashMap::new();
    for global_vars in root
        .descendants()
        .filter(|node| is_element_named_ci(*node, "globalVars"))
        .filter(|node| {
            attribute_ci(*node, "constant")
                .is_some_and(|value| value.trim().eq_ignore_ascii_case("true"))
        })
    {
        let list_key = global_vars_list_name(global_vars, "GlobalVars").to_ascii_lowercase();
        let names = names_by_list.entry(list_key).or_default();
        for variable in global_vars
            .descendants()
            .filter(|node| is_element_named_ci(*node, "variable"))
        {
            if let Some(name) = attribute_ci(variable, "name") {
                names.insert(name.trim().to_ascii_lowercase());
                continue;
            }
            if let Some(name) = variable
                .children()
                .find(|child| is_element_named_ci(*child, "name"))
                .and_then(extract_text_content)
            {
                names.insert(name.trim().to_ascii_lowercase());
            }
        }
    }
    names_by_list
}

/// Stable partition of a merged list's declarations into (plain,
/// constant) by membership in the same-named vendor constant mirror.
fn split_constant_variables<'a>(
    variables: &'a [GlobalVarVariableDecl],
    constant_names: &[String],
) -> (Vec<&'a GlobalVarVariableDecl>, Vec<&'a GlobalVarVariableDecl>) {
    if constant_names.is_empty() {
        return (variables.iter().collect(), Vec::new());
    }
    let constant_names: HashSet<String> = constant_names
        .iter()
        .map(|name| name.to_ascii_lowercase())
        .collect();
    let mut plain = Vec::new();
    let mut constant = Vec::new();
    for variable in variables {
        if constant_names.contains(&variable.name.to_ascii_lowercase()) {
            constant.push(variable);
        } else {
            plain.push(variable);
        }
    }
    (plain, constant)
}

/// Renders one `VAR_GLOBAL [flags]` section with its declarations.
/// Empty sections are skipped (an empty block is not importable ST).
fn push_global_var_section(
    out: &mut String,
    header_suffix: &str,
    variables: &[&GlobalVarVariableDecl],
) {
    if variables.is_empty() {
        return;
    }
    out.push_str(&render_var_global_header(header_suffix));
    out.push('\n');
    for variable in variables {
        out.push_str("    ");
        out.push_str(&format_global_var_declaration(variable));
        out.push('\n');
    }
    out.push_str("END_VAR\n");
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
    for line in source.lines() {
        let lowered = line.trim().to_ascii_lowercase();
        if lowered.starts_with("{attribute") && lowered.contains("qualified_only") {
            if !lowered.contains("false") && !lowered.contains(":= 0") {
                return true;
            }
            return false;
        }
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
        if let Some(text) = extract_text_content(data)
            && !text.trim().is_empty()
        {
            return Some(text);
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

fn structured_global_var_entries_are_well_formed(node: roxmltree::Node<'_, '_>) -> bool {
    let has_structured_entries = node
        .children()
        .any(|child| is_element_named_ci(child, "variable"));
    if !has_structured_entries {
        return true;
    }
    let mut names = HashSet::new();
    for variable in node
        .children()
        .filter(|child| is_element_named_ci(*child, "variable"))
    {
        let Some(name) = attribute_ci(variable, "name")
            .or_else(|| {
                variable
                    .children()
                    .find(|child| is_element_named_ci(*child, "name"))
                    .and_then(extract_text_content)
            })
            .map(|value| value.trim().to_string())
            .filter(|value| is_valid_st_identifier(value))
        else {
            return false;
        };
        // Duplicate identities are tolerated here and resolved by
        // `dedupe_global_var_entries` (identical repeats are dropped,
        // conflicting redefinitions reject the list).
        names.insert(name.to_ascii_lowercase());
        let Some(type_node) = first_child_element_ci(variable, "type")
            .or_else(|| first_child_element_ci(variable, "baseType"))
        else {
            return false;
        };
        if parse_type_expression_container(type_node).is_none() {
            return false;
        }
    }
    true
}

fn global_var_source_is_well_formed(source: &str) -> bool {
    let mut in_block = false;
    let mut names = HashSet::new();
    let mut declaration_count = 0usize;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("{") || trimmed.starts_with("//") {
            continue;
        }
        if trimmed.to_ascii_uppercase().starts_with("VAR_GLOBAL") {
            in_block = true;
            continue;
        }
        if trimmed.eq_ignore_ascii_case("END_VAR") {
            in_block = false;
            continue;
        }
        if !in_block || trimmed.starts_with("(*") {
            continue;
        }
        let declaration = trimmed.trim_end_matches(';').trim();
        let Some((raw_names, raw_type)) = declaration.split_once(':') else {
            return false;
        };
        let type_expr = raw_type
            .split_once(":=")
            .map_or(raw_type, |(type_part, _)| type_part)
            .trim();
        if type_expr.is_empty() {
            return false;
        }
        for raw_name in raw_names.split(',') {
            let name = raw_name.trim();
            if !is_valid_st_identifier(name) {
                return false;
            }
            // Duplicate identities are tolerated and resolved by
            // `dedupe_global_var_entries` (identical repeats drop,
            // conflicting redefinitions reject the list).
            names.insert(name.to_ascii_lowercase());
            declaration_count += 1;
        }
    }
    !in_block && declaration_count > 0
}

/// e!COCKPIT MixedAttrsVarList parity: drops identical duplicate
/// declarations (same name, type, and initializer) and reports conflicting
/// redefinitions by name. The first occurrence wins.
fn dedupe_global_var_entries(
    variables: Vec<GlobalVarVariableDecl>,
) -> (Vec<GlobalVarVariableDecl>, Vec<String>, usize) {
    let mut seen: HashMap<String, GlobalVarVariableDecl> = HashMap::new();
    let mut ordered: Vec<GlobalVarVariableDecl> = Vec::new();
    let mut conflicts: Vec<String> = Vec::new();
    let mut dropped = 0usize;
    for variable in variables {
        let key = variable.name.to_ascii_lowercase();
        match seen.get(&key) {
            None => {
                seen.insert(key, variable.clone());
                ordered.push(variable);
            }
            Some(existing) => {
                let same_type = existing
                    .type_expr
                    .trim()
                    .eq_ignore_ascii_case(variable.type_expr.trim());
                let same_initial = match (&existing.initial_value, &variable.initial_value) {
                    (Some(existing_value), Some(value)) => {
                        existing_value.trim().eq_ignore_ascii_case(value.trim())
                    }
                    (None, None) => true,
                    _ => false,
                };
                if same_type && same_initial {
                    dropped += 1;
                } else {
                    conflicts.push(variable.name.clone());
                }
            }
        }
    }
    (ordered, conflicts, dropped)
}

/// e!COCKPIT serializes a per-variable attributes view of every GVL as a
/// `mixedattrsvarlist` addData block holding full copies of the lists. The
/// copies are view artifacts, not separate declarations: importing them
/// would emit `_N.st` mirror files that redeclare every member.
fn global_vars_is_mixed_attrs_view(node: roxmltree::Node<'_, '_>) -> bool {
    node.ancestors()
        .filter(|ancestor| is_element_named_ci(*ancestor, "data"))
        .filter_map(|ancestor| attribute_ci(ancestor, "name"))
        .any(|name| name.to_ascii_lowercase().contains("mixedattrsvarlist"))
}

/// True when every declaration in `variables` re-declares an identical entry
/// (name, type, initializer) already imported under the same list name — the
/// e!COCKPIT whole-list duplicate shape.
fn list_is_identical_duplicate(
    variables: &[GlobalVarVariableDecl],
    existing: &[GlobalVarVariableDecl],
) -> bool {
    !variables.is_empty()
        && !existing.is_empty()
        && variables.iter().all(|variable| {
            existing.iter().any(|candidate| {
                candidate.name.eq_ignore_ascii_case(&variable.name)
                    && candidate
                        .type_expr
                        .trim()
                        .eq_ignore_ascii_case(variable.type_expr.trim())
                    && match (&candidate.initial_value, &variable.initial_value) {
                        (Some(existing_value), Some(value)) => {
                            existing_value.trim().eq_ignore_ascii_case(value.trim())
                        }
                        (None, None) => true,
                        _ => false,
                    }
            })
        })
}

/// e!COCKPIT keeps GVL retention/constancy on the `<globalVars>` container
/// (`retain="true" persistent="true"`), while the plaintext/synthesized
/// declaration header frequently drops them. Merge the container modifiers
/// into the rendered `VAR_GLOBAL` header so the mirror keeps the qualifier.
/// Returns true when a modifier was added.
fn merge_global_var_container_qualifiers(
    node: roxmltree::Node<'_, '_>,
    header_suffix: &mut String,
) -> bool {
    let mut changed = false;
    for (attribute, modifier) in [
        ("constant", "CONSTANT"),
        ("retain", "RETAIN"),
        ("nonretain", "NON_RETAIN"),
        ("persistent", "PERSISTENT"),
        ("nonpersistent", "NON_PERSISTENT"),
    ] {
        let enabled = attribute_ci(node, attribute).is_some_and(|value| {
            matches!(value.trim().to_ascii_lowercase().as_str(), "true" | "1")
        });
        if !enabled {
            continue;
        }
        let already_present = header_suffix
            .split_ascii_whitespace()
            .any(|token| token.eq_ignore_ascii_case(modifier));
        if already_present {
            continue;
        }
        if !header_suffix.trim().is_empty() {
            header_suffix.push(' ');
        }
        header_suffix.push_str(modifier);
        changed = true;
    }
    changed
}
