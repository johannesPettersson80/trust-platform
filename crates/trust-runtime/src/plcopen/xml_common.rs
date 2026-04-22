fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
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
    extract_text_content(body)
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
