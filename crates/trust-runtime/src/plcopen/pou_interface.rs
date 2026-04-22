fn detect_program_pous_used_as_types(root: roxmltree::Node<'_, '_>) -> HashSet<String> {
    let mut program_names = HashSet::new();
    for pou in root
        .descendants()
        .filter(|node| is_element_named_ci(*node, "pou"))
    {
        let Some(pou_name) = extract_pou_name(pou) else {
            continue;
        };
        let Some(raw_type) = attribute_ci(pou, "pouType").or_else(|| attribute_ci(pou, "type"))
        else {
            continue;
        };
        if PlcopenPouType::from_xml(&raw_type).is_some_and(|kind| kind == PlcopenPouType::Program) {
            program_names.insert(pou_name.to_ascii_lowercase());
        }
    }

    if program_names.is_empty() {
        return HashSet::new();
    }

    let mut promoted = HashSet::new();
    for derived in root
        .descendants()
        .filter(|node| is_element_named_ci(*node, "derived"))
    {
        let Some(name) = attribute_ci(derived, "name")
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if program_names.contains(&name) {
            promoted.insert(name);
        }
    }
    promoted
}

fn synthesize_import_pou_source(
    _root: roxmltree::Node<'_, '_>,
    pou_node: roxmltree::Node<'_, '_>,
    pou_type: PlcopenPouType,
    pou_name: &str,
    st_body: Option<&str>,
    warnings: &mut Vec<String>,
    unsupported_diagnostics: &mut Vec<PlcopenUnsupportedDiagnostic>,
) -> Option<String> {
    let normalized_body = st_body.map(normalize_body_text).unwrap_or_default();
    let has_body = !normalized_body.trim().is_empty();
    let imported_methods = extract_codesys_method_sources(
        pou_node,
        pou_type,
        pou_name,
        warnings,
        unsupported_diagnostics,
    );

    if has_body && source_has_top_level_pou_declaration(&normalized_body, pou_type) {
        return Some(merge_pou_source_with_imported_methods(
            normalized_body,
            pou_type,
            &imported_methods,
        ));
    }

    let metadata = extract_pou_interface_metadata(pou_node, pou_type);
    if !has_body && !metadata.has_details() && imported_methods.is_empty() {
        return None;
    }

    let header = render_import_pou_header(
        pou_type,
        pou_name,
        &metadata,
        warnings,
        unsupported_diagnostics,
    )?;
    let mut synthesized = String::new();
    synthesized.push_str(&header);
    synthesized.push('\n');

    for section in &metadata.sections {
        if section.declarations.is_empty() {
            continue;
        }
        synthesized.push_str(&section.header);
        synthesized.push('\n');
        for declaration in &section.declarations {
            synthesized.push_str(declaration);
            synthesized.push('\n');
        }
        synthesized.push_str("END_VAR\n");
    }

    if has_body {
        synthesized.push_str(normalized_body.trim_end());
        synthesized.push('\n');
        warnings.push(format!(
            "pou '{}' body omitted a top-level declaration wrapper; synthesized '{}'",
            pou_name,
            pou_type.declaration_keyword()
        ));
        unsupported_diagnostics.push(unsupported_diagnostic(
            "PLCO207",
            "info",
            "pou/body/ST",
            "POU ST body lacked declaration wrapper; importer synthesized one",
            Some(pou_name.to_string()),
            "Review synthesized declaration sections for vendor-specific details",
        ));
    } else {
        warnings.push(format!(
            "pou '{}' had missing/empty ST body; synthesized declaration shell from {}",
            pou_name,
            if imported_methods.is_empty() {
                "interface metadata"
            } else {
                "interface/vendor metadata"
            }
        ));
        unsupported_diagnostics.push(unsupported_diagnostic(
            "PLCO208",
            "info",
            "pou/interface",
            "POU body missing or empty; importer synthesized a declaration shell from interface/vendor metadata",
            Some(pou_name.to_string()),
            "Manual body implementation may still be required after import",
        ));
    }

    append_imported_methods(&mut synthesized, &imported_methods);

    if pou_type == PlcopenPouType::Function
        && !function_result_assignment_present(&synthesized, pou_name)
    {
        synthesized.push_str(&format!("{pou_name} := {pou_name};\n"));
        warnings.push(format!(
            "function '{}' lacked an explicit result assignment; inserted default self-assignment",
            pou_name
        ));
        unsupported_diagnostics.push(unsupported_diagnostic(
            "PLCO212",
            "info",
            "pou/body/ST",
            "Function body lacked explicit result assignment; importer inserted a default self-assignment",
            Some(pou_name.to_string()),
            "Review the inserted assignment and replace it with domain-specific return logic",
        ));
    }

    synthesized.push_str(pou_type.end_keyword());
    synthesized.push('\n');
    Some(synthesized)
}

