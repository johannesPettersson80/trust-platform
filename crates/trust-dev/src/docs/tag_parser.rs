fn parse_doc_tags(
    comment: &CommentBlock,
    file: &Path,
    kind: ApiItemKind,
    symbol_name: &str,
    declared_params: &[SmolStr],
    has_return: bool,
) -> (ApiDocTags, Vec<DocDiagnostic>) {
    let mut tags = ApiDocTags::default();
    let mut diagnostics = Vec::new();
    let mut current: Option<CurrentTag> = None;

    for line in &comment.lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(tag_line) = trimmed.strip_prefix('@') {
            let mut parts = tag_line.splitn(2, char::is_whitespace);
            let tag = parts.next().unwrap_or_default().to_ascii_lowercase();
            let remainder = parts.next().map_or("", str::trim_start);
            match tag.as_str() {
                "brief" => {
                    if tags.brief.is_some() {
                        diagnostics.push(DocDiagnostic {
                            file: file.to_path_buf(),
                            line: comment.start_line,
                            message: format!(
                                "duplicate @brief tag for {} `{}`",
                                kind.label(),
                                symbol_name
                            ),
                        });
                    }
                    if remainder.is_empty() {
                        diagnostics.push(DocDiagnostic {
                            file: file.to_path_buf(),
                            line: comment.start_line,
                            message: format!(
                                "missing description for @brief on {} `{}`",
                                kind.label(),
                                symbol_name
                            ),
                        });
                    }
                    tags.brief = if remainder.is_empty() {
                        None
                    } else {
                        Some(remainder.to_string())
                    };
                    current = Some(CurrentTag::Brief);
                }
                "param" => {
                    let mut param_parts = remainder.splitn(2, char::is_whitespace);
                    let Some(name) = param_parts.next().filter(|text| !text.trim().is_empty())
                    else {
                        diagnostics.push(DocDiagnostic {
                            file: file.to_path_buf(),
                            line: comment.start_line,
                            message: format!(
                                "malformed @param tag on {} `{}` (expected: @param <name> <description>)",
                                kind.label(),
                                symbol_name
                            ),
                        });
                        current = None;
                        continue;
                    };
                    let description = param_parts.next().map_or("", str::trim_start);
                    if description.is_empty() {
                        diagnostics.push(DocDiagnostic {
                            file: file.to_path_buf(),
                            line: comment.start_line,
                            message: format!(
                                "missing description for @param `{}` on {} `{}`",
                                name,
                                kind.label(),
                                symbol_name
                            ),
                        });
                    }
                    tags.params.push(ApiParamDoc {
                        name: SmolStr::new(name),
                        description: description.to_string(),
                    });
                    current = Some(CurrentTag::Param(tags.params.len() - 1));
                }
                "return" => {
                    if tags.returns.is_some() {
                        diagnostics.push(DocDiagnostic {
                            file: file.to_path_buf(),
                            line: comment.start_line,
                            message: format!(
                                "duplicate @return tag for {} `{}`",
                                kind.label(),
                                symbol_name
                            ),
                        });
                    }
                    if remainder.is_empty() {
                        diagnostics.push(DocDiagnostic {
                            file: file.to_path_buf(),
                            line: comment.start_line,
                            message: format!(
                                "missing description for @return on {} `{}`",
                                kind.label(),
                                symbol_name
                            ),
                        });
                    }
                    tags.returns = if remainder.is_empty() {
                        None
                    } else {
                        Some(remainder.to_string())
                    };
                    current = Some(CurrentTag::Return);
                }
                other => {
                    diagnostics.push(DocDiagnostic {
                        file: file.to_path_buf(),
                        line: comment.start_line,
                        message: format!(
                            "unknown documentation tag `@{}` on {} `{}`",
                            other,
                            kind.label(),
                            symbol_name
                        ),
                    });
                    current = None;
                }
            }
            continue;
        }

        match current {
            Some(CurrentTag::Brief) => append_with_space(&mut tags.brief, trimmed),
            Some(CurrentTag::Param(index)) => {
                if let Some(param) = tags.params.get_mut(index) {
                    append_string_with_space(&mut param.description, trimmed);
                }
            }
            Some(CurrentTag::Return) => append_with_space(&mut tags.returns, trimmed),
            Some(CurrentTag::Detail) | None => {
                tags.details.push(trimmed.to_string());
                current = Some(CurrentTag::Detail);
            }
        }
    }

    let mut seen_params = HashSet::new();
    let declared: HashMap<String, &SmolStr> = declared_params
        .iter()
        .map(|name| (name.as_str().to_ascii_uppercase(), name))
        .collect();

    for param in &tags.params {
        let normalized = param.name.as_str().to_ascii_uppercase();
        if !seen_params.insert(normalized.clone()) {
            diagnostics.push(DocDiagnostic {
                file: file.to_path_buf(),
                line: comment.start_line,
                message: format!(
                    "duplicate @param entry for `{}` on {} `{}`",
                    param.name,
                    kind.label(),
                    symbol_name
                ),
            });
        }
        if !declared.contains_key(&normalized) {
            diagnostics.push(DocDiagnostic {
                file: file.to_path_buf(),
                line: comment.start_line,
                message: format!(
                    "@param `{}` does not match any declared parameter on {} `{}`",
                    param.name,
                    kind.label(),
                    symbol_name
                ),
            });
        }
    }

    if tags.returns.is_some() && !has_return {
        diagnostics.push(DocDiagnostic {
            file: file.to_path_buf(),
            line: comment.start_line,
            message: format!(
                "@return used on non-returning {} `{}`",
                kind.label(),
                symbol_name
            ),
        });
    }

    (tags, diagnostics)
}

fn append_with_space(target: &mut Option<String>, value: &str) {
    if let Some(existing) = target {
        append_string_with_space(existing, value);
    } else {
        *target = Some(value.to_string());
    }
}

fn append_string_with_space(target: &mut String, value: &str) {
    if target.is_empty() {
        target.push_str(value);
    } else {
        target.push(' ');
        target.push_str(value);
    }
}

