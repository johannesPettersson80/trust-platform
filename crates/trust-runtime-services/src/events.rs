//! Runtime events: the scheduling and diagnostic events a runtime records, their JSON form in
//! `events.tail` and `faults`, and the bounds of the stores that keep them (specification 11,
//! event store).
//!
//! Scan events carry the scan's logical time. The control events `AuditDropped` and
//! `FeatureDisabled` carry the wall-clock time, which a device whose clock is not set yet does not
//! have (specification 34, TARGET-06); their `time_ns` is then `null`.

use alloc::collections::VecDeque;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use serde_json::json;
use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::fault::{FaultMessage, FaultRecord, FaultSite, NO_TASK};
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::RetainApplyEvent;

/// Runtime scheduling and diagnostic events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeEvent {
    /// A queued/forced engineering value was rejected by the installed owner.
    /// Queue acceptance is not evidence that a value reached PLC storage.
    WriteRejected {
        /// Requested destination, for operator/debugger reporting.
        target: SmolStr,
        /// The checked application error; this is not a PLC execution fault.
        error: String,
        /// Cycle-boundary logical time of the attempted application.
        time: Duration,
    },
    /// Cycle start event.
    CycleStart {
        /// Cycle counter value.
        cycle: u64,
        /// Time at cycle start.
        time: Duration,
    },
    /// Cycle end event.
    CycleEnd {
        /// Cycle counter value.
        cycle: u64,
        /// Time at cycle end.
        time: Duration,
    },
    /// Task execution start.
    TaskStart {
        /// Task name.
        name: SmolStr,
        /// Task priority (0 is highest).
        priority: u32,
        /// Time at task start.
        time: Duration,
    },
    /// Task execution end.
    TaskEnd {
        /// Task name.
        name: SmolStr,
        /// Task priority (0 is highest).
        priority: u32,
        /// Time at task end.
        time: Duration,
    },
    /// Task missed one or more periodic activations.
    TaskOverrun {
        /// Task name.
        name: SmolStr,
        /// Missed activation count.
        missed: u64,
        /// Time when the overrun was detected.
        time: Duration,
    },
    /// Resource fault event.
    Fault {
        /// The fixed fault record the engine latched (specification 34 MEM-05); `faults` and
        /// `events.tail` render its text at reply time.
        fault: FaultRecord,
        /// The hosted message the error carried, when its kind has one (specification 34
        /// MEM-05). The record's code and this shared payload render today's text at reply time.
        message: Option<FaultMessage>,
        /// Time when the fault was recorded.
        time: Duration,
    },
    /// Safe-state application failed while handling a resource fault.
    SafeStateFailed {
        /// Original fault message.
        root: String,
        /// Safe-state failure message.
        error: String,
        /// Time when the safe-state failure was recorded.
        time: Duration,
    },
    /// A retained value no longer has a matching retained global and was dropped.
    RetainOrphanDropped {
        /// Retained variable name.
        name: SmolStr,
        /// Time when the orphan was dropped.
        time: Duration,
    },
    /// A retained value was explicitly migrated to the current declared type.
    RetainMigrationApplied {
        /// Retained variable name.
        name: SmolStr,
        /// Migration detail.
        detail: String,
        /// Time when the migration was applied.
        time: Duration,
    },
    /// A control audit event could not be delivered to the configured audit sink.
    AuditDropped {
        /// Control request id associated with the dropped audit event.
        request_id: u64,
        /// Control request type associated with the dropped audit event.
        request_type: SmolStr,
        /// Send/write failure detail.
        error: String,
        /// Wall-clock time when the audit drop was observed; `None` while the clock is not set.
        time: Option<Duration>,
    },
    /// A request reached a surface that is disabled by feature/configuration.
    FeatureDisabled {
        /// Disabled feature name.
        feature: SmolStr,
        /// Optional request type that attempted to use the feature.
        request_type: Option<SmolStr>,
        /// Wall-clock time when the disabled feature request was observed; `None` while the
        /// clock is not set.
        time: Option<Duration>,
    },
}

impl RuntimeEvent {
    /// A fault event for `error` at logical `time`, latching its fixed record with no failure
    /// site and no task, for faults that no dispatched instruction raised (specification 34
    /// MEM-05).
    #[must_use]
    pub fn fault(error: &RuntimeError, time: Duration) -> Self {
        Self::fault_with_record(
            FaultRecord::latch(error, FaultSite::NONE, NO_TASK, time),
            error,
            time,
        )
    }