fn source_has_top_level_pou_declaration(source: &str, pou_type: PlcopenPouType) -> bool {
    let parsed = parser::parse(source);
    parsed
        .syntax()
        .children()
        .any(|node| node_to_pou_type(&node).is_some_and(|candidate| candidate == pou_type))
}

fn extract_pou_interface_metadata(
    pou_node: roxmltree::Node<'_, '_>,
    _pou_type: PlcopenPouType,
) -> PouInterfaceMetadata {
    let mut metadata = PouInterfaceMetadata::default();

    if let Some(interface) = first_child_element_ci(pou_node, "interface") {
        metadata.function_return_type = first_child_element_ci(interface, "returnType")
            .and_then(parse_type_expression_container)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let section_defs = [
            ("inputVars", "VAR_INPUT"),
            ("outputVars", "VAR_OUTPUT"),
            ("inOutVars", "VAR_IN_OUT"),
            ("externalVars", "VAR_EXTERNAL"),
            ("localVars", "VAR"),
            ("tempVars", "VAR_TEMP"),
            ("globalVars", "VAR_GLOBAL"),
            ("accessVars", "VAR_ACCESS"),
        ];
        for (xml_name, st_keyword) in section_defs {
            for section in interface
                .children()
                .filter(|child| is_element_named_ci(*child, xml_name))
            {
                let declarations = parse_interface_section_declarations(section, st_keyword);
                if !declarations.is_empty() {
                    metadata.sections.push(InterfaceVarSection {
                        header: build_interface_section_header(section, st_keyword),
                        declarations,
                    });
                }
            }
        }
    }

    metadata.header_hint = extract_interface_plaintext_header(pou_node);
    metadata
}

fn parse_interface_section_declarations(
    section: roxmltree::Node<'_, '_>,
    st_keyword: &str,
) -> Vec<String> {
    if st_keyword.eq_ignore_ascii_case("VAR_ACCESS") {
        return parse_access_var_declarations(section);
    }
    parse_interface_var_declarations(section)
}

fn build_interface_section_header(section: roxmltree::Node<'_, '_>, st_keyword: &str) -> String {
    if st_keyword.eq_ignore_ascii_case("VAR_ACCESS") {
        return st_keyword.to_string();
    }

    let mut header = st_keyword.to_string();
    for (xml_attr, st_modifier) in [
        ("constant", "CONSTANT"),
        ("retain", "RETAIN"),
        ("nonretain", "NON_RETAIN"),
        ("persistent", "PERSISTENT"),
        ("nonpersistent", "NON_PERSISTENT"),
    ] {
        if attribute_ci(section, xml_attr).is_some_and(|value| {
            matches!(value.trim().to_ascii_lowercase().as_str(), "true" | "1")
        }) {
            header.push(' ');
            header.push_str(st_modifier);
        }
    }
    header
}

fn parse_interface_var_declarations(section: roxmltree::Node<'_, '_>) -> Vec<String> {
    let mut declarations = Vec::new();
    for variable in section
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
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let Some(type_expr) = first_child_element_ci(variable, "type")
            .and_then(parse_type_expression_container)
            .or_else(|| {
                first_child_element_ci(variable, "baseType")
                    .and_then(parse_type_expression_container)
            })
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let initializer = first_child_element_ci(variable, "initialValue")
            .and_then(parse_initial_value)
            .map_or_else(String::new, |value| format!(" := {value}"));
        declarations.push(format!("    {name} : {type_expr}{initializer};"));
    }
    declarations
}

