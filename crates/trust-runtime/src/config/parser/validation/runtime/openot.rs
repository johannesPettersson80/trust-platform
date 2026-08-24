fn parse_openot_section(section: Option<OpenOtSection>) -> Result<ParsedOpenOt, RuntimeError> {
    let Some(section) = section else {
        return Ok(ParsedOpenOt {
            config: OpenOtTelemetryConfig::default(),
        });
    };

    let enabled = section.enabled.unwrap_or(false);
    let path = parse_optional_path("runtime.openot.path", section.path)?;
    if enabled && path.is_none() {
        return Err(RuntimeError::InvalidConfig(
            "runtime.openot.path must not be empty when runtime.openot.enabled=true".into(),
        ));
    }

    let capacity = section.capacity.unwrap_or(4096);
    if capacity == 0 {
        return Err(RuntimeError::InvalidConfig(
            "runtime.openot.capacity must be >= 1".into(),
        ));
    }

    let fence_mode = match section.fence_mode.as_deref() {
        Some(value) => OpenOtTelemetryFenceMode::parse(value)?,
        None => OpenOtTelemetryFenceMode::Fenced,
    };
    let allow_unfenced_for_proof = section.allow_unfenced_for_proof.unwrap_or(false);
    if fence_mode == OpenOtTelemetryFenceMode::Unfenced && !allow_unfenced_for_proof {
        return Err(RuntimeError::InvalidConfig(
            "runtime.openot.fence_mode='unfenced' requires runtime.openot.allow_unfenced_for_proof=true"
                .into(),
        ));
    }

    let source = match section.source.as_deref() {
        Some(value) => OpenOtTelemetrySource::parse(value)?,
        None => OpenOtTelemetrySource::Heartbeat,
    };
    let producer_instance = section
        .producer_instance
        .map(|value| parse_nonempty_entry(value, "runtime.openot.producer_instance"))
        .transpose()?
        .map(SmolStr::new);
    let configured_instances = section.producer_instances.unwrap_or_default();
    let producer_instances = configured_instances
        .into_iter()
        .map(|value| {
            parse_nonempty_entry(value, "runtime.openot.producer_instances").map(SmolStr::new)
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;

    if producer_instance.is_some() && !producer_instances.is_empty() {
        return Err(RuntimeError::InvalidConfig(
            "runtime.openot.producer_instance and runtime.openot.producer_instances are aliases; set only one"
                .into(),
        ));
    }
    let normalized_producer_instances = if producer_instances.is_empty() {
        producer_instance.iter().cloned().collect::<Vec<_>>()
    } else {
        producer_instances
    };
    let mut seen_instances = std::collections::BTreeSet::<String>::new();
    for path in &normalized_producer_instances {
        if !seen_instances.insert(path.to_string()) {
            return Err(RuntimeError::InvalidConfig(
                format!("runtime.openot.producer_instances contains duplicate path '{path}'")
                    .into(),
            ));
        }
    }

    match source {
        OpenOtTelemetrySource::Heartbeat => {
            if !normalized_producer_instances.is_empty() {
                return Err(RuntimeError::InvalidConfig(
                    "runtime.openot.producer_instance(s) are only valid when runtime.openot.source='st-fb'"
                        .into(),
                ));
            }
        }
        OpenOtTelemetrySource::StFb => {
            if normalized_producer_instances.is_empty() {
                return Err(RuntimeError::InvalidConfig(
                    "runtime.openot.producer_instance or runtime.openot.producer_instances is required when runtime.openot.source='st-fb'"
                        .into(),
                ));
            }
            for path in &normalized_producer_instances {
                if !is_qualified_openot_producer_path(path.as_str()) {
                    return Err(RuntimeError::InvalidConfig(
                        "runtime.openot.producer_instance(s) must be qualified paths like 'Main.Producer'"
                            .into(),
                    ));
                }
            }
        }
    }

    Ok(ParsedOpenOt {
        config: OpenOtTelemetryConfig {
            enabled,
            path: path.unwrap_or_default(),
            capacity,
            fence_mode,
            allow_unfenced_for_proof,
            source,
            producer_instance,
            producer_instances: normalized_producer_instances,
        },
    })
}

fn is_qualified_openot_producer_path(path: &str) -> bool {
    let mut parts = path.split('.');
    let Some(program) = parts.next() else {
        return false;
    };
    let Some(instance) = parts.next() else {
        return false;
    };
    parts.next().is_none()
        && !program.trim().is_empty()
        && !instance.trim().is_empty()
        && program == program.trim()
        && instance == instance.trim()
}
