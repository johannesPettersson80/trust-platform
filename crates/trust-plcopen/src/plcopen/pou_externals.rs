fn inject_required_var_external_declarations(
    source: &str,
    externals: &[QualifiedGlobalListExternalDecl],
) -> (String, Vec<String>) {
    if externals.is_empty() {
        return (source.to_string(), Vec::new());
    }

    let mut required = Vec::new();
    for external in externals {
        if source_uses_qualified_global_list(source, &external.list_name)
            && !source_has_var_external_symbol(source, &external.list_name)
        {
            required.push(external.clone());
        }
    }

    if required.is_empty() {
        return (source.to_string(), Vec::new());
    }

    let lines = source.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return (source.to_string(), Vec::new());
    }
    let insert_at = find_var_section_insertion_index(&lines);

    let mut rendered = String::new();
    for (index, line) in lines.iter().enumerate() {
        if index == insert_at {
            rendered.push_str("VAR_EXTERNAL\n");
            for external in &required {
                rendered.push_str(&format!(
                    "    {} : {};\n",
                    external.list_name, external.type_name
                ));
            }
            rendered.push_str("END_VAR\n");
        }
        rendered.push_str(line);
        rendered.push('\n');
    }
    if insert_at >= lines.len() {
        rendered.push_str("VAR_EXTERNAL\n");
        for external in &required {
            rendered.push_str(&format!(
                "    {} : {};\n",
                external.list_name, external.type_name
            ));
        }
        rendered.push_str("END_VAR\n");
    }

    let inserted = required.into_iter().map(|decl| decl.list_name).collect();
    (rendered, inserted)
}

fn source_uses_qualified_global_list(source: &str, list_name: &str) -> bool {
    if list_name.trim().is_empty() {
        return false;
    }
    let lowered_source = source.to_ascii_lowercase();
    let needle = format!("{}.", list_name.trim().to_ascii_lowercase());

    for (index, _) in lowered_source.match_indices(&needle) {
        if index == 0 {
            return true;
        }
        let prev = lowered_source[..index]
            .chars()
            .next_back()
            .unwrap_or_default();
        if !is_identifier_char(prev) {
            return true;
        }
    }

    false
}

fn source_has_var_external_symbol(source: &str, symbol_name: &str) -> bool {
    let target = symbol_name.trim();
    if target.is_empty() {
        return false;
    }

    let mut in_external = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.to_ascii_uppercase().starts_with("VAR_EXTERNAL") {
            in_external = true;
            continue;
        }
        if in_external && trimmed.eq_ignore_ascii_case("END_VAR") {
            in_external = false;
            continue;
        }
        if !in_external {
            continue;
        }

        let Some((lhs, _rhs)) = trimmed.trim_end_matches(';').split_once(':') else {
            continue;
        };
        let name = lhs
            .split(',')
            .next()
            .map(str::trim)
            .unwrap_or_default()
            .split_whitespace()
            .next()
            .unwrap_or_default();
        if name.eq_ignore_ascii_case(target) {
            return true;
        }
    }

    false
}

fn find_var_section_insertion_index(lines: &[&str]) -> usize {
    if lines.is_empty() {
        return 0;
    }

    let mut index = 1usize;
    while index < lines.len() {
        let trimmed = lines[index].trim();
        if trimmed.is_empty() {
            index += 1;
            continue;
        }
        if is_var_section_header(trimmed) {
            index += 1;
            while index < lines.len() && !lines[index].trim().eq_ignore_ascii_case("END_VAR") {
                index += 1;
            }
            if index < lines.len() {
                index += 1;
            }
            continue;
        }
        break;
    }

    index
}

fn is_var_section_header(line: &str) -> bool {
    let upper = line.trim().to_ascii_uppercase();
    upper == "VAR" || upper.starts_with("VAR_")
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn unique_source_path(
    sources_root: &Path,
    base_name: &str,
    seen_files: &mut HashSet<PathBuf>,
) -> PathBuf {
    unique_source_path_with_segments(sources_root, &[], base_name, seen_files)
}

fn unique_source_path_with_segments(
    sources_root: &Path,
    folder_segments: &[String],
    base_name: &str,
    seen_files: &mut HashSet<PathBuf>,
) -> PathBuf {
    let mut file_name = sanitize_filename(base_name);
    if file_name.is_empty() {
        file_name = "unnamed".to_string();
    }
    let mut directory = sources_root.to_path_buf();
    for segment in folder_segments {
        directory = directory.join(sanitize_path_segment(segment, "folder"));
    }

    let mut candidate = directory.join(format!("{file_name}.st"));
    let mut duplicate_index = 2usize;
    while !seen_files.insert(candidate.clone()) {
        candidate = directory.join(format!("{file_name}_{duplicate_index}.st"));
        duplicate_index += 1;
    }
    candidate
}

fn write_text_file_with_parents(path: &Path, text: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create '{}'", parent.display()))?;
    }
    std::fs::write(path, text).with_context(|| format!("failed to write '{}'", path.display()))?;
    Ok(())
}

fn track_imported_folder_path(
    file_path: &Path,
    sources_root: &Path,
    imported_folder_paths: &mut HashSet<String>,
) {
    let Some(parent) = file_path.parent() else {
        return;
    };
    let Ok(relative) = parent.strip_prefix(sources_root) else {
        return;
    };
    if relative.as_os_str().is_empty() {
        return;
    }
    imported_folder_paths.insert(relative.to_string_lossy().replace('\\', "/"));
}
