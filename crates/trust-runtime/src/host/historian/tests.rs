use super::*;
use crate::debug::DebugSnapshot;
use crate::memory::VariableStorage;
use crate::value::{Duration as PlcDuration, Value};

fn temp_path(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("trust-historian-{name}-{stamp}.jsonl"))
}

fn snapshot_with_values(counter: i16, temp: f64, active: bool) -> DebugSnapshot {
    let mut storage = VariableStorage::default();
    storage.set_global("Counter", Value::Int(counter));
    storage.set_global("Temp", Value::LReal(temp));
    storage.set_global("Active", Value::Bool(active));
    storage.set_global("Label", Value::String(SmolStr::new("Pump-A")));
    DebugSnapshot {
        storage,
        now: PlcDuration::from_millis(1_000),
    }
}

fn basic_config(path: PathBuf) -> HistorianConfig {
    HistorianConfig {
        enabled: true,
        sample_interval_ms: 100,
        mode: RecordingMode::All,
        include: Vec::new(),
        history_path: path,
        max_entries: 1_000,
        prometheus_enabled: true,
        prometheus_path: SmolStr::new("/metrics"),
        alerts: Vec::new(),
    }
}

#[test]
fn recording_fidelity_and_sample_interval_are_enforced() {
    let path = temp_path("fidelity");
    let service = HistorianService::new(basic_config(path.clone()), None).expect("service");
    let first = snapshot_with_values(7, 21.5, true);
    let second = snapshot_with_values(8, 25.0, false);

    let captured = service
        .capture_snapshot_at(&first, 1_000)
        .expect("capture first");
    assert!(captured >= 4);
    let skipped = service
        .capture_snapshot_at(&first, 1_050)
        .expect("capture skipped");
    assert_eq!(skipped, 0, "sample interval should suppress early capture");
    let captured_again = service
        .capture_snapshot_at(&second, 1_150)
        .expect("capture second");
    assert!(captured_again >= 4);

    let counter = service.query(Some("Counter"), None, 10);
    assert_eq!(counter.len(), 2);
    assert_eq!(counter[0].value, HistorianValue::Integer(7));
    assert_eq!(counter[1].value, HistorianValue::Integer(8));

    let active = service.query(Some("Active"), None, 10);
    assert_eq!(active[0].value, HistorianValue::Bool(true));
    assert_eq!(active[1].value, HistorianValue::Bool(false));

    let label = service.query(Some("Label"), None, 10);
    assert_eq!(label[0].value, HistorianValue::String("Pump-A".to_string()));

    let _ = std::fs::remove_file(path);
}

#[test]
fn persistent_backend_reloads_across_service_restart() {
    let path = temp_path("durability");
    {
        let service = HistorianService::new(basic_config(path.clone()), None).expect("service");
        let snapshot = snapshot_with_values(42, 10.0, true);
        service
            .capture_snapshot_at(&snapshot, 2_000)
            .expect("capture");
    }
    let restarted = HistorianService::new(basic_config(path.clone()), None).expect("restart");
    let counter = restarted.query(Some("Counter"), None, 10);
    assert_eq!(counter.len(), 1);
    assert_eq!(counter[0].value, HistorianValue::Integer(42));

    let _ = std::fs::remove_file(path);
}

#[test]
fn alert_threshold_debounce_and_file_hook_contract() {
    let history_path = temp_path("alerts-history");
    let hook_path = temp_path("alerts-hook");
    let mut config = basic_config(history_path.clone());
    config.sample_interval_ms = 1;
    config.alerts = vec![AlertRule {
        name: SmolStr::new("high_temp"),
        variable: SmolStr::new("Temp"),
        above: Some(50.0),
        below: None,
        debounce_samples: 2,
        hook: Some(SmolStr::new(hook_path.to_string_lossy())),
    }];

    let service = HistorianService::new(config, None).expect("service");

    service
        .capture_snapshot_at(&snapshot_with_values(1, 40.0, true), 1_000)
        .expect("below threshold");
    service
        .capture_snapshot_at(&snapshot_with_values(1, 60.0, true), 1_010)
        .expect("first breach");
    service
        .capture_snapshot_at(&snapshot_with_values(1, 61.0, true), 1_020)
        .expect("second breach");
    service
        .capture_snapshot_at(&snapshot_with_values(1, 45.0, true), 1_030)
        .expect("clear");

    let events = service.alerts(10);
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].state, AlertState::Triggered);
    assert_eq!(events[1].state, AlertState::Cleared);

    let hook_lines = std::fs::read_to_string(&hook_path).expect("hook file");
    let hook_count = hook_lines.lines().count();
    assert_eq!(hook_count, 2);

    let _ = std::fs::remove_file(history_path);
    let _ = std::fs::remove_file(hook_path);
}

#[test]
fn allowlist_mode_records_matching_paths_only() {
    let path = temp_path("allowlist");
    let mut config = basic_config(path.clone());
    config.mode = RecordingMode::Allowlist;
    config.include = vec![SmolStr::new("Temp"), SmolStr::new("retain.*")];
    let service = HistorianService::new(config, None).expect("service");

    let mut storage = VariableStorage::default();
    storage.set_global("Counter", Value::Int(9));
    storage.set_global("Temp", Value::LReal(5.0));
    storage.set_retain("Persist", Value::Bool(true));
    let snapshot = DebugSnapshot {
        storage,
        now: PlcDuration::from_millis(500),
    };
    service
        .capture_snapshot_at(&snapshot, 3_000)
        .expect("capture");

    assert_eq!(service.query(Some("Counter"), None, 10).len(), 0);
    assert_eq!(service.query(Some("Temp"), None, 10).len(), 1);
    assert_eq!(service.query(Some("retain.Persist"), None, 10).len(), 1);

    let _ = std::fs::remove_file(path);
}

#[test]
fn prometheus_render_includes_runtime_and_historian_metrics() {
    let runtime = RuntimeMetricsSnapshot {
        uptime_ms: 1200,
        faults: 1,
        overruns: 2,
        ..RuntimeMetricsSnapshot::default()
    };
    let body = render_prometheus(
        &runtime,
        Some(HistorianPrometheusSnapshot {
            samples_total: 10,
            series_total: 3,
            alerts_total: 4,
        }),
    );
    assert!(body.contains("trust_runtime_uptime_ms 1200"));
    assert!(body.contains("trust_runtime_faults_total 1"));
    assert!(body.contains("trust_runtime_historian_samples_total 10"));
    assert!(body.contains("trust_runtime_historian_alerts_total 4"));
}
