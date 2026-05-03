fn vendor_library_shims_for_ecosystem(ecosystem: &str) -> &'static [VendorLibraryShim] {
    match ecosystem {
        "siemens-tia" => SIEMENS_LIBRARY_SHIMS,
        "rockwell-studio5000" => ROCKWELL_LIBRARY_SHIMS,
        "schneider-ecostruxure" | "codesys" | "openplc" => SCHNEIDER_LIBRARY_SHIMS,
        "mitsubishi-gxworks3" => MITSUBISHI_LIBRARY_SHIMS,
        _ => &[],
    }
}

fn apply_vendor_library_shims(
    body: &str,
    ecosystem: &str,
) -> (String, Vec<PlcopenLibraryShimApplication>) {
    let shims = vendor_library_shims_for_ecosystem(ecosystem);
    if shims.is_empty() {
        return (body.to_string(), Vec::new());
    }

    let tokens = lex(body);
    if tokens.is_empty() {
        return (body.to_string(), Vec::new());
    }

    let mut output = String::with_capacity(body.len());
    let mut cursor = 0usize;
    let mut counts: BTreeMap<(String, String, String, String), usize> = BTreeMap::new();

    for (index, token) in tokens.iter().enumerate() {
        let start = usize::from(token.range.start());
        let end = usize::from(token.range.end());
        if start > cursor {
            output.push_str(&body[cursor..start]);
        }

        let token_text = &body[start..end];
        if let Some(shim) = match_library_shim(shims, token_text, &tokens, index) {
            output.push_str(shim.replacement_symbol);
            let key = (
                ecosystem.to_string(),
                shim.source_symbol.to_string(),
                shim.replacement_symbol.to_string(),
                shim.notes.to_string(),
            );
            *counts.entry(key).or_insert(0) += 1;
        } else {
            output.push_str(token_text);
        }
        cursor = end;
    }

    if cursor < body.len() {
        output.push_str(&body[cursor..]);
    }

    let applications = counts
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
        .collect();
    (output, applications)
}

fn match_library_shim<'a>(
    shims: &'a [VendorLibraryShim],
    token_text: &str,
    tokens: &[trust_syntax::lexer::Token],
    index: usize,
) -> Option<&'a VendorLibraryShim> {
    let upper = token_text.to_ascii_uppercase();
    let shim = shims
        .iter()
        .find(|candidate| candidate.source_symbol == upper)?;

    let previous = previous_non_trivia_token_kind(tokens, index);
    let next = next_non_trivia_token_kind(tokens, index);
    if previous == Some(TokenKind::Dot) {
        return None;
    }

    let type_position = matches!(
        previous,
        Some(TokenKind::Colon)
            | Some(TokenKind::KwOf)
            | Some(TokenKind::KwExtends)
            | Some(TokenKind::KwRefTo)
    );
    let call_position = next == Some(TokenKind::LParen);
    if type_position || call_position {
        Some(shim)
    } else {
        None
    }
}

fn previous_non_trivia_token_kind(
    tokens: &[trust_syntax::lexer::Token],
    index: usize,
) -> Option<TokenKind> {
    let mut current = index;
    while current > 0 {
        current -= 1;
        let kind = tokens[current].kind;
        if !kind.is_trivia() {
            return Some(kind);
        }
    }
    None
}

fn next_non_trivia_token_kind(
    tokens: &[trust_syntax::lexer::Token],
    index: usize,
) -> Option<TokenKind> {
    let mut current = index + 1;
    while current < tokens.len() {
        let kind = tokens[current].kind;
        if !kind.is_trivia() {
            return Some(kind);
        }
        current += 1;
    }
    None
}

fn calculate_source_coverage(imported: usize, discovered: usize) -> f64 {
    if discovered == 0 {
        return 0.0;
    }
    round_percent((imported as f64 / discovered as f64) * 100.0)
}

fn calculate_semantic_loss(
    imported: usize,
    discovered: usize,
    unsupported_nodes: usize,
    loss_warnings: usize,
) -> f64 {
    if discovered == 0 {
        return 100.0;
    }

    let skipped = discovered.saturating_sub(imported);
    let skipped_ratio = skipped as f64 / discovered as f64;
    let unsupported_ratio =
        unsupported_nodes as f64 / (unsupported_nodes as f64 + discovered as f64);
    let warning_ratio = (loss_warnings as f64 / (discovered as f64 * 2.0)).min(1.0);

    round_percent((skipped_ratio * 70.0) + (unsupported_ratio * 20.0) + (warning_ratio * 10.0))
}

