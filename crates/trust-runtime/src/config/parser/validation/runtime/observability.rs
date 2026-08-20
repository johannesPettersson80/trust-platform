fn parse_observability_section(
    section: Option<ObservabilitySection>,
) -> Result<HistorianConfig, RuntimeError> {
    let observability_section = section.unwrap_or(ObservabilitySection {
        enabled: Some(false),
        sample_interval_ms: Some(1_000),
        mode: Some("all".into()),
        include: Some(Vec::new()),
        history_path: Some("history/historian.jsonl".into()),
        max_entries: Some(20_000),
        prometheus_enabled: Some(true),
        prometheus_path: Some("/metrics".into()),
        alerts: Some(Vec::new()),
    });

    let sample_interval_ms = observability_section.sample_interval_ms.unwrap_or(1_000);
    if sample_interval_ms == 0 {
        return Err(RuntimeError::InvalidConfig(
            "runtime.observability.sample_interval_ms must be >= 1".into(),
        ));
    }

    let max_entries = observability_section.max_entries.unwrap_or(20_000);
    if max_entries == 0 {
        return Err(RuntimeError::InvalidConfig(
            "runtime.observability.max_entries must be >= 1".into(),
        ));
    }

    let mode = match observability_section
        .mode
        .as_deref()
        .unwrap_or("all")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "all" => RecordingMode::All,
        "allowlist" => RecordingMode::Allowlist,
        other => {
            return Err(RuntimeError::InvalidConfig(
                format!("invalid runtime.observability.mode '{other}'").into(),
            ))
        }
    };

    let include = observability_section
        .include
        .unwrap_or_default()
        .into_iter()
        .map(|entry| {
            parse_nonempty_entry(entry, "runtime.observability.include").map(SmolStr::new)
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    for pattern in &include {
        Pattern::new(pattern.as_str()).map_err(|err| {
            RuntimeError::InvalidConfig(
                format!(
                    "runtime.observability.include invalid pattern '{}': {err}",
                    pattern
                )
                .into(),
            )
        })?;
    }
    if matches!(mode, RecordingMode::Allowlist) && include.is_empty() {
        return Err(RuntimeError::InvalidConfig(
            "runtime.observability.include must not be empty when mode='allowlist'".into(),
        ));
    }

    let history_path = parse_optional_path(
        "runtime.observability.history_path",
        observability_section.history_path,
    )?
    .unwrap_or_else(|| PathBuf::from("history/historian.jsonl"));
    let prometheus_path = observability_section
        .prometheus_path
        .map(|path| parse_nonempty_entry(path, "runtime.observability.prometheus_path"))
        .transpose()?
        .unwrap_or_else(|| "/metrics".to_string());
    if !prometheus_path.starts_with('/') {
        return Err(RuntimeError::InvalidConfig(
            "runtime.observability.prometheus_path must start with '/'".into(),
        ));
    }

    let alerts = observability_section
        .alerts
        .unwrap_or_default()
        .into_iter()
        .map(parse_alert)
        .collect::<Result<Vec<_>, RuntimeError>>()?;

    Ok(HistorianConfig {
        enabled: observability_section.enabled.unwrap_or(false),
        sample_interval_ms,
        mode,
        include,
        history_path,
        max_entries,
        prometheus_enabled: observability_section.prometheus_enabled.unwrap_or(true),
        prometheus_path: SmolStr::new(prometheus_path),
        alerts,
    })
}

fn parse_alert(alert: AlertSection) -> Result<AlertRule, RuntimeError> {
    if alert.name.trim().is_empty() {
        return Err(RuntimeError::InvalidConfig(
            "runtime.observability.alerts[].name must not be empty".into(),
        ));
    }
    if alert.variable.trim().is_empty() {
        return Err(RuntimeError::InvalidConfig(
            "runtime.observability.alerts[].variable must not be empty".into(),
        ));
    }
    if alert.above.is_none() && alert.below.is_none() {
        return Err(RuntimeError::InvalidConfig(
            "runtime.observability.alerts[] requires above and/or below".into(),
        ));
    }
    let debounce_samples = alert.debounce_samples.unwrap_or(1);
    if debounce_samples == 0 {
        return Err(RuntimeError::InvalidConfig(
            "runtime.observability.alerts[].debounce_samples must be >= 1".into(),
        ));
    }
    let hook = alert
        .hook
        .map(|value| parse_nonempty_entry(value, "runtime.observability.alerts[].hook"))
        .transpose()?
        .map(SmolStr::new);

    Ok(AlertRule {
        name: SmolStr::new(alert.name.trim()),
        variable: SmolStr::new(alert.variable.trim()),
        above: alert.above,
        below: alert.below,
        debounce_samples,
        hook,
    })
}
