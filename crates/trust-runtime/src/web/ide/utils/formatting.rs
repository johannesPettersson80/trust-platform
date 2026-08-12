pub(in crate::web::ide) fn format_structured_text_document(source: &str) -> String {
    let ends_with_newline = source.ends_with('\n');
    let mut indent_level = 0_usize;
    let mut out_lines = Vec::new();

    for raw_line in source.lines() {
        let line_no_trailing = raw_line.trim_end_matches([' ', '\t']);
        let trimmed = line_no_trailing.trim_start();
        if trimmed.is_empty() {
            out_lines.push(String::new());
            continue;
        }
        if trimmed.starts_with("//") || trimmed.starts_with("(*") {
            out_lines.push(format!("{}{}", "  ".repeat(indent_level), trimmed));
            continue;
        }

        let upper = trimmed.to_ascii_uppercase();
        let dedent_before = is_dedent_line(upper.as_str());
        if dedent_before && indent_level > 0 {
            indent_level = indent_level.saturating_sub(1);
        }

        out_lines.push(format!("{}{}", "  ".repeat(indent_level), trimmed));

        if is_indent_line(upper.as_str()) {
            indent_level = indent_level.saturating_add(1);
        }
    }

    let mut formatted = out_lines.join("\n");
    if !formatted.is_empty() && (ends_with_newline || !source.is_empty()) {
        formatted.push('\n');
    }
    formatted
}

pub(in crate::web::ide) fn is_dedent_line(upper_trimmed: &str) -> bool {
    if upper_trimmed.starts_with("END_") {
        return true;
    }
    upper_trimmed == "ELSE"
        || upper_trimmed.starts_with("ELSE ")
        || upper_trimmed.starts_with("ELSIF ")
        || upper_trimmed.starts_with("UNTIL ")
}

pub(in crate::web::ide) fn is_indent_line(upper_trimmed: &str) -> bool {
    if upper_trimmed.starts_with("PROGRAM ")
        || upper_trimmed.starts_with("FUNCTION ")
        || upper_trimmed.starts_with("FUNCTION_BLOCK ")
        || upper_trimmed.starts_with("CONFIGURATION ")
        || upper_trimmed.starts_with("RESOURCE ")
        || upper_trimmed.starts_with("CLASS ")
        || upper_trimmed.starts_with("INTERFACE ")
        || upper_trimmed.starts_with("METHOD ")
        || upper_trimmed.starts_with("PROPERTY ")
        || upper_trimmed.starts_with("ACTION ")
        || upper_trimmed.starts_with("TRANSITION ")
        || upper_trimmed == "ELSE"
        || upper_trimmed.starts_with("ELSE ")
        || upper_trimmed.starts_with("ELSIF ")
        || upper_trimmed.starts_with("REPEAT")
    {
        return true;
    }
    if upper_trimmed == "VAR"
        || upper_trimmed.starts_with("VAR ")
        || upper_trimmed == "VAR_INPUT"
        || upper_trimmed.starts_with("VAR_INPUT ")
        || upper_trimmed == "VAR_OUTPUT"
        || upper_trimmed.starts_with("VAR_OUTPUT ")
        || upper_trimmed == "VAR_IN_OUT"
        || upper_trimmed.starts_with("VAR_IN_OUT ")
        || upper_trimmed == "VAR_TEMP"
        || upper_trimmed.starts_with("VAR_TEMP ")
        || upper_trimmed == "VAR_GLOBAL"
        || upper_trimmed.starts_with("VAR_GLOBAL ")
        || upper_trimmed == "VAR_EXTERNAL"
        || upper_trimmed.starts_with("VAR_EXTERNAL ")
        || upper_trimmed == "VAR_CONFIG"
        || upper_trimmed.starts_with("VAR_CONFIG ")
        || upper_trimmed == "VAR_ACCESS"
        || upper_trimmed.starts_with("VAR_ACCESS ")
    {
        return true;
    }
    (upper_trimmed.starts_with("IF ") && upper_trimmed.contains(" THEN"))
        || (upper_trimmed.starts_with("CASE ") && upper_trimmed.contains(" OF"))
        || (upper_trimmed.starts_with("FOR ") && upper_trimmed.contains(" DO"))
        || (upper_trimmed.starts_with("WHILE ") && upper_trimmed.contains(" DO"))
}

#[cfg(test)]
#[path = "formatting/contract_tests.rs"]
mod contract_tests;
