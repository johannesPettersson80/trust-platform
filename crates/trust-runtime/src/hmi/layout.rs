fn resolve_pages(
    points: &mut [HmiPoint],
    customization: Option<&HmiCustomization>,
) -> (Vec<HmiPageSchema>, HashMap<String, i32>) {
    let trend_capable = points.iter().any(is_trend_capable_widget);
    let alarm_capable = points
        .iter()
        .any(|point| point.min.is_some() || point.max.is_some());
    let mut pages = customization
        .map(|config| {
            config
                .pages
                .iter()
                .map(|page| {
                    (
                        page.id.clone(),
                        HmiPageSchema {
                            id: page.id.clone(),
                            title: page.title.clone(),
                            order: page.order,
                            kind: normalize_page_kind(Some(page.kind.as_str())).to_string(),
                            icon: page.icon.clone(),
                            duration_ms: page.duration_ms,
                            svg: page.svg.clone(),
                            view: page.view.clone(),
                            scene_view: page.scene_view.clone(),
                            hidden: page.hidden,
                            signals: page.signals.clone(),
                            sections: Vec::new(),
                            bindings: page.bindings.clone(),
                            bindings3d: page.bindings3d.clone(),
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();

    if pages.is_empty() {
        pages.insert(
            DEFAULT_PAGE_ID.to_string(),
            HmiPageSchema {
                id: DEFAULT_PAGE_ID.to_string(),
                title: "Overview".to_string(),
                order: 0,
                kind: "dashboard".to_string(),
                icon: None,
                duration_ms: None,
                svg: None,
                view: None,
                scene_view: None,
                hidden: false,
                signals: Vec::new(),
                sections: Vec::new(),
                bindings: Vec::new(),
                bindings3d: Vec::new(),
            },
        );
    }
    if trend_capable && !pages.contains_key(DEFAULT_TREND_PAGE_ID) {
        pages.insert(
            DEFAULT_TREND_PAGE_ID.to_string(),
            HmiPageSchema {
                id: DEFAULT_TREND_PAGE_ID.to_string(),
                title: "Trends".to_string(),
                order: 50,
                kind: "trend".to_string(),
                icon: None,
                duration_ms: Some(10 * 60 * 1_000),
                svg: None,
                view: None,
                scene_view: None,
                hidden: false,
                signals: Vec::new(),
                sections: Vec::new(),
                bindings: Vec::new(),
                bindings3d: Vec::new(),
            },
        );
    }
    if alarm_capable && !pages.contains_key(DEFAULT_ALARM_PAGE_ID) {
        pages.insert(
            DEFAULT_ALARM_PAGE_ID.to_string(),
            HmiPageSchema {
                id: DEFAULT_ALARM_PAGE_ID.to_string(),
                title: "Alarms".to_string(),
                order: 60,
                kind: "alarm".to_string(),
                icon: None,
                duration_ms: None,
                svg: None,
                view: None,
                scene_view: None,
                hidden: false,
                signals: Vec::new(),
                sections: Vec::new(),
                bindings: Vec::new(),
                bindings3d: Vec::new(),
            },
        );
    }

    for point in points.iter_mut() {
        normalize_point(point);
        if !pages.contains_key(point.page.as_str()) {
            pages.insert(
                point.page.clone(),
                HmiPageSchema {
                    id: point.page.clone(),
                    title: title_case(&point.page),
                    order: 1000,
                    kind: "dashboard".to_string(),
                    icon: None,
                    duration_ms: None,
                    svg: None,
                    view: None,
                    scene_view: None,
                    hidden: false,
                    signals: Vec::new(),
                    sections: Vec::new(),
                    bindings: Vec::new(),
                    bindings3d: Vec::new(),
                },
            );
        }
    }

    if let Some(customization) = customization {
        let id_by_path = points
            .iter()
            .map(|point| (point.path.as_str(), point.id.as_str()))
            .collect::<HashMap<_, _>>();
        let dir_desc = customization.dir_descriptor.as_ref();
        for page in &customization.pages {
            let Some(page_schema) = pages.get_mut(page.id.as_str()) else {
                continue;
            };
            if page.sections.is_empty() {
                continue;
            }
            let dir_page = dir_desc.and_then(|d| d.pages.iter().find(|p| p.id == page.id));
            page_schema.sections = page
                .sections
                .iter()
                .enumerate()
                .map(|(section_idx, section)| {
                    let widget_ids: Vec<String> = section
                        .widget_paths
                        .iter()
                        .filter_map(|path| {
                            id_by_path.get(path.as_str()).map(|id| (*id).to_string())
                        })
                        .collect();
                    let module_meta = if section.tier.as_deref() == Some("module") {
                        dir_page
                            .and_then(|dp| dp.sections.get(section_idx))
                            .map(|ds| {
                                ds.widgets
                                    .iter()
                                    .filter_map(|w| {
                                        let widget_id =
                                            id_by_path.get(w.bind.as_str())?.to_string();
                                        Some(HmiModuleMeta {
                                            id: widget_id,
                                            label: w.label.clone().unwrap_or_default(),
                                            detail_page: w.detail_page.clone(),
                                            unit: w.unit.clone(),
                                        })
                                    })
                                    .collect()
                            })
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                    HmiSectionSchema {
                        title: section.title.clone(),
                        span: section.span.clamp(1, 12),
                        tier: section.tier.clone(),
                        widget_ids,
                        module_meta,
                    }
                })
                .collect();
        }
    }

    let mut ordered = pages.into_values().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        left.order
            .cmp(&right.order)
            .then_with(|| left.id.cmp(&right.id))
    });
    let page_order = ordered
        .iter()
        .map(|page| (page.id.clone(), page.order))
        .collect::<HashMap<_, _>>();
    (ordered, page_order)
}

fn normalize_page_kind(value: Option<&str>) -> &'static str {
    match value
        .map(|raw| raw.trim().to_ascii_lowercase())
        .as_deref()
        .unwrap_or("dashboard")
    {
        "dashboard" => "dashboard",
        "trend" => "trend",
        "alarm" => "alarm",
        "table" => "table",
        "process" => "process",
        "scene3d" => "scene3d",
        _ => "dashboard",
    }
}

fn is_safe_process_selector(selector: &str) -> bool {
    let mut chars = selector.chars();
    if chars.next() != Some('#') {
        return false;
    }
    if selector.len() > 128 {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | ':' | '.'))
}

fn normalize_process_attribute(attribute: &str) -> Option<String> {
    let normalized = attribute.trim().to_ascii_lowercase();
    if matches!(
        normalized.as_str(),
        "text"
            | "fill"
            | "stroke"
            | "opacity"
            | "x"
            | "y"
            | "width"
            | "height"
            | "class"
            | "transform"
            | "data-value"
    ) {
        Some(normalized)
    } else {
        None
    }
}

fn normalize_process_scale(scale: HmiProcessScaleToml) -> Option<HmiProcessScaleSchema> {
    if !scale.min.is_finite()
        || !scale.max.is_finite()
        || !scale.output_min.is_finite()
        || !scale.output_max.is_finite()
    {
        return None;
    }
    if scale.max <= scale.min {
        return None;
    }
    if (scale.output_max - scale.output_min).abs() < f64::EPSILON {
        return None;
    }
    Some(HmiProcessScaleSchema {
        min: scale.min,
        max: scale.max,
        output_min: scale.output_min,
        output_max: scale.output_max,
    })
}

fn resolve_responsive(config: Option<&HmiResponsiveConfig>) -> HmiResponsiveSchema {
    let mode = config
        .and_then(|value| value.mode.as_deref())
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| matches!(value.as_str(), "auto" | "mobile" | "tablet" | "kiosk"))
        .unwrap_or_else(|| DEFAULT_RESPONSIVE_MODE.to_string());
    HmiResponsiveSchema {
        mode,
        mobile_max_px: 680,
        tablet_max_px: 1024,
    }
}

fn resolve_export(config: Option<&HmiExportConfig>) -> HmiExportSchema {
    HmiExportSchema {
        enabled: config.and_then(|value| value.enabled).unwrap_or(true),
        route: "/hmi/export.json".to_string(),
    }
}

fn resolve_theme(theme: Option<&HmiThemeConfig>) -> HmiThemeSchema {
    let requested_style = theme
        .and_then(|config| config.style.as_ref())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_else(|| "classic".to_string());
    let palette = theme_palette(requested_style.as_str())
        .unwrap_or(theme_palette("classic").expect("classic theme"));
    let accent = theme
        .and_then(|config| config.accent.as_ref())
        .filter(|value| is_hex_color(value))
        .cloned()
        .unwrap_or_else(|| palette.accent.to_string());
    HmiThemeSchema {
        style: palette.style.to_string(),
        accent,
        background: palette.background.to_string(),
        surface: palette.surface.to_string(),
        text: palette.text.to_string(),
    }
}

fn theme_palette(style: &str) -> Option<ThemePalette> {
    match style {
        "classic" => Some(ThemePalette {
            style: "classic",
            accent: "#0f766e",
            background: "#f3f5f8",
            surface: "#ffffff",
            text: "#142133",
        }),
        "industrial" => Some(ThemePalette {
            style: "industrial",
            accent: "#c2410c",
            background: "#f5f3ef",
            surface: "#ffffff",
            text: "#221a14",
        }),
        "mint" => Some(ThemePalette {
            style: "mint",
            accent: "#0d9488",
            background: "#ecfdf5",
            surface: "#f8fffc",
            text: "#0b3b35",
        }),
        "control-room" => Some(ThemePalette {
            style: "control-room",
            accent: "#14b8a6",
            background: "#0f1115",
            surface: "#171a21",
            text: "#f2f2f2",
        }),
        _ => None,
    }
}

fn apply_widget_override(point: &mut HmiPoint, override_spec: &HmiWidgetOverride) {
    if let Some(label) = override_spec.label.as_ref() {
        point.label = label.clone();
    }
    if let Some(unit) = override_spec.unit.as_ref() {
        point.unit = Some(unit.clone());
    }
    if let Some(min) = override_spec.min {
        point.min = Some(min);
    }
    if let Some(max) = override_spec.max {
        point.max = Some(max);
    }
    if let Some(widget) = override_spec.widget.as_ref() {
        point.widget = widget.clone();
    }
    if let Some(page) = override_spec.page.as_ref() {
        point.page = page.clone();
    }
    if let Some(group) = override_spec.group.as_ref() {
        point.group = group.clone();
    }
    if let Some(order) = override_spec.order {
        point.order = order;
    }
    if !override_spec.zones.is_empty() {
        point.zones = override_spec.zones.clone();
    }
    if let Some(on_color) = override_spec.on_color.as_ref() {
        point.on_color = Some(on_color.clone());
    }
    if let Some(off_color) = override_spec.off_color.as_ref() {
        point.off_color = Some(off_color.clone());
    }
    if let Some(section_title) = override_spec.section_title.as_ref() {
        point.section_title = Some(section_title.clone());
    }
    if let Some(widget_span) = override_spec.widget_span {
        point.widget_span = Some(widget_span);
    }
    if let Some(alarm_deadband) = override_spec.alarm_deadband {
        point.alarm_deadband = Some(alarm_deadband.max(0.0));
    }
    if let Some(inferred_interface) = override_spec.inferred_interface {
        point.inferred_interface = inferred_interface;
    }
    if let Some(detail_page) = override_spec.detail_page.as_ref() {
        point.detail_page = Some(detail_page.clone());
    }
}

fn normalize_point(point: &mut HmiPoint) {
    if point.page.trim().is_empty() {
        point.page = DEFAULT_PAGE_ID.to_string();
    }
    if point.group.trim().is_empty() {
        point.group = DEFAULT_GROUP_NAME.to_string();
    }
    if point.widget.trim().is_empty() {
        point.widget = "value".to_string();
    }
    point.zones.sort_by(|left, right| {
        left.from
            .total_cmp(&right.from)
            .then_with(|| left.to.total_cmp(&right.to))
    });
    if let Some(section_title) = point.section_title.as_ref() {
        if section_title.trim().is_empty() {
            point.section_title = None;
        }
    }
    if let Some(span) = point.widget_span {
        point.widget_span = Some(span.clamp(1, 12));
    }
    if let Some(deadband) = point.alarm_deadband {
        point.alarm_deadband = Some(deadband.max(0.0));
    }
}

fn is_trend_capable_widget(point: &HmiPoint) -> bool {
    is_numeric_data_type(point.data_type.as_str())
        || matches!(point.widget.as_str(), "value" | "slider")
}

fn is_trend_capable_widget_schema(widget: &HmiWidgetSchema) -> bool {
    is_numeric_data_type(widget.data_type.as_str())
        || matches!(widget.widget.as_str(), "value" | "slider")
}

fn is_supported_widget_kind(kind: &str) -> bool {
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

fn widget_kind_matches_point(kind: &str, point: &HmiPoint) -> bool {
    let point_kind = point.widget.as_str();
    match point_kind {
        "indicator" | "toggle" => matches!(kind, "indicator" | "toggle"),
        "selector" | "readout" => matches!(kind, "selector" | "readout"),
        "table" => kind == "table",
        "tree" => kind == "tree",
        "text" => kind == "text",
        "value" | "slider" => matches!(
            kind,
            "gauge" | "sparkline" | "bar" | "tank" | "value" | "slider"
        ),
        _ => true,
    }
}

fn is_numeric_data_type(data_type: &str) -> bool {
    matches!(
        data_type.to_ascii_uppercase().as_str(),
        "SINT"
            | "INT"
            | "DINT"
            | "LINT"
            | "USINT"
            | "UINT"
            | "UDINT"
            | "ULINT"
            | "BYTE"
            | "WORD"
            | "DWORD"
            | "LWORD"
            | "REAL"
            | "LREAL"
            | "TIME"
            | "LTIME"
            | "DATE"
            | "LDATE"
            | "TOD"
            | "LTOD"
            | "DT"
            | "LDT"
    )
}
