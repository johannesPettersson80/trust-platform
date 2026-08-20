fn sanitize_filename(name: &str) -> String {
    let mut sanitized = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    while sanitized.ends_with('.') || sanitized.ends_with(' ') {
        sanitized.pop();
    }
    if sanitized.is_empty() {
        sanitized.push('_');
    }
    let device_name = sanitized
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(
        device_name.as_str(),
        "con" | "prn" | "aux" | "nul" | "com1" | "com2" | "com3" | "com4"
            | "com5" | "com6" | "com7" | "com8" | "com9" | "lpt1" | "lpt2"
            | "lpt3" | "lpt4" | "lpt5" | "lpt6" | "lpt7" | "lpt8" | "lpt9"
    ) {
        sanitized.insert(0, '_');
    }
    sanitized
}

#[cfg(test)]
fn is_element_named(node: roxmltree::Node<'_, '_>, name: &str) -> bool {
    node.is_element() && node.tag_name().name() == name
}

fn is_element_named_ci(node: roxmltree::Node<'_, '_>, name: &str) -> bool {
    node.is_element() && node.tag_name().name().eq_ignore_ascii_case(name)
}

fn attribute_ci(node: roxmltree::Node<'_, '_>, name: &str) -> Option<String> {
    node.attributes()
        .find(|attribute| attribute.name().eq_ignore_ascii_case(name))
        .map(|attribute| attribute.value().to_string())
}

fn extract_pou_name(node: roxmltree::Node<'_, '_>) -> Option<String> {
    attribute_ci(node, "name")
        .or_else(|| attribute_ci(node, "pouName"))
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .or_else(|| {
            node.children()
                .find(|child| is_element_named_ci(*child, "name"))
                .and_then(extract_text_content)
        })
}

fn extract_st_body(node: roxmltree::Node<'_, '_>) -> Option<String> {
    let mut worksheets = Vec::new();
    for body in node
        .children()
        .filter(|child| is_element_named_ci(*child, "body"))
    {
        if let Some(candidate) = extract_single_st_body(body) {
            worksheets.push(candidate);
        }
    }
    if worksheets.is_empty() {
        None
    } else {
        Some(worksheets.join("\n\n"))
    }
}

