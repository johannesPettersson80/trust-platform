pub fn import_xml_to_project(
    xml_path: &Path,
    project_root: &Path,
) -> anyhow::Result<PlcopenImportReport> {
    import_xml_to_project_with_options(xml_path, project_root, PlcopenImportOptions::default())
}

pub fn import_xml_to_project_with_options(
    xml_path: &Path,
    project_root: &Path,
    options: PlcopenImportOptions,
) -> anyhow::Result<PlcopenImportReport> {
    let xml_text = std::fs::read_to_string(xml_path)
        .with_context(|| format!("failed to read PLCopen XML '{}'", xml_path.display()))?;
    let document = roxmltree::Document::parse(&xml_text)
        .with_context(|| format!("failed to parse PLCopen XML '{}'", xml_path.display()))?;

    let root = document.root_element();
    if root.tag_name().name() != "project" {
        anyhow::bail!(
            "invalid PLCopen XML: expected root <project>, found <{}>",
            root.tag_name().name()
        );
    }

    let mut warnings = Vec::new();
    let mut unsupported_nodes = Vec::new();
    let mut unsupported_diagnostics = Vec::new();
    let mut written_sources = Vec::new();
    let mut seen_files = HashSet::new();
    let mut migration_entries = Vec::new();
    let mut applied_shim_counts: BTreeMap<(String, String, String, String), usize> =
        BTreeMap::new();
    let mut discovered_pous = 0usize;
    let mut loss_warnings = 0usize;
    let mut imported_data_types = 0usize;

    if let Some(namespace) = root.tag_name().namespace()
        && namespace != PLCOPEN_NAMESPACE
    {
        warnings.push(format!(
            "unexpected namespace '{}'; expected '{}'",
            namespace, PLCOPEN_NAMESPACE
        ));
    }

    inspect_unsupported_structure(
        root,
        &mut unsupported_nodes,
        &mut warnings,
        &mut unsupported_diagnostics,
    );

    let source_map = parse_embedded_source_map(root);
    let detected_ecosystem = detect_vendor_ecosystem(root, &xml_text);
    let promoted_program_pous = detect_program_pous_used_as_types(root);
    let project_structure_map = parse_codesys_project_structure(root);
    let mut imported_folder_paths = HashSet::new();
    let mut deferred_actions = collect_deferred_action_pous(root);

    let sources_root = resolve_or_create_source_root(project_root)?;
    if sources_root.exists() {
        reject_source_tree_symlinks(&sources_root)?;
    } else if root
        .descendants()
        .any(|node| is_element_named_ci(node, "configuration"))
    {
        std::fs::create_dir_all(&sources_root)
            .with_context(|| format!("failed to create '{}'", sources_root.display()))?;
    }
    ensure_publish_target_is_not_directory_or_symlink(
        &project_root.join(MIGRATION_REPORT_FILE),
        "migration report",
    )?;
    ensure_publish_target_is_not_directory_or_symlink(
        &project_root.join(IMPORTED_VENDOR_EXTENSION_FILE),
        "imported vendor extension file",
    )?;

    if let Some((path, count)) = import_data_types_to_sources(
        root,
        &sources_root,
        &mut seen_files,
        &mut warnings,
        &mut unsupported_nodes,
        &mut unsupported_diagnostics,
        &mut loss_warnings,
    )? {
        imported_data_types = count;
        written_sources.push(path);
    }

    let interface_migration_entries = import_interfaces_to_sources(
        root,
        &sources_root,
        &mut seen_files,
        &mut written_sources,
        &mut warnings,
        &mut unsupported_nodes,
        &mut unsupported_diagnostics,
        &mut loss_warnings,
        &project_structure_map,
        &mut imported_folder_paths,
    )?;
    let imported_interfaces = interface_migration_entries.len();
    migration_entries.extend(interface_migration_entries);

    let project_model_stats = import_project_model_to_sources(
        root,
        &sources_root,
        &mut seen_files,
        &mut warnings,
        &mut unsupported_nodes,
        &mut unsupported_diagnostics,
        &mut loss_warnings,
    )?;
    written_sources.extend(project_model_stats.written_sources.iter().cloned());

    let global_var_stats = import_global_var_lists_to_sources(
        root,
        &sources_root,
        &mut seen_files,
        &mut warnings,
        &mut unsupported_nodes,
        &mut unsupported_diagnostics,
        &mut loss_warnings,
        &project_structure_map,
        &mut imported_folder_paths,
        options.global_var_mode,
    )?;
    written_sources.extend(global_var_stats.written_sources.iter().cloned());

    for pou in collect_import_pou_nodes(root) {
        discovered_pous += 1;
        let pou_name = extract_pou_name(pou);
        let entry_name = pou_name
            .clone()
            .unwrap_or_else(|| format!("unnamed_{discovered_pous}"));
        let pou_type_raw = attribute_ci(pou, "pouType").or_else(|| attribute_ci(pou, "type"));
        let resolved_pou_type = pou_type_raw.as_deref().and_then(PlcopenPouType::from_xml);

        let Some(name) = pou_name else {
            warnings.push("skipping <pou> without name attribute".to_string());
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO201",
                "warning",
                "pou",
                "POU skipped because required name attribute is missing",
                None,
                "Skipped from import and counted in semantic-loss scoring",
            ));
            loss_warnings += 1;
            migration_entries.push(PlcopenMigrationEntry {
                name: entry_name,
                pou_type_raw,
                resolved_pou_type: resolved_pou_type.map(|kind| kind.as_xml().to_string()),
                status: "skipped".to_string(),
                reason: Some("missing name".to_string()),
            });
            continue;
        };

        let Some(pou_type_raw) = pou_type_raw else {
            warnings.push(format!("skipping pou '{}': missing pouType", name));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO202",
                "warning",
                "pou",
                "POU skipped because pouType/type attribute is missing",
                Some(name.clone()),
                "Skipped from import and counted in semantic-loss scoring",
            ));
            loss_warnings += 1;
            migration_entries.push(PlcopenMigrationEntry {
                name,
                pou_type_raw: None,
                resolved_pou_type: None,
                status: "skipped".to_string(),
                reason: Some("missing pouType/type attribute".to_string()),
            });
            continue;
        };

        let Some(mut pou_type) = PlcopenPouType::from_xml(&pou_type_raw) else {
            warnings.push(format!(
                "skipping pou '{}': unsupported pouType '{}'",
                name, pou_type_raw
            ));
            unsupported_nodes.push(format!("pouType:{}", pou_type_raw));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO203",
                "warning",
                format!("pouType:{pou_type_raw}"),
                format!("POU type '{pou_type_raw}' is outside the ST-complete subset"),
                Some(name.clone()),
                "POU skipped; convert to PROGRAM/FUNCTION/FUNCTION_BLOCK or supported aliases",
            ));
            loss_warnings += 1;
            migration_entries.push(PlcopenMigrationEntry {
                name,
                pou_type_raw: Some(pou_type_raw),
                resolved_pou_type: None,
                status: "skipped".to_string(),
                reason: Some("unsupported pouType".to_string()),
            });
            continue;
        };

        if pou_type == PlcopenPouType::Program
            && promoted_program_pous.contains(&name.to_ascii_lowercase())
        {
            pou_type = PlcopenPouType::FunctionBlock;
            warnings.push(format!(
                "promoted pou '{}' from program to functionBlock because it is referenced as a type",
                name
            ));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO210",
                "info",
                "pou/pouType",
                "POU pouType promoted from program to functionBlock based on cross-reference usage",
                Some(name.clone()),
                "Review promoted POU kind for semantic parity with the source vendor model",
            ));
        }

        // CODESYS textual actions (vendor extension) are merged into their
        // parent POU's source instead of being emitted as standalone POUs.
        if pou_type == PlcopenPouType::Action {
            continue;
        }

        let non_st_body_diagnostics = collect_non_st_body_diagnostics(pou);
        if !non_st_body_diagnostics.is_empty() {
            let body_kinds = non_st_body_diagnostics
                .iter()
                .map(|diagnostic| diagnostic.kind.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            warnings.push(format!(
                "skipping pou '{}': unsupported non-ST body ({body_kinds})",
                name
            ));
            for diagnostic in non_st_body_diagnostics {
                unsupported_nodes.push(diagnostic.node.clone());
                unsupported_diagnostics.push(unsupported_diagnostic(
                    diagnostic.code,
                    "error",
                    diagnostic.node,
                    diagnostic.message,
                    Some(name.clone()),
                    diagnostic.action,
                ));
            }
            loss_warnings += 1;
            migration_entries.push(PlcopenMigrationEntry {
                name,
                pou_type_raw: Some(pou_type_raw),
                resolved_pou_type: Some(pou_type.as_xml().to_string()),
                status: "skipped".to_string(),
                reason: Some(format!("unsupported non-ST body: {body_kinds}")),
            });
            continue;
        }

        let st_body = extract_st_body(pou);

        let synthesized = synthesize_import_pou_source(
            root,
            pou,
            pou_type,
            &name,
            st_body.as_deref(),
            &mut warnings,
            &mut unsupported_diagnostics,
        );
        if synthesized.is_none() && st_body.is_none() {
            warnings.push(format!("skipping pou '{}': missing body/ST", name));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO204",
                "warning",
                "pou/body",
                "POU skipped because body/ST payload is missing",
                Some(name.clone()),
                "POU skipped; provide an ST body in PLCopen XML",
            ));
            loss_warnings += 1;
            migration_entries.push(PlcopenMigrationEntry {
                name,
                pou_type_raw: Some(pou_type_raw),
                resolved_pou_type: Some(pou_type.as_xml().to_string()),
                status: "skipped".to_string(),
                reason: Some("missing body/ST".to_string()),
            });
            continue;
        }
        let Some(mut reconstructed_source) = synthesized else {
            warnings.push(format!("skipping pou '{}': empty ST body", name));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO205",
                "warning",
                "pou/body/ST",
                "POU skipped because ST body is empty",
                Some(name.clone()),
                "POU skipped; provide non-empty ST source text",
            ));
            loss_warnings += 1;
            migration_entries.push(PlcopenMigrationEntry {
                name,
                pou_type_raw: Some(pou_type_raw),
                resolved_pou_type: Some(pou_type.as_xml().to_string()),
                status: "skipped".to_string(),
                reason: Some("empty ST body".to_string()),
            });
            continue;
        };

        // CODESYS textual actions (vendor extension) merge into their parent
        // POU's source before the parent is written.
        if let Some(actions) = deferred_actions.remove(&name.to_ascii_lowercase()) {
            reconstructed_source = apply_deferred_actions(
                reconstructed_source,
                name.as_str(),
                actions,
                &mut warnings,
                &mut unsupported_diagnostics,
                &mut loss_warnings,
                &mut migration_entries,
            );
        }

        let strict_var_external_adapter =
            options.global_var_mode == PlcopenImportGlobalVarMode::StrictIecAdapter;
        let (reconstructed_source, inserted_externals) = if strict_var_external_adapter {
            inject_required_var_external_declarations(
                &reconstructed_source,
                &global_var_stats.qualified_list_externals,
            )
        } else {
            (reconstructed_source, Vec::new())
        };
        if !inserted_externals.is_empty() {
            warnings.push(format!(
                "inserted strict-IEC VAR_EXTERNAL adapter(s) into pou '{}': {}",
                name,
                inserted_externals.join(", ")
            ));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO603",
                "info",
                format!("pou:{name}/var_external_adapter"),
                "Strict-IEC import adapter synthesized VAR_EXTERNAL declarations for qualified global list access",
                Some(name.clone()),
                "Disable strict adapter mode to keep native vendor-parity globals without injected externals",
            ));
        }

        let folder_segments =
            resolve_codesys_folder_segments_for_node(pou, &name, &project_structure_map);
        let candidate = unique_source_path_with_segments(
            &sources_root,
            &folder_segments,
            &name,
            &mut seen_files,
        );

        let (shimmed_body, shim_applications) =
            apply_vendor_library_shims(&reconstructed_source, &detected_ecosystem);
        for application in shim_applications {
            warnings.push(format!(
                "applied vendor library shim in pou '{}': {} -> {} ({} occurrence(s))",
                name,
                application.source_symbol,
                application.replacement_symbol,
                application.occurrences
            ));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO301",
                "info",
                format!("vendor-shim:{}", application.source_symbol),
                format!(
                    "Vendor library shim mapped '{}' to '{}'",
                    application.source_symbol, application.replacement_symbol
                ),
                Some(name.clone()),
                application.notes.clone(),
            ));
            let key = (
                application.vendor,
                application.source_symbol,
                application.replacement_symbol,
                application.notes,
            );
            *applied_shim_counts.entry(key).or_insert(0) += application.occurrences;
        }

        write_text_file_with_parents(&candidate, &shimmed_body)?;
        track_imported_folder_path(&candidate, &sources_root, &mut imported_folder_paths);
        written_sources.push(candidate);

        migration_entries.push(PlcopenMigrationEntry {
            name: name.clone(),
            pou_type_raw: Some(pou_type_raw),
            resolved_pou_type: Some(pou_type.as_xml().to_string()),
            status: "imported".to_string(),
            reason: None,
        });

        if let Some(entry) = source_map.as_ref().and_then(|map| {
            map.entries
                .iter()
                .find(|entry| entry.name.eq_ignore_ascii_case(&name))
        }) {
            warnings.push(format!(
                "source map: pou '{}' originated from {}:{}",
                name, entry.source, entry.line
            ));
        }
    }

    // Action POUs whose parent was never seen (or had no `Parent.Action`
    // linkage) keep the fail-closed skip.
    for (parent, actions) in deferred_actions.iter() {
        for action in actions {
            let full_name = if parent.is_empty() {
                action.name.clone()
            } else {
                format!("{parent}.{}", action.name)
            };
            warnings.push(format!(
                "skipping action pou '{}': parent POU is missing or was not imported",
                full_name
            ));
            unsupported_nodes.push("pouType:action".to_string());
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO203",
                "warning",
                "pouType:action",
                "action pou skipped because its parent POU is missing or was not imported",
                Some(full_name.clone()),
                "Name the action 'Parent.Action' in the source XML or import the parent POU",
            ));
            loss_warnings += 1;
            migration_entries.push(PlcopenMigrationEntry {
                name: full_name,
                pou_type_raw: Some("action".to_string()),
                resolved_pou_type: Some("action".to_string()),
                status: "skipped".to_string(),
                reason: Some("parent POU missing or not imported".to_string()),
            });
        }
    }

    if discovered_pous == 0 && imported_data_types == 0 && imported_interfaces == 0 {
        warnings.push("no <pou> nodes discovered in input XML".to_string());
        unsupported_diagnostics.push(unsupported_diagnostic(
            "PLCO206",
            "warning",
            "types/pous",
            "Input XML does not contain importable <pou> elements",
            None,
            "Provide PLCopen ST POUs under project/types/pous",
        ));
        loss_warnings += 1;
    }

    let imported_pous = migration_entries
        .iter()
        .filter(|entry| entry.status == "imported")
        .count();
    let importable_pous = imported_pous;
    let skipped_pous = discovered_pous.saturating_sub(imported_pous);
    let source_coverage_percent = calculate_source_coverage(imported_pous, discovered_pous);
    let semantic_loss_percent = calculate_semantic_loss(
        imported_pous,
        discovered_pous,
        unsupported_nodes.len(),
        loss_warnings,
    );
    let applied_library_shims = applied_shim_counts
        .into_iter()
        .map(
            |((vendor, source_symbol, replacement_symbol, notes), occurrences)| {
                PlcopenLibraryShimApplication {
                    vendor,
                    source_symbol,
                    replacement_symbol,
                    occurrences,
                    notes,
                }
            },
        )
        .collect::<Vec<_>>();
    let shimmed_occurrences = applied_library_shims
        .iter()
        .map(|entry| entry.occurrences)
        .sum::<usize>();
    let compatibility_coverage = calculate_compatibility_coverage(
        imported_pous,
        skipped_pous,
        unsupported_nodes.len(),
        shimmed_occurrences,
    );

    let preserved_vendor_extensions =
        preserve_vendor_extensions(root, &xml_text, project_root, &mut warnings)?;
    let migration_report = PlcopenMigrationReport {
        profile: PROFILE_NAME.to_string(),
        namespace: root
            .tag_name()
            .namespace()
            .unwrap_or(PLCOPEN_NAMESPACE)
            .to_string(),
        source_xml: xml_path.to_path_buf(),
        project_root: project_root.to_path_buf(),
        detected_ecosystem: detected_ecosystem.clone(),
        discovered_pous,
        importable_pous,
        imported_pous,
        skipped_pous,
        imported_data_types,
        imported_interfaces,
        discovered_configurations: project_model_stats.discovered_configurations,
        imported_configurations: project_model_stats.imported_configurations,
        imported_resources: project_model_stats.imported_resources,
        imported_tasks: project_model_stats.imported_tasks,
        imported_program_instances: project_model_stats.imported_program_instances,
        discovered_global_var_lists: global_var_stats.discovered_global_var_lists,
        imported_global_var_lists: global_var_stats.imported_global_var_lists,
        imported_project_structure_nodes: project_structure_map.object_count,
        imported_folder_paths: imported_folder_paths.len(),
        source_coverage_percent,
        semantic_loss_percent,
        compatibility_coverage: compatibility_coverage.clone(),
        unsupported_nodes: unsupported_nodes.clone(),
        unsupported_diagnostics: unsupported_diagnostics.clone(),
        applied_library_shims: applied_library_shims.clone(),
        warnings: warnings.clone(),
        entries: migration_entries,
    };
    let migration_report_path = write_migration_report(project_root, &migration_report)?;

    if written_sources.is_empty() {
        anyhow::bail!(
            "no importable PLCopen ST content found in {} (migration report: {})",
            xml_path.display(),
            migration_report_path.display()
        );
    }

    Ok(PlcopenImportReport {
        project_root: project_root.to_path_buf(),
        imported_pous,
        discovered_pous,
        imported_data_types,
        imported_interfaces,
        discovered_configurations: project_model_stats.discovered_configurations,
        imported_configurations: project_model_stats.imported_configurations,
        imported_resources: project_model_stats.imported_resources,
        imported_tasks: project_model_stats.imported_tasks,
        imported_program_instances: project_model_stats.imported_program_instances,
        discovered_global_var_lists: global_var_stats.discovered_global_var_lists,
        imported_global_var_lists: global_var_stats.imported_global_var_lists,
        imported_project_structure_nodes: project_structure_map.object_count,
        imported_folder_paths: imported_folder_paths.len(),
        written_sources,
        warnings,
        unsupported_nodes,
        preserved_vendor_extensions,
        migration_report_path,
        source_coverage_percent,
        semantic_loss_percent,
        detected_ecosystem,
        compatibility_coverage,
        unsupported_diagnostics,
        applied_library_shims,
    })
}

