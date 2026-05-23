pub(super) fn load_hmi_dir_impl(root: &Path) -> anyhow::Result<HmiDirDescriptor> {
    let dir = root.join("hmi");
    if !dir.is_dir() {
        anyhow::bail!("hmi directory not found");
    }

    let config = load_hmi_dir_config(&dir)?;
    let mut page_paths = BTreeSet::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        if path.file_name().and_then(|name| name.to_str()) == Some("_config.toml") {
            continue;
        }
        page_paths.insert(path);
    }

    let mut pages = Vec::with_capacity(page_paths.len());
    for path in page_paths {
        let Some(stem) = path.file_stem().and_then(|name| name.to_str()) else {
            continue;
        };
        let id = stem.trim();
        if id.is_empty() {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        let parsed = toml::from_str::<HmiDirPageToml>(&text)?;
        pages.push((id.to_string(), parsed));
    }

    pages.sort_by(|left, right| left.0.cmp(&right.0));
    let mut parsed_pages = Vec::with_capacity(pages.len());
    for (idx, (id, parsed)) in pages.into_iter().enumerate() {
        parsed_pages.push(map_hmi_dir_page(id, idx, parsed)?);
    }
    parsed_pages.sort_by(|left, right| {
        left.order
            .cmp(&right.order)
            .then_with(|| left.id.cmp(&right.id))
    });
    promote_process_pages_to_custom_svg_if_available(&dir, &mut parsed_pages);
    attach_scene_view_payloads(&dir, &mut parsed_pages)?;

    Ok(HmiDirDescriptor {
        config,
        pages: parsed_pages,
    })
}

fn attach_scene_view_payloads(dir: &Path, pages: &mut [HmiDirPage]) -> anyhow::Result<()> {
    for page in pages {
        if !page.kind.eq_ignore_ascii_case("scene3d") {
            continue;
        }
        let Some(view_ref) = page.view.as_deref() else {
            continue;
        };
        let path = dir.join(view_ref);
        if !path.is_file() {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|err| {
            anyhow::anyhow!("failed to read hmi scene view '{}': {err}", path.display())
        })?;
        let payload = toml::from_str::<HmiSceneViewPayload>(&text).map_err(|err| {
            anyhow::anyhow!("failed to parse hmi scene view '{}': {err}", path.display())
        })?;
        page.scene_view = Some(payload);
    }
    Ok(())
}

fn promote_process_pages_to_custom_svg_if_available(dir: &Path, pages: &mut [HmiDirPage]) {
    let Some(candidate) = find_custom_process_svg_candidate(dir) else {
        return;
    };
    for page in pages {
        if !page.kind.eq_ignore_ascii_case("process") {
            continue;
        }
        let svg_is_auto = page
            .svg
            .as_ref()
            .map(|value| value.trim().eq_ignore_ascii_case("process.auto.svg"))
            .unwrap_or(true);
        if svg_is_auto {
            page.svg = Some(candidate.clone());
        }
    }
}

fn find_custom_process_svg_candidate(dir: &Path) -> Option<String> {
    let mut svg_files = Vec::new();
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if !path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
        {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.eq_ignore_ascii_case("process.auto.svg") {
            continue;
        }
        svg_files.push(name.to_string());
    }
    svg_files.sort();
    svg_files.into_iter().next()
}

fn load_hmi_dir_config(dir: &Path) -> anyhow::Result<HmiDirConfig> {
    let path = dir.join("_config.toml");
    if !path.is_file() {
        return Ok(HmiDirConfig::default());
    }
    let text = std::fs::read_to_string(path)?;
    let parsed = toml::from_str::<HmiDirConfigToml>(&text)?;
    let mut alarms = parsed
        .alarms
        .into_iter()
        .filter_map(|alarm| {
            let bind = alarm.bind.trim();
            if bind.is_empty() {
                return None;
            }
            let label = alarm
                .label
                .map(|label| label.trim().to_string())
                .filter(|label| !label.is_empty());
            Some(HmiDirAlarm {
                bind: bind.to_string(),
                high: alarm.high,
                low: alarm.low,
                deadband: alarm.deadband.map(|value| value.max(0.0)),
                inferred: alarm.inferred,
                label,
            })
        })
        .collect::<Vec<_>>();
    alarms.sort_by(|left, right| left.bind.cmp(&right.bind));
    Ok(HmiDirConfig {
        version: parsed.version.or(Some(HMI_DESCRIPTOR_VERSION)),
        theme: parsed.theme,
        layout: parsed.layout,
        write: HmiDirWrite {
            enabled: parsed.write.enabled,
            allow: parsed
                .write
                .allow
                .into_iter()
                .map(|entry| entry.trim().to_string())
                .filter(|entry| !entry.is_empty())
                .collect(),
        },
        alarms,
    })
}