fn extract_single_st_body(body: roxmltree::Node<'_, '_>) -> Option<String> {
    for preferred in ["ST", "st", "text", "Text", "xhtml"] {
        if let Some(candidate) = body
            .descendants()
            .find(|entry| is_element_named_ci(*entry, preferred))
            .and_then(extract_text_content)
        {
            return Some(candidate);
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NonStBodyDiagnostic {
    code: &'static str,
    node: String,
    kind: String,
    message: String,
    action: &'static str,
}

fn collect_non_st_body_diagnostics(pou: roxmltree::Node<'_, '_>) -> Vec<NonStBodyDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen = HashSet::new();

    for body in pou
        .children()
        .filter(|child| is_element_named_ci(*child, "body"))
    {
        let mut has_body_payload = false;
        let mut emitted_body_diagnostic = false;
        let body_elements = body
            .children()
            .filter(|child| child.is_element())
            .collect::<Vec<_>>();
        let has_supported_body = body_elements
            .iter()
            .any(|child| is_supported_st_body_element(*child));

        for child in body.children() {
            if child.is_text() {
                has_body_payload |= child.text().is_some_and(|text| !text.trim().is_empty());
                continue;
            }
            if !child.is_element() {
                continue;
            }

            has_body_payload = true;
            if is_supported_st_body_element(child) {
                continue;
            }
            if has_supported_body && is_benign_body_metadata_element(child) {
                continue;
            }

            let diagnostic = non_st_body_diagnostic_for(child.tag_name().name());
            if seen.insert((diagnostic.code, diagnostic.node.clone())) {
                diagnostics.push(diagnostic);
            }
            emitted_body_diagnostic = true;
        }

        if !has_supported_body && !emitted_body_diagnostic && has_body_payload {
            let diagnostic = non_st_body_diagnostic_for("unknown");
            if seen.insert((diagnostic.code, diagnostic.node.clone())) {
                diagnostics.push(diagnostic);
            }
        }
    }

    diagnostics
}

fn is_supported_st_body_element(node: roxmltree::Node<'_, '_>) -> bool {
    ["ST", "st", "text", "Text", "xhtml"]
        .iter()
        .any(|name| is_element_named_ci(node, name))
}

fn is_benign_body_metadata_element(node: roxmltree::Node<'_, '_>) -> bool {
    ["addData", "documentation", "comment"]
        .iter()
        .any(|name| is_element_named_ci(node, name))
}

fn non_st_body_diagnostic_for(tag_name: &str) -> NonStBodyDiagnostic {
    let normalized = tag_name.to_ascii_uppercase();
    match normalized.as_str() {
        "FBD" => NonStBodyDiagnostic {
            code: "PLCO215",
            node: "pou/body/FBD".to_string(),
            kind: "FBD".to_string(),
            message:
                "POU skipped because it contains an unsupported FBD graphical body".to_string(),
            action: "Export the POU as Structured Text or manually translate the FBD network before import",
        },
        "LD" => NonStBodyDiagnostic {
            code: "PLCO216",
            node: "pou/body/LD".to_string(),
            kind: "LD".to_string(),
            message: "POU skipped because it contains an unsupported LD graphical body".to_string(),
            action: "Export the POU as Structured Text or manually translate the ladder network before import",
        },
        "SFC" => NonStBodyDiagnostic {
            code: "PLCO217",
            node: "pou/body/SFC".to_string(),
            kind: "SFC".to_string(),
            message:
                "POU skipped because it contains an unsupported SFC graphical body".to_string(),
            action: "Export the POU as Structured Text or manually translate the SFC chart before import",
        },
        _ => NonStBodyDiagnostic {
            code: "PLCO218",
            node: format!("pou/body/{tag_name}"),
            kind: format!("unknown non-ST body '{tag_name}'"),
            message: format!(
                "POU skipped because body element '<{tag_name}>' is not an ST body"
            ),
            action: "Provide a PLCopen body/ST, body/text, or body/xhtml payload containing Structured Text",
        },
    }
}

fn extract_interface_plaintext(node: roxmltree::Node<'_, '_>) -> Option<String> {
    let add_data = first_child_element_ci(node, "addData")?;
    for data in add_data
        .children()
        .filter(|child| is_element_named_ci(*child, "data"))
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
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn extract_text_content(node: roxmltree::Node<'_, '_>) -> Option<String> {
    let text = node
        .descendants()
        .filter(|entry| entry.is_text())
        .filter_map(|entry| entry.text())
        .collect::<String>();
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn collect_import_pou_nodes<'a, 'input>(
    root: roxmltree::Node<'a, 'input>,
) -> Vec<roxmltree::Node<'a, 'input>> {
    let mut standard = Vec::new();
    for types in root
        .children()
        .filter(|child| is_element_named_ci(*child, "types"))
    {
        for pous in types
            .children()
            .filter(|child| is_element_named_ci(*child, "pous"))
        {
            for pou in pous
                .children()
                .filter(|child| is_element_named_ci(*child, "pou"))
            {
                standard.push(pou);
            }
        }
    }

    if !standard.is_empty() {
        return standard;
    }

    root.descendants()
        .filter(|node| is_element_named_ci(*node, "pou"))
        .collect()
}

fn sanitize_path_segment(name: &str, fallback: &str) -> String {
    let mut segment = sanitize_filename(name.trim());
    while segment.starts_with('_') {
        segment.remove(0);
    }
    if segment.is_empty() {
        fallback.to_string()
    } else {
        segment
    }
}

fn extract_object_id_from_node(node: roxmltree::Node<'_, '_>) -> Option<String> {
    let direct_add_data = first_child_element_ci(node, "addData");
    let mut data_nodes = Vec::new();
    if let Some(add_data) = direct_add_data {
        data_nodes.extend(
            add_data
                .children()
                .filter(|child| is_element_named_ci(*child, "data")),
        );
    } else {
        data_nodes.extend(
            node.descendants()
                .filter(|entry| is_element_named_ci(*entry, "data")),
        );
    }

    for data in data_nodes {
        let Some(name) = attribute_ci(data, "name") else {
            continue;
        };
        if !name.to_ascii_lowercase().contains("objectid")
            && !name.eq_ignore_ascii_case(CODESYS_OBJECT_ID_DATA_NAME)
        {
            continue;
        }
        if let Some(object_id_node) = data
            .descendants()
            .find(|entry| is_element_named_ci(*entry, "ObjectId"))
            .or_else(|| {
                data.descendants()
                    .find(|entry| is_element_named_ci(*entry, "objectId"))
            })
        {
            if let Some(text) = extract_text_content(object_id_node) {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
        if let Some(text) = extract_text_content(data) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}
