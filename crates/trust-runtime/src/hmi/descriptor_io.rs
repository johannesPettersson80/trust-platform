pub(super) fn load_hmi_toml(root: &Path) -> anyhow::Result<HmiTomlFile> {
    let path = root.join("hmi.toml");
    if !path.is_file() {
        return Ok(HmiTomlFile::default());
    }
    let text = std::fs::read_to_string(&path)?;
    Ok(toml::from_str::<HmiTomlFile>(&text)?)
}

pub fn load_hmi_dir(root: &Path) -> Option<HmiDirDescriptor> {
    load_hmi_dir_impl(root).ok()
}

pub fn write_hmi_dir_descriptor(
    root: &Path,
    descriptor: &HmiDirDescriptor,
) -> anyhow::Result<Vec<String>> {
    let dir = root.join("hmi");
    std::fs::create_dir_all(&dir).map_err(|err| {
        anyhow::anyhow!(
            "failed to create hmi descriptor directory '{}': {err}",
            dir.display()
        )
    })?;

    let mut written = Vec::new();
    let mut normalized_pages = descriptor
        .pages
        .iter()
        .filter_map(normalize_descriptor_page)
        .collect::<Vec<_>>();
    normalized_pages.sort_by(|left, right| {
        left.order
            .cmp(&right.order)
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut normalized_config = descriptor.config.clone();
    if normalized_config.version.is_none() {
        normalized_config.version = Some(HMI_DESCRIPTOR_VERSION);
    }
    let config_text = render_hmi_dir_config_toml(&normalized_config);
    write_scaffold_file(&dir.join("_config.toml"), config_text.as_str())?;
    written.push("_config.toml".to_string());

    for page in &normalized_pages {
        let page_text = render_hmi_dir_page_toml(page);
        let file_name = format!("{}.toml", page.id);
        write_scaffold_file(&dir.join(&file_name), page_text.as_str())?;
        written.push(file_name);
    }

    let keep = written
        .iter()
        .map(|file| file.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("toml") {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if keep.contains(&name.to_ascii_lowercase()) {
            continue;
        }
        std::fs::remove_file(&path).map_err(|err| {
            anyhow::anyhow!(
                "failed to remove stale hmi descriptor file '{}': {err}",
                path.display()
            )
        })?;
    }

    Ok(written)
}

fn normalize_descriptor_page(page: &HmiDirPage) -> Option<HmiDirPage> {
    let id = page.id.trim();
    if id.is_empty() {
        return None;
    }
    let title = page
        .title
        .trim()
        .strip_prefix('\u{feff}')
        .unwrap_or(page.title.trim());
    let title = if title.is_empty() {
        title_case(id)
    } else {
        title.to_string()
    };

    let mut sections = Vec::new();
    for (section_idx, section) in page.sections.iter().enumerate() {
        let section_title = section
            .title
            .trim()
            .strip_prefix('\u{feff}')
            .unwrap_or(section.title.trim());
        let section_title = if section_title.is_empty() {
            format!("Section {}", section_idx + 1)
        } else {
            section_title.to_string()
        };
        let mut widgets = Vec::new();
        for widget in &section.widgets {
            let bind = widget.bind.trim();
            if bind.is_empty() {
                continue;
            }
            widgets.push(HmiDirWidget {
                widget_type: widget
                    .widget_type
                    .as_ref()
                    .map(|kind| kind.trim().to_ascii_lowercase())
                    .filter(|kind| !kind.is_empty()),
                bind: bind.to_string(),
                label: widget
                    .label
                    .as_ref()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
                unit: widget
                    .unit
                    .as_ref()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
                min: widget.min,
                max: widget.max,
                span: widget.span.map(|span| span.clamp(1, 12)),
                on_color: widget
                    .on_color
                    .as_ref()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
                off_color: widget
                    .off_color
                    .as_ref()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
                inferred_interface: widget.inferred_interface,
                detail_page: widget.detail_page.clone(),
                zones: widget.zones.clone(),
            });
        }
        if widgets.is_empty() {
            continue;
        }
        sections.push(HmiDirSection {
            title: section_title,
            span: section.span.clamp(1, 12),
            tier: section.tier.clone(),
            widgets,
        });
    }

    let mut bindings = Vec::new();
    for binding in &page.bindings {
        let selector = binding.selector.trim();
        let Some(attribute) = normalize_process_attribute(binding.attribute.as_str()) else {
            continue;
        };
        let source = binding.source.trim();
        if !is_safe_process_selector(selector) || source.is_empty() {
            continue;
        }
        bindings.push(HmiDirProcessBinding {
            selector: selector.to_string(),
            attribute,
            source: source.to_string(),
            format: binding
                .format
                .as_ref()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            map: binding
                .map
                .iter()
                .filter_map(|(key, value)| {
                    let key = key.trim();
                    let value = value.trim();
                    if key.is_empty() || value.is_empty() {
                        return None;
                    }
                    Some((key.to_string(), value.to_string()))
                })
                .collect(),
            scale: binding.scale.clone(),
        });
    }
    bindings.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then_with(|| left.selector.cmp(&right.selector))
            .then_with(|| left.attribute.cmp(&right.attribute))
    });

    let mut bindings3d = Vec::new();
    for binding in &page.bindings3d {
        let node = binding.node.trim();
        let source = binding.source.trim();
        let Ok(property) = normalize_scene_binding_property(binding.property.as_str()) else {
            continue;
        };
        if !is_safe_scene_node_ref(node) || source.is_empty() {
            continue;
        }
        bindings3d.push(HmiSceneBindingSchema {
            node: node.to_string(),
            property,
            source: source.to_string(),
            format: binding
                .format
                .as_ref()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            map: binding
                .map
                .iter()
                .filter_map(|(key, value)| {
                    let key = key.trim();
                    let value = value.trim();
                    if key.is_empty() || value.is_empty() {
                        return None;
                    }
                    Some((key.to_string(), value.to_string()))
                })
                .collect(),
            scale: binding.scale.clone(),
        });
    }
    bindings3d.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then_with(|| left.node.cmp(&right.node))
            .then_with(|| left.property.cmp(&right.property))
    });

    Some(HmiDirPage {
        id: id.to_string(),
        title,
        icon: page
            .icon
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        order: page.order,
        kind: normalize_page_kind(Some(page.kind.as_str())).to_string(),
        duration_ms: page.duration_ms,
        svg: page
            .svg
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        view: page
            .view
            .as_deref()
            .and_then(|value| normalize_scene_view_ref(value).ok()),
        scene_view: page.scene_view.clone(),
        hidden: page.hidden,
        signals: page
            .signals
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect(),
        sections,
        bindings,
        bindings3d,
    })
}