fn parse_access_var_declarations(section: roxmltree::Node<'_, '_>) -> Vec<String> {
    let mut declarations = Vec::new();
    for variable in section
        .children()
        .filter(|child| is_element_named_ci(*child, "accessVariable"))
    {
        let Some(alias) = attribute_ci(variable, "alias")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let Some(instance_path) = attribute_ci(variable, "instancePathAndName")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let Some(type_expr) = first_child_element_ci(variable, "type")
            .and_then(parse_type_expression_container)
            .or_else(|| {
                first_child_element_ci(variable, "baseType")
                    .and_then(parse_type_expression_container)
            })
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let direction = attribute_ci(variable, "direction")
            .map(|value| value.trim().to_ascii_lowercase())
            .map(|value| {
                if value == "readonly" {
                    "READ_ONLY"
                } else {
                    "READ_WRITE"
                }
            })
            .unwrap_or("READ_WRITE");
        declarations.push(format!(
            "    {alias} : {instance_path} : {type_expr} {direction};"
        ));
    }
    declarations
}

fn render_import_pou_header(
    pou_type: PlcopenPouType,
    pou_name: &str,
    metadata: &PouInterfaceMetadata,
    warnings: &mut Vec<String>,
    unsupported_diagnostics: &mut Vec<PlcopenUnsupportedDiagnostic>,
) -> Option<String> {
    match pou_type {
        PlcopenPouType::Program => Some(format!("PROGRAM {pou_name}")),
        PlcopenPouType::FunctionBlock => Some(format!("FUNCTION_BLOCK {pou_name}")),
        PlcopenPouType::Function => {
            if let Some(return_type) = metadata.function_return_type.as_deref() {
                return Some(format!("FUNCTION {pou_name} : {}", return_type.trim()));
            }
            if let Some(return_type) = metadata
                .header_hint
                .as_deref()
                .and_then(parse_function_return_type_from_header)
            {
                return Some(format!("FUNCTION {pou_name} : {}", return_type.trim()));
            }
            warnings.push(format!(
                "function '{}' did not provide an importable return type; defaulting to INT",
                pou_name
            ));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO211",
                "warning",
                "pou/interface/returnType",
                "Function return type missing in PLCopen interface metadata; defaulted to INT",
                Some(pou_name.to_string()),
                "Review the imported FUNCTION signature and adjust the return type manually",
            ));
            Some(format!("FUNCTION {pou_name} : INT"))
        }
    }
}

fn extract_interface_plaintext_header(pou_node: roxmltree::Node<'_, '_>) -> Option<String> {
    const POU_PREFIXES: [&str; 3] = ["PROGRAM ", "FUNCTION_BLOCK ", "FUNCTION "];
    let text = extract_interface_plaintext(pou_node)?;
    for line in text.lines() {
        let trimmed = line.trim();
        let upper = trimmed.to_ascii_uppercase();
        if POU_PREFIXES.iter().any(|prefix| upper.starts_with(prefix)) {
            return Some(trimmed.to_string());
        }
    }
    None
}

