fn parse_ads_section(section: Option<AdsSection>) -> Result<AdsRuntimeConfig, RuntimeError> {
    let section = section.unwrap_or(AdsSection {
        enabled: Some(false),
        config_path: Some("ads.toml".to_string()),
        worker_tick_interval_ms: Some(20),
    });
    let config_path = parse_optional_path("runtime.ads.config_path", section.config_path)?
        .unwrap_or_else(|| PathBuf::from("ads.toml"));
    let worker_tick_interval_ms = section.worker_tick_interval_ms.unwrap_or(20);

    Ok(AdsRuntimeConfig {
        enabled: section.enabled.unwrap_or(false),
        config_path,
        worker_tick_interval: parse_runtime_duration_millis(
            worker_tick_interval_ms,
            1,
            "runtime.ads.worker_tick_interval_ms",
        )?,
    })
}
