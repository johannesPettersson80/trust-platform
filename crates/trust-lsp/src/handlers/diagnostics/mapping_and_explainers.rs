fn is_config_uri(uri: &Url) -> bool {
    let Some(path) = uri_to_path(uri) else {
        return false;
    };
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| CONFIG_FILES.iter().any(|candidate| candidate == &name))
        .unwrap_or(false)
}

fn is_hmi_toml_uri(uri: &Url) -> bool {
    let Some(path) = uri_to_path(uri) else {
        return false;
    };
    if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
        return false;
    }
    path.components()
        .any(|component| component.as_os_str() == "hmi")
}

fn collect_hmi_toml_diagnostics(
    _state: &ServerState,
    _uri: &Url,
    content: &str,
) -> Vec<Diagnostic> {
    let mut diagnostics = collect_hmi_toml_parse_diagnostics(content);
    if !diagnostics.is_empty() {
        return diagnostics;
    }

    diagnostics.extend(collect_hmi_toml_local_diagnostics(content));
    diagnostics
}

fn collect_hmi_toml_parse_diagnostics(content: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    if let Err(error) = toml::from_str::<toml::Value>(content) {
        let range = if let Some(span) = error.span() {
            Range {
                start: offset_to_position(content, span.start as u32),
                end: offset_to_position(content, span.end as u32),
            }
        } else {
            fallback_range(content)
        };
        diagnostics.push(Diagnostic {
            range,
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String("HMI_TOML_PARSE".to_string())),
            source: Some("truST".to_string()),
            message: error.to_string(),
            ..Default::default()
        });
    }
    diagnostics
}

fn collect_hmi_toml_local_diagnostics(content: &str) -> Vec<Diagnostic> {
    let Ok(value) = toml::from_str::<toml::Value>(content) else {
        return Vec::new();
    };
    let Some(sections) = value.get("section").and_then(toml::Value::as_array) else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for section in sections {
        let Some(widgets) = section.get("widget").and_then(toml::Value::as_array) else {
            continue;
        };
        for widget in widgets {
            let Some(kind) = hmi_toml_widget_string(widget, "type")
                .map(|kind| kind.trim().to_ascii_lowercase())
                .filter(|kind| !kind.is_empty())
            else {
                continue;
            };
            let bind = hmi_toml_widget_string(widget, "bind")
                .map(str::trim)
                .unwrap_or_default();
            if !hmi_toml_supported_widget_kind(kind.as_str()) {
                diagnostics.push(Diagnostic {
                    range: find_name_range(content, bind),
                    severity: Some(DiagnosticSeverity::WARNING),
                    code: Some(NumberOrString::String(
                        "HMI_UNKNOWN_WIDGET_KIND".to_string(),
                    )),
                    source: Some("truST".to_string()),
                    message: format!("unknown widget kind '{kind}'"),
                    ..Default::default()
                });
                continue;
            }
            if let (Some(min), Some(max)) = (
                hmi_toml_widget_number(widget, "min"),
                hmi_toml_widget_number(widget, "max"),
            ) {
                if min > max {
                    diagnostics.push(Diagnostic {
                        range: find_name_range(content, bind),
                        severity: Some(DiagnosticSeverity::WARNING),
                        code: Some(NumberOrString::String(
                            HMI_DIAG_INVALID_PROPERTIES.to_string(),
                        )),
                        source: Some("truST".to_string()),
                        message: format!(
                            "invalid widget property combination: min ({min}) is greater than max ({max})"
                        ),
                        ..Default::default()
                    });
                }
            }
            if kind != "indicator"
                && (hmi_toml_widget_has_key(widget, "on_color")
                    || hmi_toml_widget_has_key(widget, "off_color"))
            {
                diagnostics.push(Diagnostic {
                    range: find_name_range(content, bind),
                    severity: Some(DiagnosticSeverity::WARNING),
                    code: Some(NumberOrString::String(
                        HMI_DIAG_INVALID_PROPERTIES.to_string(),
                    )),
                    source: Some("truST".to_string()),
                    message: format!(
                        "invalid widget property combination: on_color/off_color only apply to indicator widgets (found '{kind}')"
                    ),
                    ..Default::default()
                });
            }
            if kind == "indicator"
                && (hmi_toml_widget_has_key(widget, "min")
                    || hmi_toml_widget_has_key(widget, "max"))
            {
                diagnostics.push(Diagnostic {
                    range: find_name_range(content, bind),
                    severity: Some(DiagnosticSeverity::WARNING),
                    code: Some(NumberOrString::String(
                        HMI_DIAG_INVALID_PROPERTIES.to_string(),
                    )),
                    source: Some("truST".to_string()),
                    message:
                        "invalid widget property combination: indicator widgets do not support min/max"
                            .to_string(),
                    ..Default::default()
                });
            }
        }
    }

    diagnostics.sort_by(|left, right| {
        let left_code = diagnostic_code(left).unwrap_or_default();
        let right_code = diagnostic_code(right).unwrap_or_default();
        left_code
            .cmp(&right_code)
            .then_with(|| left.message.cmp(&right.message))
    });
    diagnostics
}