fn extract_codesys_method_sources(
    pou_node: roxmltree::Node<'_, '_>,
    pou_type: PlcopenPouType,
    pou_name: &str,
    warnings: &mut Vec<String>,
    unsupported_diagnostics: &mut Vec<PlcopenUnsupportedDiagnostic>,
) -> Vec<(String, String)> {
    let Some(add_data) = first_child_element_ci(pou_node, "addData") else {
        return Vec::new();
    };

    let mut method_nodes = Vec::new();
    for data in add_data
        .children()
        .filter(|child| is_element_named_ci(*child, "data"))
    {
        let Some(name) = attribute_ci(data, "name") else {
            continue;
        };
        if !name.to_ascii_lowercase().contains("plcopenxml/method")
            && !name.eq_ignore_ascii_case(CODESYS_METHOD_DATA_NAME)
        {
            continue;
        }
        method_nodes.extend(
            data.children()
                .filter(|child| is_element_named_ci(*child, "Method")),
        );
    }

    if method_nodes.is_empty() {
        return Vec::new();
    }
    if pou_type != PlcopenPouType::FunctionBlock {
        warnings.push(format!(
            "pou '{}' contains {} CODESYS method object(s) on '{}'; truST only imports vendor method metadata into FUNCTION_BLOCK sources",
            pou_name,
            method_nodes.len(),
            pou_type.as_xml()
        ));
        unsupported_diagnostics.push(unsupported_diagnostic(
            "PLCO213",
            "warning",
            "addData/data[method]",
            "CODESYS method objects were skipped because the owning POU is not imported as a FUNCTION_BLOCK",
            Some(pou_name.to_string()),
            "Convert the source POU to a FUNCTION_BLOCK or move the methods to a supported owner type",
        ));
        return Vec::new();
    }

    method_nodes
        .into_iter()
        .filter_map(|method_node| {
            render_codesys_method_source(
                method_node,
                pou_name,
                warnings,
                unsupported_diagnostics,
            )
        })
        .collect()
}

fn render_codesys_method_source(
    method_node: roxmltree::Node<'_, '_>,
    owner_name: &str,
    warnings: &mut Vec<String>,
    unsupported_diagnostics: &mut Vec<PlcopenUnsupportedDiagnostic>,
) -> Option<(String, String)> {
    let name = extract_pou_name(method_node)?;
    let header = render_codesys_method_header(
        method_node,
        owner_name,
        &name,
        warnings,
        unsupported_diagnostics,
    );
    let body = extract_st_body(method_node)
        .map(normalize_body_text)
        .unwrap_or_default();

    let mut rendered = String::new();
    rendered.push_str(&header);
    rendered.push('\n');

    if let Some(interface) = first_child_element_ci(method_node, "interface") {
        let section_defs = [
            ("inputVars", "VAR_INPUT"),
            ("outputVars", "VAR_OUTPUT"),
            ("inOutVars", "VAR_IN_OUT"),
            ("externalVars", "VAR_EXTERNAL"),
            ("localVars", "VAR"),
            ("tempVars", "VAR_TEMP"),
            ("globalVars", "VAR_GLOBAL"),
            ("accessVars", "VAR_ACCESS"),
        ];
        for (xml_name, st_keyword) in section_defs {
            for section in interface
                .children()
                .filter(|child| is_element_named_ci(*child, xml_name))
            {
                let declarations = parse_interface_section_declarations(section, st_keyword);
                if declarations.is_empty() {
                    continue;
                }
                rendered.push_str(&build_interface_section_header(section, st_keyword));
                rendered.push('\n');
                for declaration in declarations {
                    rendered.push_str(&declaration);
                    rendered.push('\n');
                }
                rendered.push_str("END_VAR\n");
            }
        }
    }

    if !body.trim().is_empty() {
        rendered.push_str(body.trim_end());
        rendered.push('\n');
    }
    rendered.push_str("END_METHOD\n");

    Some((name, rendered))
}

fn render_codesys_method_header(
    method_node: roxmltree::Node<'_, '_>,
    owner_name: &str,
    method_name: &str,
    warnings: &mut Vec<String>,
    unsupported_diagnostics: &mut Vec<PlcopenUnsupportedDiagnostic>,
) -> String {
    if let Some(text) = extract_interface_plaintext(method_node) {
        for line in text.lines() {
            let trimmed = line.trim().trim_end_matches(';').trim();
            let upper = trimmed.to_ascii_uppercase();
            let tokens = upper.split_whitespace().collect::<Vec<_>>();
            if !upper.starts_with("END_METHOD")
                && (matches!(tokens.first(), Some(&"METHOD"))
                    || matches!(tokens.get(1), Some(&"METHOD")))
            {
                return trimmed.to_string();
            }
        }
    }

    warnings.push(format!(
        "method '{}.{}' lacked interface-as-plaintext header metadata; defaulted visibility to PUBLIC during import",
        owner_name, method_name
    ));
    unsupported_diagnostics.push(unsupported_diagnostic(
        "PLCO214",
        "info",
        "addData/data[method]/InterfaceAsPlainText",
        "CODESYS method metadata lacked a plaintext declaration header; importer defaulted the method to PUBLIC",
        Some(owner_name.to_string()),
        "Review imported method visibility/modifiers and adjust them manually if the source project used non-default access",
    ));

    let return_type = first_child_element_ci(method_node, "interface")
        .and_then(|interface| first_child_element_ci(interface, "returnType"))
        .and_then(parse_type_expression_container)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let mut header = format!("METHOD PUBLIC {method_name}");
    if let Some(return_type) = return_type {
        header.push_str(" : ");
        header.push_str(&return_type);
    }
    header
}

