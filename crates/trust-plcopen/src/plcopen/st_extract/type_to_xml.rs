fn type_expression_to_plcopen_base_type_xml(type_expr: &str) -> Option<String> {
    let trimmed = type_expr.trim();
    if trimmed.is_empty() {
        return None;
    }
    let upper = trimmed.to_ascii_uppercase();

    if upper.starts_with("ARRAY[") {
        return type_expr_array_to_xml(trimmed);
    }
    if upper.starts_with("STRUCT") {
        return type_expr_struct_to_xml(trimmed);
    }
    if trimmed.starts_with('(') && trimmed.ends_with(')') {
        return type_expr_enum_to_xml(trimmed);
    }
    if let Some(value) = type_expr_subrange_to_xml(trimmed) {
        return Some(value);
    }
    type_expr_simple_to_xml(trimmed)
}

fn type_expr_array_to_xml(type_expr: &str) -> Option<String> {
    let open = type_expr.find('[')?;
    let close = type_expr.find(']')?;
    if close <= open {
        return None;
    }
    let dims_text = type_expr[open + 1..close].trim();
    let base_text = type_expr[close + 1..].trim();
    let of_pos = base_text.to_ascii_uppercase().find("OF")?;
    let base_expr = base_text[of_pos + 2..].trim();
    let base_xml = type_expression_to_plcopen_base_type_xml(base_expr)?;

    let mut xml = String::from("<array>\n");
    for dimension in dims_text.split(',') {
        let (lower, upper) = dimension.split_once("..")?;
        xml.push_str(&format!(
            "  <dimension lower=\"{}\" upper=\"{}\"/>\n",
            escape_xml_attr(lower.trim()),
            escape_xml_attr(upper.trim())
        ));
    }
    xml.push_str("  <baseType>\n");
    for line in base_xml.lines() {
        xml.push_str("    ");
        xml.push_str(line);
        xml.push('\n');
    }
    xml.push_str("  </baseType>\n");
    xml.push_str("</array>");
    Some(xml)
}

fn type_expr_struct_to_xml(type_expr: &str) -> Option<String> {
    let upper = type_expr.to_ascii_uppercase();
    let end_index = upper.rfind("END_STRUCT")?;
    let body = type_expr.get("STRUCT".len()..end_index)?.trim();
    let mut xml = String::from("<struct>\n");

    for raw_line in body.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let line = line.trim_end_matches(';').trim();
        let (name, rhs) = line.split_once(':')?;
        let field_name = name.trim();
        if field_name.is_empty() {
            continue;
        }
        let (field_type, field_init) = match rhs.split_once(":=") {
            Some((type_part, init_part)) => (type_part.trim(), Some(init_part.trim())),
            None => (rhs.trim(), None),
        };
        let field_xml = type_expression_to_plcopen_base_type_xml(field_type)?;

        xml.push_str(&format!(
            "  <variable name=\"{}\">\n",
            escape_xml_attr(field_name)
        ));
        xml.push_str("    <type>\n");
        for line in field_xml.lines() {
            xml.push_str("      ");
            xml.push_str(line);
            xml.push('\n');
        }
        xml.push_str("    </type>\n");
        if let Some(initial_value) = field_init.filter(|value| !value.is_empty()) {
            xml.push_str("    <initialValue>\n");
            xml.push_str(&format!(
                "      <simpleValue value=\"{}\"/>\n",
                escape_xml_attr(initial_value)
            ));
            xml.push_str("    </initialValue>\n");
        }
        xml.push_str("  </variable>\n");
    }

    xml.push_str("</struct>");
    Some(xml)
}

fn type_expr_enum_to_xml(type_expr: &str) -> Option<String> {
    let inner = type_expr
        .trim()
        .strip_prefix('(')?
        .strip_suffix(')')?
        .trim();
    if inner.is_empty() {
        return None;
    }

    let mut xml = String::from("<enum>\n  <values>\n");
    for item in inner.split(',') {
        let value = item.trim();
        if value.is_empty() {
            continue;
        }
        if let Some((name, raw)) = value.split_once(":=") {
            xml.push_str(&format!(
                "    <value name=\"{}\" value=\"{}\"/>\n",
                escape_xml_attr(name.trim()),
                escape_xml_attr(raw.trim())
            ));
        } else {
            xml.push_str(&format!(
                "    <value name=\"{}\"/>\n",
                escape_xml_attr(value)
            ));
        }
    }
    xml.push_str("  </values>\n</enum>");
    Some(xml)
}

fn type_expr_subrange_to_xml(type_expr: &str) -> Option<String> {
    let open = type_expr.rfind('(')?;
    let close = type_expr.rfind(')')?;
    if close <= open {
        return None;
    }
    let base_expr = type_expr[..open].trim();
    let range = type_expr[open + 1..close].trim();
    let (lower, upper) = range.split_once("..")?;
    let base_xml = type_expression_to_plcopen_base_type_xml(base_expr)?;

    let mut xml = String::from(&format!(
        "<subrange lower=\"{}\" upper=\"{}\">\n",
        escape_xml_attr(lower.trim()),
        escape_xml_attr(upper.trim())
    ));
    xml.push_str("  <baseType>\n");
    for line in base_xml.lines() {
        xml.push_str("    ");
        xml.push_str(line);
        xml.push('\n');
    }
    xml.push_str("  </baseType>\n");
    xml.push_str("</subrange>");
    Some(xml)
}

fn type_expr_simple_to_xml(type_expr: &str) -> Option<String> {
    let trimmed = type_expr.trim();
    let upper = trimmed.to_ascii_uppercase();
    if upper.starts_with("STRING[") && upper.ends_with(']') {
        let length = trimmed[7..trimmed.len() - 1].trim();
        return Some(format!("<string length=\"{}\"/>", escape_xml_attr(length)));
    }
    if upper.starts_with("WSTRING[") && upper.ends_with(']') {
        let length = trimmed[8..trimmed.len() - 1].trim();
        return Some(format!("<wstring length=\"{}\"/>", escape_xml_attr(length)));
    }
    if upper == "STRING" {
        return Some("<string />".to_string());
    }
    if upper == "WSTRING" {
        return Some("<wstring />".to_string());
    }

    if is_elementary_type_tag(&upper.to_ascii_lowercase()) {
        return Some(format!("<{} />", upper.to_ascii_lowercase()));
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.')
    {
        return Some(format!("<derived name=\"{}\"/>", escape_xml_attr(trimmed)));
    }
    None
}

