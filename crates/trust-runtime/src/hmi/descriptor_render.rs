fn render_hmi_dir_config_toml(config: &HmiDirConfig) -> String {
    let mut out = String::new();
    if let Some(version) = config.version {
        let _ = writeln!(out, "version = {}", version.max(1));
        let _ = writeln!(out);
    }
    if config.theme.style.is_some() || config.theme.accent.is_some() {
        let _ = writeln!(out, "[theme]");
        if let Some(style) = config.theme.style.as_ref() {
            let _ = writeln!(out, "style = \"{}\"", escape_toml_string(style.trim()));
        }
        if let Some(accent) = config.theme.accent.as_ref() {
            let _ = writeln!(out, "accent = \"{}\"", escape_toml_string(accent.trim()));
        }
        let _ = writeln!(out);
    }
    if config.layout.navigation.is_some()
        || config.layout.header.is_some()
        || config.layout.header_title.is_some()
    {
        let _ = writeln!(out, "[layout]");
        if let Some(navigation) = config.layout.navigation.as_ref() {
            let _ = writeln!(
                out,
                "navigation = \"{}\"",
                escape_toml_string(navigation.trim())
            );
        }
        if let Some(header) = config.layout.header {
            let _ = writeln!(out, "header = {header}");
        }
        if let Some(header_title) = config.layout.header_title.as_ref() {
            let _ = writeln!(
                out,
                "header_title = \"{}\"",
                escape_toml_string(header_title.trim())
            );
        }
        let _ = writeln!(out);
    }
    if config.write.enabled.is_some() || !config.write.allow.is_empty() {
        let _ = writeln!(out, "[write]");
        if let Some(enabled) = config.write.enabled {
            let _ = writeln!(out, "enabled = {enabled}");
        }
        let allow = config
            .write
            .allow
            .iter()
            .map(|entry| format!("\"{}\"", escape_toml_string(entry.trim())))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(out, "allow = [{}]", allow);
        let _ = writeln!(out);
    }
    for alarm in &config.alarms {
        let bind = alarm.bind.trim();
        if bind.is_empty() {
            continue;
        }
        let _ = writeln!(out, "[[alarm]]");
        let _ = writeln!(out, "bind = \"{}\"", escape_toml_string(bind));
        if let Some(high) = alarm.high {
            let _ = writeln!(out, "high = {}", format_toml_number(high));
        }
        if let Some(low) = alarm.low {
            let _ = writeln!(out, "low = {}", format_toml_number(low));
        }
        if let Some(deadband) = alarm.deadband {
            let _ = writeln!(out, "deadband = {}", format_toml_number(deadband.max(0.0)));
        }
        if let Some(inferred) = alarm.inferred {
            let _ = writeln!(out, "inferred = {inferred}");
        }
        if let Some(label) = alarm.label.as_ref() {
            let _ = writeln!(out, "label = \"{}\"", escape_toml_string(label.trim()));
        }
        let _ = writeln!(out);
    }
    out.trim().to_string()
}

