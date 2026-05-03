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

    if let Some(namespace) = root.tag_name().namespace() {
        if namespace != PLCOPEN_NAMESPACE {
            warnings.push(format!(
                "unexpected namespace '{}'; expected '{}'",
                namespace, PLCOPEN_NAMESPACE
            ));
        }
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

    let sources_root = resolve_or_create_source_root(project_root)?;

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
        let st_body = extract_st_body(pou);

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

        let Some(reconstructed_source) = synthesize_import_pou_source(
            root,
            pou,
            pou_type,
            &name,
            st_body.as_deref(),
            &mut warnings,
            &mut unsupported_diagnostics,
        ) else {
            if st_body.is_none() {
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

    if discovered_pous == 0 && imported_data_types == 0 {
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