    /// A fault event for a record the engine already latched, with the message `error` carried
    /// (specification 34 MEM-05).
    #[must_use]
    pub fn fault_with_record(record: FaultRecord, error: &RuntimeError, time: Duration) -> Self {
        Self::Fault {
            fault: record,
            message: FaultMessage::from_error(error),
            time,
        }
    }
}

/// Newest runtime events the product event store keeps for `events.tail` and `faults`.
///
/// It is the largest limit either operation accepts.
pub const EVENT_STORE_CAPACITY: usize = 1_000;

/// Runtime events a debugger without an event listener buffers for direct draining.
pub const DEBUG_EVENT_BUFFER_CAPACITY: usize = 1_024;

/// Appends `event` to the product event store and drops the oldest events beyond
/// [`EVENT_STORE_CAPACITY`].
pub fn store_event(store: &mut VecDeque<RuntimeEvent>, event: RuntimeEvent) {
    store.push_back(event);
    while store.len() > EVENT_STORE_CAPACITY {
        store.pop_front();
    }
}

/// Appends `event` to a debugger's event buffer.
///
/// At [`DEBUG_EVENT_BUFFER_CAPACITY`] the oldest buffered event is dropped and counted in
/// `dropped`, so the reader can tell that it missed events.
pub fn buffer_event(buffer: &mut VecDeque<RuntimeEvent>, dropped: &mut u64, event: RuntimeEvent) {
    while buffer.len() >= DEBUG_EVENT_BUFFER_CAPACITY {
        buffer.pop_front();
        *dropped = dropped.saturating_add(1);
    }
    buffer.push_back(event);
}

/// The event time of a wall-clock reading in Unix milliseconds, as `AuditDropped` and
/// `FeatureDisabled` carry it: `None` while the clock is not set.
#[must_use]
pub fn wall_time(utc_ms: Option<u64>) -> Option<Duration> {
    utc_ms.map(|ms| Duration::from_millis(i64::try_from(ms).unwrap_or(i64::MAX)))
}

/// The newest `limit` events of `store`, newest first.
#[must_use]
pub fn newest_events(store: &VecDeque<RuntimeEvent>, limit: usize) -> Vec<RuntimeEvent> {
    store.iter().rev().take(limit).cloned().collect()
}

/// The newest `limit` fault events of `store`, newest first; other events do not count toward
/// `limit`.
#[must_use]
pub fn newest_faults(store: &VecDeque<RuntimeEvent>, limit: usize) -> Vec<RuntimeEvent> {
    store
        .iter()
        .rev()
        .filter(|event| matches!(event, RuntimeEvent::Fault { .. }))
        .take(limit)
        .cloned()
        .collect()
}

/// The JSON form of `event` in `events.tail` and `faults`.
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn runtime_event_to_json(event: RuntimeEvent) -> serde_json::Value {
    match event {
        RuntimeEvent::CycleStart { cycle, time } => json!({
            "type": "cycle_start",
            "cycle": cycle,
            "time_ns": time.as_nanos(),
        }),
        RuntimeEvent::CycleEnd { cycle, time } => json!({
            "type": "cycle_end",
            "cycle": cycle,
            "time_ns": time.as_nanos(),
        }),
        RuntimeEvent::TaskStart {
            name,
            priority,
            time,
        } => json!({
            "type": "task_start",
            "name": name.as_str(),
            "priority": priority,
            "time_ns": time.as_nanos(),
        }),
        RuntimeEvent::TaskEnd {
            name,
            priority,
            time,
        } => json!({
            "type": "task_end",
            "name": name.as_str(),
            "priority": priority,
            "time_ns": time.as_nanos(),
        }),
        RuntimeEvent::TaskOverrun { name, missed, time } => json!({
            "type": "task_overrun",
            "name": name.as_str(),
            "missed": missed,
            "time_ns": time.as_nanos(),
        }),
        RuntimeEvent::WriteRejected {
            target,
            error,
            time,
        } => json!({
            "type": "write_rejected",
            "target": target.as_str(),
            "error": error,
            "time_ns": time.as_nanos(),
        }),
        RuntimeEvent::Fault {
            fault,
            message,
            time,
        } => json!({
            "type": "fault",
            "error": fault.render(message.as_ref()),
            "time_ns": time.as_nanos(),
        }),
        RuntimeEvent::SafeStateFailed { root, error, time } => json!({
            "type": "safe_state_failed",
            "root": root,
            "error": error,
            "time_ns": time.as_nanos(),
        }),
        RuntimeEvent::RetainOrphanDropped { name, time } => json!({
            "type": "retain_orphan_dropped",
            "name": name.as_str(),
            "time_ns": time.as_nanos(),
        }),
        RuntimeEvent::RetainMigrationApplied { name, detail, time } => json!({
            "type": "retain_migration_applied",
            "name": name.as_str(),
            "detail": detail,
            "time_ns": time.as_nanos(),
        }),
        RuntimeEvent::AuditDropped {
            request_id,
            request_type,
            error,
            time,
        } => json!({
            "type": "audit_dropped",
            "request_id": request_id,
            "request_type": request_type.as_str(),
            "error": error,
            "time_ns": time.map(Duration::as_nanos),
        }),
        RuntimeEvent::FeatureDisabled {
            feature,
            request_type,
            time,
        } => json!({
            "type": "feature_disabled",
            "feature": feature.as_str(),
            "request_type": request_type.as_deref(),
            "time_ns": time.map(Duration::as_nanos),
        }),
    }
}

