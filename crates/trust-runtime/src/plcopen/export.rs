pub fn export_project_to_xml(
    project_root: &Path,
    output_path: &Path,
) -> anyhow::Result<PlcopenExportReport> {
    export_project_to_xml_with_target(project_root, output_path, PlcopenExportTarget::Generic)
}

fn resolve_existing_source_root(project_root: &Path) -> anyhow::Result<PathBuf> {
    let src_root = project_root.join("src");
    if src_root.is_dir() {
        return Ok(src_root);
    }

    anyhow::bail!(
        "invalid project folder '{}': missing src/ directory",
        project_root.display()
    );
}

fn resolve_or_create_source_root(project_root: &Path) -> anyhow::Result<PathBuf> {
    let src_root = project_root.join("src");
    if src_root.is_dir() {
        return Ok(src_root);
    }

    std::fs::create_dir_all(&src_root)
        .with_context(|| format!("failed to create '{}'", src_root.display()))?;
    Ok(src_root)
}

pub fn export_project_to_xml_with_target(
    project_root: &Path,
    output_path: &Path,
    target: PlcopenExportTarget,
) -> anyhow::Result<PlcopenExportReport> {
    let sources_root = resolve_existing_source_root(project_root)?;

    let sources = load_sources(project_root, &sources_root)?;
    if sources.is_empty() {
        anyhow::bail!("no ST sources found under {}", sources_root.display());
    }
    let source_analysis = analyze_export_sources(&sources);

    let mut warnings = Vec::new();
    let mut declarations = Vec::new();
    let mut data_type_decls = Vec::new();
    let mut global_var_lists = Vec::new();
    let mut configurations = Vec::new();

    for source in &sources {
        let (mut declared, mut source_warnings) = extract_pou_declarations(source);
        declarations.append(&mut declared);
        warnings.append(&mut source_warnings);

        let (mut declared_types, mut type_warnings) = extract_data_type_declarations(source);
        data_type_decls.append(&mut declared_types);
        warnings.append(&mut type_warnings);

        let (mut source_configs, mut config_warnings) = extract_configuration_declarations(source);
        configurations.append(&mut source_configs);
        warnings.append(&mut config_warnings);

        let (mut source_globals, mut global_warnings) = extract_global_var_declarations(source);
        global_var_lists.append(&mut source_globals);
        warnings.append(&mut global_warnings);
    }

    if declarations.is_empty()
        && data_type_decls.is_empty()
        && global_var_lists.is_empty()
        && configurations.is_empty()
    {
        anyhow::bail!(
            "no PLCopen ST-complete declarations discovered (supported: POUs, TYPE blocks, VAR_GLOBAL blocks, CONFIGURATION/RESOURCE/TASK/PROGRAM)"
        );
    }

    declarations.sort_by(|left, right| {
        left.pou_type
            .as_xml()
            .cmp(right.pou_type.as_xml())
            .then(left.name.cmp(&right.name))
            .then(left.source.cmp(&right.source))
    });

    data_type_decls.sort_by(|left, right| {
        left.name
            .to_ascii_lowercase()
            .cmp(&right.name.to_ascii_lowercase())
            .then(left.source.cmp(&right.source))
            .then(left.line.cmp(&right.line))
    });

    global_var_lists.sort_by(|left, right| {
        left.name
            .to_ascii_lowercase()
            .cmp(&right.name.to_ascii_lowercase())
            .then(left.source.cmp(&right.source))
            .then(left.line.cmp(&right.line))
    });

    let mut deduped_types = Vec::new();
    let mut seen_type_names = BTreeSet::new();
    for decl in data_type_decls {
        let key = decl.name.to_ascii_lowercase();
        if seen_type_names.insert(key) {
            deduped_types.push(decl);
        } else {
            warnings.push(format!(
                "{}:{} duplicate TYPE declaration '{}' skipped for PLCopen export",
                decl.source, decl.line, decl.name
            ));
        }
    }

    for config in &mut configurations {
        config.tasks.sort_by(|left, right| {
            left.name
                .to_ascii_lowercase()
                .cmp(&right.name.to_ascii_lowercase())
        });
        config.programs.sort_by(|left, right| {
            left.instance_name
                .to_ascii_lowercase()
                .cmp(&right.instance_name.to_ascii_lowercase())
        });
        config.resources.sort_by(|left, right| {
            left.name
                .to_ascii_lowercase()
                .cmp(&right.name.to_ascii_lowercase())
        });
        for resource in &mut config.resources {
            resource.tasks.sort_by(|left, right| {
                left.name
                    .to_ascii_lowercase()
                    .cmp(&right.name.to_ascii_lowercase())
            });
            resource.programs.sort_by(|left, right| {
                left.instance_name
                    .to_ascii_lowercase()
                    .cmp(&right.instance_name.to_ascii_lowercase())
            });
        }
    }
    configurations.sort_by(|left, right| {
        left.name
            .to_ascii_lowercase()
            .cmp(&right.name.to_ascii_lowercase())
    });

    let source_map = SourceMapPayload {
        profile: PROFILE_NAME.to_string(),
        namespace: PLCOPEN_NAMESPACE.to_string(),
        entries: declarations
            .iter()
            .map(|decl| SourceMapEntry {
                name: decl.name.clone(),
                pou_type: decl.pou_type.as_xml().to_string(),
                source: decl.source.clone(),
                line: decl.line,
            })
            .collect(),
    };
    let source_map_json = serde_json::to_string_pretty(&source_map)?;
    let codesys_metadata =
        build_codesys_export_metadata(&declarations, &global_var_lists, &mut warnings);

    let project_name = project_root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("project");
    let generated_at = now_iso8601();

    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str(&format!(
        "<project xmlns=\"{}\" profile=\"{}\">\n",
        PLCOPEN_NAMESPACE, PROFILE_NAME
    ));
    xml.push_str(&format!(
        "  <fileHeader companyName=\"truST\" productName=\"trust-runtime\" productVersion=\"{}\" creationDateTime=\"{}\"/>\n",
        escape_xml_attr(env!("CARGO_PKG_VERSION")),
        escape_xml_attr(&generated_at)
    ));
    xml.push_str(&format!(
        "  <contentHeader name=\"{}\"/>\n",
        escape_xml_attr(project_name)
    ));
    xml.push_str("  <types>\n");

    let mut exported_data_type_count = 0usize;
    if !deduped_types.is_empty() {
        xml.push_str("    <dataTypes>\n");
        for data_type in &deduped_types {
            if let Some(base_type_xml) =
                type_expression_to_plcopen_base_type_xml(&data_type.type_expr)
            {
                xml.push_str(&format!(
                    "      <dataType name=\"{}\">\n",
                    escape_xml_attr(&data_type.name)
                ));
                xml.push_str("        <baseType>\n");
                for line in base_type_xml.lines() {
                    xml.push_str("          ");
                    xml.push_str(line);
                    xml.push('\n');
                }
                xml.push_str("        </baseType>\n");
                xml.push_str("      </dataType>\n");
                exported_data_type_count += 1;
            } else {
                warnings.push(format!(
                    "{}:{} unsupported TYPE expression for '{}' skipped in PLCopen dataTypes export",
                    data_type.source, data_type.line, data_type.name
                ));
            }
        }
        xml.push_str("    </dataTypes>\n");
    }

    xml.push_str("    <pous>\n");

    for decl in &declarations {
        xml.push_str(&format!(
            "      <pou name=\"{}\" pouType=\"{}\">\n",
            escape_xml_attr(&decl.name),
            decl.pou_type.as_xml()
        ));
        xml.push_str("        <body>\n");
        xml.push_str("          <ST><![CDATA[");
        xml.push_str(&escape_cdata(&decl.body));
        xml.push_str("]]></ST>\n");
        xml.push_str("        </body>\n");
        if let Some((_meta_decl, object_entry)) = codesys_metadata.pou_entries.iter().find(
            |(candidate, _entry)| {
                candidate.name == decl.name
                    && candidate.pou_type == decl.pou_type
                    && candidate.source == decl.source
                    && candidate.line == decl.line
            },
        ) {
            let methods = codesys_metadata
                .method_entries
                .iter()
                .filter(|(method, _entry)| method.owner_name.eq_ignore_ascii_case(&decl.name))
                .collect::<Vec<_>>();
            xml.push_str("        <addData>\n");
            xml.push_str(&format!(
                "          <data name=\"{}\" handleUnknown=\"implementation\">\n",
                CODESYS_INTERFACE_PLAINTEXT_DATA_NAME
            ));
            xml.push_str("            <InterfaceAsPlainText>\n");
            xml.push_str(&format!(
                "              <xhtml xmlns=\"http://www.w3.org/1999/xhtml\">{}</xhtml>\n",
                escape_xml_attr(&decl.body)
            ));
            xml.push_str("            </InterfaceAsPlainText>\n");
            xml.push_str("          </data>\n");
            if !methods.is_empty() {
                xml.push_str(&format!(
                    "          <data name=\"{}\" handleUnknown=\"implementation\">\n",
                    CODESYS_METHOD_DATA_NAME
                ));
                for (method, method_entry) in methods {
                    append_codesys_method_xml(
                        &mut xml,
                        method,
                        method_entry,
                        12,
                        &mut warnings,
                    );
                }
                xml.push_str("          </data>\n");
            }
            xml.push_str(&format!(
                "          <data name=\"{}\" handleUnknown=\"discard\">\n",
                CODESYS_OBJECT_ID_DATA_NAME
            ));
            xml.push_str(&format!(
                "            <ObjectId>{}</ObjectId>\n",
                escape_xml_attr(&object_entry.object_id)
            ));
            xml.push_str("          </data>\n");
            xml.push_str("        </addData>\n");
        }
        xml.push_str("      </pou>\n");
    }

    xml.push_str("    </pous>\n");
    xml.push_str("  </types>\n");

    let mut exported_resource_count = 0usize;
    let mut exported_task_count = 0usize;
    let mut exported_program_instance_count = 0usize;
    if !configurations.is_empty() {
        xml.push_str("  <instances>\n");
        xml.push_str("    <configurations>\n");
        for configuration in &configurations {
            xml.push_str(&format!(
                "      <configuration name=\"{}\">\n",
                escape_xml_attr(&configuration.name)
            ));

            for task in &configuration.tasks {
                append_task_xml(&mut xml, task, 8);
                exported_task_count += 1;
            }
            for program in &configuration.programs {
                append_program_instance_xml(&mut xml, program, 8);
                exported_program_instance_count += 1;
            }

            for resource in &configuration.resources {
                exported_resource_count += 1;
                xml.push_str(&format!(
                    "        <resource name=\"{}\" target=\"{}\">\n",
                    escape_xml_attr(&resource.name),
                    escape_xml_attr(&resource.target)
                ));
                for task in &resource.tasks {
                    append_task_xml(&mut xml, task, 10);
                    exported_task_count += 1;
                }
                for program in &resource.programs {
                    append_program_instance_xml(&mut xml, program, 10);
                    exported_program_instance_count += 1;
                }
                xml.push_str("        </resource>\n");
            }

            xml.push_str("      </configuration>\n");
        }
        xml.push_str("    </configurations>\n");
        xml.push_str("  </instances>\n");
    }

    let validation_context = ExportTargetValidationContext {
        pou_count: declarations.len(),
        data_type_count: exported_data_type_count,
        configuration_count: configurations.len(),
        resource_count: exported_resource_count,
        task_count: exported_task_count,
        program_instance_count: exported_program_instance_count,
        source_count: sources.len(),
        analysis: source_analysis,
    };
    let adapter_contract = build_export_adapter_contract(target, &validation_context);
    if let Some(contract) = &adapter_contract {
        for diagnostic in &contract.diagnostics {
            if !diagnostic.severity.eq_ignore_ascii_case("info") {
                warnings.push(format!(
                    "{} [{}]: {}",
                    diagnostic.code,
                    target.id(),
                    diagnostic.message
                ));
            }
        }
    }

    xml.push_str("  <addData>\n");
    xml.push_str(&format!(
        "    <data name=\"{}\" handleUnknown=\"implementation\"><text><![CDATA[{}]]></text></data>\n",
        SOURCE_MAP_DATA_NAME,
        escape_cdata(&source_map_json)
    ));
    if let Some(contract) = &adapter_contract {
        let adapter_payload = serde_json::json!({
            "target": target.id(),
            "target_label": target.label(),
            "diagnostics": contract.diagnostics,
            "manual_steps": contract.manual_steps,
            "limitations": contract.limitations,
        });
        let adapter_json = serde_json::to_string_pretty(&adapter_payload)?;
        xml.push_str(&format!(
            "    <data name=\"{}\" handleUnknown=\"implementation\"><text><![CDATA[{}]]></text></data>\n",
            EXPORT_ADAPTER_DATA_NAME,
            escape_cdata(&adapter_json)
        ));
    }
    append_codesys_export_add_data(&mut xml, &codesys_metadata, &mut warnings);

    let vendor_hook_path = project_root.join(VENDOR_EXTENSION_HOOK_FILE);
    if vendor_hook_path.is_file() {
        let vendor_text = std::fs::read_to_string(&vendor_hook_path).with_context(|| {
            format!(
                "failed to read vendor extension hook '{}'",
                vendor_hook_path.display()
            )
        })?;
        xml.push_str(&format!(
            "    <data name=\"{}\" handleUnknown=\"implementation\"><text><![CDATA[{}]]></text></data>\n",
            VENDOR_EXT_DATA_NAME,
            escape_cdata(&vendor_text)
        ));
    }
    xml.push_str("  </addData>\n");
    xml.push_str("</project>\n");

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create PLCopen output directory '{}'",
                parent.display()
            )
        })?;
    }

    std::fs::write(output_path, xml)
        .with_context(|| format!("failed to write '{}'", output_path.display()))?;

    let source_map_path = output_path.with_extension("source-map.json");
    std::fs::write(&source_map_path, format!("{}\n", source_map_json)).with_context(|| {
        format!(
            "failed to write source-map sidecar '{}'",
            source_map_path.display()
        )
    })?;

    let mut siemens_scl_bundle_dir = None;
    let mut siemens_scl_files = Vec::new();
    if target == PlcopenExportTarget::Siemens {
        let (bundle_dir, bundle_files) = export_siemens_scl_bundle(
            output_path,
            &declarations,
            &deduped_types,
            &configurations,
            &mut warnings,
        )?;
        siemens_scl_bundle_dir = Some(bundle_dir);
        siemens_scl_files = bundle_files;
    }

    let mut adapter_report_path = None;
    let mut adapter_diagnostics = Vec::new();
    let mut adapter_manual_steps = Vec::new();
    let mut adapter_limitations = Vec::new();
    if let Some(contract) = adapter_contract {
        let adapter_path = adapter_report_path_for_output(output_path);
        let adapter_report = PlcopenExportAdapterReport {
            target: target.id().to_string(),
            target_label: target.label().to_string(),
            source_xml: output_path.to_path_buf(),
            source_map_path: source_map_path.clone(),
            siemens_scl_bundle_dir: siemens_scl_bundle_dir.clone(),
            siemens_scl_files: siemens_scl_files.clone(),
            diagnostics: contract.diagnostics,
            manual_steps: contract.manual_steps,
            limitations: contract.limitations,
        };
        let adapter_json = serde_json::to_string_pretty(&adapter_report)?;
        std::fs::write(&adapter_path, format!("{adapter_json}\n")).with_context(|| {
            format!(
                "failed to write target adapter report '{}'",
                adapter_path.display()
            )
        })?;

        adapter_report_path = Some(adapter_path);
        adapter_diagnostics = adapter_report.diagnostics;
        adapter_manual_steps = adapter_report.manual_steps;
        adapter_limitations = adapter_report.limitations;
    }

    Ok(PlcopenExportReport {
        target: target.id().to_string(),
        output_path: output_path.to_path_buf(),
        source_map_path,
        adapter_report_path,
        siemens_scl_bundle_dir,
        siemens_scl_files,
        adapter_diagnostics,
        adapter_manual_steps,
        adapter_limitations,
        pou_count: declarations.len(),
        data_type_count: exported_data_type_count,
        configuration_count: configurations.len(),
        resource_count: exported_resource_count,
        task_count: exported_task_count,
        program_instance_count: exported_program_instance_count,
        exported_global_var_lists: codesys_metadata.global_var_lists.len(),
        exported_project_structure_nodes: codesys_metadata.exported_project_structure_nodes,
        exported_folder_paths: codesys_metadata.exported_folder_paths,
        source_count: sources.len(),
        warnings,
    })
}