fn calculate_compatibility_coverage(
    imported_pous: usize,
    skipped_pous: usize,
    unsupported_nodes: usize,
    shimmed_occurrences: usize,
) -> PlcopenCompatibilityCoverage {
    let supported_items = imported_pous;
    let partial_items = unsupported_nodes + shimmed_occurrences;
    let unsupported_items = skipped_pous;
    let total = supported_items + partial_items + unsupported_items;
    let support_percent = if total == 0 {
        0.0
    } else {
        round_percent((supported_items as f64 / total as f64) * 100.0)
    };
    let verdict = if total == 0 {
        "none"
    } else if unsupported_items == 0 && partial_items == 0 {
        "full"
    } else if supported_items > 0 {
        "partial"
    } else {
        "low"
    };
    PlcopenCompatibilityCoverage {
        supported_items,
        partial_items,
        unsupported_items,
        support_percent,
        verdict: verdict.to_string(),
    }
}

fn unsupported_diagnostic(
    code: &str,
    severity: &str,
    node: impl Into<String>,
    message: impl Into<String>,
    pou: Option<String>,
    action: impl Into<String>,
) -> PlcopenUnsupportedDiagnostic {
    PlcopenUnsupportedDiagnostic {
        code: code.to_string(),
        severity: severity.to_string(),
        node: node.into(),
        message: message.into(),
        pou,
        action: action.into(),
    }
}

fn round_percent(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn detect_vendor_ecosystem(root: roxmltree::Node<'_, '_>, xml_text: &str) -> String {
    let mut hints = String::new();
    for node in root.descendants().filter(|node| node.is_element()) {
        for attribute in node.attributes() {
            hints.push_str(attribute.value());
            hints.push(' ');
        }
        if is_element_named_ci(node, "data") {
            if let Some(name) = attribute_ci(node, "name") {
                hints.push_str(&name);
                hints.push(' ');
            }
        }
    }
    hints.push_str(xml_text);
    let normalized = hints.to_ascii_lowercase();

    if normalized.contains("twincat") || normalized.contains("beckhoff") {
        "beckhoff-twincat".to_string()
    } else if normalized.contains("openplc")
        || normalized.contains("open plc")
        || normalized.contains("openplc editor")
    {
        "openplc".to_string()
    } else if normalized.contains("schneider")
        || normalized.contains("ecostruxure")
        || normalized.contains("unity pro")
        || normalized.contains("control expert")
    {
        "schneider-ecostruxure".to_string()
    } else if normalized.contains("codesys")
        || normalized.contains("3s-smart")
        || normalized.contains("machine expert")
    {
        "codesys".to_string()
    } else if normalized.contains("siemens")
        || normalized.contains("tia portal")
        || normalized.contains("step7")
    {
        "siemens-tia".to_string()
    } else if normalized.contains("rockwell")
        || normalized.contains("studio 5000")
        || normalized.contains("allen-bradley")
    {
        "rockwell-studio5000".to_string()
    } else if normalized.contains("mitsubishi")
        || normalized.contains("gx works")
        || normalized.contains("gxworks")
        || normalized.contains("melsoft")
    {
        "mitsubishi-gxworks3".to_string()
    } else {
        "generic-plcopen".to_string()
    }
}

fn write_migration_report(
    project_root: &Path,
    report: &PlcopenMigrationReport,
) -> anyhow::Result<PathBuf> {
    let report_path = project_root.join(MIGRATION_REPORT_FILE);
    if let Some(parent) = report_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create migration report directory '{}'",
                parent.display()
            )
        })?;
    }
    let json = serde_json::to_string_pretty(report)?;
    std::fs::write(&report_path, format!("{json}\n")).with_context(|| {
        format!(
            "failed to write PLCopen migration report '{}'",
            report_path.display()
        )
    })?;
    Ok(report_path)
}

