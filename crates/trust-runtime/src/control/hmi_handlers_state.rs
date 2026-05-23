fn hmi_descriptor_snapshot(state: &ControlState) -> HmiRuntimeDescriptor {
    state
        .hmi_descriptor
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_else(|_| {
            HmiRuntimeDescriptor::from_sources(state.project_root.as_deref(), &state.sources)
        })
}

pub(super) fn reload_hmi_descriptor_state(state: &ControlState) -> Result<u64, String> {
    let customization = match load_hmi_customization_strict_from_sources(
        state.project_root.as_deref(),
        &state.sources,
    ) {
        Ok(customization) => customization,
        Err(err) => {
            if let Ok(mut descriptor) = state.hmi_descriptor.lock() {
                descriptor.last_error = Some(err.clone());
            }
            return Err(err);
        }
    };
    let mut descriptor = state
        .hmi_descriptor
        .lock()
        .map_err(|_| "hmi descriptor state unavailable".to_string())?;
    descriptor.customization = customization;
    descriptor.schema_revision = descriptor.schema_revision.saturating_add(1);
    descriptor.last_error = None;
    Ok(descriptor.schema_revision)
}

pub(super) fn load_hmi_customization_from_sources(
    project_root: Option<&Path>,
    sources: &SourceRegistry,
) -> crate::hmi::HmiCustomization {
    let source_refs = sources
        .files()
        .iter()
        .map(|file| crate::hmi::HmiSourceRef {
            path: &file.path,
            text: file.text.as_str(),
        })
        .collect::<Vec<_>>();
    crate::hmi::load_customization(project_root, &source_refs)
}

fn load_hmi_customization_strict_from_sources(
    project_root: Option<&Path>,
    sources: &SourceRegistry,
) -> Result<crate::hmi::HmiCustomization, String> {
    let source_refs = sources
        .files()
        .iter()
        .map(|file| crate::hmi::HmiSourceRef {
            path: &file.path,
            text: file.text.as_str(),
        })
        .collect::<Vec<_>>();
    crate::hmi::try_load_customization(project_root, &source_refs).map_err(|err| err.to_string())
}

pub(super) fn hmi_event_matches_descriptor(event: &Event, project_root: &Path) -> bool {
    if !matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) {
        return false;
    }
    let hmi_dir = project_root.join("hmi");
    let canonical_hmi_dir = std::fs::canonicalize(&hmi_dir).ok();
    event.paths.iter().any(|path| {
        let is_toml = path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"));
        if !is_toml {
            return false;
        }
        if path.starts_with(&hmi_dir) {
            return true;
        }
        let Some(canonical_hmi_dir) = canonical_hmi_dir.as_ref() else {
            return false;
        };
        if path.starts_with(canonical_hmi_dir) {
            return true;
        }
        if let Ok(canonical_path) = std::fs::canonicalize(path) {
            if canonical_path.starts_with(canonical_hmi_dir) {
                return true;
            }
        }
        path.parent()
            .and_then(|parent| std::fs::canonicalize(parent).ok())
            .is_some_and(|canonical_parent| canonical_parent.starts_with(canonical_hmi_dir))
    })
}

fn load_runtime_snapshot(state: &ControlState) -> Option<crate::debug::DebugSnapshot> {
    let (tx, rx) = std::sync::mpsc::channel();
    let request = ResourceCommand::Snapshot { respond_to: tx };
    if state.resource.send_command(request).is_ok() {
        if let Ok(snapshot) = rx.recv_timeout(std::time::Duration::from_millis(250)) {
            return Some(snapshot);
        }
    }
    state.debug.snapshot()
}

