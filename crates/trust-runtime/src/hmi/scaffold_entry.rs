pub fn scaffold_hmi_dir(
    root: &Path,
    metadata: &RuntimeMetadata,
    style: &str,
) -> anyhow::Result<HmiScaffoldSummary> {
    scaffold_hmi_dir_with_sources_mode(
        root,
        metadata,
        None,
        &[],
        style,
        HmiScaffoldMode::Reset,
        true,
    )
}

pub fn scaffold_hmi_dir_with_sources(
    root: &Path,
    metadata: &RuntimeMetadata,
    snapshot: Option<&DebugSnapshot>,
    sources: &[HmiSourceRef<'_>],
    style: &str,
) -> anyhow::Result<HmiScaffoldSummary> {
    scaffold_hmi_dir_with_sources_mode(
        root,
        metadata,
        snapshot,
        sources,
        style,
        HmiScaffoldMode::Reset,
        true,
    )
}

pub fn scaffold_hmi_dir_with_sources_mode(
    root: &Path,
    metadata: &RuntimeMetadata,
    snapshot: Option<&DebugSnapshot>,
    sources: &[HmiSourceRef<'_>],
    style: &str,
    mode: HmiScaffoldMode,
    force: bool,
) -> anyhow::Result<HmiScaffoldSummary> {
    let style = normalize_scaffold_style(style);
    let palette = theme_palette(style.as_str())
        .or_else(|| theme_palette("industrial"))
        .expect("industrial theme");
    let source_index = collect_source_symbol_index(sources);
    let points = collect_scaffold_points(metadata, snapshot, &source_index);
    let overview_points = select_scaffold_overview_points(points.clone());
    let overview_result = build_tiered_overview_sections(overview_points);
    let overview_icon = infer_icon_for_points(&points);
    let overview_text = render_overview_toml(
        overview_icon.as_str(),
        &overview_result.sections,
        &overview_result.equipment_groups,
    );

    let numeric_signals = select_scaffold_trend_signals(&points);

    let mut alarms = points
        .iter()
        .filter_map(|point| match (point.writable, point.min, point.max) {
            (false, Some(min), Some(max)) if point.type_bucket == ScaffoldTypeBucket::Numeric => {
                let span = (max - min).abs();
                let deadband = if span > f64::EPSILON {
                    Some(span * 0.02)
                } else {
                    None
                };
                Some((point.path.clone(), point.label.clone(), min, max, deadband))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    alarms.sort_by(|left, right| left.0.cmp(&right.0));

    let mut program_names = metadata
        .programs()
        .keys()
        .map(|name| infer_label(name.as_str()))
        .collect::<Vec<_>>();
    program_names.sort();
    let header_title = program_names
        .first()
        .map(|name| format!("{name} HMI"))
        .unwrap_or_else(|| "trueST HMI".to_string());
    let config_text = render_config_toml(
        style.as_str(),
        palette.accent,
        header_title.as_str(),
        &alarms,
    );

    let hmi_dir = root.join("hmi");
    let hmi_exists = hmi_dir.is_dir();
    let hmi_has_files = hmi_exists
        && hmi_dir
            .read_dir()
            .ok()
            .is_some_and(|mut it| it.next().is_some());
    if mode == HmiScaffoldMode::Init && hmi_has_files && !force {
        anyhow::bail!(
            "hmi directory already exists at '{}' (run 'trust-runtime hmi update' to merge missing pages, 'trust-runtime hmi reset' to overwrite, or pass --force to init)",
            hmi_dir.display()
        );
    }
    std::fs::create_dir_all(&hmi_dir).map_err(|err| {
        anyhow::anyhow!(
            "failed to create scaffold directory '{}': {err}",
            hmi_dir.display()
        )
    })?;

    let mut files = Vec::new();
    if mode == HmiScaffoldMode::Reset && hmi_has_files {
        let backup_name = backup_existing_hmi_dir(root, &hmi_dir)?;
        files.push(HmiScaffoldFileSummary {
            path: backup_name,
            detail: "backup snapshot created before reset".to_string(),
        });
    }

    let has_writable_points = points.iter().any(|point| point.writable);
    let custom_process_pages_present = mode == HmiScaffoldMode::Update
        && hmi_has_custom_page_kind(&hmi_dir, "process", "process.toml");
    let skip_default_process_page = mode == HmiScaffoldMode::Update
        && !hmi_dir.join("process.toml").is_file()
        && custom_process_pages_present;
    let skip_default_control_page = mode == HmiScaffoldMode::Update
        && !hmi_dir.join("control.toml").is_file()
        && !has_writable_points;

    let process_text = render_process_toml(&points, "process.auto.svg");
    let process_svg_text = render_process_auto_svg();
    let control_text = render_control_toml(&points);
    let trends_text = render_trends_toml(&numeric_signals);
    let alarms_text = render_alarms_toml();

    let mut artifacts = vec![
        (
            "overview.toml",
            overview_text,
            format!(
                "{} sections, {} widgets",
                overview_result.sections.len(),
                overview_result
                    .sections
                    .iter()
                    .map(|section| section.widgets.len())
                    .sum::<usize>()
            ),
        ),
        (
            "trends.toml",
            trends_text,
            format!("{} curated numeric signals", numeric_signals.len()),
        ),
        (
            "alarms.toml",
            alarms_text,
            format!("{} alarm points", alarms.len()),
        ),
        (
            "_config.toml",
            config_text,
            format!("theme {style}, accent {}", palette.accent),
        ),
    ];
    if !skip_default_process_page {
        artifacts.push((
            "process.toml",
            process_text,
            "process page (auto-schematic mode)".to_string(),
        ));
        artifacts.push((
            "process.auto.svg",
            process_svg_text,
            "generated process topology SVG".to_string(),
        ));
    } else {
        files.push(HmiScaffoldFileSummary {
            path: "process.toml".to_string(),
            detail: "skipped (custom process page exists)".to_string(),
        });
        files.push(HmiScaffoldFileSummary {
            path: "process.auto.svg".to_string(),
            detail: "skipped (custom process page exists)".to_string(),
        });
    }
    if !skip_default_control_page {
        artifacts.push((
            "control.toml",
            control_text,
            "control page (commands/setpoints/modes)".to_string(),
        ));
    } else {
        files.push(HmiScaffoldFileSummary {
            path: "control.toml".to_string(),
            detail: "skipped (no writable points discovered)".to_string(),
        });
    }

    let overwrite = !matches!(mode, HmiScaffoldMode::Update);
    artifacts.sort_by(|left, right| left.0.cmp(right.0));
    for (name, text, detail) in artifacts {
        let path = hmi_dir.join(name);
        if !overwrite && path.exists() {
            if name.ends_with(".toml") && name != "_config.toml" {
                if let Ok(Some((merged_text, merge_detail))) =
                    merge_scaffold_update_page(path.as_path(), name, text.as_str())
                {
                    write_scaffold_file(&path, merged_text.as_str())?;
                    files.push(HmiScaffoldFileSummary {
                        path: name.to_string(),
                        detail: merge_detail,
                    });
                    continue;
                }
            }
            files.push(HmiScaffoldFileSummary {
                path: name.to_string(),
                detail: "preserved existing".to_string(),
            });
            continue;
        }
        write_scaffold_file(&path, text.as_str())?;
        files.push(HmiScaffoldFileSummary {
            path: name.to_string(),
            detail,
        });
    }

    // Generate equipment detail pages (hidden, accessible via equipment strip click-through)
    for (idx, group) in overview_result.equipment_groups.iter().enumerate() {
        let filename = format!("{}.toml", group.detail_page_id);
        let path = hmi_dir.join(&filename);
        if !overwrite && path.exists() {
            files.push(HmiScaffoldFileSummary {
                path: filename,
                detail: "preserved existing".to_string(),
            });
            continue;
        }
        let detail_text = render_equipment_detail_toml(group, 100 + idx as i32);
        write_scaffold_file(&path, detail_text.as_str())?;
        files.push(HmiScaffoldFileSummary {
            path: filename,
            detail: format!(
                "equipment detail: {} ({} signals)",
                group.title,
                group.widgets.len()
            ),
        });
    }

    files.push(HmiScaffoldFileSummary {
        path: "mode".to_string(),
        detail: mode.as_str().to_string(),
    });

    Ok(HmiScaffoldSummary { style, files })
}

fn hmi_has_custom_page_kind(hmi_dir: &Path, kind: &str, default_file_name: &str) -> bool {
    let normalized_kind = kind.trim().to_ascii_lowercase();
    let Ok(entries) = std::fs::read_dir(hmi_dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if !path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"))
        {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if file_name.eq_ignore_ascii_case("_config.toml")
            || file_name.eq_ignore_ascii_case(default_file_name)
        {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let parsed = match toml::from_str::<toml::Value>(&text) {
            Ok(parsed) => parsed,
            Err(_) => continue,
        };
        let page_kind = parsed
            .get("kind")
            .and_then(toml::Value::as_str)
            .map(|value| value.to_ascii_lowercase())
            .unwrap_or_else(|| "dashboard".to_string());
        if page_kind == normalized_kind {
            return true;
        }
    }
    false
}

fn backup_existing_hmi_dir(root: &Path, hmi_dir: &Path) -> anyhow::Result<String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let backup_name = format!("hmi.backup.{stamp}");
    let backup_dir = root.join(&backup_name);
    copy_dir_recursive(hmi_dir, &backup_dir)?;
    Ok(backup_name)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dst).map_err(|err| {
        anyhow::anyhow!(
            "failed to create backup directory '{}': {err}",
            dst.display()
        )
    })?;
    for entry in std::fs::read_dir(src).map_err(|err| {
        anyhow::anyhow!("failed to read source directory '{}': {err}", src.display())
    })? {
        let entry =
            entry.map_err(|err| anyhow::anyhow!("failed to read source directory entry: {err}"))?;
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(path.as_path(), dest_path.as_path())?;
        } else if path.is_file() {
            std::fs::copy(path.as_path(), dest_path.as_path()).map_err(|err| {
                anyhow::anyhow!(
                    "failed to backup '{}' to '{}': {err}",
                    path.display(),
                    dest_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn merge_scaffold_update_page(
    path: &Path,
    file_name: &str,
    generated_text: &str,
) -> anyhow::Result<Option<(String, String)>> {
    let page_id = file_name.trim_end_matches(".toml");
    if page_id.is_empty() {
        return Ok(None);
    }
    let existing_text = std::fs::read_to_string(path)?;
    let existing_toml = toml::from_str::<HmiDirPageToml>(&existing_text)?;
    let generated_toml = toml::from_str::<HmiDirPageToml>(generated_text)?;

    let existing_page = map_hmi_dir_page(page_id.to_string(), 0, existing_toml)?;
    let generated_page = map_hmi_dir_page(page_id.to_string(), 0, generated_toml)?;
    let (merged, changed) = merge_scaffold_page(existing_page, generated_page);
    if !changed {
        return Ok(None);
    }
    Ok(Some((
        render_hmi_dir_page_toml(&merged),
        "merged missing scaffold signals".to_string(),
    )))
}

fn merge_scaffold_page(existing: HmiDirPage, generated: HmiDirPage) -> (HmiDirPage, bool) {
    let mut merged = existing;
    let mut changed = false;

    if merged.kind == "trend" {
        let mut seen = merged
            .signals
            .iter()
            .map(|signal| signal.to_ascii_lowercase())
            .collect::<HashSet<_>>();
        for signal in generated.signals {
            let key = signal.to_ascii_lowercase();
            if seen.insert(key) {
                merged.signals.push(signal);
                changed = true;
            }
        }
        return (merged, changed);
    }

    if merged.kind == "process" {
        if merged.svg.is_none() && generated.svg.is_some() {
            merged.svg = generated.svg;
            changed = true;
        }
        let mut seen = merged
            .bindings
            .iter()
            .map(|binding| {
                (
                    binding.source.to_ascii_lowercase(),
                    binding.selector.to_ascii_lowercase(),
                    binding.attribute.to_ascii_lowercase(),
                )
            })
            .collect::<HashSet<_>>();
        for binding in generated.bindings {
            let key = (
                binding.source.to_ascii_lowercase(),
                binding.selector.to_ascii_lowercase(),
                binding.attribute.to_ascii_lowercase(),
            );
            if seen.insert(key) {
                merged.bindings.push(binding);
                changed = true;
            }
        }
        return (merged, changed);
    }

    let mut placed = HashSet::new();
    for section in &merged.sections {
        for widget in &section.widgets {
            placed.insert(widget.bind.to_ascii_lowercase());
        }
    }

    for generated_section in generated.sections {
        let mut additions = generated_section
            .widgets
            .into_iter()
            .filter(|widget| placed.insert(widget.bind.to_ascii_lowercase()))
            .collect::<Vec<_>>();
        if additions.is_empty() {
            continue;
        }
        if let Some(existing_section) = merged.sections.iter_mut().find(|section| {
            section
                .title
                .eq_ignore_ascii_case(generated_section.title.as_str())
        }) {
            existing_section.widgets.append(&mut additions);
        } else {
            merged.sections.push(HmiDirSection {
                title: generated_section.title,
                span: generated_section.span,
                tier: generated_section.tier.clone(),
                widgets: additions,
            });
        }
        changed = true;
    }

    (merged, changed)
}
