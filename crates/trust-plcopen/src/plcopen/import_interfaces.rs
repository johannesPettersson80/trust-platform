/// CODESYS/e!COCKPIT interface-definition objects. e!COCKPIT exports an
/// interface as a project-level `<Interface>` element (the PLCopen XSD
/// `pouType` enum has no `interface`, so there is no `<pou>`), which is why
/// such files previously imported as empty. The projection emits
/// declaration-only `INTERFACE` blocks — METHOD signature stubs, no bodies
/// (accessor bodies are owner-owned and completeness is checked against the
/// implementing FB's members, not the interface's).
#[allow(clippy::too_many_arguments)]
fn import_interfaces_to_sources(
    root: roxmltree::Node<'_, '_>,
    sources_root: &Path,
    seen_files: &mut HashSet<PathBuf>,
    written_sources: &mut Vec<PathBuf>,
    warnings: &mut Vec<String>,
    unsupported_nodes: &mut Vec<String>,
    unsupported_diagnostics: &mut Vec<PlcopenUnsupportedDiagnostic>,
    loss_warnings: &mut usize,
    project_structure_map: &CodesysProjectStructureMap,
    imported_folder_paths: &mut HashSet<String>,
) -> anyhow::Result<Vec<PlcopenMigrationEntry>> {
    let mut migration_entries = Vec::new();
    let mut seen_names = BTreeSet::new();

    for interface in root
        .descendants()
        .filter(|node| is_element_named(*node, "Interface"))
        .filter(|node| {
            !node
                .ancestors()
                .skip(1)
                .any(|ancestor| is_element_named_ci(ancestor, "pou"))
        })
    {
        let Some(name) = attribute_ci(interface, "name")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            unsupported_nodes.push("interfaces/unnamed".to_string());
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO405",
                "warning",
                "interfaces/Interface",
                "interface object skipped because required name attribute is missing",
                None,
                "Provide a non-empty interface name before import",
            ));
            *loss_warnings += 1;
            continue;
        };

        let name_key = name.to_ascii_lowercase();
        if !seen_names.insert(name_key) {
            unsupported_nodes.push(format!("interfaces/{name}"));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO403",
                "warning",
                format!("interfaces/{name}"),
                format!("interface '{name}' skipped because the name is duplicated"),
                None,
                "Rename duplicate interface entries to unique names before import",
            ));
            *loss_warnings += 1;
            continue;
        }

        let Some(source) = render_interface_source(interface, &name, warnings) else {
            unsupported_nodes.push(format!("interfaces/{name}"));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO406",
                "warning",
                format!("interfaces/{name}"),
                format!("interface '{name}' uses an unsupported member representation"),
                None,
                "Provide METHOD/PROPERTY members with PLCopen interface variable nodes",
            ));
            *loss_warnings += 1;
            continue;
        };

        let folder_segments =
            resolve_codesys_folder_segments_for_node(interface, &name, project_structure_map);
        let candidate =
            unique_source_path_with_segments(sources_root, &folder_segments, &name, seen_files);
        write_text_file_with_parents(&candidate, &source)?;
        track_imported_folder_path(&candidate, sources_root, imported_folder_paths);
        warnings.push(format!(
            "imported interface '{name}' into {}",
            candidate.display()
        ));
        unsupported_diagnostics.push(unsupported_diagnostic(
            "PLCO222",
            "info",
            format!("interfaces/{name}"),
            "interface object imported as a textual INTERFACE declaration",
            Some(name.clone()),
            "Review interface METHOD signatures for semantic parity with the source vendor model",
        ));
        migration_entries.push(PlcopenMigrationEntry {
            name,
            pou_type_raw: Some("interface".to_string()),
            resolved_pou_type: None,
            status: "imported".to_string(),
            reason: None,
        });
        written_sources.push(candidate);
    }

    Ok(migration_entries)
}

/// Renders the vendor interface as mirror ST. Member shape (signature-only
/// METHOD blocks with one-line VAR_INPUT/VAR_IN_OUT sections) follows the
/// e!COCKPIT field-verified mirror layout.
fn render_interface_source(
    interface: roxmltree::Node<'_, '_>,
    name: &str,
    warnings: &mut Vec<String>,
) -> Option<String> {
    let mut header = format!("INTERFACE {name}");
    for inheritance in interface
        .children()
        .filter(|child| is_element_named_ci(*child, "Inheritance"))
    {
        for extends in inheritance
            .children()
            .filter(|child| is_element_named_ci(*child, "Extends"))
        {
            let Some(text) = extract_text_content(extends) else {
                continue;
            };
            let base = text.trim();
            if is_valid_qualified_identifier(base) {
                header.push_str(&format!(" EXTENDS {base}"));
            } else {
                warnings.push(format!(
                    "interface '{name}': EXTENDS base '{base}' is not a projectable identifier and was dropped"
                ));
            }
        }
    }

    let mut lines = vec![header, String::new()];
    let mut member_count = 0usize;
    for (container, member_tag) in [("Methods", "Method"), ("Properties", "Property")] {
        for container_node in interface
            .children()
            .filter(|child| is_element_named_ci(*child, container))
        {
            for member in container_node
                .children()
                .filter(|child| is_element_named_ci(*child, member_tag))
            {
                if let Some(block) = render_interface_member(member, name, warnings) {
                    lines.push(block);
                    lines.push(String::new());
                    member_count += 1;
                }
            }
        }
    }
    if member_count == 0 {
        warnings.push(format!(
            "interface '{name}' declared no METHOD/PROPERTY members; imported as an empty INTERFACE declaration"
        ));
    }

    lines.push("END_INTERFACE".to_string());
    Some(format!("{}\n", lines.join("\n")))
}

fn render_interface_member(
    member: roxmltree::Node<'_, '_>,
    interface_name: &str,
    warnings: &mut Vec<String>,
) -> Option<String> {
    let Some(member_name) = attribute_ci(member, "name")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        warnings.push(format!(
            "interface '{interface_name}': member without a name was skipped"
        ));
        return None;
    };

    let member_interface = first_child_element_ci(member, "interface");
    let return_type = member_interface
        .and_then(|interface| first_child_element_ci(interface, "returnType"))
        .and_then(parse_type_expression_container);

    let mut block = format!("METHOD {member_name}");
    if let Some(return_type) = &return_type {
        block.push_str(" : ");
        block.push_str(return_type.trim());
    }
    if let Some(member_interface) = member_interface {
        for (section_xml, st_keyword) in [("inputVars", "VAR_INPUT"), ("inOutVars", "VAR_IN_OUT")]
        {
            for section in member_interface
                .children()
                .filter(|child| is_element_named_ci(*child, section_xml))
            {
                for variable in section
                    .children()
                    .filter(|child| is_element_named_ci(*child, "variable"))
                {
                    let Some(variable_name) = attribute_ci(variable, "name")
                        .map(|value| value.trim().to_string())
                        .filter(|value| !value.is_empty())
                    else {
                        continue;
                    };
                    let type_expr = first_child_element_ci(variable, "type")
                        .and_then(parse_type_expression_container);
                    let type_expr = match type_expr {
                        Some(type_expr) => type_expr,
                        None => {
                            warnings.push(format!(
                                "interface '{interface_name}': member '{member_name}' variable '{variable_name}' has no projectable type; defaulted to STRING"
                            ));
                            "STRING".to_string()
                        }
                    };
                    block.push('\n');
                    block.push_str(&format!(
                        "    {st_keyword} {variable_name} : {type_expr}; END_VAR"
                    ));
                }
            }
        }
    }
    block.push('\n');
    block.push_str("END_METHOD");
    Some(block)
}
