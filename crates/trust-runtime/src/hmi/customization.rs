pub fn load_customization(
    project_root: Option<&Path>,
    sources: &[HmiSourceRef<'_>],
) -> HmiCustomization {
    let mut customization = HmiCustomization {
        annotation_overrides: parse_annotations(sources),
        ..HmiCustomization::default()
    };

    if let Some(root) = project_root {
        if let Some(dir_descriptor) = load_hmi_dir(root) {
            apply_hmi_dir_descriptor(&mut customization, &dir_descriptor);
            customization.dir_descriptor = Some(dir_descriptor);
        } else if let Ok(parsed) = load_hmi_toml(root) {
            apply_legacy_hmi_toml(&mut customization, parsed);
        }
    }

    customization
}

pub fn try_load_customization(
    project_root: Option<&Path>,
    sources: &[HmiSourceRef<'_>],
) -> anyhow::Result<HmiCustomization> {
    let mut customization = HmiCustomization {
        annotation_overrides: parse_annotations(sources),
        ..HmiCustomization::default()
    };

    let Some(root) = project_root else {
        return Ok(customization);
    };

    if root.join("hmi").is_dir() {
        let dir_descriptor = load_hmi_dir_impl(root)?;
        apply_hmi_dir_descriptor(&mut customization, &dir_descriptor);
        customization.dir_descriptor = Some(dir_descriptor);
        return Ok(customization);
    }

    if root.join("hmi.toml").is_file() {
        let parsed = load_hmi_toml(root)?;
        apply_legacy_hmi_toml(&mut customization, parsed);
    }

    Ok(customization)
}

fn apply_legacy_hmi_toml(customization: &mut HmiCustomization, parsed: HmiTomlFile) {
    customization.theme.style = parsed.theme.style;
    customization.theme.accent = parsed.theme.accent;
    customization.responsive.mode = parsed.responsive.mode;
    customization.export.enabled = parsed.export.enabled;
    customization.write.enabled = parsed.write.enabled;
    customization.write.allow = parsed
        .write
        .allow
        .into_iter()
        .map(|entry| entry.trim().to_string())
        .filter(|entry| !entry.is_empty())
        .collect();
    customization.pages = parsed
        .pages
        .into_iter()
        .enumerate()
        .filter_map(|(idx, page)| {
            let id = page.id.trim();
            if id.is_empty() {
                return None;
            }
            let order = page.order.unwrap_or((idx as i32) * 10);
            let title = page
                .title
                .filter(|title| !title.trim().is_empty())
                .unwrap_or_else(|| title_case(id));
            let kind = normalize_page_kind(page.kind.as_deref()).to_string();
            let signals = page
                .signals
                .unwrap_or_default()
                .into_iter()
                .map(|entry| entry.trim().to_string())
                .filter(|entry| !entry.is_empty())
                .collect::<Vec<_>>();
            Some(HmiPageConfig {
                id: id.to_string(),
                title,
                icon: None,
                order,
                kind,
                duration_ms: page.duration_s.map(|seconds| seconds.saturating_mul(1_000)),
                svg: None,
                view: None,
                scene_view: None,
                hidden: false,
                signals,
                sections: Vec::new(),
                bindings: Vec::new(),
                bindings3d: Vec::new(),
            })
        })
        .collect();
    customization.widget_overrides = parsed
        .widgets
        .into_iter()
        .filter_map(|(path, override_spec)| {
            let key = path.trim();
            if key.is_empty() {
                return None;
            }
            Some((key.to_string(), HmiWidgetOverride::from(override_spec)))
        })
        .collect();
}

pub fn validate_hmi_bindings(
    resource_name: &str,
    metadata: &RuntimeMetadata,
    snapshot: Option<&DebugSnapshot>,
    descriptor: &HmiDirDescriptor,
) -> Vec<HmiBindingDiagnostic> {
    let points = collect_points(resource_name, metadata, snapshot, true);
    let by_path = points
        .iter()
        .map(|point| (point.path.as_str(), point))
        .collect::<HashMap<_, _>>();
    let mut diagnostics = Vec::new();

    for page in &descriptor.pages {
        for section in &page.sections {
            for widget in &section.widgets {
                let bind = widget.bind.trim();
                if bind.is_empty() {
                    continue;
                }
                let widget_kind = widget
                    .widget_type
                    .as_ref()
                    .map(|kind| kind.trim().to_ascii_lowercase())
                    .filter(|kind| !kind.is_empty());
                let Some(point) = by_path.get(bind) else {
                    diagnostics.push(HmiBindingDiagnostic {
                        code: HMI_DIAG_UNKNOWN_BIND,
                        message: format!("unknown binding path '{bind}'"),
                        bind: bind.to_string(),
                        widget: widget_kind.clone(),
                        page: page.id.clone(),
                        section: Some(section.title.clone()),
                    });
                    continue;
                };
                let Some(widget_kind) = widget_kind else {
                    continue;
                };
                if !is_supported_widget_kind(widget_kind.as_str()) {
                    diagnostics.push(HmiBindingDiagnostic {
                        code: HMI_DIAG_UNKNOWN_WIDGET,
                        message: format!("unknown widget kind '{widget_kind}'"),
                        bind: bind.to_string(),
                        widget: Some(widget_kind),
                        page: page.id.clone(),
                        section: Some(section.title.clone()),
                    });
                    continue;
                }
                if !widget_kind_matches_point(widget_kind.as_str(), point) {
                    diagnostics.push(HmiBindingDiagnostic {
                        code: HMI_DIAG_TYPE_MISMATCH,
                        message: format!(
                            "widget '{widget_kind}' is incompatible with '{}' ({})",
                            point.path, point.data_type
                        ),
                        bind: bind.to_string(),
                        widget: Some(widget_kind),
                        page: page.id.clone(),
                        section: Some(section.title.clone()),
                    });
                }
            }
        }
        for binding in &page.bindings {
            let bind = binding.source.trim();
            if bind.is_empty() {
                continue;
            }
            if !by_path.contains_key(bind) {
                diagnostics.push(HmiBindingDiagnostic {
                    code: HMI_DIAG_UNKNOWN_BIND,
                    message: format!("unknown binding path '{bind}'"),
                    bind: bind.to_string(),
                    widget: Some("process.bind".to_string()),
                    page: page.id.clone(),
                    section: None,
                });
            }
        }
        for binding in &page.bindings3d {
            let bind = binding.source.trim();
            if bind.is_empty() {
                continue;
            }
            if !by_path.contains_key(bind) {
                diagnostics.push(HmiBindingDiagnostic {
                    code: HMI_DIAG_UNKNOWN_BIND,
                    message: format!("unknown binding path '{bind}'"),
                    bind: bind.to_string(),
                    widget: Some("scene3d.bind3d".to_string()),
                    page: page.id.clone(),
                    section: None,
                });
            }
        }
    }

    diagnostics.sort_by(|left, right| {
        left.code
            .cmp(right.code)
            .then_with(|| left.page.cmp(&right.page))
            .then_with(|| left.bind.cmp(&right.bind))
            .then_with(|| left.section.cmp(&right.section))
    });
    diagnostics
}