fn reject_source_tree_symlinks(root: &Path) -> anyhow::Result<()> {
    let metadata = std::fs::symlink_metadata(root)
        .with_context(|| format!("failed to inspect source root '{}'", root.display()))?;
    if metadata.file_type().is_symlink() {
        anyhow::bail!(
            "symbolic link is not allowed in source tree: {}",
            root.display()
        );
    }
    if !metadata.is_dir() {
        anyhow::bail!("source root is not a directory: {}", root.display());
    }
    for entry in std::fs::read_dir(root)
        .with_context(|| format!("failed to read source root '{}'", root.display()))?
    {
        let path = entry?.path();
        let child_metadata = std::fs::symlink_metadata(&path)
            .with_context(|| format!("failed to inspect source path '{}'", path.display()))?;
        if child_metadata.file_type().is_symlink() {
            anyhow::bail!(
                "symbolic link is not allowed in source tree: {}",
                path.display()
            );
        }
        if child_metadata.is_dir() {
            reject_source_tree_symlinks(&path)?;
        }
    }
    Ok(())
}

/// CODESYS textual action (vendor extension): an action POU deferred for
/// merging into its parent POU's source.
struct DeferredActionPou {
    parent: Option<String>,
    name: String,
    body: Option<String>,
}

/// Collects `pouType="action"` POUs, splitting `Parent.Action` names into
/// parent linkage and action name, plus the e!COCKPIT structural form where
/// textual actions nest under the parent POU itself (`<pou><actions><action
/// name="Plain">`) with no standalone action POUs at all.
fn collect_deferred_action_pous(root: roxmltree::Node<'_, '_>) -> HashMap<String, Vec<DeferredActionPou>> {
    let mut deferred: HashMap<String, Vec<DeferredActionPou>> = HashMap::new();
    for pou in collect_import_pou_nodes(root) {
        let Some(name) = extract_pou_name(pou) else {
            continue;
        };
        collect_structural_actions(pou, &name, &mut deferred);
        let Some(pou_type_raw) = attribute_ci(pou, "pouType").or_else(|| attribute_ci(pou, "type"))
        else {
            continue;
        };
        if PlcopenPouType::from_xml(&pou_type_raw) != Some(PlcopenPouType::Action) {
            continue;
        }
        let (parent, action_name) = match name.split_once('.') {
            Some((parent, action)) if !parent.is_empty() && !action.is_empty() => {
                (Some(parent.to_string()), action.to_string())
            }
            _ => (None, name.clone()),
        };
        let body = extract_st_body(pou);
        let key = parent
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();
        deferred.entry(key).or_default().push(DeferredActionPou {
            parent,
            name: action_name,
            body,
        });
    }
    deferred
}

