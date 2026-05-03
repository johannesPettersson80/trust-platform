//! Local discovery (mDNS) for runtimes.

#![allow(missing_docs)]

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use indexmap::IndexMap;
use mdns_sd::{ResolvedService, ServiceDaemon, ServiceEvent, ServiceInfo};
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
    pub web_tls: bool,
    pub mesh_port: Option<u16>,
    pub control: Option<SmolStr>,
    pub host_group: Option<SmolStr>,
    pub last_seen_ns: u64,
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

    pub fn replace_entries(&self, entries: Vec<DiscoveryEntry>) {
        if let Ok(mut guard) = self.entries.lock() {
            guard.clear();
            let now = now_ns();
            for mut entry in entries {
                if entry.last_seen_ns == 0 {
                    entry.last_seen_ns = now;
                }
                guard.insert(entry.id.clone(), entry);
            }
        }
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
    web_tls: bool,
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
    props.insert("web_tls".to_string(), web_tls.to_string());
    if let Some(mesh_port) = mesh_port {
        props.insert("mesh_port".to_string(), mesh_port.to_string());
    }
    props.insert("control".to_string(), format_endpoint(control_endpoint));
    if let Some(host_group) = config.host_group.as_deref() {
        let host_group = host_group.trim();
        if !host_group.is_empty() {
            props.insert("host_group".to_string(), host_group.to_string());
        }
    }

    let info = build_service_info(&instance_name, &host, port, props)?;
    if config.advertise {
        daemon
            .register(info)
            .map_err(|err| RuntimeError::ControlError(format!("mdns register: {err}").into()))?;
    }

    let receiver = daemon
        .browse(SERVICE_TYPE)
        .map_err(|err| RuntimeError::ControlError(format!("mdns browse: {err}").into()))?;
    let state_clone = state.clone();
    thread::spawn(move || {
        for event in receiver {
            match event {
                ServiceEvent::ServiceResolved(info) => {
                    let entry = resolved_to_entry(info.as_ref());
                    if let Ok(mut guard) = state_clone.entries.lock() {
                        guard.insert(entry.id.clone(), entry);
                    }
                }
                ServiceEvent::ServiceRemoved(_, fullname) => {
                    if let Ok(mut guard) = state_clone.entries.lock() {
                        guard.retain(|_, entry| {
                            !service_removed_matches_entry(fullname.as_str(), entry)
                        });
                    }
                }
                _ => {}
            }
        }
    });

    Ok(DiscoveryHandle { daemon, state })
}

fn resolved_to_entry(info: &ResolvedService) -> DiscoveryEntry {
    let id = info
        .get_property_val_str("id")
        .map(str::to_string)
        .unwrap_or_else(|| info.get_fullname().to_string());
    let name = info
        .get_property_val_str("name")
        .map(str::to_string)
        .unwrap_or_else(|| info.get_fullname().to_string());
    let web_port = info
        .get_property_val_str("web_port")
        .and_then(|value| value.parse::<u16>().ok());
    let web_tls = info
        .get_property_val_str("web_tls")
        .map(|value| value.to_ascii_lowercase())
        .map(|value| matches!(value.as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);
    let mesh_port = info
        .get_property_val_str("mesh_port")
        .and_then(|value| value.parse::<u16>().ok());
    let control = info.get_property_val_str("control").map(str::to_string);
    let host_group = info
        .get_property_val_str("host_group")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let addresses = info
        .get_addresses()
        .iter()
        .map(|value| value.to_ip_addr())
        .collect::<Vec<_>>();
    DiscoveryEntry {
        id: SmolStr::new(id),
        name: SmolStr::new(name),
        addresses,
        web_port,
        web_tls,
        mesh_port,
        control: control.map(SmolStr::new),
        host_group: host_group.map(SmolStr::new),
        last_seen_ns: now_ns(),
    }
}

fn service_removed_matches_entry(fullname: &str, entry: &DiscoveryEntry) -> bool {
    let instance = service_instance_name(fullname);
    let runtime_name = entry.name.as_str();
    let runtime_name_suffix = format!("-{runtime_name}");
    fullname == entry.id.as_str()
        || fullname == runtime_name
        || instance == entry.id.as_str()
        || instance == runtime_name
        || instance.ends_with(runtime_name_suffix.as_str())
}

fn service_instance_name(fullname: &str) -> &str {
    fullname.split("._").next().unwrap_or(fullname)
}

#[cfg(test)]
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
    let web_tls = props
        .get("web_tls")
        .map(|value| value.val_str().to_ascii_lowercase())
        .map(|value| matches!(value.as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);
    let mesh_port = props
        .get("mesh_port")
        .and_then(|value| value.val_str().parse::<u16>().ok());
    let control = props
        .get("control")
        .map(|value| value.val_str().to_string());
    let host_group = props
        .get("host_group")
        .map(|value| value.val_str().trim().to_string())
        .filter(|value| !value.is_empty());
    let addresses = info.get_addresses().iter().copied().collect::<Vec<_>>();
    DiscoveryEntry {
        id: SmolStr::new(id),
        name: SmolStr::new(name),
        addresses,
        web_port,
        web_tls,
        mesh_port,
        control: control.map(SmolStr::new),
        host_group: host_group.map(SmolStr::new),
        last_seen_ns: now_ns(),
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

fn build_service_info(
    instance_name: &str,
    host: &str,
    port: u16,
    props: HashMap<String, String>,
) -> Result<ServiceInfo, RuntimeError> {
    ServiceInfo::new(SERVICE_TYPE, instance_name, host, (), port, props)
        .map(|info| info.enable_addr_auto())
        .map_err(|err| RuntimeError::ControlError(format!("mdns info: {err}").into()))
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
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
        props.insert("web_tls".to_string(), "true".to_string());
        props.insert("mesh_port".to_string(), "5200".to_string());
        props.insert("control".to_string(), "unix:///tmp/test.sock".to_string());
        props.insert("host_group".to_string(), "hq-vm-cluster".to_string());
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
        assert!(entry.web_tls);
        assert_eq!(entry.mesh_port, Some(5200));
        assert_eq!(entry.control.as_deref(), Some("unix:///tmp/test.sock"));
        assert_eq!(entry.host_group.as_deref(), Some("hq-vm-cluster"));
        assert!(entry.last_seen_ns > 0);
    }

    #[test]
    fn discovery_service_info_enables_auto_addresses() {
        let info = build_service_info(
            "trust-runtime-a",
            "host.local.",
            8080,
            std::collections::HashMap::new(),
        )
        .expect("service info");
        assert!(info.is_addr_auto());
    }

    #[test]
    fn service_removed_match_accepts_instance_suffix_runtime_name() {
        let entry = DiscoveryEntry {
            id: SmolStr::new("runtime-a-1234"),
            name: SmolStr::new("runtime-a"),
            addresses: Vec::new(),
            web_port: Some(18081),
            web_tls: false,
            mesh_port: Some(5201),
            control: Some(SmolStr::new("unix:///tmp/trust-runtime-a.sock")),
            host_group: None,
            last_seen_ns: 1,
        };
        assert!(service_removed_matches_entry(
            "runtime-a-runtime-a._trust._plc._tcp.local.",
            &entry
        ));
    }
}
