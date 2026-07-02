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

fn collect_hmi_toml_diagnostics(state: &ServerState, uri: &Url, content: &str) -> Vec<Diagnostic> {
    let mut diagnostics = collect_hmi_toml_parse_diagnostics(content);
    if !diagnostics.is_empty() {
        return diagnostics;
    }

    let Some(path) = uri_to_path(uri) else {
        return diagnostics;
    };

    let root = state
        .workspace_config_for_uri(uri)
        .map(|config| config.root)
        .or_else(|| infer_hmi_root_from_path(path.as_path()));
    let Some(root) = root else {
        return diagnostics;
    };

    diagnostics.extend(collect_hmi_toml_semantic_diagnostics(
        root.as_path(),
        path.as_path(),
        content,
    ));
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

fn infer_hmi_root_from_path(path: &Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    if parent.file_name().and_then(|name| name.to_str()) != Some("hmi") {
        return None;
    }
    parent.parent().map(Path::to_path_buf)
}

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
struct LoadedHmiSource {
    path: PathBuf,
    text: String,
}

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

