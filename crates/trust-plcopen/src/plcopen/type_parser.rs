fn format_task_declaration(task: &TaskDecl) -> String {
    let mut elements = Vec::new();
    if let Some(single) = task
        .single
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        elements.push(format!("SINGLE := {}", single.trim()));
    }
    if let Some(interval) = task
        .interval
        .as_ref()
        .map(|value| normalize_task_interval_literal(value))
    {
        elements.push(format!("INTERVAL := {}", interval.trim()));
    } else if task.single.is_none() {
        elements.push("INTERVAL := T#100ms".to_string());
    }
    if let Some(priority) = task
        .priority
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        elements.push(format!("PRIORITY := {}", priority.trim()));
    } else {
        elements.push("PRIORITY := 1".to_string());
    }
    format!("TASK {} ({});", task.name, elements.join(", "))
}

fn format_program_binding(program: &ProgramBindingDecl) -> String {
    if let Some(task_name) = &program.task_name {
        format!(
            "PROGRAM {} WITH {} : {};",
            program.instance_name, task_name, program.type_name
        )
    } else {
        format!("PROGRAM {} : {};", program.instance_name, program.type_name)
    }
}

fn sanitize_st_identifier(raw: &str, fallback: &str) -> String {
    let mut out = String::new();
    for (index, ch) in raw.chars().enumerate() {
        let valid = if index == 0 {
            ch.is_ascii_alphabetic() || ch == '_'
        } else {
            ch.is_ascii_alphanumeric() || ch == '_'
        };
        if valid {
            out.push(ch);
        } else if ch.is_ascii_alphanumeric() {
            if index == 0 {
                out.push('_');
                out.push(ch);
            } else {
                out.push(ch);
            }
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        return fallback.to_string();
    }
    if out.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

fn unique_identifier(candidate: String, used_lowercase: &mut HashSet<String>) -> String {
    let base = candidate;
    let mut output = base.clone();
    let mut index = 2usize;
    while !used_lowercase.insert(output.to_ascii_lowercase()) {
        output = format!("{base}_{index}");
        index += 1;
    }
    output
}

fn attribute_ci_any(node: &roxmltree::Node<'_, '_>, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| attribute_ci(*node, name))
}

fn format_data_type_declaration(name: &str, type_expr: &str) -> String {
    if !type_expr.contains('\n') {
        return format!("  {name} : {type_expr};");
    }

    let mut lines = type_expr.lines();
    let first = lines.next().unwrap_or_default();
    let mut declaration = format!("  {name} : {first}\n");
    for line in lines {
        declaration.push_str("  ");
        declaration.push_str(line);
        declaration.push('\n');
    }
    if declaration.ends_with('\n') {
        declaration.pop();
    }
    declaration.push(';');
    declaration
}

fn parse_data_type_expression(data_type: roxmltree::Node<'_, '_>) -> Option<String> {
    if let Some(base_type) = first_child_element_ci(data_type, "baseType") {
        if let Some(expr) = parse_type_expression_container(base_type) {
            return Some(expr);
        }
    }
    if let Some(type_node) = first_child_element_ci(data_type, "type") {
        if let Some(expr) = parse_type_expression_container(type_node) {
            return Some(expr);
        }
    }
    parse_type_expression_container(data_type)
}

fn parse_type_expression_container(container: roxmltree::Node<'_, '_>) -> Option<String> {
    container
        .children()
        .find(|child| child.is_element())
        .and_then(parse_type_expression_node)
}

fn parse_type_expression_node(node: roxmltree::Node<'_, '_>) -> Option<String> {
    let kind = node.tag_name().name().to_ascii_lowercase();
    if is_elementary_type_tag(&kind) {
        return Some(kind.to_ascii_uppercase());
    }

    match kind.as_str() {
        "derived" => attribute_ci(node, "name")
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty()),
        "string" | "wstring" => {
            let mut base = kind.to_ascii_uppercase();
            if let Some(length) = attribute_ci(node, "length")
                .or_else(|| attribute_ci(node, "maxLength"))
                .map(|raw| raw.trim().to_string())
                .filter(|raw| !raw.is_empty())
            {
                base.push('[');
                base.push_str(&length);
                base.push(']');
            }
            Some(base)
        }
        "array" => parse_array_type_expression(node),
        "struct" => parse_struct_type_expression(node),
        "enum" => parse_enum_type_expression(node),
        "subrange" => parse_subrange_type_expression(node),
        _ => None,
    }
}