/// e!COCKPIT serializes a POU's textual actions as `<action name="Plain">`
/// children of an `<actions>` container inside the POU itself; parent
/// linkage is the containment, and the action names carry no dot. Collect
/// them with the enclosing POU as the merge parent.
fn collect_structural_actions(
    pou: roxmltree::Node<'_, '_>,
    parent_name: &str,
    deferred: &mut HashMap<String, Vec<DeferredActionPou>>,
) {
    for actions in pou
        .children()
        .filter(|child| is_element_named_ci(*child, "actions"))
    {
        for action in actions
            .children()
            .filter(|child| is_element_named_ci(*child, "action"))
        {
            let Some(action_name) = attribute_ci(action, "name")
                .map(|name| name.trim().to_string())
                .filter(|name| !name.is_empty())
            else {
                continue;
            };
            deferred
                .entry(parent_name.to_ascii_lowercase())
                .or_default()
                .push(DeferredActionPou {
                    parent: Some(parent_name.to_string()),
                    name: action_name,
                    body: extract_st_body(action),
                });
        }
    }
}

/// Renders merged textual actions and splices them before the parent POU's
/// end keyword; actions without a body are skipped fail-closed.
fn apply_deferred_actions(
    mut source: String,
    parent_name: &str,
    actions: Vec<DeferredActionPou>,
    warnings: &mut Vec<String>,
    unsupported_diagnostics: &mut Vec<PlcopenUnsupportedDiagnostic>,
    loss_warnings: &mut usize,
    migration_entries: &mut Vec<PlcopenMigrationEntry>,
) -> String {
    let mut blocks: Vec<String> = Vec::new();
    for action in actions {
        let full_name = match &action.parent {
            Some(parent) => format!("{parent}.{}", action.name),
            None => format!("{parent_name}.{}", action.name),
        };
        let Some(body) = action.body.as_deref().map(str::trim).filter(|body| !body.is_empty())
        else {
            warnings.push(format!("skipping action pou '{}': missing body/ST", full_name));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO204",
                "warning",
                "pou/body",
                "action pou skipped because body/ST payload is missing",
                Some(full_name.clone()),
                "Provide an ST body for the action in PLCopen XML",
            ));
            *loss_warnings += 1;
            migration_entries.push(PlcopenMigrationEntry {
                name: full_name,
                pou_type_raw: Some("action".to_string()),
                resolved_pou_type: Some("action".to_string()),
                status: "skipped".to_string(),
                reason: Some("missing body/ST".to_string()),
            });
            continue;
        };

        let mut block = String::new();
        block.push_str(&format!("ACTION {}:", action.name));
        block.push('\n');
        block.push_str(normalize_body_text(body).trim());
        block.push('\n');
        block.push_str("END_ACTION");
        blocks.push(block);

        warnings.push(format!(
            "merged action pou '{}' into parent POU '{}'",
            full_name, parent_name
        ));
        unsupported_diagnostics.push(unsupported_diagnostic(
            "PLCO219",
            "info",
            "pou/pouType:action",
            "action pou merged into its parent POU as a textual ACTION block",
            Some(full_name.clone()),
            "Review the merged ACTION block for semantic parity with the source vendor model",
        ));
        migration_entries.push(PlcopenMigrationEntry {
            name: full_name,
            pou_type_raw: Some("action".to_string()),
            resolved_pou_type: Some("action".to_string()),
            status: "imported".to_string(),
            reason: Some("action merged into parent POU".to_string()),
        });
    }

    if blocks.is_empty() {
        return source;
    }

    let Some(insert_at) = source.rfind("END_PROGRAM")
        .or_else(|| source.rfind("END_FUNCTION_BLOCK"))
        .or_else(|| source.rfind("END_FUNCTION"))
    else {
        return source;
    };

    let mut insertion = String::new();
    if !source[..insert_at].ends_with("\n\n") {
        if !source[..insert_at].ends_with('\n') {
            insertion.push('\n');
        }
        insertion.push('\n');
    }
    for (index, block) in blocks.iter().enumerate() {
        insertion.push_str(block);
        insertion.push('\n');
        if index + 1 < blocks.len() {
            insertion.push('\n');
        }
    }

    source.insert_str(insert_at, &insertion);
    normalize_body_text(source)
}