/// The runtime event of a retained value that a restart or a checkpoint dropped or migrated,
/// at logical time `time`.
#[must_use]
pub fn retain_apply_event(event: RetainApplyEvent, time: Duration) -> RuntimeEvent {
    match event {
        RetainApplyEvent::Orphan { name } => RuntimeEvent::RetainOrphanDropped { name, time },
        RetainApplyEvent::Migrated {
            name,
            before,
            after,
        } => RuntimeEvent::RetainMigrationApplied {
            name,
            detail: retain_migration_detail(&before, &after),
            time,
        },
    }
}

/// What a retained-value migration changed: the dropped and added fields of a structure, or the
/// value kinds before and after.
fn retain_migration_detail(before: &Value, after: &Value) -> String {
    if let (Value::Struct(before), Value::Struct(after)) = (before, after) {
        let mut changes = Vec::new();
        for name in before.fields().keys() {
            if !after
                .fields()
                .keys()
                .any(|after_name| after_name.eq_ignore_ascii_case(name.as_str()))
            {
                changes.push(format!("dropped field {name}"));
            }
        }
        for name in after.fields().keys() {
            if !before
                .fields()
                .keys()
                .any(|before_name| before_name.eq_ignore_ascii_case(name.as_str()))
            {
                changes.push(format!("added field {name}"));
            }
        }
        if !changes.is_empty() {
            return changes.join(", ");
        }
    }
    format!(
        "migrated retained value from {} to {}",
        retain_value_kind(before),
        retain_value_kind(after)
    )
}

fn retain_value_kind(value: &Value) -> &'static str {
    match value {
        Value::Bool(_) => "BOOL",
        Value::SInt(_) => "SINT",
        Value::Int(_) => "INT",
        Value::DInt(_) => "DINT",
        Value::LInt(_) => "LINT",
        Value::USInt(_) => "USINT",
        Value::UInt(_) => "UINT",
        Value::UDInt(_) => "UDINT",
        Value::ULInt(_) => "ULINT",
        Value::Real(_) => "REAL",
        Value::LReal(_) => "LREAL",
        Value::Byte(_) => "BYTE",
        Value::Word(_) => "WORD",
        Value::DWord(_) => "DWORD",
        Value::LWord(_) => "LWORD",
        Value::Time(_) => "TIME",
        Value::LTime(_) => "LTIME",
        Value::Date(_) => "DATE",
        Value::LDate(_) => "LDATE",
        Value::Tod(_) => "TOD",
        Value::LTod(_) => "LTOD",
        Value::Dt(_) => "DT",
        Value::Ldt(_) => "LDT",
        Value::String(_) => "STRING",
        Value::WString(_) => "WSTRING",
        Value::Char(_) => "CHAR",
        Value::WChar(_) => "WCHAR",
        Value::Array(_) => "ARRAY",
        Value::Struct(_) => "STRUCT",
        Value::Enum(_) => "ENUM",
        Value::Reference(_) => "REFERENCE",
        Value::Instance(_) => "INSTANCE",
        Value::Null => "NULL",
    }
}

#[cfg(test)]
mod contract_tests;