fn parse_array_type_expression(array_node: roxmltree::Node<'_, '_>) -> Option<String> {
    let dimensions = array_node
        .children()
        .filter(|child| is_element_named_ci(*child, "dimension"))
        .filter_map(|dimension| {
            let lower = attribute_ci(dimension, "lower")
                .or_else(|| attribute_ci(dimension, "lowerLimit"))?;
            let upper = attribute_ci(dimension, "upper")
                .or_else(|| attribute_ci(dimension, "upperLimit"))?;
            Some(format!("{}..{}", lower.trim(), upper.trim()))
        })
        .collect::<Vec<_>>();
    if dimensions.is_empty() {
        return None;
    }

    let base_expr = first_child_element_ci(array_node, "baseType")
        .and_then(parse_type_expression_container)
        .or_else(|| {
            first_child_element_ci(array_node, "type").and_then(parse_type_expression_container)
        })?;
    Some(format!("ARRAY[{}] OF {}", dimensions.join(", "), base_expr))
}

fn parse_struct_type_expression(struct_node: roxmltree::Node<'_, '_>) -> Option<String> {
    let mut fields = Vec::new();
    for variable in struct_node.children().filter(|child| {
        is_element_named_ci(*child, "variable") || is_element_named_ci(*child, "member")
    }) {
        let Some(name) = attribute_ci(variable, "name")
            .or_else(|| {
                variable
                    .children()
                    .find(|child| is_element_named_ci(*child, "name"))
                    .and_then(extract_text_content)
            })
            .map(|raw| raw.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let Some(var_type) = first_child_element_ci(variable, "type")
            .and_then(parse_type_expression_container)
            .or_else(|| {
                first_child_element_ci(variable, "baseType")
                    .and_then(parse_type_expression_container)
            })
        else {
            continue;
        };
        let initializer = first_child_element_ci(variable, "initialValue")
            .and_then(parse_initial_value)
            .map_or_else(String::new, |value| format!(" := {value}"));
        fields.push(format!("    {name} : {var_type}{initializer};"));
    }

    let mut out = String::from("STRUCT\n");
    for field in fields {
        out.push_str(&field);
        out.push('\n');
    }
    out.push_str("END_STRUCT");
    Some(out)
}

fn parse_enum_type_expression(enum_node: roxmltree::Node<'_, '_>) -> Option<String> {
    let values_parent = first_child_element_ci(enum_node, "values").unwrap_or(enum_node);
    let mut values = Vec::new();
    for value in values_parent
        .children()
        .filter(|child| is_element_named_ci(*child, "value"))
    {
        let Some(name) = attribute_ci(value, "name")
            .or_else(|| extract_text_content(value))
            .map(|raw| raw.trim().to_string())
            .filter(|raw| !raw.is_empty())
        else {
            continue;
        };
        if let Some(raw_value) = attribute_ci(value, "value")
            .map(|raw| raw.trim().to_string())
            .filter(|raw| !raw.is_empty())
        {
            values.push(format!("{name} := {raw_value}"));
        } else {
            values.push(name);
        }
    }

    if values.is_empty() {
        None
    } else {
        Some(format!("({})", values.join(", ")))
    }
}

fn parse_subrange_type_expression(subrange_node: roxmltree::Node<'_, '_>) -> Option<String> {
    let lower = attribute_ci(subrange_node, "lower")
        .or_else(|| {
            first_child_element_ci(subrange_node, "range")
                .and_then(|range| attribute_ci(range, "lower"))
        })
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())?;
    let upper = attribute_ci(subrange_node, "upper")
        .or_else(|| {
            first_child_element_ci(subrange_node, "range")
                .and_then(|range| attribute_ci(range, "upper"))
        })
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())?;
    let base_expr = first_child_element_ci(subrange_node, "baseType")
        .and_then(parse_type_expression_container)
        .or_else(|| {
            first_child_element_ci(subrange_node, "type").and_then(parse_type_expression_container)
        })?;
    Some(format!("{base_expr}({lower}..{upper})"))
}

fn parse_initial_value(initial_value: roxmltree::Node<'_, '_>) -> Option<String> {
    first_child_element_ci(initial_value, "simpleValue")
        .and_then(|simple| attribute_ci(simple, "value").or_else(|| extract_text_content(simple)))
        .or_else(|| extract_text_content(initial_value))
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())
}

fn first_child_element_ci<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
    name: &str,
) -> Option<roxmltree::Node<'a, 'input>> {
    node.children()
        .find(|child| is_element_named_ci(*child, name))
}

fn is_elementary_type_tag(name: &str) -> bool {
    matches!(
        name,
        "bool"
            | "byte"
            | "word"
            | "dword"
            | "lword"
            | "sint"
            | "int"
            | "dint"
            | "lint"
            | "usint"
            | "uint"
            | "udint"
            | "ulint"
            | "real"
            | "lreal"
            | "time"
            | "ltime"
            | "date"
            | "ldate"
            | "tod"
            | "ltod"
            | "dt"
            | "ldt"
            | "char"
            | "wchar"
    )
}

