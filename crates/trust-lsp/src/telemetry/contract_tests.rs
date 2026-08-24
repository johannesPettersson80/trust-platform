use super::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TELEMETRY_ID: AtomicU64 = AtomicU64::new(0);

struct TelemetryProject {
    root: PathBuf,
}

impl TelemetryProject {
    fn new(label: &str) -> Self {
        let id = NEXT_TELEMETRY_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "trust-lsp-telemetry-contract-{}-{label}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create telemetry contract project");
        Self { root }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn config(&self, relative: &str, flush_every: usize) -> TelemetryConfig {
        TelemetryConfig {
            enabled: true,
            path: Some(self.path(relative)),
            flush_every,
        }
    }
}

impl Drop for TelemetryProject {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).ok();
    }
}

fn json_lines(path: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(path)
        .expect("read telemetry JSONL")
        .lines()
        .map(|line| serde_json::from_str(line).expect("valid telemetry JSON line"))
        .collect()
}

#[test]
fn event_vocabulary_is_complete_and_stable() {
    let events = [
        TelemetryEvent::Hover,
        TelemetryEvent::Completion,
        TelemetryEvent::SignatureHelp,
        TelemetryEvent::Definition,
        TelemetryEvent::Declaration,
        TelemetryEvent::TypeDefinition,
        TelemetryEvent::Implementation,
        TelemetryEvent::References,
        TelemetryEvent::DocumentSymbol,
        TelemetryEvent::WorkspaceSymbol,
        TelemetryEvent::CodeAction,
        TelemetryEvent::Rename,
        TelemetryEvent::PrepareRename,
        TelemetryEvent::Formatting,
        TelemetryEvent::RangeFormatting,
        TelemetryEvent::SemanticTokensFull,
        TelemetryEvent::SemanticTokensDelta,
        TelemetryEvent::Diagnostic,
        TelemetryEvent::WorkspaceDiagnostic,
        TelemetryEvent::InlineValue,
    ];
    assert_eq!(
        events
            .into_iter()
            .map(TelemetryEvent::as_str)
            .collect::<Vec<_>>(),
        vec![
            "hover",
            "completion",
            "signature_help",
            "definition",
            "declaration",
            "type_definition",
            "implementation",
            "references",
            "document_symbol",
            "workspace_symbol",
            "code_action",
            "rename",
            "prepare_rename",
            "formatting",
            "range_formatting",
            "semantic_tokens_full",
            "semantic_tokens_delta",
            "diagnostic",
            "workspace_diagnostic",
            "inline_value",
        ]
    );
}

#[test]
fn first_metric_sample_sets_all_aggregates() {
    let mut metric = TelemetryMetric::default();
    metric.record(12);
    assert_eq!(metric.count, 1);
    assert_eq!(metric.total_ms, 12);
    assert_eq!(metric.min_ms, 12);
    assert_eq!(metric.max_ms, 12);
}

#[test]
fn metric_aggregates_count_total_minimum_and_maximum() {
    let mut metric = TelemetryMetric::default();
    for duration in [12, 3, 40, 8] {
        metric.record(duration);
    }
    assert_eq!(metric.count, 4);
    assert_eq!(metric.total_ms, 63);
    assert_eq!(metric.min_ms, 3);
    assert_eq!(metric.max_ms, 40);
}

#[test]
fn metric_count_and_total_saturate() {
    let mut metric = TelemetryMetric {
        count: u64::MAX,
        total_ms: u64::MAX,
        min_ms: 1,
        max_ms: 2,
    };
    metric.record(3);
    assert_eq!(metric.count, u64::MAX);
    assert_eq!(metric.total_ms, u64::MAX);
    assert_eq!(metric.min_ms, 1);
    assert_eq!(metric.max_ms, 3);
}

#[test]
fn duration_conversion_clamps_instead_of_wrapping() {
    let mut sink = TelemetrySink::new(PathBuf::from("unused.jsonl"), 10);
    let duration = Duration::from_secs(u64::MAX / 1000 + 1);
    sink.record("hover", duration);
    let metric = sink.metrics.get("hover").expect("hover metric");
    assert_eq!(metric.total_ms, u64::MAX);
    assert_eq!(metric.min_ms, u64::MAX);
    assert_eq!(metric.max_ms, u64::MAX);
}

#[test]
fn zero_duration_is_recorded_exactly() {
    let mut metric = TelemetryMetric::default();
    metric.record(0);
    assert_eq!(metric.count, 1);
    assert_eq!(metric.total_ms, 0);
    assert_eq!(metric.min_ms, 0);
    assert_eq!(metric.max_ms, 0);
}

