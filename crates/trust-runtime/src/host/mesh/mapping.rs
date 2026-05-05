use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration as StdDuration, SystemTime, UNIX_EPOCH};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::scheduler::{ResourceCommand, ResourceControl, StdClock};
use crate::value::Value;

use super::models::MeshQosProfile;

fn mesh_snapshot_timeout() -> StdDuration {
    if cfg!(test) {
        StdDuration::from_millis(750)
    } else {
        StdDuration::from_millis(200)
    }
}

pub(crate) const DEFAULT_SITE: &str = "default-site";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MeshSnapshotError {
    Command(String),
    Timeout,
}

impl std::fmt::Display for MeshSnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Command(message) => write!(f, "mesh snapshot command failed: {message}"),
            Self::Timeout => write!(f, "mesh snapshot timed out"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MeshEnvelope {
    pub source: String,
    pub sequence: u64,
    pub published_at_ns: u64,
    pub value: serde_json::Value,
}

#[must_use]
pub(crate) fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

#[must_use]
pub(crate) fn mesh_data_key(site: &str, runtime_id: &str, remote_key: &str) -> String {
    format!("truST/{site}/{runtime_id}/mesh/data/{remote_key}")
}

#[must_use]
pub(crate) fn mesh_liveliness_expr() -> &'static str {
    "truST/*/*/svc/liveliness/*"
}

#[must_use]
pub(crate) fn mesh_qos_profile_for_key(key_expr: &str) -> MeshQosProfile {
    if key_expr.contains("/active/") {
        MeshQosProfile::Active
    } else if key_expr.contains("/cfg/") {
        MeshQosProfile::Config
    } else if key_expr.contains("/diag/") {
        MeshQosProfile::Diagnostics
    } else {
        MeshQosProfile::Fast
    }
}

#[must_use]
pub(crate) fn parse_subscribe_mapping(remote: &str) -> Option<(SmolStr, SmolStr)> {
    let (peer, key) = remote.split_once(':')?;
    let peer = peer.trim();
    let key = key.trim();
    if peer.is_empty() || key.is_empty() {
        return None;
    }
    Some((SmolStr::new(peer), SmolStr::new(key)))
}

pub(crate) fn snapshot_globals(
    resource: &ResourceControl<StdClock>,
    names: &[SmolStr],
) -> Result<IndexMap<SmolStr, Value>, MeshSnapshotError> {
    let (tx, rx) = mpsc::channel();
    resource
        .send_command(ResourceCommand::MeshSnapshot {
            names: names.to_vec(),
            respond_to: tx,
        })
        .map_err(|err| MeshSnapshotError::Command(err.to_string()))?;
    wait_snapshot(rx)
}

fn wait_snapshot(
    rx: Receiver<IndexMap<SmolStr, Value>>,
) -> Result<IndexMap<SmolStr, Value>, MeshSnapshotError> {
    rx.recv_timeout(mesh_snapshot_timeout())
        .map_err(|err| match err {
            RecvTimeoutError::Timeout => MeshSnapshotError::Timeout,
            RecvTimeoutError::Disconnected => MeshSnapshotError::Command(
                "mesh snapshot response channel disconnected".to_string(),
            ),
        })
}

#[must_use]
pub(crate) fn build_mesh_payload(source: &str, sequence: u64, value: &Value) -> Option<Vec<u8>> {
    let value_json = value_to_json(value)?;
    let payload = MeshEnvelope {
        source: source.to_string(),
        sequence,
        published_at_ns: now_ns(),
        value: value_json,
    };
    serde_json::to_vec(&payload).ok()
}

pub(crate) fn decode_mesh_payload(
    payload: &[u8],
    template: &Value,
) -> Option<(Value, Option<SmolStr>, Option<u64>)> {
    if let Ok(envelope) = serde_json::from_slice::<MeshEnvelope>(payload) {
        let value = json_to_value(&envelope.value, template)?;
        let source = Some(SmolStr::new(envelope.source));
        let sequence = Some(envelope.sequence);
        return Some((value, source, sequence));
    }
    let json = serde_json::from_slice::<serde_json::Value>(payload).ok()?;
    json_to_value(&json, template).map(|value| (value, None, None))
}

fn value_to_json(value: &Value) -> Option<serde_json::Value> {
    match value {
        Value::Bool(value) => Some(serde_json::Value::Bool(*value)),
        Value::SInt(value) => Some(serde_json::Value::Number((*value as i64).into())),
        Value::Int(value) => Some(serde_json::Value::Number((*value as i64).into())),
        Value::DInt(value) => Some(serde_json::Value::Number((*value as i64).into())),
        Value::LInt(value) => Some(serde_json::Value::Number((*value).into())),
        Value::USInt(value) => Some(serde_json::Value::Number((*value as u64).into())),
        Value::UInt(value) => Some(serde_json::Value::Number((*value as u64).into())),
        Value::UDInt(value) => Some(serde_json::Value::Number((*value as u64).into())),
        Value::ULInt(value) => Some(serde_json::Value::Number((*value).into())),
        Value::Real(value) => {
            serde_json::Number::from_f64(*value as f64).map(serde_json::Value::Number)
        }
        Value::LReal(value) => serde_json::Number::from_f64(*value).map(serde_json::Value::Number),
        Value::String(value) => Some(serde_json::Value::String(value.as_str().to_string())),
        Value::WString(value) => Some(serde_json::Value::String(value.clone())),
        _ => None,
    }
}

fn json_to_value(json: &serde_json::Value, template: &Value) -> Option<Value> {
    match (json, template) {
        (serde_json::Value::Bool(value), Value::Bool(_)) => Some(Value::Bool(*value)),
        (serde_json::Value::Number(value), Value::SInt(_)) => {
            Some(Value::SInt(value.as_i64()? as i8))
        }
        (serde_json::Value::Number(value), Value::Int(_)) => {
            Some(Value::Int(value.as_i64()? as i16))
        }
        (serde_json::Value::Number(value), Value::DInt(_)) => {
            Some(Value::DInt(value.as_i64()? as i32))
        }
        (serde_json::Value::Number(value), Value::LInt(_)) => Some(Value::LInt(value.as_i64()?)),
        (serde_json::Value::Number(value), Value::USInt(_)) => {
            Some(Value::USInt(value.as_u64()? as u8))
        }
        (serde_json::Value::Number(value), Value::UInt(_)) => {
            Some(Value::UInt(value.as_u64()? as u16))
        }
        (serde_json::Value::Number(value), Value::UDInt(_)) => {
            Some(Value::UDInt(value.as_u64()? as u32))
        }
        (serde_json::Value::Number(value), Value::ULInt(_)) => Some(Value::ULInt(value.as_u64()?)),
        (serde_json::Value::Number(value), Value::Real(_)) => {
            Some(Value::Real(value.as_f64()? as f32))
        }
        (serde_json::Value::Number(value), Value::LReal(_)) => Some(Value::LReal(value.as_f64()?)),
        (serde_json::Value::String(value), Value::String(_)) => {
            Some(Value::String(SmolStr::new(value)))
        }
        (serde_json::Value::String(value), Value::WString(_)) => {
            Some(Value::WString(value.clone()))
        }
        _ => None,
    }
}
