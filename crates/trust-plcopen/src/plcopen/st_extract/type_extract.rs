fn extract_data_type_declarations(source: &LoadedSource) -> (Vec<DataTypeDecl>, Vec<String>) {
    let mut declarations = Vec::new();
    let mut warnings = Vec::new();
    let lines = source.text.lines().collect::<Vec<_>>();
    let mut line_index = 0usize;

    while line_index < lines.len() {
        if !lines[line_index].trim().eq_ignore_ascii_case("TYPE") {
            line_index += 1;
            continue;
        }

        line_index += 1;
        let mut declaration_text = String::new();
        let mut declaration_start_line = line_index + 1;
        let mut struct_depth = 0usize;

        while line_index < lines.len() {
            let raw_line = lines[line_index];
            let trimmed = raw_line.trim();

            if trimmed.eq_ignore_ascii_case("END_TYPE") {
                if !declaration_text.trim().is_empty() {
                    warnings.push(format!(
                        "{}:{} unfinished TYPE declaration skipped during PLCopen export",
                        source.path.display(),
                        declaration_start_line
                    ));
                }
                break;
            }

            if trimmed.is_empty() {
                line_index += 1;
                continue;
            }

            if declaration_text.trim().is_empty() {
                declaration_start_line = line_index + 1;
            }

            if !declaration_text.is_empty() {
                declaration_text.push('\n');
            }
            declaration_text.push_str(raw_line.trim_end());

            let upper = trimmed.to_ascii_uppercase();
            if upper.contains(": STRUCT") || upper == "STRUCT" {
                struct_depth = struct_depth.saturating_add(1);
            }
            if upper.contains("END_STRUCT") {
                struct_depth = struct_depth.saturating_sub(1);
            }

            if struct_depth == 0 && trimmed.ends_with(';') {
                if let Some((name, type_expr)) = parse_type_declaration_text(&declaration_text) {
                    declarations.push(DataTypeDecl {
                        name,
                        type_expr,
                        source: source.path.display().to_string(),
                        line: declaration_start_line,
                    });
                } else {
                    warnings.push(format!(
                        "{}:{} unsupported TYPE declaration skipped during PLCopen export",
                        source.path.display(),
                        declaration_start_line
                    ));
                }
                declaration_text.clear();
            }

            line_index += 1;
        }

        line_index += 1;
    }

    (declarations, warnings)
}

fn parse_type_declaration_text(text: &str) -> Option<(String, String)> {
    let trimmed = text.trim();
    let colon = trimmed.find(':')?;
    let name = trimmed[..colon].trim().to_string();
    if name.is_empty() {
        return None;
    }
    let mut expr = trimmed[colon + 1..].trim().to_string();
    if expr.ends_with(';') {
        expr.pop();
    }
    let expr = expr.trim().to_string();
    if expr.is_empty() {
        None
    } else {
        Some((name, expr))
    }
}
