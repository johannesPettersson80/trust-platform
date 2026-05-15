#[derive(Debug, Clone)]
pub struct HmiPersistenceConfig {
    pub enabled: bool,
    pub history_path: std::path::PathBuf,
    pub max_entries: usize,
}

impl Default for HmiPersistenceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            history_path: std::path::PathBuf::from("history/hmi.jsonl"),
            max_entries: 20_000,
        }
    }
}

#[derive(Debug)]
pub struct HmiPersistenceService {
    config: HmiPersistenceConfig,
    seen: std::sync::Mutex<HmiPersistenceSeen>,
}

#[derive(Debug, Default)]
struct HmiPersistenceSeen {
    trends: BTreeSet<(String, u128)>,
    alarms: BTreeSet<(String, &'static str, u128)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum HmiPersistenceRecord {
    Trend {
        widget_id: String,
        ts_ms: u64,
        value: f64,
    },
    Alarm {
        id: String,
        widget_id: String,
        path: String,
        label: String,
        event: String,
        timestamp_ms: u64,
        value: f64,
    },
}

impl HmiPersistenceService {
    pub fn new(
        config: HmiPersistenceConfig,
        bundle_root: Option<&Path>,
    ) -> Result<std::sync::Arc<Self>, RuntimeError> {
        if config.max_entries == 0 {
            return Err(RuntimeError::InvalidConfig(
                "runtime.hmi_persistence.max_entries must be >= 1".into(),
            ));
        }
        let history_path = resolve_hmi_persistence_path(config.history_path.as_path(), bundle_root);
        if let Some(parent) = history_path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| {
                RuntimeError::ControlError(format!("hmi persistence path setup failed: {err}").into())
            })?;
        }
        let mut runtime_config = config;
        runtime_config.history_path = history_path;
        let state = load_hmi_persistence_state(
            runtime_config.history_path.as_path(),
            runtime_config.max_entries,
        )?;
        Ok(std::sync::Arc::new(Self {
            config: runtime_config,
            seen: std::sync::Mutex::new(seen_from_hmi_state(&state)),
        }))
    }

    #[must_use]
    pub fn config(&self) -> &HmiPersistenceConfig {
        &self.config
    }

    pub fn load_state(&self) -> Result<HmiLiveState, RuntimeError> {
        load_hmi_persistence_state(self.config.history_path.as_path(), self.config.max_entries)
    }

    pub fn persist_state(&self, state: &HmiLiveState) -> Result<usize, RuntimeError> {
        let mut seen = self
            .seen
            .lock()
            .map_err(|_| RuntimeError::ControlError("hmi persistence unavailable".into()))?;
        let mut records = Vec::new();
        for (widget_id, samples) in &state.trend_samples {
            let mut window = samples
                .iter()
                .rev()
                .take(self.config.max_entries)
                .collect::<Vec<_>>();
            window.reverse();
            for sample in window {
                let key = (widget_id.clone(), sample.ts_ms);
                if seen.trends.contains(&key) {
                    continue;
                }
                records.push(HmiPersistenceRecord::Trend {
                    widget_id: widget_id.clone(),
                    ts_ms: hmi_persistence_u64_ms(sample.ts_ms),
                    value: sample.value,
                });
                seen.trends.insert(key);
            }
        }
        for event in &state.history {
            let key = (event.id.clone(), event.event, event.timestamp_ms);
            if seen.alarms.contains(&key) {
                continue;
            }
            records.push(HmiPersistenceRecord::Alarm {
                id: event.id.clone(),
                widget_id: event.widget_id.clone(),
                path: event.path.clone(),
                label: event.label.clone(),
                event: event.event.to_string(),
                timestamp_ms: hmi_persistence_u64_ms(event.timestamp_ms),
                value: event.value,
            });
            seen.alarms.insert(key);
        }
        append_hmi_persistence_records(self.config.history_path.as_path(), &records)?;
        Ok(records.len())
    }
}