fn inspect_unsupported_structure(
    root: roxmltree::Node<'_, '_>,
    unsupported_nodes: &mut Vec<String>,
    warnings: &mut Vec<String>,
    unsupported_diagnostics: &mut Vec<PlcopenUnsupportedDiagnostic>,
) {
    for child in root.children().filter(|child| child.is_element()) {
        let name = child.tag_name().name();
        if !matches!(
            name.to_ascii_lowercase().as_str(),
            "fileheader" | "contentheader" | "types" | "instances" | "adddata"
        ) {
            unsupported_nodes.push(name.to_string());
            warnings.push(format!(
                "unsupported PLCopen node '<{}>' preserved as metadata only",
                name
            ));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO101",
                "warning",
                name,
                format!("Unsupported top-level PLCopen node '<{}>'", name),
                None,
                "Preserved as metadata only; not imported into runtime semantics",
            ));
        }
        if name.eq_ignore_ascii_case("types") {
            for type_child in child.children().filter(|entry| entry.is_element()) {
                let type_name = type_child.tag_name().name();
                if !type_name.eq_ignore_ascii_case("pous")
                    && !type_name.eq_ignore_ascii_case("dataTypes")
                {
                    unsupported_nodes.push(format!("types/{}", type_name));
                    warnings.push(format!(
                        "unsupported PLCopen node '<types>/<{}>' skipped (ST-complete subset)",
                        type_name
                    ));
                    unsupported_diagnostics.push(unsupported_diagnostic(
                        "PLCO102",
                        "warning",
                        format!("types/{type_name}"),
                        format!("Unsupported PLCopen <types>/<{}> section", type_name),
                        None,
                        "Skipped in ST-complete subset; migrate supported ST declarations manually",
                    ));
                }
            }
        } else if name.eq_ignore_ascii_case("instances") {
            for instances_child in child.children().filter(|entry| entry.is_element()) {
                let instances_name = instances_child.tag_name().name();
                if !instances_name.eq_ignore_ascii_case("configurations")
                    && !instances_name.eq_ignore_ascii_case("configuration")
                    && !instances_name.eq_ignore_ascii_case("resource")
                {
                    unsupported_nodes.push(format!("instances/{instances_name}"));
                    warnings.push(format!(
                        "unsupported PLCopen node '<instances>/<{}>' skipped",
                        instances_name
                    ));
                    unsupported_diagnostics.push(unsupported_diagnostic(
                        "PLCO103",
                        "warning",
                        format!("instances/{instances_name}"),
                        format!("Unsupported PLCopen <instances>/<{}> section", instances_name),
                        None,
                        "Skipped in ST-complete subset; provide configurations/resources/tasks/program instances",
                    ));
                }
            }
        }
    }
}

fn parse_embedded_source_map(root: roxmltree::Node<'_, '_>) -> Option<SourceMapPayload> {
    let payload = root
        .descendants()
        .find(|node| {
            is_element_named_ci(*node, "data")
                && attribute_ci(*node, "name").is_some_and(|name| name == SOURCE_MAP_DATA_NAME)
        })
        .and_then(|node| {
            node.children()
                .find(|child| is_element_named_ci(*child, "text"))
                .and_then(extract_text_content)
        })?;
    serde_json::from_str::<SourceMapPayload>(&payload).ok()
}

fn preserve_vendor_extensions(
    root: roxmltree::Node<'_, '_>,
    xml_text: &str,
    project_root: &Path,
    warnings: &mut Vec<String>,
) -> anyhow::Result<Option<PathBuf>> {
    let mut preserved = Vec::new();

    for node in root.descendants().filter(|node| {
        is_element_named_ci(*node, "data")
            && attribute_ci(*node, "name").is_none_or(|name| name != SOURCE_MAP_DATA_NAME)
    }) {
        let range = node.range();
        if let Some(slice) = xml_text.get(range) {
            preserved.push(slice.trim().to_string());
        }
    }

    if preserved.is_empty() {
        return Ok(None);
    }

    let output = project_root.join(IMPORTED_VENDOR_EXTENSION_FILE);
    let mut content = String::from("<vendorExtensions>\n");
    for fragment in preserved {
        content.push_str("  ");
        content.push_str(&fragment);
        content.push('\n');
    }
    content.push_str("</vendorExtensions>\n");
    std::fs::write(&output, content)
        .with_context(|| format!("failed to write '{}'", output.display()))?;
    warnings.push(format!(
        "preserved vendor extension nodes in {}",
        output.display()
    ));
    Ok(Some(output))
}
