//! Runtime-to-runtime mesh data sharing.

#![allow(missing_docs)]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;
use std::time::Duration as StdDuration;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::config::MeshConfig;
use crate::discovery::DiscoveryState;
use crate::error::RuntimeError;
use crate::scheduler::{ResourceCommand, ResourceControl, StdClock};
use crate::value::Value;

#[derive(Debug)]
pub struct MeshService {
    // Reserved for diagnostics/status surfaces once mesh management commands are exposed.
    #[allow(dead_code)]
    listen: SocketAddr,
    // Kept until lifecycle/stop APIs are implemented for controlled thread shutdown.
    #[allow(dead_code)]
    _publisher: thread::JoinHandle<()>,
    // Kept until lifecycle/stop APIs are implemented for controlled thread shutdown.
    #[allow(dead_code)]
    _listener: thread::JoinHandle<()>,
}

#[derive(Debug, Clone)]
struct MeshState {
    name: SmolStr,
    auth_token: Option<SmolStr>,
    publish: Vec<SmolStr>,
    subscribe: IndexMap<SmolStr, SmolStr>,
    discovery: Option<Arc<DiscoveryState>>,
    resource: ResourceControl<StdClock>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MeshMessage {
    r#type: String,
    from: String,
    token: Option<String>,
    data: Option<BTreeMap<String, serde_json::Value>>,
}

pub fn start_mesh(
    config: &MeshConfig,
    name: SmolStr,
    resource: ResourceControl<StdClock>,
    discovery: Option<Arc<DiscoveryState>>,
) -> Result<Option<MeshService>, RuntimeError> {
    if !config.enabled {
        return Ok(None);
    }
    let listen = parse_addr(&config.listen)?;
    let state = MeshState {
        name,
        auth_token: config.auth_token.clone(),
        publish: config.publish.clone(),
        subscribe: config.subscribe.clone(),
        discovery,
        resource,
    };

    let listener_state = state.clone();
    let listener = thread::spawn(move || {
        if let Ok(listener) = TcpListener::bind(listen) {
            for stream in listener.incoming().map_while(Result::ok) {
                let state = listener_state.clone();
                thread::spawn(move || handle_peer(stream, state));
            }
        }
    });

    let publisher_state = state.clone();
    let publisher = thread::spawn(move || publish_loop(publisher_state));

    Ok(Some(MeshService {
        listen,
        _publisher: publisher,
        _listener: listener,
    }))
}

fn publish_loop(state: MeshState) {
    if state.publish.is_empty() {
        return;
    }
    loop {
        let snapshot = snapshot_globals(&state.resource, &state.publish);
        let data = snapshot
            .iter()
            .filter_map(|(name, value)| value_to_json(value).map(|json| (name.to_string(), json)))
            .collect::<BTreeMap<_, _>>();
        if let Some(discovery) = state.discovery.as_ref() {
            for entry in discovery.snapshot() {
                let Some(port) = entry.mesh_port else {
                    continue;
                };
                if entry.name == state.name {
                    continue;
                }
                for addr in &entry.addresses {
                    let target = SocketAddr::new(*addr, port);
                    let _ = send_publish(&target, &state, &data);
                }
            }
        }
        thread::sleep(StdDuration::from_millis(1000));
    }
}

fn send_publish(
    target: &SocketAddr,
    state: &MeshState,
    data: &BTreeMap<String, serde_json::Value>,
) -> Result<(), RuntimeError> {
    let mut stream = TcpStream::connect(target).map_err(|err| {
        RuntimeError::ControlError(format!("mesh connect {target}: {err}").into())
    })?;
    let msg = MeshMessage {
        r#type: "publish".into(),
        from: state.name.to_string(),
        token: state.auth_token.as_ref().map(|t| t.to_string()),
        data: Some(data.clone()),
    };
    let line = serde_json::to_string(&msg).unwrap_or_default();
    writeln!(stream, "{line}")
        .map_err(|err| RuntimeError::ControlError(format!("mesh send: {err}").into()))?;
    Ok(())
}

fn handle_peer(stream: TcpStream, state: MeshState) {
    let reader = BufReader::new(stream);
    for line in reader.lines().map_while(Result::ok) {
        let Ok(msg) = serde_json::from_str::<MeshMessage>(&line) else {
            continue;
        };
        if msg.r#type != "publish" {
            continue;
        }
        if let Some(expected) = state.auth_token.as_ref() {
            if msg.token.as_deref() != Some(expected.as_str()) {
                continue;
            }
        }
        let data = msg.data.unwrap_or_default();
        let updates = map_subscribe(&state, msg.from.as_str(), &data);
        if updates.is_empty() {
            continue;
        }
        let _ = state
            .resource
            .send_command(ResourceCommand::MeshApply { updates });
    }
}

fn map_subscribe(
    state: &MeshState,
    peer: &str,
    data: &BTreeMap<String, serde_json::Value>,
) -> IndexMap<SmolStr, Value> {
    let mut updates = IndexMap::new();
    let local_names = state
        .subscribe
        .iter()
        .filter_map(|(remote, local)| {
            remote
                .strip_prefix(&format!("{peer}:"))
                .map(|key| (key, local))
        })
        .collect::<Vec<_>>();
    if local_names.is_empty() {
        return updates;
    }
    let template_names = local_names
        .iter()
        .map(|(_, local)| (*local).clone())
        .collect::<Vec<_>>();
    let templates = snapshot_globals(&state.resource, &template_names);
    for (remote_key, local) in local_names {
        let Some(json) = data.get(remote_key) else {
            continue;
        };
        let Some(template) = templates.get(local) else {
            continue;
        };
        if let Some(value) = json_to_value(json, template) {
            updates.insert(local.clone(), value);
        }
    }
    updates
}

fn snapshot_globals(
    resource: &ResourceControl<StdClock>,
    names: &[SmolStr],
) -> IndexMap<SmolStr, Value> {
    let (tx, rx) = mpsc::channel();
    let _ = resource.send_command(ResourceCommand::MeshSnapshot {
        names: names.to_vec(),
        respond_to: tx,
    });
    wait_snapshot(rx)
}

fn wait_snapshot(rx: Receiver<IndexMap<SmolStr, Value>>) -> IndexMap<SmolStr, Value> {
    rx.recv_timeout(StdDuration::from_millis(200))
        .unwrap_or_default()
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

fn parse_addr(text: &SmolStr) -> Result<SocketAddr, RuntimeError> {
    text.parse::<SocketAddr>()
        .map_err(|err| RuntimeError::ControlError(format!("invalid mesh.listen: {err}").into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mesh_value_to_json_roundtrip() {
        let value = Value::Int(42);
        let json_value = value_to_json(&value).expect("int json");
        assert_eq!(json_value, json!(42));
        let roundtrip = json_to_value(&json_value, &value).expect("int roundtrip");
        assert_eq!(roundtrip, Value::Int(42));
    }

    #[test]
    fn mesh_json_type_mismatch_rejected() {
        let template = Value::Bool(false);
        let json_value = json!("not-bool");
        assert!(json_to_value(&json_value, &template).is_none());
    }
}