pub(super) fn map_hmi_dir_page(
    id: String,
    default_index: usize,
    page: HmiDirPageToml,
) -> anyhow::Result<HmiDirPage> {
    let title = page
        .title
        .map(|title| title.trim().to_string())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| title_case(id.as_str()));
    let icon = page
        .icon
        .map(|icon| icon.trim().to_string())
        .filter(|icon| !icon.is_empty());
    let svg = page
        .svg
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty());
    let view = page
        .view
        .as_deref()
        .map(normalize_scene_view_ref)
        .transpose()?;
    let mut sections = Vec::with_capacity(page.sections.len());
    for (idx, section) in page.sections.into_iter().enumerate() {
        let title = section
            .title
            .map(|title| title.trim().to_string())
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| format!("Section {}", idx + 1));
        let span = section.span.unwrap_or(12).clamp(1, 12);
        let mut widgets = Vec::with_capacity(section.widgets.len());
        for widget in section.widgets {
            let bind = widget.bind.unwrap_or_default();
            let bind = bind.trim();
            if bind.is_empty() {
                continue;
            }
            let mut zones = widget.zones;
            zones.sort_by(|left, right| {
                left.from
                    .total_cmp(&right.from)
                    .then_with(|| left.to.total_cmp(&right.to))
            });
            widgets.push(HmiDirWidget {
                widget_type: widget
                    .widget_type
                    .map(|kind| kind.trim().to_ascii_lowercase())
                    .filter(|kind| !kind.is_empty()),
                bind: bind.to_string(),
                label: widget
                    .label
                    .map(|label| label.trim().to_string())
                    .filter(|label| !label.is_empty()),
                unit: widget
                    .unit
                    .map(|unit| unit.trim().to_string())
                    .filter(|unit| !unit.is_empty()),
                min: widget.min,
                max: widget.max,
                span: widget.span.map(|span| span.clamp(1, 12)),
                on_color: widget
                    .on_color
                    .map(|color| color.trim().to_string())
                    .filter(|color| !color.is_empty()),
                off_color: widget
                    .off_color
                    .map(|color| color.trim().to_string())
                    .filter(|color| !color.is_empty()),
                inferred_interface: widget.inferred_interface,
                detail_page: widget.detail_page.clone(),
                zones,
            });
        }
        sections.push(HmiDirSection {
            title,
            span,
            tier: section
                .tier
                .map(|t| t.trim().to_ascii_lowercase())
                .filter(|t| !t.is_empty()),
            widgets,
        });
    }

    let mut bindings = Vec::with_capacity(page.bindings.len());
    for binding in page.bindings {
        let selector = binding.selector.unwrap_or_default();
        let selector = selector.trim();
        if !is_safe_process_selector(selector) {
            continue;
        }
        let attribute = binding.attribute.unwrap_or_default();
        let attribute = attribute.trim();
        let Some(attribute) = normalize_process_attribute(attribute) else {
            continue;
        };
        let source = binding.source.unwrap_or_default();
        let source = source.trim();
        if source.is_empty() {
            continue;
        }
        let format = binding
            .format
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let map = binding
            .map
            .into_iter()
            .filter_map(|(key, value)| {
                let key = key.trim().to_string();
                let value = value.trim().to_string();
                if key.is_empty() || value.is_empty() {
                    return None;
                }
                Some((key, value))
            })
            .collect::<BTreeMap<_, _>>();
        let scale = binding.scale.and_then(normalize_process_scale);
        bindings.push(HmiDirProcessBinding {
            selector: selector.to_string(),
            attribute,
            source: source.to_string(),
            format,
            map,
            scale,
        });
    }
    bindings.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then_with(|| left.selector.cmp(&right.selector))
            .then_with(|| left.attribute.cmp(&right.attribute))
    });

    let mut bindings3d = Vec::with_capacity(page.bindings3d.len());
    for binding in page.bindings3d {
        let node = binding.node.unwrap_or_default();
        let node = node.trim();
        if !is_safe_scene_node_ref(node) {
            continue;
        }
        let Some(property) = binding.property.map(HmiScenePropertyToml::into_inner) else {
            continue;
        };
        let source = binding.source.unwrap_or_default();
        let source = source.trim();
        if source.is_empty() {
            continue;
        }
        let format = binding
            .format
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let map = binding
            .map
            .into_iter()
            .filter_map(|(key, value)| {
                let key = key.trim().to_string();
                let value = value.trim().to_string();
                if key.is_empty() || value.is_empty() {
                    return None;
                }
                Some((key, value))
            })
            .collect::<BTreeMap<_, _>>();
        let scale = binding.scale.and_then(normalize_process_scale);
        bindings3d.push(HmiSceneBindingSchema {
            node: node.to_string(),
            property,
            source: source.to_string(),
            format,
            map,
            scale,
        });
    }
    bindings3d.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then_with(|| left.node.cmp(&right.node))
            .then_with(|| left.property.cmp(&right.property))
    });

    Ok(HmiDirPage {
        id,
        title,
        icon,
        order: page.order.unwrap_or((default_index as i32) * 10),
        kind: normalize_page_kind(page.kind.as_deref()).to_string(),
        duration_ms: page.duration_s.map(|seconds| seconds.saturating_mul(1_000)),
        svg,
        view,
        scene_view: None,
        hidden: page.hidden.unwrap_or(false),
        signals: page
            .signals
            .into_iter()
            .map(|signal| signal.trim().to_string())
            .filter(|signal| !signal.is_empty())
            .collect(),
        sections,
        bindings,
        bindings3d,
    })
}