fn hmi_toml_widget_string<'a>(widget: &'a toml::Value, key: &str) -> Option<&'a str> {
    widget.get(key).and_then(toml::Value::as_str)
}

fn hmi_toml_widget_number(widget: &toml::Value, key: &str) -> Option<f64> {
    match widget.get(key) {
        Some(toml::Value::Integer(value)) => Some(*value as f64),
        Some(toml::Value::Float(value)) => Some(*value),
        _ => None,
    }
}

fn hmi_toml_widget_has_key(widget: &toml::Value, key: &str) -> bool {
    widget
        .as_table()
        .is_some_and(|table| table.contains_key(key))
}

fn hmi_toml_supported_widget_kind(kind: &str) -> bool {
    matches!(
        kind,
        "gauge"
            | "sparkline"
            | "bar"
            | "tank"
            | "value"
            | "slider"
            | "indicator"
            | "toggle"
            | "selector"
            | "readout"
            | "text"
            | "table"
            | "tree"
    )
}

#[cfg(test)]
fn collect_hmi_toml_semantic_diagnostics(
    root: &Path,
    current_file: &Path,
    content: &str,
) -> Vec<Diagnostic> {
    let Some(descriptor) = runtime_hmi::load_hmi_dir(root) else {
        return Vec::new();
    };
    let loaded_sources = match load_hmi_sources_for_diagnostics(root) {
        Ok(sources) => sources,
        Err(_error) => return Vec::new(),
    };
    let compile_sources = loaded_sources
        .iter()
        .map(|source| {
            HarnessSourceFile::with_path(
                source.path.to_string_lossy().as_ref(),
                source.text.clone(),
            )
        })
        .collect::<Vec<_>>();
    let runtime = match CompileSession::from_sources(compile_sources).build_runtime() {
        Ok(runtime) => runtime,
        Err(_error) => return Vec::new(),
    };
    let metadata = runtime.metadata_snapshot();
    let snapshot = DebugSnapshot {
        storage: runtime.storage().clone(),
        now: runtime.current_time(),
    };
    let source_refs = loaded_sources
        .iter()
        .map(|source| HmiSourceRef {
            path: source.path.as_path(),
            text: source.text.as_str(),
        })
        .collect::<Vec<_>>();
    let catalog =
        runtime_hmi::collect_hmi_bindings_catalog(&metadata, Some(&snapshot), &source_refs);
    let known_paths = catalog
        .programs
        .iter()
        .flat_map(|program| program.variables.iter().map(|entry| entry.path.clone()))
        .chain(catalog.globals.iter().map(|entry| entry.path.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let file_name = current_file
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let current_page_id = if file_name == "_config.toml" {
        None
    } else {
        current_file
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(ToString::to_string)
    };

    let mut diagnostics = Vec::new();
    let binding_diagnostics =
        runtime_hmi::validate_hmi_bindings("RESOURCE", &metadata, Some(&snapshot), &descriptor);
    for binding in binding_diagnostics {
        if let Some(page_id) = current_page_id.as_ref() {
            if binding.page != *page_id {
                continue;
            }
        } else {
            continue;
        }
        let mut message = binding.message.clone();
        if binding.code == HMI_DIAG_UNKNOWN_BIND {
            let suggestions = top_ranked_suggestions(binding.bind.as_str(), &known_paths);
            if !suggestions.is_empty() {
                message = format!(
                    "{message}. Did you mean {}?",
                    format_suggestion_list(&suggestions)
                );
            }
        }
        diagnostics.push(Diagnostic {
            range: find_name_range(content, binding.bind.as_str()),
            severity: Some(DiagnosticSeverity::WARNING),
            code: Some(NumberOrString::String(binding.code.to_string())),
            source: Some("truST".to_string()),
            message,
            ..Default::default()
        });
    }

    if let Some(page_id) = current_page_id {
        if let Some(page) = descriptor.pages.iter().find(|page| page.id == page_id) {
            for section in &page.sections {
                for widget in &section.widgets {
                    let Some(kind) = widget.widget_type.as_ref() else {
                        continue;
                    };
                    let kind = kind.trim().to_ascii_lowercase();
                    if kind.is_empty() {
                        continue;
                    }
                    let bind = widget.bind.trim();
                    if let (Some(min), Some(max)) = (widget.min, widget.max) {
                        if min > max {
                            diagnostics.push(Diagnostic {
                                range: find_name_range(content, bind),
                                severity: Some(DiagnosticSeverity::WARNING),
                                code: Some(NumberOrString::String(
                                    HMI_DIAG_INVALID_PROPERTIES.to_string(),
                                )),
                                source: Some("truST".to_string()),
                                message: format!(
                                    "invalid widget property combination: min ({min}) is greater than max ({max})"
                                ),
                                ..Default::default()
                            });
                        }
                    }
                    if kind != "indicator"
                        && (widget.on_color.is_some() || widget.off_color.is_some())
                    {
                        diagnostics.push(Diagnostic {
                            range: find_name_range(content, bind),
                            severity: Some(DiagnosticSeverity::WARNING),
                            code: Some(NumberOrString::String(
                                HMI_DIAG_INVALID_PROPERTIES.to_string(),
                            )),
                            source: Some("truST".to_string()),
                            message: format!(
                                "invalid widget property combination: on_color/off_color only apply to indicator widgets (found '{kind}')"
                            ),
                            ..Default::default()
                        });
                    }
                    if kind == "indicator" && (widget.min.is_some() || widget.max.is_some()) {
                        diagnostics.push(Diagnostic {
                            range: find_name_range(content, bind),
                            severity: Some(DiagnosticSeverity::WARNING),
                            code: Some(NumberOrString::String(
                                HMI_DIAG_INVALID_PROPERTIES.to_string(),
                            )),
                            source: Some("truST".to_string()),
                            message:
                                "invalid widget property combination: indicator widgets do not support min/max"
                                    .to_string(),
                            ..Default::default()
                        });
                    }
                }
            }
        }
    }

    diagnostics.sort_by(|left, right| {
        let left_code = diagnostic_code(left).unwrap_or_default();
        let right_code = diagnostic_code(right).unwrap_or_default();
        left_code
            .cmp(&right_code)
            .then_with(|| left.message.cmp(&right.message))
    });
    diagnostics
}

#[derive(Debug, Clone)]
#[cfg(test)]
struct LoadedHmiSource {
    path: PathBuf,
    text: String,
}

#[cfg(test)]
fn load_hmi_sources_for_diagnostics(root: &Path) -> anyhow::Result<Vec<LoadedHmiSource>> {
    let sources_root = resolve_sources_root(root, None)?;
    let mut source_paths = BTreeSet::new();
    for pattern in ["**/*.st", "**/*.ST", "**/*.pou", "**/*.POU"] {
        let glob_pattern = format!("{}/{}", sources_root.display(), pattern);
        let entries = glob::glob(&glob_pattern)?;
        for entry in entries {
            source_paths.insert(entry?);
        }
    }
    if source_paths.is_empty() {
        anyhow::bail!("no ST sources found under {}", sources_root.display());
    }

    let mut sources = Vec::with_capacity(source_paths.len());
    for path in source_paths {
        let text = std::fs::read_to_string(&path)?;
        sources.push(LoadedHmiSource { path, text });
    }
    Ok(sources)
}