#[test]
fn disabled_collector_does_not_create_sink_or_file() {
    let project = TelemetryProject::new("disabled");
    let path = project.path("events.jsonl");
    let config = TelemetryConfig {
        enabled: false,
        path: Some(path.clone()),
        flush_every: 1,
    };
    let collector = TelemetryCollector::new();
    collector.record(
        Some(&config),
        TelemetryEvent::Hover,
        Duration::from_millis(1),
    );
    assert!(collector.sink.lock().is_none());
    assert!(!path.exists());
}

#[test]
fn missing_configuration_disables_existing_sink_after_flush() {
    let project = TelemetryProject::new("missing-config");
    let config = project.config("events.jsonl", 10);
    let collector = TelemetryCollector::new();
    collector.record(
        Some(&config),
        TelemetryEvent::Hover,
        Duration::from_millis(7),
    );
    collector.record(None, TelemetryEvent::Completion, Duration::from_millis(2));

    assert!(collector.sink.lock().is_none());
    let lines = json_lines(&project.path("events.jsonl"));
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["metrics"]["hover"]["count"], 1);
    assert!(lines[0]["metrics"].get("completion").is_none());
}

#[test]
fn empty_explicit_flush_creates_no_record() {
    let project = TelemetryProject::new("empty-flush");
    let collector = TelemetryCollector::new();
    collector.flush();
    assert!(!project.path("events.jsonl").exists());
}

#[test]
fn threshold_flushes_one_aggregate_record_and_resets() {
    let project = TelemetryProject::new("threshold");
    let config = project.config("events.jsonl", 3);
    let collector = TelemetryCollector::new();
    for duration in [5, 9, 2] {
        collector.record(
            Some(&config),
            TelemetryEvent::Hover,
            Duration::from_millis(duration),
        );
    }

    let lines = json_lines(&project.path("events.jsonl"));
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["metrics"]["hover"]["count"], 3);
    assert_eq!(lines[0]["metrics"]["hover"]["total_ms"], 16);
    assert_eq!(lines[0]["metrics"]["hover"]["min_ms"], 2);
    assert_eq!(lines[0]["metrics"]["hover"]["max_ms"], 9);
    let guard = collector.sink.lock();
    let sink = guard.as_ref().expect("active sink");
    assert_eq!(sink.pending_events, 0);
    assert!(sink.metrics.is_empty());
}

#[test]
fn zero_threshold_is_normalized_to_one() {
    let project = TelemetryProject::new("zero-threshold");
    let config = project.config("events.jsonl", 0);
    let collector = TelemetryCollector::new();
    collector.record(
        Some(&config),
        TelemetryEvent::Hover,
        Duration::from_millis(1),
    );
    assert_eq!(json_lines(&project.path("events.jsonl")).len(), 1);
}

#[test]
fn explicit_flush_appends_one_record_and_resets() {
    let project = TelemetryProject::new("explicit");
    let config = project.config("events.jsonl", 10);
    let collector = TelemetryCollector::new();
    collector.record(
        Some(&config),
        TelemetryEvent::Completion,
        Duration::from_millis(4),
    );
    collector.flush();
    collector.flush();

    let lines = json_lines(&project.path("events.jsonl"));
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["metrics"]["completion"]["count"], 1);
}

#[test]
fn independent_event_identities_have_independent_metrics() {
    let project = TelemetryProject::new("independent-events");
    let config = project.config("events.jsonl", 10);
    let collector = TelemetryCollector::new();
    collector.record(
        Some(&config),
        TelemetryEvent::Hover,
        Duration::from_millis(2),
    );
    collector.record(
        Some(&config),
        TelemetryEvent::Completion,
        Duration::from_millis(8),
    );
    collector.flush();

    let lines = json_lines(&project.path("events.jsonl"));
    assert_eq!(lines[0]["metrics"]["hover"]["total_ms"], 2);
    assert_eq!(lines[0]["metrics"]["completion"]["total_ms"], 8);
}

#[test]
fn successful_path_change_flushes_old_sink_before_switching() {
    let project = TelemetryProject::new("path-change");
    let first = project.config("first.jsonl", 10);
    let second = project.config("second.jsonl", 10);
    let collector = TelemetryCollector::new();
    collector.record(
        Some(&first),
        TelemetryEvent::Hover,
        Duration::from_millis(2),
    );
    collector.record(
        Some(&second),
        TelemetryEvent::Completion,
        Duration::from_millis(3),
    );
    collector.flush();

    assert_eq!(json_lines(&project.path("first.jsonl")).len(), 1);
    assert_eq!(json_lines(&project.path("second.jsonl")).len(), 1);
}