fn append_imported_methods(target: &mut String, imported_methods: &[(String, String)]) {
    if imported_methods.is_empty() {
        return;
    }

    if !target.ends_with("\n\n") {
        if !target.ends_with('\n') {
            target.push('\n');
        }
        target.push('\n');
    }

    for (index, (_name, method_source)) in imported_methods.iter().enumerate() {
        target.push_str(method_source.trim_end());
        target.push('\n');
        if index + 1 < imported_methods.len() {
            target.push('\n');
        }
    }
}

fn merge_pou_source_with_imported_methods(
    source: String,
    pou_type: PlcopenPouType,
    imported_methods: &[(String, String)],
) -> String {
    if imported_methods.is_empty() {
        return source;
    }

    let missing_methods = imported_methods
        .iter()
        .filter(|(name, _)| !source_contains_method_named(&source, name))
        .collect::<Vec<_>>();
    if missing_methods.is_empty() {
        return source;
    }

    let Some(insert_at) = source.rfind(pou_type.end_keyword()) else {
        return source;
    };

    let mut insertion = String::new();
    if !source[..insert_at].ends_with("\n\n") {
        if !source[..insert_at].ends_with('\n') {
            insertion.push('\n');
        }
        insertion.push('\n');
    }
    for (index, (_name, method_source)) in missing_methods.iter().enumerate() {
        insertion.push_str(method_source.trim_end());
        insertion.push('\n');
        if index + 1 < missing_methods.len() {
            insertion.push('\n');
        }
    }

    let mut merged = source;
    merged.insert_str(insert_at, &insertion);
    normalize_body_text(merged)
}

fn source_contains_method_named(source: &str, method_name: &str) -> bool {
    let parsed = parser::parse(source);
    parsed.syntax().children().any(|node| {
        node.children()
            .filter(|child| child.kind() == SyntaxKind::Method)
            .filter_map(|child| declaration_name(&child))
            .any(|candidate| candidate.eq_ignore_ascii_case(method_name))
    })
}

fn parse_function_return_type_from_header(header: &str) -> Option<String> {
    let (_, suffix) = header.split_once(':')?;
    let return_type = suffix.trim().trim_end_matches(';').trim().to_string();
    if return_type.is_empty() {
        None
    } else {
        Some(return_type)
    }
}

fn function_result_assignment_present(source: &str, function_name: &str) -> bool {
    let target = function_name.trim();
    if target.is_empty() {
        return false;
    }

    let mut in_block_comment = false;
    for line in source.lines() {
        let mut text = line.to_string();

        if in_block_comment {
            if let Some(end) = text.find("*)") {
                text = text[end + 2..].to_string();
                in_block_comment = false;
            } else {
                continue;
            }
        }

        while let Some(start) = text.find("(*") {
            if let Some(end_rel) = text[start + 2..].find("*)") {
                let end = start + 2 + end_rel;
                text.replace_range(start..end + 2, "");
            } else {
                text.truncate(start);
                in_block_comment = true;
                break;
            }
        }

        if let Some(comment) = text.find("//") {
            text.truncate(comment);
        }

        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Some((lhs, _rhs)) = trimmed.split_once(":=") else {
            continue;
        };
        if lhs.trim().eq_ignore_ascii_case(target) {
            return true;
        }
    }

    false
}