fn descriptor_from_schema(schema: &crate::hmi::HmiSchemaResult) -> crate::hmi::HmiDirDescriptor {
    let mut pages = Vec::new();
    let widgets_by_page = schema.widgets.iter().fold(
        BTreeMap::<String, Vec<&crate::hmi::HmiWidgetSchema>>::new(),
        |mut acc, widget| {
            acc.entry(widget.page.clone()).or_default().push(widget);
            acc
        },
    );
    let widget_by_id = schema
        .widgets
        .iter()
        .map(|widget| (widget.id.as_str(), widget))
        .collect::<BTreeMap<_, _>>();

    for page in &schema.pages {
        let page_widgets = widgets_by_page
            .get(page.id.as_str())
            .cloned()
            .unwrap_or_default();
        let mut sections = Vec::new();
        if !page.sections.is_empty() {
            for section in &page.sections {
                let widgets = section
                    .widget_ids
                    .iter()
                    .filter_map(|id| widget_by_id.get(id.as_str()).copied())
                    .map(|widget| crate::hmi::HmiDirWidget {
                        widget_type: Some(widget.widget.clone()),
                        bind: widget.path.clone(),
                        label: Some(widget.label.clone()),
                        unit: widget.unit.clone(),
                        min: widget.min,
                        max: widget.max,
                        span: widget.widget_span,
                        on_color: widget.on_color.clone(),
                        off_color: widget.off_color.clone(),
                        inferred_interface: widget.inferred_interface.then_some(true),
                        detail_page: widget.detail_page.clone(),
                        zones: widget.zones.clone(),
                    })
                    .collect::<Vec<_>>();
                if widgets.is_empty() {
                    continue;
                }
                sections.push(crate::hmi::HmiDirSection {
                    title: section.title.clone(),
                    span: section.span.clamp(1, 12),
                    tier: section.tier.clone(),
                    widgets,
                });
            }
        }
        if sections.is_empty() {
            let mut grouped = BTreeMap::<String, Vec<&crate::hmi::HmiWidgetSchema>>::new();
            for widget in &page_widgets {
                grouped
                    .entry(widget.group.clone())
                    .or_default()
                    .push(*widget);
            }
            for (group, widgets) in grouped {
                let mapped = widgets
                    .into_iter()
                    .map(|widget| crate::hmi::HmiDirWidget {
                        widget_type: Some(widget.widget.clone()),
                        bind: widget.path.clone(),
                        label: Some(widget.label.clone()),
                        unit: widget.unit.clone(),
                        min: widget.min,
                        max: widget.max,
                        span: widget.widget_span,
                        on_color: widget.on_color.clone(),
                        off_color: widget.off_color.clone(),
                        inferred_interface: widget.inferred_interface.then_some(true),
                        detail_page: widget.detail_page.clone(),
                        zones: widget.zones.clone(),
                    })
                    .collect::<Vec<_>>();
                if mapped.is_empty() {
                    continue;
                }
                sections.push(crate::hmi::HmiDirSection {
                    title: if group.trim().is_empty() {
                        "General".to_string()
                    } else {
                        group
                    },
                    span: 12,
                    tier: None,
                    widgets: mapped,
                });
            }
        }

        pages.push(crate::hmi::HmiDirPage {
            id: page.id.clone(),
            title: page.title.clone(),
            icon: page.icon.clone(),
            order: page.order,
            kind: page.kind.clone(),
            duration_ms: page.duration_ms,
            svg: page.svg.clone(),
            view: page.view.clone(),
            scene_view: page.scene_view.clone(),
            hidden: page.hidden,
            signals: page.signals.clone(),
            sections,
            bindings: page
                .bindings
                .iter()
                .map(|binding| crate::hmi::HmiDirProcessBinding {
                    selector: binding.selector.clone(),
                    attribute: binding.attribute.clone(),
                    source: binding.source.clone(),
                    format: binding.format.clone(),
                    map: binding.map.clone(),
                    scale: binding.scale.clone(),
                })
                .collect(),
            bindings3d: page.bindings3d.clone(),
        });
    }

    crate::hmi::HmiDirDescriptor {
        config: crate::hmi::HmiDirConfig {
            version: Some(1),
            theme: crate::hmi::HmiDirTheme {
                style: Some(schema.theme.style.clone()),
                accent: Some(schema.theme.accent.clone()),
            },
            layout: crate::hmi::HmiDirLayout::default(),
            write: crate::hmi::HmiDirWrite::default(),
            alarms: Vec::new(),
        },
        pages,
    }
}
