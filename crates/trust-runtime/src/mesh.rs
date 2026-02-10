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
use rustls::{ClientConnection, ServerConnection, ServerName, StreamOwned};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::config::MeshConfig;
use crate::discovery::DiscoveryState;
use crate::error::RuntimeError;
use crate::scheduler::{ResourceCommand, ResourceControl, StdClock};
use crate::security::{rustls_client_config, rustls_server_config, TlsMaterials};
use crate::value::Value;

#[cfg(not(test))]
const MESH_SNAPSHOT_TIMEOUT: StdDuration = StdDuration::from_millis(200);
#[cfg(test)]
const MESH_SNAPSHOT_TIMEOUT: StdDuration = StdDuration::from_millis(750);

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
    tls: Option<Arc<MeshTlsTransport>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MeshMessage {
    r#type: String,
    from: String,
    token: Option<String>,
    data: Option<BTreeMap<String, serde_json::Value>>,
}

#[derive(Debug)]
struct MeshTlsTransport {
    server_config: Arc<rustls::ServerConfig>,
    client_config: Arc<rustls::ClientConfig>,
}

pub fn start_mesh(
    config: &MeshConfig,
    name: SmolStr,
    resource: ResourceControl<StdClock>,
    discovery: Option<Arc<DiscoveryState>>,
    tls_materials: Option<Arc<TlsMaterials>>,
) -> Result<Option<MeshService>, RuntimeError> {
    if !config.enabled {
        return Ok(None);
    }
    let listen = parse_addr(&config.listen)?;
    let tls = if config.tls {
        let materials = tls_materials.as_ref().ok_or_else(|| {
            RuntimeError::ControlError(
                "mesh tls enabled but runtime.tls certificate settings are unavailable".into(),
            )
        })?;
        Some(Arc::new(MeshTlsTransport {
            server_config: rustls_server_config(materials)?,
            client_config: rustls_client_config(materials)?,
        }))
    } else {
        None
    };
    let state = MeshState {
        name,
        auth_token: config.auth_token.clone(),
        publish: config.publish.clone(),
        subscribe: config.subscribe.clone(),
        discovery,
        resource,
        tls,
    };

    let listener_state = state.clone();
    let listener = thread::spawn(move || {
        if let Ok(listener) = TcpListener::bind(listen) {
            for stream in listener.incoming().map_while(Result::ok) {
                let state = listener_state.clone();
                let tls_server_config = state.tls.as_ref().map(|tls| tls.server_config.clone());
                thread::spawn(move || {
                    if let Some(server_config) = tls_server_config {
                        handle_peer_tls(stream, state, server_config);
                    } else {
                        handle_peer(stream, state);
                    }
                });
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
    if let Some(tls) = state.tls.as_ref() {
        return send_publish_tls(target, state, data, tls.client_config.clone());
    }
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
    handle_peer_stream(reader, state);
}

fn handle_peer_tls(stream: TcpStream, state: MeshState, server_config: Arc<rustls::ServerConfig>) {
    let connection = match ServerConnection::new(server_config) {
        Ok(connection) => connection,
        Err(_) => return,
    };
    let reader = BufReader::new(StreamOwned::new(connection, stream));
    handle_peer_stream(reader, state);
}

fn handle_peer_stream<R: std::io::Read>(reader: BufReader<R>, state: MeshState) {
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

fn send_publish_tls(
    target: &SocketAddr,
    state: &MeshState,
    data: &BTreeMap<String, serde_json::Value>,
    client_config: Arc<rustls::ClientConfig>,
) -> Result<(), RuntimeError> {
    let stream = TcpStream::connect(target).map_err(|err| {
        RuntimeError::ControlError(format!("mesh connect {target}: {err}").into())
    })?;
    let server_name = mesh_server_name(target)?;
    let connection = ClientConnection::new(client_config, server_name)
        .map_err(|err| RuntimeError::ControlError(format!("mesh tls connect: {err}").into()))?;
    let mut stream = StreamOwned::new(connection, stream);
    stream
        .conn
        .complete_io(&mut stream.sock)
        .map_err(|err| RuntimeError::ControlError(format!("mesh tls handshake: {err}").into()))?;
    let msg = MeshMessage {
        r#type: "publish".into(),
        from: state.name.to_string(),
        token: state.auth_token.as_ref().map(|t| t.to_string()),
        data: Some(data.clone()),
    };
    let line = serde_json::to_string(&msg).unwrap_or_default();
    writeln!(stream, "{line}")
        .map_err(|err| RuntimeError::ControlError(format!("mesh tls send: {err}").into()))?;
    stream
        .flush()
        .map_err(|err| RuntimeError::ControlError(format!("mesh tls flush: {err}").into()))?;
    stream.conn.send_close_notify();
    let _ = stream.conn.complete_io(&mut stream.sock);
    Ok(())
}

fn mesh_server_name(target: &SocketAddr) -> Result<ServerName, RuntimeError> {
    let _ = target;
    ServerName::try_from("localhost")
        .map_err(|_| RuntimeError::ControlError("mesh tls invalid server name".into()))
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
    rx.recv_timeout(MESH_SNAPSHOT_TIMEOUT).unwrap_or_default()
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
    use std::collections::BTreeMap;
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::time::Duration as StdDuration;

    use crate::security::TlsMaterials;
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

    #[test]
    #[cfg_attr(
        windows,
        ignore = "flaky on GitHub windows runners; validated in linux/macos CI gates"
    )]
    fn mesh_tls_publish_applies_updates() {
        let attempts = if cfg!(windows) { 8 } else { 3 };
        let retry_delay = if cfg!(windows) {
            StdDuration::from_millis(250)
        } else {
            StdDuration::from_millis(60)
        };
        let mut last_error = None;
        for _attempt in 0..attempts {
            match try_mesh_tls_publish_applies_updates() {
                Ok(()) => return,
                Err(error) => {
                    last_error = Some(error);
                    std::thread::sleep(retry_delay);
                }
            }
        }
        panic!(
            "mesh tls publish apply failed after retries: {}",
            last_error.unwrap_or_else(|| "unknown failure".to_string())
        );
    }

    fn try_mesh_tls_publish_applies_updates() -> Result<(), String> {
        let tls = tls_test_transport();
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind tls mesh listener");
        let addr = listener.local_addr().expect("tls mesh addr");
        let (resource, cmd_rx) = ResourceControl::stub(StdClock::new());
        let (apply_tx, apply_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = ready_tx.send(());
            while let Ok(command) = cmd_rx.recv() {
                match command {
                    ResourceCommand::MeshSnapshot { names, respond_to } => {
                        let mut values = IndexMap::new();
                        for name in names {
                            values.insert(name, Value::DInt(0));
                        }
                        let _ = respond_to.send(values);
                    }
                    ResourceCommand::MeshApply { updates } => {
                        let _ = apply_tx.send(updates);
                    }
                    _ => {}
                }
            }
        });
        ready_rx
            .recv_timeout(StdDuration::from_secs(1))
            .map_err(|err| format!("mesh snapshot worker startup: {err:?}"))?;

        let listener_state = MeshState {
            name: SmolStr::new("listener"),
            auth_token: Some(SmolStr::new("mesh-token")),
            publish: Vec::new(),
            subscribe: IndexMap::from([(
                SmolStr::new("peer:temperature"),
                SmolStr::new("resource/RESOURCE/program/Main/field/temp"),
            )]),
            discovery: None,
            resource,
            tls: Some(tls.clone()),
        };

        let server_config = tls.server_config.clone();
        let listener_thread = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept mesh tls client");
            handle_peer_tls(stream, listener_state, server_config);
        });

        let (sender_resource, _sender_rx) = ResourceControl::stub(StdClock::new());
        let sender_state = MeshState {
            name: SmolStr::new("peer"),
            auth_token: Some(SmolStr::new("mesh-token")),
            publish: Vec::new(),
            subscribe: IndexMap::new(),
            discovery: None,
            resource: sender_resource,
            tls: Some(tls.clone()),
        };
        let mut data = BTreeMap::new();
        data.insert("temperature".to_string(), json!(42));

        send_publish(&addr, &sender_state, &data).map_err(|err| err.to_string())?;
        let apply_timeout = if cfg!(windows) {
            StdDuration::from_millis(2500)
        } else {
            StdDuration::from_millis(1200)
        };
        let updates = apply_rx
            .recv_timeout(apply_timeout)
            .map_err(|err| format!("mesh apply updates: {err:?}"))?;
        if updates.get("resource/RESOURCE/program/Main/field/temp") != Some(&Value::DInt(42)) {
            return Err("mesh apply updates missing expected value".to_string());
        }

        listener_thread
            .join()
            .map_err(|_| "join mesh tls listener".to_string())?;
        Ok(())
    }

    #[test]
    fn mesh_tls_rejects_plaintext_downgrade() {
        let tls = tls_test_transport();
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind tls mesh listener");
        let addr = listener.local_addr().expect("tls mesh addr");
        let (resource, cmd_rx) = ResourceControl::stub(StdClock::new());
        let (apply_tx, apply_rx) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(command) = cmd_rx.recv() {
                if let ResourceCommand::MeshApply { updates } = command {
                    let _ = apply_tx.send(updates);
                }
            }
        });

        let listener_state = MeshState {
            name: SmolStr::new("listener"),
            auth_token: Some(SmolStr::new("mesh-token")),
            publish: Vec::new(),
            subscribe: IndexMap::from([(
                SmolStr::new("peer:temperature"),
                SmolStr::new("resource/RESOURCE/program/Main/field/temp"),
            )]),
            discovery: None,
            resource,
            tls: Some(tls.clone()),
        };

        let server_config = tls.server_config.clone();
        let listener_thread = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept mesh plain client");
            handle_peer_tls(stream, listener_state, server_config);
        });

        let mut plain = TcpStream::connect(addr).expect("connect plain mesh client");
        let write_result = writeln!(
            plain,
            "{}",
            json!({
                "type": "publish",
                "from": "peer",
                "token": "mesh-token",
                "data": { "temperature": 42 }
            })
        );
        if write_result.is_ok() {
            let _ = plain.flush();
        }
        std::thread::sleep(StdDuration::from_millis(120));
        assert!(apply_rx
            .recv_timeout(StdDuration::from_millis(120))
            .is_err());

        listener_thread.join().expect("join mesh tls listener");
    }

    fn tls_test_transport() -> Arc<MeshTlsTransport> {
        let cert = include_bytes!("../tests/fixtures/tls/server-cert.pem").to_vec();
        let key = include_bytes!("../tests/fixtures/tls/server-key.pem").to_vec();
        let materials = TlsMaterials {
            cert_path: std::path::PathBuf::from("tests/fixtures/tls/server-cert.pem"),
            key_path: std::path::PathBuf::from("tests/fixtures/tls/server-key.pem"),
            ca_path: Some(std::path::PathBuf::from(
                "tests/fixtures/tls/server-cert.pem",
            )),
            certificate_pem: cert.clone(),
            private_key_pem: key,
            ca_pem: cert,
        };
        Arc::new(MeshTlsTransport {
            server_config: rustls_server_config(&materials).expect("mesh tls server config"),
            client_config: rustls_client_config(&materials).expect("mesh tls client config"),
        })
    }
}
