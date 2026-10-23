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
    let requires_filename_sanitization = pou_name.contains('/')
        || pou_name.contains('\\')
        || pou_name.contains("..")
        || pou_name
            .chars()
            .any(|ch| !ch.is_ascii_alphanumeric() && ch != '_' && ch != ' ');
    let safe_pou_name = if requires_filename_sanitization {
        sanitize_st_identifier(pou_name, "ImportedPou")
    } else {
        pou_name.to_string()
    };
    if !is_valid_st_identifier(&safe_pou_name) {
        return None;
    }
    let normalized_body = st_body.map(normalize_body_text).unwrap_or_default();
    let has_body = !normalized_body.trim().is_empty();
    let mut imported_methods = extract_codesys_method_sources(
        pou_node,
        pou_type,
        pou_name,
        warnings,
        unsupported_diagnostics,
    );
    let property_stubs = extract_codesys_property_stubs(
        pou_node,
        pou_type,
        pou_name,
        warnings,
        unsupported_diagnostics,
    );
    for (property_name, property_source) in property_stubs {
        if imported_methods
            .iter()
            .any(|(method_name, _)| method_name.eq_ignore_ascii_case(&property_name))
        {
            continue;
        }
        imported_methods.push((property_name, property_source));
    }
    let (extends_base, implements_list) = extract_pou_inheritance_clauses(
        pou_node,
        pou_type,
        pou_name,
        warnings,
        unsupported_diagnostics,
    );

    if has_body && source_has_top_level_pou_declaration(&normalized_body, pou_type) {
        let merged =
            merge_pou_source_with_imported_methods(normalized_body, pou_type, &imported_methods);
        return Some(apply_pou_inheritance_clauses(
            merged,
            pou_type,
            &safe_pou_name,
            extends_base.as_deref(),
            implements_list.as_deref(),
            warnings,
            unsupported_diagnostics,
        ));
    }

    let metadata = extract_pou_interface_metadata(pou_node, pou_type);
    if metadata.invalid {
        return None;
    }
    if !has_body
        && !metadata.has_details()
        && imported_methods.is_empty()
        && extends_base.is_none()
        && implements_list.is_none()
    {
        return None;
    }

    let header = render_import_pou_header(
        pou_type,
        &safe_pou_name,
        &metadata,
        extends_base.as_deref(),
        implements_list.as_deref(),
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
        // Keep the plain local header discoverable as `VAR\n`; a trailing
        // space on other section terminators avoids matching `END_VAR\n`
        // when consumers scan the ordered section headers textually.
        if section.header.eq_ignore_ascii_case("VAR") {
            synthesized.push_str("END_VAR\n");
        } else {
            synthesized.push_str("END_VAR \n");
        }
    }

    if has_body {
        synthesized.push_str(normalized_body.trim_end());
        synthesized.push('\n');
        warnings.push(format!(
            "pou '{}' body omitted a top-level declaration wrapper; synthesized '{}'",
            safe_pou_name,
            pou_type.declaration_keyword()
        ));
        unsupported_diagnostics.push(unsupported_diagnostic(
            "PLCO207",
            "info",
            "pou/body/ST",
            "POU ST body lacked declaration wrapper; importer synthesized one",
            Some(safe_pou_name.clone()),
            "Review synthesized declaration sections for vendor-specific details",
        ));
    } else {
        warnings.push(format!(
            "pou '{}' had missing/empty ST body; synthesized declaration shell from {}",
            safe_pou_name,
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
            Some(safe_pou_name.clone()),
            "Manual body implementation may still be required after import",
        ));
    }

    append_imported_methods(&mut synthesized, &imported_methods);

    if pou_type == PlcopenPouType::Function
        && !function_result_assignment_present(&synthesized, &safe_pou_name)
    {
        synthesized.push_str(&format!("{safe_pou_name} := {safe_pou_name};\n"));
        warnings.push(format!(
            "function '{}' lacked an explicit result assignment; inserted default self-assignment",
            safe_pou_name
        ));
        unsupported_diagnostics.push(unsupported_diagnostic(
            "PLCO212",
            "info",
            "pou/body/ST",
            "Function body lacked explicit result assignment; importer inserted a default self-assignment",
            Some(safe_pou_name.clone()),
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
                if interface_section_has_contradictory_modifiers(section)
                    || !interface_section_is_well_formed(section, st_keyword)
                {
                    metadata.invalid = true;
                    continue;
                }
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

fn interface_section_has_contradictory_modifiers(section: roxmltree::Node<'_, '_>) -> bool {
    let enabled = |name: &str| {
        attribute_ci(section, name).is_some_and(|value| {
            matches!(value.trim().to_ascii_lowercase().as_str(), "true" | "1")
        })
    };
    (enabled("retain") && enabled("nonretain"))
        || (enabled("persistent") && enabled("nonpersistent"))
}

fn interface_section_is_well_formed(
    section: roxmltree::Node<'_, '_>,
    st_keyword: &str,
) -> bool {
    let access = st_keyword.eq_ignore_ascii_case("VAR_ACCESS");
    let element_name = if access { "accessVariable" } else { "variable" };
    let mut identities = HashSet::new();
    for variable in section
        .children()
        .filter(|child| is_element_named_ci(*child, element_name))
    {
        let identity = if access {
            attribute_ci(variable, "alias")
        } else {
            attribute_ci(variable, "name").or_else(|| {
                variable
                    .children()
                    .find(|child| is_element_named_ci(*child, "name"))
                    .and_then(extract_text_content)
            })
        };
        let Some(identity) = identity
            .map(|value| value.trim().to_string())
            .filter(|value| is_valid_st_identifier(value))
        else {
            return false;
        };
        if !identities.insert(identity.to_ascii_lowercase()) {
            return false;
        }
        if access
            && attribute_ci(variable, "instancePathAndName")
                .is_none_or(|value| value.trim().is_empty())
        {
            return false;
        }
        let has_type = first_child_element_ci(variable, "type")
            .and_then(parse_type_expression_container)
            .or_else(|| {
                first_child_element_ci(variable, "baseType")
                    .and_then(parse_type_expression_container)
            })
            .is_some_and(|value| !value.trim().is_empty());
        if !has_type {
            return false;
        }
    }
    true
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
    extends_base: Option<&str>,
    implements_list: Option<&str>,
    warnings: &mut Vec<String>,
    unsupported_diagnostics: &mut Vec<PlcopenUnsupportedDiagnostic>,
) -> Option<String> {
    if !is_valid_st_identifier(pou_name) {
        return None;
    }
    let inheritance_suffix = match pou_type {
        PlcopenPouType::FunctionBlock => render_pou_inheritance_suffix(
            pou_name,
            extends_base,
            implements_list,
            unsupported_diagnostics,
        ),
        _ => String::new(),
    };
    match pou_type {
        PlcopenPouType::Program => Some(format!("PROGRAM {pou_name}")),
        PlcopenPouType::FunctionBlock => {
            Some(format!("FUNCTION_BLOCK {pou_name}{inheritance_suffix}"))
        }
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
        // Standalone action POUs never reach header synthesis: they are
        // merged into their parent POU before writing.
        PlcopenPouType::Action => Some(format!("PROGRAM {pou_name}")),
    }
}

/// Renders the vendor OOP inheritance suffix (`EXTENDS base`, `IMPLEMENTS
/// a, b`) for a FUNCTION_BLOCK header. Clause extraction has already
/// validated the identifiers and dropped non-FUNCTION_BLOCK owners.
fn render_pou_inheritance_suffix(
    pou_name: &str,
    extends_base: Option<&str>,
    implements_list: Option<&str>,
    unsupported_diagnostics: &mut Vec<PlcopenUnsupportedDiagnostic>,
) -> String {
    let mut suffix = String::new();
    if let Some(base) = extends_base {
        suffix.push_str(&format!(" EXTENDS {base}"));
    }
    if let Some(implements) = implements_list {
        suffix.push_str(&format!(" IMPLEMENTS {implements}"));
    }
    if !suffix.is_empty() {
        unsupported_diagnostics.push(unsupported_diagnostic(
            "PLCO221",
            "info",
            "pou/interface/pouinheritance",
            "CODESYS pouinheritance clauses carried into the FUNCTION_BLOCK declaration",
            Some(pou_name.to_string()),
            "Review the imported EXTENDS/IMPLEMENTS clauses for semantic parity with the source vendor model",
        ));
    }
    suffix
}

/// CODESYS FB inheritance lives in a `pouinheritance` addData block under the
/// POU interface: `<Inheritance><Extends>..</Extends><Implements>a,
/// b</Implements></Inheritance>`. Clause text is whitespace-normalized and
/// validated per identifier; anything unprojectable is reported and dropped
/// instead of being silently lost or emitted as invalid ST.
fn extract_pou_inheritance_clauses(
    pou_node: roxmltree::Node<'_, '_>,
    pou_type: PlcopenPouType,
    pou_name: &str,
    warnings: &mut Vec<String>,
    unsupported_diagnostics: &mut Vec<PlcopenUnsupportedDiagnostic>,
) -> (Option<String>, Option<String>) {
    let mut extends_base = None;
    let mut implements_list = None;
    let Some(interface) = first_child_element_ci(pou_node, "interface") else {
        return (None, None);
    };
    let Some(add_data) = first_child_element_ci(interface, "addData") else {
        return (None, None);
    };
    for data in add_data
        .children()
        .filter(|child| is_element_named_ci(*child, "data"))
    {
        let Some(name) = attribute_ci(data, "name") else {
            continue;
        };
        if !name.to_ascii_lowercase().contains("pouinheritance") {
            continue;
        }
        for inheritance in data
            .descendants()
            .filter(|node| is_element_named_ci(*node, "Inheritance"))
        {
            for child in inheritance.children().filter(|child| child.is_element()) {
                if is_element_named_ci(child, "Extends") {
                    extends_base = inheritance_clause_identifier(
                        child,
                        "EXTENDS",
                        pou_type,
                        pou_name,
                        warnings,
                        unsupported_diagnostics,
                    );
                } else if is_element_named_ci(child, "Implements") {
                    implements_list = inheritance_clause_list(
                        child,
                        pou_type,
                        pou_name,
                        warnings,
                        unsupported_diagnostics,
                    );
                }
            }
        }
    }
    (extends_base, implements_list)
}

fn inheritance_clause_identifier(
    extends: roxmltree::Node<'_, '_>,
    clause: &str,
    pou_type: PlcopenPouType,
    pou_name: &str,
    warnings: &mut Vec<String>,
    unsupported_diagnostics: &mut Vec<PlcopenUnsupportedDiagnostic>,
) -> Option<String> {
    let text = extract_text_content(extends)?;
    let base = text.trim();
    if !is_valid_qualified_identifier(base) {
        warnings.push(format!(
            "pou '{pou_name}': {clause} base '{base}' is not a projectable identifier and was dropped"
        ));
        unsupported_diagnostics.push(unsupported_diagnostic(
            "PLCO221",
            "warning",
            "pou/interface/pouinheritance",
            format!("{clause} base '{base}' could not be projected and was dropped"),
            Some(pou_name.to_string()),
            format!("Provide a valid identifier in the {clause} clause"),
        ));
        return None;
    }
    if pou_type != PlcopenPouType::FunctionBlock {
        warnings.push(format!(
            "pou '{pou_name}': {clause} metadata is only honored on FUNCTION_BLOCK POUs and was dropped"
        ));
        return None;
    }
    Some(base.to_string())
}

fn inheritance_clause_list(
    implements: roxmltree::Node<'_, '_>,
    pou_type: PlcopenPouType,
    pou_name: &str,
    warnings: &mut Vec<String>,
    unsupported_diagnostics: &mut Vec<PlcopenUnsupportedDiagnostic>,
) -> Option<String> {
    let text = extract_text_content(implements)?;
    if pou_type != PlcopenPouType::FunctionBlock {
        warnings.push(format!(
            "pou '{pou_name}': IMPLEMENTS metadata is only honored on FUNCTION_BLOCK POUs and was dropped"
        ));
        return None;
    }
    // Vendor exports separate interface entries with commas, newlines, or
    // both; treat every comma or whitespace run as an entry separator.
    let mut names = Vec::new();
    for token in text
        .split(|ch: char| ch == ',' || ch.is_whitespace())
        .filter(|token| !token.is_empty())
    {
        let name = token.trim();
        if !is_valid_qualified_identifier(name) {
            warnings.push(format!(
                "pou '{pou_name}': IMPLEMENTS entry '{name}' is not a projectable identifier and was dropped"
            ));
            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO221",
                "warning",
                "pou/interface/pouinheritance",
                format!("IMPLEMENTS entry '{name}' could not be projected and was dropped"),
                Some(pou_name.to_string()),
                "Provide valid identifiers in the IMPLEMENTS clause",
            ));
            continue;
        }
        names.push(name.to_string());
    }
    if names.is_empty() {
        return None;
    }
    Some(names.join(", "))
}

/// Splices `EXTENDS`/`IMPLEMENTS` into a raw FUNCTION_BLOCK source whose
/// declaration line came from the vendor ST body. Per-clause idempotent: a
/// declaration line already carrying the clause is left untouched.
fn apply_pou_inheritance_clauses(
    source: String,
    pou_type: PlcopenPouType,
    pou_name: &str,
    extends_base: Option<&str>,
    implements_list: Option<&str>,
    warnings: &mut Vec<String>,
    unsupported_diagnostics: &mut Vec<PlcopenUnsupportedDiagnostic>,
) -> String {
    if pou_type != PlcopenPouType::FunctionBlock {
        return source;
    }
    let Some(declaration_offset) = find_function_block_declaration_line(&source, pou_name) else {
        if extends_base.is_some() || implements_list.is_some() {
            warnings.push(format!(
                "pou '{pou_name}': pouinheritance metadata found but no FUNCTION_BLOCK declaration line to attach it to"
            ));
        }
        return source;
    };
    let line_end = source[declaration_offset..]
        .find('\n')
        .map_or(source.len(), |found| declaration_offset + found);
    let declaration_line = &source[declaration_offset..line_end];

    let mut insertion = String::new();
    if let Some(base) = extends_base
        && !declaration_has_clause(declaration_line, "EXTENDS")
    {
        insertion.push_str(&format!(" EXTENDS {base}"));
    }
    if let Some(implements) = implements_list
        && !declaration_has_clause(declaration_line, "IMPLEMENTS")
    {
        insertion.push_str(&format!(" IMPLEMENTS {implements}"));
    }
    if insertion.is_empty() {
        return source;
    }
    let name_end =
        function_block_declaration_name_end(declaration_line, pou_name).unwrap_or(line_end);
    let mut updated = String::with_capacity(source.len() + insertion.len());
    updated.push_str(&source[..declaration_offset + name_end]);
    updated.push_str(&insertion);
    updated.push_str(&source[declaration_offset + name_end..]);

    unsupported_diagnostics.push(unsupported_diagnostic(
        "PLCO221",
        "info",
        "pou/interface/pouinheritance",
        "CODESYS pouinheritance clauses carried into the FUNCTION_BLOCK declaration",
        Some(pou_name.to_string()),
        "Review the imported EXTENDS/IMPLEMENTS clauses for semantic parity with the source vendor model",
    ));
    updated
}

/// Byte offset of the first line whose `FUNCTION_BLOCK` declaration names
/// `pou_name` (case-insensitive whole-token match).
fn find_function_block_declaration_line(source: &str, pou_name: &str) -> Option<usize> {
    let mut offset = 0usize;
    for line in source.split_inclusive('\n') {
        let mut tokens = line.split_whitespace();
        if tokens
            .next()
            .is_some_and(|first| first.eq_ignore_ascii_case("FUNCTION_BLOCK"))
            && tokens.next().is_some_and(|second| {
                second.trim_end_matches(';').eq_ignore_ascii_case(pou_name)
            })
        {
            return Some(offset);
        }
        offset += line.len();
    }
    None
}

/// Byte offset (relative to the declaration line) just past the POU name
/// token — the splice point for inheritance clauses. `None` when the line
/// does not name `pou_name`.
fn function_block_declaration_name_end(declaration_line: &str, pou_name: &str) -> Option<usize> {
    let lowered = declaration_line.to_ascii_lowercase();
    let keyword = lowered.find("function_block")?;
    let rest = &declaration_line[keyword + "function_block".len()..];
    let name_start = rest.len() - rest.trim_start().len();
    let name_text = rest.trim_start();
    let name_len = name_text
        .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .unwrap_or(name_text.len());
    let name = &name_text[..name_len];
    if name.eq_ignore_ascii_case(pou_name) {
        Some(keyword + "function_block".len() + name_start + name_len)
    } else {
        None
    }
}

fn declaration_has_clause(declaration_line: &str, clause: &str) -> bool {
    declaration_line
        .split_whitespace()
        .any(|token| token.eq_ignore_ascii_case(clause))
}

/// e!COCKPIT FB `<property>` objects (accessor pairs) are dropped by the ST
/// projection, but interface completeness checks still require the accessor
/// method to exist. Synthesize a `METHOD PUBLIC` stub with a neutral default
/// assignment so the mirror satisfies completeness without inventing domain
/// logic.
fn extract_codesys_property_stubs(
    pou_node: roxmltree::Node<'_, '_>,
    pou_type: PlcopenPouType,
    pou_name: &str,
    warnings: &mut Vec<String>,
    unsupported_diagnostics: &mut Vec<PlcopenUnsupportedDiagnostic>,
) -> Vec<(String, String)> {
    const PROPERTY_DEFAULTS: [(&str, &str); 4] = [
        ("BOOL", "FALSE"),
        ("INT", "0"),
        ("DINT", "0"),
        ("REAL", "0.0"),
    ];

    let Some(add_data) = first_child_element_ci(pou_node, "addData") else {
        return Vec::new();
    };
    let mut property_nodes = Vec::new();
    for data in add_data
        .children()
        .filter(|child| is_element_named_ci(*child, "data"))
    {
        let Some(name) = attribute_ci(data, "name") else {
            continue;
        };
        if !name.to_ascii_lowercase().contains("plcopenxml/property") {
            continue;
        }
        property_nodes.extend(
            data.children()
                .filter(|child| is_element_named_ci(*child, "Property")),
        );
    }
    if property_nodes.is_empty() {
        return Vec::new();
    }
    if pou_type != PlcopenPouType::FunctionBlock {
        warnings.push(format!(
            "pou '{}' contains {} CODESYS property object(s); truST only imports vendor property metadata into FUNCTION_BLOCK sources",
            pou_name,
            property_nodes.len()
        ));
        unsupported_diagnostics.push(unsupported_diagnostic(
            "PLCO213",
            "warning",
            "addData/data[property]",
            "CODESYS property objects were skipped because the owning POU is not imported as a FUNCTION_BLOCK",
            Some(pou_name.to_string()),
            "Move the properties to a FUNCTION_BLOCK owner type",
        ));
        return Vec::new();
    }

    property_nodes
        .into_iter()
        .filter_map(|property| {
            let property_name = attribute_ci(property, "name")
                .map(|name| name.trim().to_string())
                .filter(|name| !name.is_empty());
            let Some(property_name) = property_name else {
                warnings.push(format!(
                    "pou '{pou_name}': CODESYS property object without a name was skipped"
                ));
                return None;
            };
            let return_type = first_child_element_ci(property, "interface")
                .and_then(|interface| first_child_element_ci(interface, "returnType"))
                .and_then(parse_type_expression_container);

            let mut rendered = format!("METHOD PUBLIC {property_name}");
            if let Some(return_type) = &return_type {
                rendered.push_str(" : ");
                rendered.push_str(return_type.trim());
            }
            if let Some((_, default)) = return_type.as_deref().and_then(|return_type| {
                PROPERTY_DEFAULTS
                    .iter()
                    .find(|(kind, _)| return_type.trim().eq_ignore_ascii_case(kind))
            }) {
                rendered.push('\n');
                rendered.push_str(&format!("    {property_name} := {default};"));
            }
            rendered.push_str("\nEND_METHOD");

            unsupported_diagnostics.push(unsupported_diagnostic(
                "PLCO225",
                "info",
                "addData/data[property]",
                "CODESYS property accessor synthesized as a METHOD stub with a neutral default assignment",
                Some(pou_name.to_string()),
                "Replace the stub accessor body with the source property logic",
            ));
            Some((property_name, rendered))
        })
        .collect()
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
    if !matches!(
        pou_type,
        PlcopenPouType::FunctionBlock | PlcopenPouType::Program
    ) {
        warnings.push(format!(
            "pou '{}' contains {} CODESYS method object(s) on '{}'; truST only imports vendor method metadata into FUNCTION_BLOCK and PROGRAM sources",
            pou_name,
            method_nodes.len(),
            pou_type.as_xml()
        ));
        unsupported_diagnostics.push(unsupported_diagnostic(
            "PLCO213",
            "warning",
            "addData/data[method]",
            "CODESYS method objects were skipped because the owning POU is not imported as a FUNCTION_BLOCK or PROGRAM",
            Some(pou_name.to_string()),
            "Move the methods to a FUNCTION_BLOCK or PROGRAM owner type",
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