pub(super) fn render_hmi_dir_page_toml(page: &HmiDirPage) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "title = \"{}\"",
        escape_toml_string(page.title.as_str())
    );
    if let Some(icon) = page.icon.as_ref() {
        let _ = writeln!(out, "icon = \"{}\"", escape_toml_string(icon.as_str()));
    }
    let _ = writeln!(out, "order = {}", page.order);
    let _ = writeln!(out, "kind = \"{}\"", escape_toml_string(page.kind.as_str()));
    if let Some(duration_ms) = page.duration_ms {
        let _ = writeln!(out, "duration_s = {}", duration_ms / 1_000);
    }
    if let Some(svg) = page.svg.as_ref() {
        let _ = writeln!(out, "svg = \"{}\"", escape_toml_string(svg.as_str()));
    }
    if let Some(view) = page.view.as_ref() {
        let _ = writeln!(out, "view = \"{}\"", escape_toml_string(view.as_str()));
    }
    if !page.signals.is_empty() {
        let values = page
            .signals
            .iter()
            .map(|entry| format!("\"{}\"", escape_toml_string(entry.as_str())))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(out, "signals = [{}]", values);
    }

    for section in &page.sections {
        let _ = writeln!(out);
        let _ = writeln!(out, "[[section]]");
        let _ = writeln!(
            out,
            "title = \"{}\"",
            escape_toml_string(section.title.as_str())
        );
        let _ = writeln!(out, "span = {}", section.span.clamp(1, 12));
        if let Some(tier) = section.tier.as_ref() {
            let _ = writeln!(out, "tier = \"{}\"", escape_toml_string(tier.as_str()));
        }
        for widget in &section.widgets {
            let bind = widget.bind.trim();
            if bind.is_empty() {
                continue;
            }
            let _ = writeln!(out);
            let _ = writeln!(out, "[[section.widget]]");
            if let Some(kind) = widget.widget_type.as_ref() {
                let _ = writeln!(out, "type = \"{}\"", escape_toml_string(kind.as_str()));
            }
            let _ = writeln!(out, "bind = \"{}\"", escape_toml_string(bind));
            if let Some(label) = widget.label.as_ref() {
                let _ = writeln!(out, "label = \"{}\"", escape_toml_string(label.as_str()));
            }
            if let Some(unit) = widget.unit.as_ref() {
                let _ = writeln!(out, "unit = \"{}\"", escape_toml_string(unit.as_str()));
            }
            if let Some(min) = widget.min {
                let _ = writeln!(out, "min = {}", format_toml_number(min));
            }
            if let Some(max) = widget.max {
                let _ = writeln!(out, "max = {}", format_toml_number(max));
            }
            if let Some(span) = widget.span {
                let _ = writeln!(out, "span = {}", span.clamp(1, 12));
            }
            if let Some(on_color) = widget.on_color.as_ref() {
                let _ = writeln!(
                    out,
                    "on_color = \"{}\"",
                    escape_toml_string(on_color.as_str())
                );
            }
            if let Some(off_color) = widget.off_color.as_ref() {
                let _ = writeln!(
                    out,
                    "off_color = \"{}\"",
                    escape_toml_string(off_color.as_str())
                );
            }
            if let Some(inferred_interface) = widget.inferred_interface {
                let _ = writeln!(out, "inferred_interface = {inferred_interface}");
            }
            if let Some(detail_page) = widget.detail_page.as_ref() {
                let _ = writeln!(
                    out,
                    "detail_page = \"{}\"",
                    escape_toml_string(detail_page.as_str())
                );
            }
            for zone in &widget.zones {
                let _ = writeln!(out);
                let _ = writeln!(out, "[[section.widget.zones]]");
                let _ = writeln!(out, "from = {}", format_toml_number(zone.from));
                let _ = writeln!(out, "to = {}", format_toml_number(zone.to));
                let _ = writeln!(
                    out,
                    "color = \"{}\"",
                    escape_toml_string(zone.color.as_str())
                );
            }
        }
    }

    for binding in &page.bindings {
        let _ = writeln!(out);
        let _ = writeln!(out, "[[bind]]");
        let _ = writeln!(
            out,
            "selector = \"{}\"",
            escape_toml_string(binding.selector.as_str())
        );
        let _ = writeln!(
            out,
            "attribute = \"{}\"",
            escape_toml_string(binding.attribute.as_str())
        );
        let _ = writeln!(
            out,
            "source = \"{}\"",
            escape_toml_string(binding.source.as_str())
        );
        if let Some(format) = binding.format.as_ref() {
            let _ = writeln!(out, "format = \"{}\"", escape_toml_string(format.as_str()));
        }
        if !binding.map.is_empty() {
            let values = binding
                .map
                .iter()
                .map(|(key, value)| {
                    format!(
                        "\"{}\" = \"{}\"",
                        escape_toml_string(key.as_str()),
                        escape_toml_string(value.as_str())
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(out, "map = {{ {values} }}");
        }
        if let Some(scale) = binding.scale.as_ref() {
            let _ = writeln!(
                out,
                "scale = {{ min = {}, max = {}, output_min = {}, output_max = {} }}",
                format_toml_number(scale.min),
                format_toml_number(scale.max),
                format_toml_number(scale.output_min),
                format_toml_number(scale.output_max)
            );
        }
    }

    for binding in &page.bindings3d {
        let _ = writeln!(out);
        let _ = writeln!(out, "[[bind3d]]");
        let _ = writeln!(out, "node = \"{}\"", escape_toml_string(binding.node.as_str()));
        let _ = writeln!(
            out,
            "property = \"{}\"",
            escape_toml_string(binding.property.as_str())
        );
        let _ = writeln!(
            out,
            "source = \"{}\"",
            escape_toml_string(binding.source.as_str())
        );
        if let Some(format) = binding.format.as_ref() {
            let _ = writeln!(out, "format = \"{}\"", escape_toml_string(format.as_str()));
        }
        if !binding.map.is_empty() {
            let values = binding
                .map
                .iter()
                .map(|(key, value)| {
                    format!(
                        "\"{}\" = \"{}\"",
                        escape_toml_string(key.as_str()),
                        escape_toml_string(value.as_str())
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(out, "map = {{ {values} }}");
        }
        if let Some(scale) = binding.scale.as_ref() {
            let _ = writeln!(
                out,
                "scale = {{ min = {}, max = {}, output_min = {}, output_max = {} }}",
                format_toml_number(scale.min),
                format_toml_number(scale.max),
                format_toml_number(scale.output_min),
                format_toml_number(scale.output_max)
            );
        }
    }

    out.trim().to_string()
}
