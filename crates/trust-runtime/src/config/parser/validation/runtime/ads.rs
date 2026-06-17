fn parse_ads_section(section: Option<AdsSection>) -> Result<AdsRuntimeConfig, RuntimeError> {
    let section = section.unwrap_or(AdsSection {
        enabled: Some(false),
        config_path: Some("ads.toml".to_string()),
        worker_tick_interval_ms: Some(20),
    });
    let config_path = section
        .config_path
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| "ads.toml".to_string());
    let worker_tick_interval_ms = section.worker_tick_interval_ms.unwrap_or(20);
    if worker_tick_interval_ms == 0 {
        return Err(RuntimeError::InvalidConfig(
            "runtime.ads.worker_tick_interval_ms must be >= 1".into(),
        ));
    }

    Ok(AdsRuntimeConfig {
        enabled: section.enabled.unwrap_or(false),
        config_path: PathBuf::from(config_path),
        worker_tick_interval: Duration::from_millis(worker_tick_interval_ms as i64),
    })
}
