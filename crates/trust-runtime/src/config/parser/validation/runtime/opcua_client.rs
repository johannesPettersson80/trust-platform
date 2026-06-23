fn parse_opcua_client_section(
    section: Option<OpcUaClientSection>,
) -> Result<OpcUaClientRuntimeConfig, RuntimeError> {
    let section = section.unwrap_or(OpcUaClientSection {
        enabled: Some(false),
        config_path: Some("opcua_client.toml".to_string()),
        poll_interval_ms: Some(250),
    });
    let config_path = section
        .config_path
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| "opcua_client.toml".to_string());
    let poll_interval_ms = section.poll_interval_ms.unwrap_or(250);
    if poll_interval_ms < 10 {
        return Err(RuntimeError::InvalidConfig(
            "runtime.opcua_client.poll_interval_ms must be >= 10".into(),
        ));
    }

    Ok(OpcUaClientRuntimeConfig {
        enabled: section.enabled.unwrap_or(false),
        config_path: PathBuf::from(config_path),
        poll_interval: Duration::from_millis(poll_interval_ms as i64),
    })
}
