fn render_overview_toml(
    icon: &str,
    sections: &[ScaffoldSection],
    equipment_groups: &[ScaffoldEquipmentGroup],
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "title = \"Overview\"");
    let _ = writeln!(out, "icon = \"{}\"", escape_toml_string(icon));
    let _ = writeln!(out, "order = 0");

    for section in sections {
        let _ = writeln!(out);
        let _ = writeln!(out, "[[section]]");
        let _ = writeln!(
            out,
            "title = \"{}\"",
            escape_toml_string(section.title.as_str())
        );
        let _ = writeln!(out, "span = {}", section.span);
        if let Some(tier) = section.tier.as_ref() {
            let _ = writeln!(out, "tier = \"{}\"", escape_toml_string(tier));
        }

        let is_module = section.tier.as_deref() == Some("module");

        for point in &section.widgets {
            let _ = writeln!(out);
            let _ = writeln!(out, "[[section.widget]]");
            let _ = writeln!(out, "type = \"{}\"", escape_toml_string(&point.widget));
            let _ = writeln!(out, "bind = \"{}\"", escape_toml_string(&point.path));
            if point.inferred_interface {
                let _ = writeln!(out, "inferred_interface = true");
            }
            let _ = writeln!(out, "label = \"{}\"", escape_toml_string(&point.label));
            // For module widgets, link to their equipment detail page.
            if is_module
                && let Some(group) = equipment_groups
                    .iter()
                    .find(|g| g.widgets.iter().any(|w| w.path == point.path))
            {
                let _ = writeln!(
                    out,
                    "detail_page = \"{}\"",
                    escape_toml_string(&group.detail_page_id)
                );
            }
            let _ = writeln!(
                out,
                "span = {}",
                overview_widget_span(point, section.tier.as_deref())
            );
            if let Some(unit) = point.unit.as_ref() {
                let _ = writeln!(out, "unit = \"{}\"", escape_toml_string(unit));
            }
            if let Some(min) = point.min {
                let _ = writeln!(out, "min = {}", format_toml_number(min));
            }
            if let Some(max) = point.max {
                let _ = writeln!(out, "max = {}", format_toml_number(max));
            }
        }
    }

    out
}

fn render_equipment_detail_toml(group: &ScaffoldEquipmentGroup, order: i32) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "title = \"{}\"", escape_toml_string(&group.title));
    let _ = writeln!(out, "icon = \"settings\"");
    let _ = writeln!(out, "order = {order}");
    let _ = writeln!(out, "kind = \"dashboard\"");
    let _ = writeln!(out, "hidden = true");

    // Status section: boolean signals
    let bools: Vec<_> = group
        .widgets
        .iter()
        .filter(|w| w.type_bucket == ScaffoldTypeBucket::Bool)
        .collect();
    if !bools.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(out, "[[section]]");
        let _ = writeln!(out, "title = \"Status\"");
        let _ = writeln!(out, "span = 12");
        for point in &bools {
            let _ = writeln!(out);
            let _ = writeln!(out, "[[section.widget]]");
            let _ = writeln!(out, "type = \"indicator\"");
            let _ = writeln!(out, "bind = \"{}\"", escape_toml_string(&point.path));
            let _ = writeln!(out, "label = \"{}\"", escape_toml_string(&point.label));
            let _ = writeln!(out, "span = 6");
        }
    }

    // Values section: numeric signals
    let numerics: Vec<_> = group
        .widgets
        .iter()
        .filter(|w| w.type_bucket == ScaffoldTypeBucket::Numeric)
        .collect();
    if !numerics.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(out, "[[section]]");
        let _ = writeln!(out, "title = \"Values\"");
        let _ = writeln!(out, "span = 12");
        for point in &numerics {
            let _ = writeln!(out);
            let _ = writeln!(out, "[[section.widget]]");
            let _ = writeln!(out, "type = \"gauge\"");
            let _ = writeln!(out, "bind = \"{}\"", escape_toml_string(&point.path));
            let _ = writeln!(out, "label = \"{}\"", escape_toml_string(&point.label));
            let _ = writeln!(out, "span = 6");
            if let Some(unit) = point.unit.as_ref() {
                let _ = writeln!(out, "unit = \"{}\"", escape_toml_string(unit));
            }
            if let Some(min) = point.min {
                let _ = writeln!(out, "min = {}", format_toml_number(min));
            }
            if let Some(max) = point.max {
                let _ = writeln!(out, "max = {}", format_toml_number(max));
            }
        }
    }

    // Text/enum section: text signals
    let strings: Vec<_> = group
        .widgets
        .iter()
        .filter(|w| w.type_bucket == ScaffoldTypeBucket::Text)
        .collect();
    if !strings.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(out, "[[section]]");
        let _ = writeln!(out, "title = \"Text\"");
        let _ = writeln!(out, "span = 12");
        for point in &strings {
            let _ = writeln!(out);
            let _ = writeln!(out, "[[section.widget]]");
            let _ = writeln!(out, "type = \"text\"");
            let _ = writeln!(out, "bind = \"{}\"", escape_toml_string(&point.path));
            let _ = writeln!(out, "label = \"{}\"", escape_toml_string(&point.label));
            let _ = writeln!(out, "span = 6");
        }
    }

    out
}
