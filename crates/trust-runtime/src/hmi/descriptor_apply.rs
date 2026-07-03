pub(super) fn apply_hmi_dir_descriptor(
    customization: &mut HmiCustomization,
    descriptor: &HmiDirDescriptor,
) {
    customization.theme.style = descriptor.config.theme.style.clone();
    customization.theme.accent = descriptor.config.theme.accent.clone();
    customization.write.enabled = descriptor.config.write.enabled;
    customization.write.allow = descriptor
        .config
        .write
        .allow
        .iter()
        .map(|entry| entry.trim().to_string())
        .filter(|entry| !entry.is_empty())
        .collect();

    customization.pages = descriptor
        .pages
        .iter()
        .map(|page| HmiPageConfig {
            id: page.id.clone(),
            title: page.title.clone(),
            icon: page.icon.clone(),
            order: page.order,
            kind: page.kind.clone(),
            duration_ms: page.duration_ms,
            svg: page.svg.clone(),
            hidden: page.hidden,
            signals: page.signals.clone(),
            sections: page
                .sections
                .iter()
                .map(|section| HmiSectionConfig {
                    title: section.title.clone(),
                    span: section.span,
                    tier: section.tier.clone(),
                    widget_paths: section
                        .widgets
                        .iter()
                        .map(|widget| widget.bind.clone())
                        .collect(),
                })
                .collect(),
            bindings: page
                .bindings
                .iter()
                .map(|binding| HmiProcessBindingSchema {
                    selector: binding.selector.clone(),
                    attribute: binding.attribute.clone(),
                    source: binding.source.clone(),
                    format: binding.format.clone(),
                    map: binding.map.clone(),
                    scale: binding.scale.clone(),
                })
                .collect(),
        })
        .collect();

    let mut overrides = BTreeMap::<String, HmiWidgetOverride>::new();
    for (page_idx, page) in descriptor.pages.iter().enumerate() {
        // Hidden pages (equipment detail pages) must not steal widget
        // page/label/type assignments from visible pages.
        if page.hidden {
            continue;
        }
        for (section_idx, section) in page.sections.iter().enumerate() {
            for (widget_idx, widget) in section.widgets.iter().enumerate() {
                let key = widget.bind.trim();
                if key.is_empty() {
                    continue;
                }
                let entry = overrides.entry(key.to_string()).or_default();
                entry.merge_from(&HmiWidgetOverride {
                    label: widget.label.clone(),
                    unit: widget.unit.clone(),
                    min: widget.min,
                    max: widget.max,
                    widget: widget.widget_type.clone(),
                    page: Some(page.id.clone()),
                    group: Some(section.title.clone()),
                    order: Some(
                        ((page_idx as i32) * 10_000)
                            + ((section_idx as i32) * 100)
                            + widget_idx as i32,
                    ),
                    zones: widget.zones.clone(),
                    on_color: widget.on_color.clone(),
                    off_color: widget.off_color.clone(),
                    section_title: Some(section.title.clone()),
                    widget_span: widget.span,
                    alarm_deadband: None,
                    inferred_interface: widget.inferred_interface,
                    detail_page: widget.detail_page.clone(),
                });
            }
        }
    }

    for alarm in &descriptor.config.alarms {
        let key = alarm.bind.trim();
        if key.is_empty() {
            continue;
        }
        let entry = overrides.entry(key.to_string()).or_default();
        if let Some(low) = alarm.low {
            entry.min = Some(low);
        }
        if let Some(high) = alarm.high {
            entry.max = Some(high);
        }
        if let Some(deadband) = alarm.deadband {
            entry.alarm_deadband = Some(deadband.max(0.0));
        }
        if entry.label.is_none() {
            entry.label = alarm.label.clone();
        }
    }

    customization.widget_overrides = overrides;
}
