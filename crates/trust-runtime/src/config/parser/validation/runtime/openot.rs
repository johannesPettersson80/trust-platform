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

    Ok(ParsedOpenOt {
        config: OpenOtTelemetryConfig {
            enabled,
            path: PathBuf::from(raw_path),
            capacity,
            fence_mode,
            allow_unfenced_for_proof,
        },
    })
}
