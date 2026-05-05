#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    fn parse(text: &str) -> Self {
        match text.trim().to_ascii_lowercase().as_str() {
            "error" => Self::Error,
            "warn" | "warning" => Self::Warn,
            "debug" => Self::Debug,
            "trace" => Self::Trace,
            _ => Self::Info,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }
}

#[derive(Debug, Clone)]
struct RuntimeLogger {
    level: LogLevel,
}

impl RuntimeLogger {
    fn new(level: LogLevel) -> Self {
        Self { level }
    }

    fn enabled(&self, level: LogLevel) -> bool {
        level <= self.level
    }

    fn log(&self, level: LogLevel, event: &str, data: serde_json::Value) {
        if !self.enabled(level) {
            return;
        }
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let payload = json!({
            "ts": timestamp,
            "level": level.as_str(),
            "event": event,
            "data": data,
        });
        println!("{payload}");
    }
}

fn log_runtime_event(logger: &RuntimeLogger, event: &trust_runtime::debug::RuntimeEvent) {
    match event {
        trust_runtime::debug::RuntimeEvent::TaskOverrun { name, missed, time } => {
            logger.log(
                LogLevel::Warn,
                "runtime_overrun",
                json!({
                    "event_id": "TRUST-RT-OVERRUN-001",
                    "task": name.as_str(),
                    "missed": missed,
                    "time_ms": time.as_millis(),
                }),
            );
        }
        trust_runtime::debug::RuntimeEvent::Fault { error, time } => {
            logger.log(
                LogLevel::Error,
                "runtime_fault",
                json!({
                    "event_id": "TRUST-RT-FAULT-001",
                    "error": error,
                    "time_ms": time.as_millis(),
                }),
            );
        }
        trust_runtime::debug::RuntimeEvent::SafeStateFailed { root, error, time } => {
            logger.log(
                LogLevel::Error,
                "runtime_safe_state_failed",
                json!({
                    "event_id": "TRUST-RT-SAFE-STATE-001",
                    "root": root,
                    "error": error,
                    "time_ms": time.as_millis(),
                }),
            );
        }
        trust_runtime::debug::RuntimeEvent::RetainOrphanDropped { name, time } => {
            logger.log(
                LogLevel::Warn,
                "runtime_retain_orphan_dropped",
                json!({
                    "event_id": "TRUST-RT-RETAIN-ORPHAN-001",
                    "name": name.as_str(),
                    "time_ms": time.as_millis(),
                }),
            );
        }
        trust_runtime::debug::RuntimeEvent::RetainMigrationApplied { name, detail, time } => {
            logger.log(
                LogLevel::Info,
                "runtime_retain_migration_applied",
                json!({
                    "event_id": "TRUST-RT-RETAIN-MIGRATION-001",
                    "name": name.as_str(),
                    "detail": detail,
                    "time_ms": time.as_millis(),
                }),
            );
        }
        _ => {}
    }
}

fn log_control_audit(logger: &RuntimeLogger, event: trust_runtime::control::ControlAuditEvent) {
    logger.log(
        LogLevel::Debug,
        "control_audit",
        json!({
            "event_id": event.event_id.as_str(),
            "request_id": event.request_id,
            "request_type": event.request_type.as_str(),
            "correlation_id": event.correlation_id.as_ref().map(|id| id.as_str()),
            "ok": event.ok,
            "error": event.error.as_ref().map(|err| err.as_str()),
            "auth_present": event.auth_present,
            "client": event.client.as_ref().map(|client| client.as_str()),
            "timestamp_ms": event.timestamp_ms,
        }),
    );
}
