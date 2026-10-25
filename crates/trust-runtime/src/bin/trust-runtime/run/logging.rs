use trust_runtime_core::deployment::LogLevel;

/// The runtime's log lines on standard output, each written when its level is at most the
/// configured one, `[runtime.log] level`.
#[derive(Debug, Clone)]
struct RuntimeLogger {
    /// The level the runtime starts with.
    level: LogLevel,
    /// The runtime's settings once they exist, shared by every clone: from then on the level is
    /// their `log.level`, which `config.set` changes while the runtime runs (specification 11, the
    /// runtime configuration contract).
    settings: Arc<std::sync::OnceLock<Arc<Mutex<RuntimeSettings>>>>,
}

impl RuntimeLogger {
    fn new(level: LogLevel) -> Self {
        Self {
            level,
            settings: Arc::new(std::sync::OnceLock::new()),
        }
    }

    /// Every clone takes its level from `settings` from now on.
    fn follow(&self, settings: Arc<Mutex<RuntimeSettings>>) {
        let _ = self.settings.set(settings);
    }

    /// The configured level: the settings' `log.level`, or the starting level before the settings
    /// exist or while they cannot be read.
    fn level(&self) -> LogLevel {
        self.settings
            .get()
            .and_then(|settings| {
                let settings = settings.lock().ok()?;
                LogLevel::parse(&settings.log_level)
            })
            .unwrap_or(self.level)
    }

    fn enabled(&self, level: LogLevel) -> bool {
        level <= self.level()
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
        trust_runtime::debug::RuntimeEvent::WriteRejected {
            target,
            error,
            time,
        } => {
            logger.log(
                LogLevel::Warn,
                "write_rejected",
                json!({
                    "target": target.as_str(), "error": error, "time_ms": time.as_millis(),
                }),
            );
        }
        trust_runtime::debug::RuntimeEvent::Fault {
            fault,
            message,
            time,
        } => {
            logger.log(
                LogLevel::Error,
                "runtime_fault",
                json!({
                    "event_id": "TRUST-RT-FAULT-001",
                    "error": fault.render(message.as_ref()),
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
        trust_runtime::debug::RuntimeEvent::OnlineChangeValue { event, time } => {
            let (name, detail) = trust_runtime_services::events::carried_value_text(event);
            logger.log(
                LogLevel::Info,
                "runtime_online_change_value",
                json!({
                    "event_id": "TRUST-RT-ONLINE-CHANGE-VALUE-001",
                    "name": name,
                    "detail": detail,
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
            "details": event.details,
            "timestamp_ms": event.timestamp_ms,
        }),
    );
}
