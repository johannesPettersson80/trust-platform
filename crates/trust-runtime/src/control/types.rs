use crate::debug::RuntimeEvent;
use crate::io::{IoAddress, IoSnapshot};
use serde::{Deserialize, Serialize};
use serde_json::json;
use smol_str::SmolStr;

#[derive(Debug, Deserialize)]
pub(super) struct ControlRequest {
    pub(super) id: u64,
    #[serde(rename = "type")]
    pub(super) r#type: String,
    pub(super) params: Option<serde_json::Value>,
    pub(super) auth: Option<String>,
    #[serde(default, alias = "correlation_id")]
    pub(super) request_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ControlResponse {
    id: u64,
    pub(super) ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    audit_id: Option<String>,
}

impl ControlResponse {
    pub(super) fn ok(id: u64, result: serde_json::Value) -> Self {
        Self {
            id,
            ok: true,
            result: Some(result),
            error: None,
            error_code: None,
            audit_id: None,
        }
    }

    pub(super) fn error(id: u64, error: String) -> Self {
        Self {
            id,
            ok: false,
            result: None,
            error: Some(error),
            error_code: None,
            audit_id: None,
        }
    }

    pub(super) fn error_with_code(id: u64, error: String, error_code: &'static str) -> Self {
        Self {
            id,
            ok: false,
            result: None,
            error: Some(error),
            error_code: Some(error_code.to_string()),
            audit_id: None,
        }
    }

    pub(super) fn with_audit_id(mut self, audit_id: Option<SmolStr>) -> Self {
        self.audit_id = audit_id.map(|value| value.to_string());
        self
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct BreakpointsParams {
    pub(super) source: String,
    pub(super) lines: Vec<u32>,
}

#[derive(Debug, Deserialize)]
pub(super) struct BreakpointsClearIdParams {
    pub(super) file_id: u32,
}

#[derive(Debug, Deserialize)]
pub(super) struct DebugScopesParams {
    pub(super) frame_id: u32,
}

#[derive(Debug, Deserialize)]
pub(super) struct DebugVariablesParams {
    pub(super) variables_reference: u32,
}

#[derive(Debug, Deserialize)]
pub(super) struct DebugEvaluateParams {
    pub(super) expression: String,
    pub(super) frame_id: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(super) struct DebugBreakpointLocationsParams {
    pub(super) source: String,
    pub(super) line: u32,
    pub(super) end_line: Option<u32>,
    pub(super) column: Option<u32>,
    pub(super) end_column: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(super) struct HmiValuesParams {
    pub(super) ids: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct HmiTrendsParams {
    pub(super) ids: Option<Vec<String>>,
    pub(super) duration_ms: Option<u64>,
    pub(super) buckets: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct HmiAlarmsParams {
    pub(super) limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(super) struct HmiAlarmAckParams {
    pub(super) id: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct HmiWriteParams {
    #[serde(alias = "path", alias = "target")]
    pub(super) id: String,
    pub(super) value: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub(super) struct HmiDescriptorUpdateParams {
    pub(super) descriptor: crate::hmi::HmiDirDescriptor,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct HmiScaffoldResetParams {
    pub(super) mode: Option<String>,
    pub(super) style: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct HistorianQueryParams {
    pub(super) variable: Option<String>,
    pub(super) since_ms: Option<u128>,
    pub(super) limit: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct HistorianAlertsParams {
    pub(super) limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(super) struct IoWriteParams {
    pub(super) address: String,
    pub(super) value: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct IoAddressParams {
    pub(super) address: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct RestartParams {
    pub(super) mode: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct BytecodeReloadParams {
    pub(super) bytes: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct EvalParams {
    pub(super) expr: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct SetParams {
    pub(super) target: String,
    pub(super) value: String,
}

pub(super) enum VarTarget {
    Global(String),
    Retain(String),
    Instance(u32, String),
}

#[derive(Deserialize)]
pub(super) struct VarForceParams {
    pub(super) target: String,
    pub(super) value: String,
}

#[derive(Deserialize)]
pub(super) struct VarTargetParams {
    pub(super) target: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct PairClaimParams {
    pub(super) code: String,
    pub(super) role: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct PairRevokeParams {
    pub(super) id: String,
}

pub(super) trait IoSnapshotJson {
    fn into_json(self) -> serde_json::Value;
}

impl IoSnapshotJson for IoSnapshot {
    fn into_json(self) -> serde_json::Value {
        json!({
            "inputs": self.inputs.iter().map(entry_to_json).collect::<Vec<_>>(),
            "outputs": self.outputs.iter().map(entry_to_json).collect::<Vec<_>>(),
            "memory": self.memory.iter().map(entry_to_json).collect::<Vec<_>>(),
        })
    }
}

fn entry_to_json(entry: &crate::io::IoSnapshotEntry) -> serde_json::Value {
    json!({
        "name": entry.name.as_ref().map(|name| name.as_str()),
        "address": format_address(&entry.address),
        "value": format_snapshot_value(&entry.value),
    })
}

fn format_snapshot_value(value: &crate::io::IoSnapshotValue) -> serde_json::Value {
    match value {
        crate::io::IoSnapshotValue::Value(value) => json!(format!("{value:?}")),
        crate::io::IoSnapshotValue::Error(err) => json!({ "error": err }),
        crate::io::IoSnapshotValue::Unresolved => json!("unresolved"),
    }
}

fn format_address(address: &IoAddress) -> String {
    let area = match address.area {
        crate::memory::IoArea::Input => "I",
        crate::memory::IoArea::Output => "Q",
        crate::memory::IoArea::Memory => "M",
    };
    let size = match address.size {
        crate::io::IoSize::Bit => "X",
        crate::io::IoSize::Byte => "B",
        crate::io::IoSize::Word => "W",
        crate::io::IoSize::DWord => "D",
        crate::io::IoSize::LWord => "L",
    };
    if address.wildcard {
        return format!("%{area}{size}*");
    }
    if address.size == crate::io::IoSize::Bit {
        format!("%{area}{size}{}.{}", address.byte, address.bit)
    } else {
        format!("%{area}{size}{}", address.byte)
    }
}

pub(super) fn runtime_event_to_json(event: RuntimeEvent) -> serde_json::Value {
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
        RuntimeEvent::Fault { error, time } => json!({
            "type": "fault",
            "error": error,
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
            "time_ns": time.as_nanos(),
        }),
        RuntimeEvent::FeatureDisabled {
            feature,
            request_type,
            time,
        } => json!({
            "type": "feature_disabled",
            "feature": feature.as_str(),
            "request_type": request_type.as_deref(),
            "time_ns": time.as_nanos(),
        }),
    }
}
