//! Telemetry collection (opt-in, privacy-safe).

use crate::config::TelemetryConfig;
use parking_lot::Mutex;
use serde_json::json;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::warn;

#[derive(Debug, Clone, Copy)]
pub enum TelemetryEvent {
    Hover,
    Completion,
    SignatureHelp,
    Definition,
    Declaration,
    TypeDefinition,
    Implementation,
    References,
    DocumentSymbol,
    WorkspaceSymbol,
    CodeAction,
    Rename,
    PrepareRename,
    Formatting,
    RangeFormatting,
    SemanticTokensFull,
    SemanticTokensDelta,
    Diagnostic,
    WorkspaceDiagnostic,
    InlineValue,
}

impl TelemetryEvent {
    pub fn as_str(self) -> &'static str {
        match self {
            TelemetryEvent::Hover => "hover",
            TelemetryEvent::Completion => "completion",
            TelemetryEvent::SignatureHelp => "signature_help",
            TelemetryEvent::Definition => "definition",
            TelemetryEvent::Declaration => "declaration",
            TelemetryEvent::TypeDefinition => "type_definition",
            TelemetryEvent::Implementation => "implementation",
            TelemetryEvent::References => "references",
            TelemetryEvent::DocumentSymbol => "document_symbol",
            TelemetryEvent::WorkspaceSymbol => "workspace_symbol",
            TelemetryEvent::CodeAction => "code_action",
            TelemetryEvent::Rename => "rename",
            TelemetryEvent::PrepareRename => "prepare_rename",
            TelemetryEvent::Formatting => "formatting",
            TelemetryEvent::RangeFormatting => "range_formatting",
            TelemetryEvent::SemanticTokensFull => "semantic_tokens_full",
            TelemetryEvent::SemanticTokensDelta => "semantic_tokens_delta",
            TelemetryEvent::Diagnostic => "diagnostic",
            TelemetryEvent::WorkspaceDiagnostic => "workspace_diagnostic",
            TelemetryEvent::InlineValue => "inline_value",
        }
    }
}

pub struct TelemetryCollector {
    sink: Mutex<Option<TelemetrySink>>,
}

impl TelemetryCollector {
    pub fn new() -> Self {
        Self {
            sink: Mutex::new(None),
        }
    }

    pub fn record(
        &self,
        config: Option<&TelemetryConfig>,
        event: TelemetryEvent,
        duration: Duration,
    ) {
        let Some(config) = config.filter(|config| config.enabled) else {
            self.disable();
            return;
        };
        let Some(path) = config.path.clone() else {
            return;
        };

        let mut guard = self.sink.lock();
        let needs_reset = guard
            .as_ref()
            .map(|sink| sink.path != path || sink.flush_every != config.flush_every)
            .unwrap_or(true);
        if needs_reset {
            if let Some(sink) = guard.as_mut() {
                if !sink.flush() {
                    return;
                }
            }
            *guard = Some(TelemetrySink::new(path, config.flush_every));
        }
        if let Some(sink) = guard.as_mut() {
            sink.record(event.as_str(), duration);
            if sink.pending_events >= sink.flush_every && sink.flush() {
                sink.reset();
            }
        }
    }

    pub fn flush(&self) {
        if let Some(sink) = self.sink.lock().as_mut() {
            if sink.flush() {
                sink.reset();
            }
        }
    }

    fn disable(&self) {
        let mut guard = self.sink.lock();
        if let Some(sink) = guard.as_mut() {
            if !sink.flush() {
                return;
            }
        }
        *guard = None;
    }
}

#[derive(Debug, Default)]
struct TelemetryMetric {
    count: u64,
    total_ms: u64,
    min_ms: u64,
    max_ms: u64,
}

impl TelemetryMetric {
    fn record(&mut self, duration_ms: u64) {
        if self.count == 0 {
            self.min_ms = duration_ms;
            self.max_ms = duration_ms;
        } else {
            self.min_ms = self.min_ms.min(duration_ms);
            self.max_ms = self.max_ms.max(duration_ms);
        }
        self.count = self.count.saturating_add(1);
        self.total_ms = self.total_ms.saturating_add(duration_ms);
    }
}

struct TelemetrySink {
    path: PathBuf,
    metrics: HashMap<&'static str, TelemetryMetric>,
    pending_events: usize,
    flush_every: usize,
}

impl TelemetrySink {
    fn new(path: PathBuf, flush_every: usize) -> Self {
        Self {
            path,
            metrics: HashMap::new(),
            pending_events: 0,
            flush_every: flush_every.max(1),
        }
    }

    fn record(&mut self, event: &'static str, duration: Duration) {
        let duration_ms = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
        self.metrics.entry(event).or_default().record(duration_ms);
        self.pending_events = self.pending_events.saturating_add(1);
    }

    fn reset(&mut self) {
        self.metrics.clear();
        self.pending_events = 0;
    }

    fn flush(&mut self) -> bool {
        if self.metrics.is_empty() {
            return true;
        }
        if let Some(parent) = self.path.parent() {
            if let Err(err) = std::fs::create_dir_all(parent) {
                warn!(
                    "Failed to create telemetry directory {}: {err}",
                    parent.display()
                );
                return false;
            }
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let mut metrics = serde_json::Map::new();
        for (event, metric) in &self.metrics {
            metrics.insert(
                (*event).to_string(),
                json!({
                    "count": metric.count,
                    "total_ms": metric.total_ms,
                    "min_ms": metric.min_ms,
                    "max_ms": metric.max_ms,
                }),
            );
        }
        let record = json!({
            "timestamp": timestamp,
            "metrics": metrics,
        });

        let mut file = match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            Ok(file) => file,
            Err(err) => {
                warn!(
                    "Failed to open telemetry file {}: {err}",
                    self.path.display()
                );
                return false;
            }
        };
        if let Err(err) = writeln!(file, "{}", record) {
            warn!("Failed to write telemetry record: {err}");
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let dir = std::env::temp_dir().join(format!("{prefix}-{stamp}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn telemetry_writes_metrics() {
        let root = temp_dir("trustlsp-telemetry");
        let path = root.join("telemetry.jsonl");
        let config = TelemetryConfig {
            enabled: true,
            path: Some(path.clone()),
            flush_every: 1,
        };
        let telemetry = TelemetryCollector::new();
        telemetry.record(
            Some(&config),
            TelemetryEvent::Hover,
            Duration::from_millis(12),
        );
        telemetry.flush();

        let contents = fs::read_to_string(&path).expect("read telemetry");
        assert!(contents.contains("\"hover\""));

        fs::remove_dir_all(root).ok();
    }
}

#[cfg(test)]
#[path = "telemetry/contract_tests.rs"]
mod contract_tests;
