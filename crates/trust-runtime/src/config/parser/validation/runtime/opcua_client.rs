fn parse_opcua_client_section(
    section: Option<OpcUaClientSection>,
) -> Result<OpcUaClientRuntimeConfig, RuntimeError> {
    let section = section.unwrap_or(OpcUaClientSection {
        enabled: Some(false),
        config_path: Some("opcua_client.toml".to_string()),
        poll_interval_ms: Some(250),
    });
    let config_path = parse_optional_path(
        "runtime.opcua_client.config_path",
        section.config_path,
    )?
    .unwrap_or_else(|| PathBuf::from("opcua_client.toml"));
    let poll_interval_ms = section.poll_interval_ms.unwrap_or(250);

    Ok(OpcUaClientRuntimeConfig {
        enabled: section.enabled.unwrap_or(false),
        config_path,
        poll_interval: parse_runtime_duration_millis(
            poll_interval_ms,
            10,
            "runtime.opcua_client.poll_interval_ms",
        )?,
    })
}