pub fn load_hmi_scene_view(root: &Path, view_ref: &str) -> anyhow::Result<HmiSceneViewPayload> {
    let normalized = normalize_scene_view_ref(view_ref)?;
    let path = root.join("hmi").join(&normalized);
    if !path.is_file() {
        anyhow::bail!("hmi scene view '{}' not found", normalized);
    }
    let text = std::fs::read_to_string(&path).map_err(|err| {
        anyhow::anyhow!("failed to read hmi scene view '{}': {err}", path.display())
    })?;
    toml::from_str::<HmiSceneViewPayload>(&text).map_err(|err| {
        anyhow::anyhow!("failed to parse hmi scene view '{}': {err}", path.display())
    })
}

fn normalize_scene_view_ref(value: &str) -> anyhow::Result<String> {
    let trimmed = value.trim().replace('\\', "/");
    if trimmed.is_empty() {
        anyhow::bail!("scene3d view path must not be empty");
    }
    if trimmed.starts_with('/') || trimmed.contains(':') {
        anyhow::bail!("scene3d view path must be relative to hmi/views");
    }
    let mut parts = trimmed
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        anyhow::bail!("scene3d view path must not be empty");
    }
    if parts.len() == 1 {
        parts.insert(0, "views");
    }
    if parts.first() != Some(&"views") {
        anyhow::bail!("scene3d view path must resolve under hmi/views");
    }
    if parts.iter().any(|part| {
        *part == "."
            || *part == ".."
            || part.len() > 96
            || !part
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
    }) {
        anyhow::bail!("scene3d view path contains unsupported path segment");
    }
    let Some(file_name) = parts.last() else {
        anyhow::bail!("scene3d view path must include a file name");
    };
    if !file_name.ends_with(".view.toml") {
        anyhow::bail!("scene3d view path must end with .view.toml");
    }
    Ok(parts.join("/"))
}

fn is_safe_scene_node_ref(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | ':' | '.' | '/'))
}
