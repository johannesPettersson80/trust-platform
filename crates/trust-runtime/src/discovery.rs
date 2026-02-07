//! Local discovery (mDNS) for runtimes.

#![allow(missing_docs)]

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::thread;

use indexmap::IndexMap;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use smol_str::SmolStr;

use crate::config::DiscoveryConfig;
use crate::control::ControlEndpoint;
use crate::error::RuntimeError;

const SERVICE_TYPE: &str = "_trust._plc._tcp.local.";

#[derive(Debug, Clone)]
pub struct DiscoveryEntry {
    pub id: SmolStr,
    pub name: SmolStr,
    pub addresses: Vec<IpAddr>,
    pub web_port: Option<u16>,
    pub mesh_port: Option<u16>,
    pub control: Option<SmolStr>,
}

#[derive(Debug, Default)]
pub struct DiscoveryState {
    entries: Arc<Mutex<IndexMap<SmolStr, DiscoveryEntry>>>,
}

impl DiscoveryState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(IndexMap::new())),
        }
    }

    pub fn snapshot(&self) -> Vec<DiscoveryEntry> {
        self.entries
            .lock()
            .map(|guard| guard.values().cloned().collect())
            .unwrap_or_default()
    }
}

pub struct DiscoveryHandle {
    // Owned daemon keeps mDNS browse/advertise resources alive for handle lifetime.
    #[allow(dead_code)]
    daemon: ServiceDaemon,
    state: Arc<DiscoveryState>,
}

impl DiscoveryHandle {
    #[must_use]
    pub fn state(&self) -> Arc<DiscoveryState> {
        self.state.clone()
    }
}

pub fn start_discovery(
    config: &DiscoveryConfig,
    runtime_name: &SmolStr,
    control_endpoint: &ControlEndpoint,
    web_listen: Option<&str>,
    mesh_listen: Option<&str>,
) -> Result<DiscoveryHandle, RuntimeError> {
    if !config.enabled {
        return Ok(DiscoveryHandle {
            daemon: ServiceDaemon::new().map_err(|err| {
                RuntimeError::ControlError(format!("discovery disabled: {err}").into())
            })?,
            state: Arc::new(DiscoveryState::new()),
        });
    }

    let daemon = ServiceDaemon::new()
        .map_err(|err| RuntimeError::ControlError(format!("mdns start: {err}").into()))?;
    let state = Arc::new(DiscoveryState::new());
    let instance_name = format!("{}-{}", config.service_name, runtime_name);
    let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "trust".into());
    let host = format!("{hostname}.local.");
    let port = parse_port(web_listen).unwrap_or(8080);
    let mesh_port = parse_port(mesh_listen);

    let id = format!("{}-{}", runtime_name, std::process::id());
    let mut props = HashMap::new();
    props.insert("id".to_string(), id.clone());
    props.insert("name".to_string(), runtime_name.to_string());
    props.insert("web_port".to_string(), port.to_string());
    if let Some(mesh_port) = mesh_port {
        props.insert("mesh_port".to_string(), mesh_port.to_string());
    }
    props.insert("control".to_string(), format_endpoint(control_endpoint));

    let info = ServiceInfo::new(SERVICE_TYPE, &instance_name, &host, (), port, props)
        .map_err(|err| RuntimeError::ControlError(format!("mdns info: {err}").into()))?;
    if config.advertise {
        let _ = daemon.register(info);
    }

    let receiver = daemon
        .browse(SERVICE_TYPE)
        .map_err(|err| RuntimeError::ControlError(format!("mdns browse: {err}").into()))?;
    let state_clone = state.clone();
    thread::spawn(move || {
        for event in receiver {
            match event {
                ServiceEvent::ServiceResolved(info) => {
                    let entry = info_to_entry(&info);
                    if let Ok(mut guard) = state_clone.entries.lock() {
                        guard.insert(entry.id.clone(), entry);
                    }
                }
                ServiceEvent::ServiceRemoved(id, _) => {
                    if let Ok(mut guard) = state_clone.entries.lock() {
                        guard.retain(|_, v| v.name.as_str() != id);
                    }
                }
                _ => {}
            }
        }
    });

    Ok(DiscoveryHandle { daemon, state })
}

fn info_to_entry(info: &ServiceInfo) -> DiscoveryEntry {
    let props = info.get_properties();
    let id = props
        .get("id")
        .map(|value| value.val_str().to_string())
        .unwrap_or_else(|| info.get_fullname().to_string());
    let name = props
        .get("name")
        .map(|value| value.val_str().to_string())
        .unwrap_or_else(|| info.get_fullname().to_string());
    let web_port = props
        .get("web_port")
        .and_then(|value| value.val_str().parse::<u16>().ok());
    let mesh_port = props
        .get("mesh_port")
        .and_then(|value| value.val_str().parse::<u16>().ok());
    let control = props
        .get("control")
        .map(|value| value.val_str().to_string());
    let addresses = info.get_addresses().iter().copied().collect::<Vec<_>>();
    DiscoveryEntry {
        id: SmolStr::new(id),
        name: SmolStr::new(name),
        addresses,
        web_port,
        mesh_port,
        control: control.map(SmolStr::new),
    }
}

fn parse_port(listen: Option<&str>) -> Option<u16> {
    let listen = listen?;
    let port = listen
        .rsplit(':')
        .next()
        .and_then(|value| value.parse::<u16>().ok());
    port
}

fn format_endpoint(endpoint: &ControlEndpoint) -> String {
    match endpoint {
        ControlEndpoint::Tcp(addr) => format!("tcp://{addr}"),
        #[cfg(unix)]
        ControlEndpoint::Unix(path) => format!("unix://{}", path.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_entry_maps_properties() {
        let mut props = std::collections::HashMap::new();
        props.insert("id".to_string(), "id-1".to_string());
        props.insert("name".to_string(), "runtime-a".to_string());
        props.insert("web_port".to_string(), "8080".to_string());
        props.insert("mesh_port".to_string(), "5200".to_string());
        props.insert("control".to_string(), "unix:///tmp/test.sock".to_string());
        let info = ServiceInfo::new(
            SERVICE_TYPE,
            "trust-runtime-a",
            "host.local.",
            (),
            8080,
            props,
        )
        .unwrap();
        let entry = info_to_entry(&info);
        assert_eq!(entry.id.as_str(), "id-1");
        assert_eq!(entry.name.as_str(), "runtime-a");
        assert_eq!(entry.web_port, Some(8080));
        assert_eq!(entry.mesh_port, Some(5200));
        assert_eq!(entry.control.as_deref(), Some("unix:///tmp/test.sock"));
    }
}
