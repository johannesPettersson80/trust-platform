fn parse_openot_section(section: Option<OpenOtSection>) -> Result<ParsedOpenOt, RuntimeError> {
    let Some(section) = section else {
        return Ok(ParsedOpenOt {
            config: OpenOtTelemetryConfig::default(),
        });
    };

    let enabled = section.enabled.unwrap_or(false);
    let raw_path = section.path.unwrap_or_default();
    if enabled && raw_path.trim().is_empty() {
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
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(SmolStr::new);

    match source {
        OpenOtTelemetrySource::Heartbeat => {
            if producer_instance.is_some() {
                return Err(RuntimeError::InvalidConfig(
                    "runtime.openot.producer_instance is only valid when runtime.openot.source='st-fb'"
                        .into(),
                ));
            }
        }
        OpenOtTelemetrySource::StFb => {
            let Some(path) = producer_instance.as_ref() else {
                return Err(RuntimeError::InvalidConfig(
                    "runtime.openot.producer_instance is required when runtime.openot.source='st-fb'"
                        .into(),
                ));
            };
            if !is_qualified_openot_producer_path(path.as_str()) {
                return Err(RuntimeError::InvalidConfig(
                    "runtime.openot.producer_instance must be a qualified path like 'Main.Producer'"
                        .into(),
                ));
            }
        }
    }

    Ok(ParsedOpenOt {
        config: OpenOtTelemetryConfig {
            enabled,
            path: PathBuf::from(raw_path),
            capacity,
            fence_mode,
            allow_unfenced_for_proof,
            source,
            producer_instance,
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