fn load_hmi_persistence_state(
    path: &Path,
    max_entries: usize,
) -> Result<HmiLiveState, RuntimeError> {
    let mut state = HmiLiveState::default();
    if !path.is_file() {
        return Ok(state);
    }
    let file = std::fs::File::open(path).map_err(|err| {
        RuntimeError::ControlError(format!("hmi persistence open failed: {err}").into())
    })?;
    let reader = std::io::BufReader::new(file);
    use std::io::BufRead as _;
    for line in reader.lines() {
        let Ok(line) = line else {
            continue;
        };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<HmiPersistenceRecord>(&line) else {
            continue;
        };
        match record {
            HmiPersistenceRecord::Trend {
                widget_id,
                ts_ms,
                value,
            } => {
                let samples = state.trend_samples.entry(widget_id).or_default();
                let ts_ms = u128::from(ts_ms);
                samples.push_back(HmiTrendSample { ts_ms, value });
                trim_trend_samples(samples, max_entries);
                state.last_connected = true;
                state.last_timestamp_ms = state.last_timestamp_ms.max(ts_ms);
            }
            HmiPersistenceRecord::Alarm {
                id,
                widget_id,
                path,
                label,
                event,
                timestamp_ms,
                value,
            } => {
                let Some(event) = normalize_alarm_history_event(event.as_str()) else {
                    continue;
                };
                let timestamp_ms = u128::from(timestamp_ms);
                state.history.push_back(HmiAlarmHistoryRecord {
                    id,
                    widget_id,
                    path,
                    label,
                    event,
                    timestamp_ms,
                    value,
                });
                trim_alarm_history(&mut state.history, max_entries);
                state.last_connected = true;
                state.last_timestamp_ms = state.last_timestamp_ms.max(timestamp_ms);
            }
        }
    }
    Ok(state)
}

fn append_hmi_persistence_records(
    path: &Path,
    records: &[HmiPersistenceRecord],
) -> Result<(), RuntimeError> {
    if records.is_empty() {
        return Ok(());
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| {
            RuntimeError::ControlError(format!("hmi persistence write failed: {err}").into())
        })?;
    for record in records {
        let line = serde_json::to_string(record).map_err(|err| {
            RuntimeError::ControlError(
                format!("hmi persistence serialization failed: {err}").into(),
            )
        })?;
        std::io::Write::write_all(&mut file, line.as_bytes()).map_err(|err| {
            RuntimeError::ControlError(format!("hmi persistence write failed: {err}").into())
        })?;
        std::io::Write::write_all(&mut file, b"\n").map_err(|err| {
            RuntimeError::ControlError(format!("hmi persistence write failed: {err}").into())
        })?;
    }
    Ok(())
}

fn seen_from_hmi_state(state: &HmiLiveState) -> HmiPersistenceSeen {
    let mut seen = HmiPersistenceSeen::default();
    for (widget_id, samples) in &state.trend_samples {
        for sample in samples {
            seen.trends.insert((widget_id.clone(), sample.ts_ms));
        }
    }
    for event in &state.history {
        seen.alarms
            .insert((event.id.clone(), event.event, event.timestamp_ms));
    }
    seen
}

fn trim_trend_samples(samples: &mut VecDeque<HmiTrendSample>, max_entries: usize) {
    let limit = max_entries.clamp(1, TREND_HISTORY_LIMIT);
    while samples.len() > limit {
        let _ = samples.pop_front();
    }
}

fn trim_alarm_history(history: &mut VecDeque<HmiAlarmHistoryRecord>, max_entries: usize) {
    let limit = max_entries.clamp(1, ALARM_HISTORY_LIMIT);
    while history.len() > limit {
        let _ = history.pop_front();
    }
}

fn normalize_alarm_history_event(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "raised" => Some("raised"),
        "acknowledged" => Some("acknowledged"),
        "cleared" => Some("cleared"),
        _ => None,
    }
}

fn resolve_hmi_persistence_path(path: &Path, bundle_root: Option<&Path>) -> std::path::PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    match bundle_root {
        Some(root) => root.join(path),
        None => path.to_path_buf(),
    }
}

fn hmi_persistence_u64_ms(value: u128) -> u64 {
    value.min(u128::from(u64::MAX)) as u64
}
