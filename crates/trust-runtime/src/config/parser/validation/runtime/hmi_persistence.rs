fn parse_hmi_persistence_section(
    section: Option<HmiPersistenceSection>,
) -> Result<HmiPersistenceConfig, RuntimeError> {
    let section = section.unwrap_or(HmiPersistenceSection {
        enabled: Some(false),
        history_path: Some("history/hmi.jsonl".to_string()),
        max_entries: Some(20_000),
    });
    let max_entries = section.max_entries.unwrap_or(20_000);
    if max_entries == 0 {
        return Err(RuntimeError::InvalidConfig(
            "runtime.hmi_persistence.max_entries must be >= 1".into(),
        ));
    }
    let history_path = section
        .history_path
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| "history/hmi.jsonl".to_string());
    Ok(HmiPersistenceConfig {
        enabled: section.enabled.unwrap_or(false),
        history_path: PathBuf::from(history_path),
        max_entries,
    })
}
