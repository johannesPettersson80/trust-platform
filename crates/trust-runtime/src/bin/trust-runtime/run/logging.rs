use trust_runtime_core::deployment::LogLevel;
use trust_runtime_core::fault::FaultSite;
use trust_runtime_services::events::NamedPlace;

/// Where a fault that the program raised lies, from the program that runs (specification 11, the
/// fault entry): the control state's lookup, which names no place for a site of a program that a
/// restart or an online change replaced.
#[derive(Clone)]
struct FaultPlaces(Arc<dyn Fn(FaultSite) -> Option<NamedPlace> + Send + Sync>);

impl std::fmt::Debug for FaultPlaces {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("FaultPlaces")
    }
}

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
    /// Where the program's faults lie, once the control state exists, shared by every clone.
    places: Arc<std::sync::OnceLock<FaultPlaces>>,
}

impl RuntimeLogger {
    fn new(level: LogLevel) -> Self {
        Self {
            level,
            settings: Arc::new(std::sync::OnceLock::new()),
            places: Arc::new(std::sync::OnceLock::new()),
        }
    }

    /// Every clone names the place of a fault the program raised from now on.
    fn locate(&self, places: FaultPlaces) {
        let _ = self.places.set(places);
    }

    /// The place of the fault at `site`, none before the control state exists, for a fault from
    /// outside the program, or for one the lookup does not name.
    fn fault_place(&self, site: FaultSite) -> Option<NamedPlace> {
        if site == FaultSite::NONE {
            return None;
        }
        self.places.get().and_then(|places| (places.0)(site))
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
        trust_runtime::debug::RuntimeEvent::RustBudgetExceeded {
            name,
            call_ns,
            budget_ns,
            time,
        } => {
            logger.log(
                LogLevel::Warn,
                "rust_budget_exceeded",
                json!({
                    "event_id": "TRUST-RT-RUST-BUDGET-001",
                    "pou": name.as_str(),
                    "call_ns": call_ns,
                    "budget_ns": budget_ns,
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
                runtime_fault_fields(
                    fault.render(message.as_ref()),
                    time.as_millis(),
                    logger.fault_place(fault.site),
                ),
            );
        }
        trust_runtime::debug::RuntimeEvent::SafeStateApplied { outputs, time } => {
            logger.log(
                LogLevel::Warn,
                "runtime_safe_state_applied",
                json!({
                    "event_id": "TRUST-RT-SAFE-STATE-002",
                    "outputs": outputs
                        .iter()
                        .map(|(address, value)| json!({
                            "address": address.to_string(),
                            "value": value.as_str(),
                        }))
                        .collect::<Vec<_>>(),
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
        trust_runtime::debug::RuntimeEvent::ControlEndpointFileUnavailable {
            endpoint,
            error,
            time,
        } => {
            logger.log(
                LogLevel::Warn,
                "control_endpoint_file_unavailable",
                json!({
                    "event_id": "TRUST-RT-ENDPOINT-FILE-001",
                    "endpoint": endpoint.as_str(),
                    "error": error,
                    "time_ms": time.as_millis(),
                }),
            );
        }
        // The fault's trace is in the slot but not in the folder of the runtime's files.
        trust_runtime::debug::RuntimeEvent::TraceFileUnavailable { path, error, time } => {
            logger.log(
                LogLevel::Warn,
                "runtime_trace_file_unavailable",
                json!({
                    "event_id": "TRUST-RT-TRACE-FILE-001",
                    "path": path,
                    "error": error,
                    "time_ms": time.as_millis(),
                }),
            );
        }
        // The resource runs without its flight recorder (specification 11 §6.8.1).
        trust_runtime::debug::RuntimeEvent::TraceMemory { needed, free, time } => {
            logger.log(
                LogLevel::Warn,
                "runtime_trace_memory",
                json!({
                    "event_id": "TRUST-RT-TRACE-MEMORY-001",
                    "needed": needed,
                    "free": free,
                    "time_ms": time.as_millis(),
                }),
            );
        }
        _ => {}
    }
}

/// The fields of the run log's `runtime_fault` line: the fault's text and logical time, and the
/// place of a fault the program raised, `pou`, `file` and `line`, as a fault entry names it
/// (specification 11, the fault entry).
fn runtime_fault_fields(error: String, time_ms: i64, place: Option<NamedPlace>) -> serde_json::Value {
    let mut fields = json!({
        "event_id": "TRUST-RT-FAULT-001",
        "error": error,
        "time_ms": time_ms,
    });
    if let Some(place) = place {
        fields["pou"] = json!(place.pou);
        fields["file"] = json!(place.file);
        fields["line"] = json!(place.line);
    }
    fields
}

/// Logs where the runtime keeps its stable ID when that is not the project's own file
/// (specification 11, runtime discovery): kept for the user, or kept nowhere, which is a warning
/// because the ID then changes with the next start (specification 34, ARCH-07).
fn log_runtime_id_place(
    logger: &RuntimeLogger,
    identity: &trust_runtime::runtime_id::RuntimeIdentity,
) {
    match &identity.place {
        trust_runtime::runtime_id::RuntimeIdPlace::Project(_) => {}
        trust_runtime::runtime_id::RuntimeIdPlace::User(path) => logger.log(
            LogLevel::Info,
            "runtime_id_kept_for_user",
            json!({
                "runtime_id": identity.id.as_str(),
                "path": path.display().to_string(),
            }),
        ),
        trust_runtime::runtime_id::RuntimeIdPlace::NotKept(reason) => logger.log(
            LogLevel::Warn,
            "runtime_id_not_kept",
            json!({
                "runtime_id": identity.id.as_str(),
                "error": reason,
            }),
        ),
    }
}

/// How loudly an audit record is logged: HMI command records (specification 39, §12.7) are
/// always kept; other control records only when debugging.
fn audit_log_level(event: &trust_runtime::control::ControlAuditEvent) -> LogLevel {
    if event.request_type.starts_with("hmi.") {
        LogLevel::Info
    } else {
        LogLevel::Debug
    }
}

fn log_control_audit(logger: &RuntimeLogger, event: trust_runtime::control::ControlAuditEvent) {
    logger.log(
        audit_log_level(&event),
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