#[test]
fn threshold_change_flushes_old_aggregate_before_reconfiguration() {
    let project = TelemetryProject::new("threshold-change");
    let first = project.config("events.jsonl", 10);
    let second = project.config("events.jsonl", 5);
    let collector = TelemetryCollector::new();
    collector.record(
        Some(&first),
        TelemetryEvent::Hover,
        Duration::from_millis(2),
    );
    collector.record(
        Some(&second),
        TelemetryEvent::Completion,
        Duration::from_millis(3),
    );
    collector.flush();

    let lines = json_lines(&project.path("events.jsonl"));
    assert_eq!(lines.len(), 2);
    assert!(lines[0]["metrics"].get("hover").is_some());
    assert!(lines[1]["metrics"].get("completion").is_some());
}

#[test]
fn successful_flush_appends_without_overwriting_prior_records() {
    let project = TelemetryProject::new("append");
    let config = project.config("events.jsonl", 1);
    let collector = TelemetryCollector::new();
    collector.record(
        Some(&config),
        TelemetryEvent::Hover,
        Duration::from_millis(1),
    );
    collector.record(
        Some(&config),
        TelemetryEvent::Completion,
        Duration::from_millis(2),
    );
    let lines = json_lines(&project.path("events.jsonl"));
    assert_eq!(lines.len(), 2);
    assert!(lines[0]["metrics"].get("hover").is_some());
    assert!(lines[1]["metrics"].get("completion").is_some());
}

#[test]
fn serialized_record_contains_only_aggregate_schema() {
    let project = TelemetryProject::new("privacy-schema");
    let config = project.config("events.jsonl", 1);
    let collector = TelemetryCollector::new();
    collector.record(
        Some(&config),
        TelemetryEvent::Diagnostic,
        Duration::from_millis(6),
    );
    let lines = json_lines(&project.path("events.jsonl"));
    let object = lines[0].as_object().expect("record object");
    let mut record_keys = object.keys().map(String::as_str).collect::<Vec<_>>();
    record_keys.sort_unstable();
    assert_eq!(record_keys, vec!["metrics", "timestamp"]);
    let metric = lines[0]["metrics"]["diagnostic"]
        .as_object()
        .expect("metric object");
    let mut metric_keys = metric.keys().map(String::as_str).collect::<Vec<_>>();
    metric_keys.sort_unstable();
    assert_eq!(metric_keys, vec!["count", "max_ms", "min_ms", "total_ms"]);
    let serialized = fs::read_to_string(project.path("events.jsonl")).unwrap();
    for forbidden in [
        "source_text",
        "workspace_path",
        "control_auth_token",
        "diagnostic_message",
        "symbol_name",
    ] {
        assert!(!serialized.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn failed_flush_retains_aggregate_for_retry() {
    let project = TelemetryProject::new("failed-flush");
    fs::write(project.path("blocker"), "not a directory").expect("write blocker");
    let config = project.config("blocker/events.jsonl", 10);
    let collector = TelemetryCollector::new();
    collector.record(
        Some(&config),
        TelemetryEvent::Hover,
        Duration::from_millis(2),
    );
    collector.flush();

    let guard = collector.sink.lock();
    let sink = guard.as_ref().expect("failed sink retained");
    assert_eq!(sink.pending_events, 1);
    assert_eq!(
        sink.metrics.get("hover").map(|metric| metric.count),
        Some(1)
    );
}

#[test]
fn failed_old_sink_flush_prevents_silent_path_replacement() {
    let project = TelemetryProject::new("failed-path-change");
    fs::write(project.path("blocker"), "not a directory").expect("write blocker");
    let first = project.config("blocker/events.jsonl", 10);
    let second = project.config("healthy/events.jsonl", 10);
    let collector = TelemetryCollector::new();
    collector.record(
        Some(&first),
        TelemetryEvent::Hover,
        Duration::from_millis(2),
    );
    collector.record(
        Some(&second),
        TelemetryEvent::Completion,
        Duration::from_millis(3),
    );

    let guard = collector.sink.lock();
    let sink = guard.as_ref().expect("failed old sink retained");
    assert_eq!(sink.path, project.path("blocker/events.jsonl"));
    assert_eq!(sink.pending_events, 1);
    assert!(sink.metrics.contains_key("hover"));
    assert!(!project.path("healthy/events.jsonl").exists());
}

#[test]
fn failed_disable_flush_keeps_sink_observable_for_retry() {
    let project = TelemetryProject::new("failed-disable");
    fs::write(project.path("blocker"), "not a directory").expect("write blocker");
    let config = project.config("blocker/events.jsonl", 10);
    let collector = TelemetryCollector::new();
    collector.record(
        Some(&config),
        TelemetryEvent::Hover,
        Duration::from_millis(2),
    );
    collector.record(None, TelemetryEvent::Completion, Duration::from_millis(3));

    let guard = collector.sink.lock();
    let sink = guard.as_ref().expect("failed sink retained");
    assert_eq!(sink.pending_events, 1);
    assert!(sink.metrics.contains_key("hover"));
}
