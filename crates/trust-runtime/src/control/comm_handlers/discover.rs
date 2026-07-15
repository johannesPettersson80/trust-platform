use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::super::{ControlResponse, ControlState};
use super::discovery_probe::{
    probe_modbus_tcp, probe_mqtt, ModbusDiscoveryProbe, ModbusSafeReadProbe,
};

const DISCOVER_SCHEMA_VERSION: u32 = 1;
const DEFAULT_TIMEOUT_MS: u64 = 150;
const MAX_TIMEOUT_MS: u64 = 2_000;
const MODBUS_PORT: u16 = 502;
const MQTT_PORT: u16 = 1883;
const MQTTS_PORT: u16 = 8883;
const OPCUA_PORT: u16 = 4840;
const TRUST_MDNS_SERVICE_TYPE: &str = "_trust._plc._tcp.local.";

#[derive(Debug, Deserialize)]
struct CommDiscoverRequest {
    protocol: String,
    #[serde(default)]
    scope: DiscoverScope,
    #[serde(default = "default_origin")]
    origin: DiscoverOrigin,
    #[serde(default = "default_passive")]
    passive: bool,
}

#[derive(Debug, Default, Deserialize)]
struct DiscoverScope {
    cidr: Option<String>,
    host: Option<String>,
    adapter: Option<String>,
    timeout_ms: Option<u64>,
    unit_id: Option<u8>,
    probe_read_address: Option<u16>,
    probe_read_quantity: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DiscoverOrigin {
    ThisHost,
    Runtime,
}

#[derive(Debug, Serialize)]
struct CommDiscoverResponse {
    schema_version: u32,
    protocol: String,
    origin: DiscoverOrigin,
    candidates: Vec<DiscoverCandidate>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
struct DiscoverCandidate {
    id: String,
    label: String,
    source: &'static str,
    confidence: &'static str,
    params: Value,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<String>,
}

pub(super) fn handle_comm_discover(
    id: u64,
    params: Option<Value>,
    state: &ControlState,
) -> ControlResponse {
    let Some(params) = params else {
        return ControlResponse::error(id, "missing comm.discover params".into());
    };
    match discover_value(params, Some(state)) {
        Ok(value) => ControlResponse::ok(id, value),
        Err(error) => ControlResponse::error(id, error),
    }
}

pub(super) fn discover_value(params: Value, state: Option<&ControlState>) -> Result<Value, String> {
    let request: CommDiscoverRequest = serde_json::from_value(params)
        .map_err(|error| format!("invalid comm.discover payload: {error}"))?;
    let protocol = canonical_protocol(request.protocol.as_str());
    let mut warnings = Vec::new();
    if !request.passive {
        warnings.push(
            "Active write probes are not supported; discovery ran in connect/read-only mode."
                .to_string(),
        );
    }
    let candidates = match protocol.as_str() {
        "modbus_tcp" => discover_modbus(&request.scope, &mut warnings)?,
        "opcua" => {
            warnings.push("OPC UA discovery is not exposed here because truST currently provides OPC UA server setup, not an OPC UA client writer.".to_string());
            Vec::new()
        }
        "opcua_client" => discover_opcua_client(&request.scope, &mut warnings)?,
        "mqtt" => discover_mqtt(&request.scope, &mut warnings)?,
        "ethercat" => discover_ethercat(&request, state, &mut warnings)?,
        "gpio" => discover_gpio(&request, &mut warnings),
        "ads" => discover_ads(&request.scope, &mut warnings)?,
        "discovery" => discover_trust_runtimes(&request, state, &mut warnings)?,
        other if known_protocol_without_discovery(other) => {
            warnings.push(format!(
                "{} discovery is not available yet.",
                protocol_title(other)
            ));
            Vec::new()
        }
        other => return Err(format!("comm.discover does not know protocol '{other}'")),
    };
    serde_json::to_value(CommDiscoverResponse {
        schema_version: DISCOVER_SCHEMA_VERSION,
        protocol,
        origin: request.origin,
        candidates,
        warnings,
    })
    .map_err(|error| format!("comm.discover serialization failed: {error}"))
}

fn discover_modbus(
    scope: &DiscoverScope,
    warnings: &mut Vec<String>,
) -> Result<Vec<DiscoverCandidate>, String> {
    let timeout = timeout(scope);
    let targets = if let Some(host) = scope.host.as_deref().filter(|host| !host.trim().is_empty()) {
        vec![modbus_socket_target(host.trim())?]
    } else {
        let cidr = scope
            .cidr
            .clone()
            .or_else(default_ipv4_cidr)
            .ok_or_else(|| {
                "Modbus discovery needs --cidr or a private IPv4 interface to derive a /24 scan"
                    .to_string()
            })?;
        ipv4_targets_for_cidr(cidr.as_str())?
            .into_iter()
            .map(|ip| SocketAddr::new(IpAddr::V4(ip), MODBUS_PORT))
            .collect()
    };

    if targets.len() > 256 {
        return Err(
            "Modbus discovery is capped at 256 hosts; use a /24 or tighter CIDR".to_string(),
        );
    }
    if targets.len() > 32 {
        warnings.push(format!(
            "Scanning {} hosts with Modbus protocol probes; this may take a moment.",
            targets.len()
        ));
    }

    let probe = modbus_discovery_probe(scope)?;
    let mut candidates = Vec::new();
    for target in targets {
        if let Some(outcome) = probe_modbus_tcp(target, timeout, probe) {
            let address = target.to_string();
            let mut params = json!({
                "address": address,
                "unit_id": probe.unit_id,
                "probe_source": outcome.source,
                "probe_detail": outcome.detail,
            });
            if let Some(safe_read) = probe.safe_read {
                params["probe_read_address"] = json!(safe_read.address);
                params["probe_read_quantity"] = json!(safe_read.quantity);
            }
            candidates.push(DiscoverCandidate {
                id: format!("modbus:{address}"),
                label: address.clone(),
                source: outcome.source,
                confidence: outcome.confidence,
                params,
                warnings: outcome.warnings,
            });
        }
    }
    Ok(candidates)
}

fn discover_mqtt(
    scope: &DiscoverScope,
    warnings: &mut Vec<String>,
) -> Result<Vec<DiscoverCandidate>, String> {
    let Some(targets) = mqtt_targets(scope, warnings)? else {
        return Ok(Vec::new());
    };
    let timeout = timeout(scope);
    let mut candidates = Vec::new();
    for target in targets {
        if let Some(outcome) = probe_mqtt(target, timeout) {
            let broker = target.to_string();
            candidates.push(DiscoverCandidate {
                id: format!("mqtt:{}", sanitize_id(broker.as_str())),
                label: format!("MQTT broker {broker}"),
                source: outcome.source,
                confidence: outcome.confidence,
                params: json!({
                    "broker": broker,
                    "tls": target.port() == MQTTS_PORT,
                    "allow_insecure_remote": target.port() != MQTTS_PORT && !target.ip().is_loopback(),
                    "auth_required": outcome.auth_required,
                    "probe_detail": outcome.detail,
                }),
                warnings: outcome.warnings,
            });
        }
    }
    if candidates.is_empty() {
        warnings
            .push("No MQTT broker accepted a TCP connection at the requested target.".to_string());
    }
    Ok(candidates)
}

fn modbus_discovery_probe(scope: &DiscoverScope) -> Result<ModbusDiscoveryProbe, String> {
    let safe_read = match (scope.probe_read_address, scope.probe_read_quantity) {
        (Some(address), quantity) => {
            let quantity = quantity.unwrap_or(1);
            if quantity == 0 || quantity > 125 {
                return Err(
                    "Modbus discovery scope.probe_read_quantity must be between 1 and 125"
                        .to_string(),
                );
            }
            Some(ModbusSafeReadProbe { address, quantity })
        }
        (None, Some(_)) => {
            return Err(
                "Modbus discovery scope.probe_read_address is required when scope.probe_read_quantity is set"
                    .to_string(),
            );
        }
        (None, None) => None,
    };
    Ok(ModbusDiscoveryProbe {
        unit_id: scope.unit_id.unwrap_or(1),
        safe_read,
    })
}

fn discover_opcua_client(
    scope: &DiscoverScope,
    warnings: &mut Vec<String>,
) -> Result<Vec<DiscoverCandidate>, String> {
    let endpoint_url = opcua_endpoint_url(scope)?;
    match crate::opcua::discover_opcua_client_endpoints(endpoint_url.as_str()) {
        Ok(endpoints) => {
            if endpoints.is_empty() {
                warnings.push(format!(
                    "No OPC UA endpoints were advertised by {endpoint_url}."
                ));
            }
            Ok(endpoints
                .into_iter()
                .map(|endpoint| {
                    let label = endpoint
                        .application_name
                        .clone()
                        .unwrap_or_else(|| "OPC UA server".to_string());
                    DiscoverCandidate {
                        id: format!(
                            "opcua:{}:{}:{}",
                            sanitize_id(endpoint.endpoint_url.as_str()),
                            endpoint.security_policy.as_config_value(),
                            endpoint.security_mode.as_config_value()
                        ),
                        label,
                        source: "opcua_get_endpoints",
                        confidence: "observed",
                        params: json!({
                            "endpoint_url": endpoint.endpoint_url,
                            "security_policy": endpoint.security_policy.as_config_value(),
                            "security_mode": endpoint.security_mode.as_config_value(),
                            "auth": if endpoint.anonymous_supported { "anonymous" } else { "username" },
                            "anonymous_supported": endpoint.anonymous_supported,
                            "username_supported": endpoint.username_supported,
                            "trust_server_certificate": false,
                        }),
                        warnings: Vec::new(),
                    }
                })
                .collect())
        }
        Err(error) => {
            warnings.push(format!("OPC UA discovery did not complete: {error}"));
            Ok(Vec::new())
        }
    }
}

fn discover_ethercat(
    request: &CommDiscoverRequest,
    state: Option<&ControlState>,
    warnings: &mut Vec<String>,
) -> Result<Vec<DiscoverCandidate>, String> {
    if request.origin != DiscoverOrigin::Runtime {
        warnings.push(
            "EtherCAT discovery must run from the runtime host that owns the fieldbus adapter."
                .to_string(),
        );
        return Ok(Vec::new());
    }
    let Some(params) = ethercat_discovery_params(&request.scope, state)? else {
        warnings.push(
            "EtherCAT discovery needs scope.adapter/scope.host or a configured EtherCAT driver in the runtime project."
                .to_string(),
        );
        return Ok(Vec::new());
    };
    match crate::io::discover_ethercat_modules(&params) {
        Ok(discovery) => Ok(discovery
            .modules
            .into_iter()
            .map(|module| DiscoverCandidate {
                id: format!("ethercat:slot:{}", module.slot),
                label: format!("{} (slot {})", module.model, module.slot),
                source: "ethercat_bus",
                confidence: "observed",
                params: json!({
                    "adapter": params.get("adapter").and_then(toml::Value::as_str),
                    "model": module.model,
                    "slot": module.slot,
                    "channels": module.channels,
                }),
                warnings: Vec::new(),
            })
            .collect()),
        Err(error) => {
            warnings.push(format!("EtherCAT discovery did not complete: {error}"));
            Ok(Vec::new())
        }
    }
}

fn ethercat_discovery_params(
    scope: &DiscoverScope,
    state: Option<&ControlState>,
) -> Result<Option<toml::Value>, String> {
    if let Some(adapter) = scope
        .adapter
        .as_deref()
        .or(scope.host.as_deref())
        .map(str::trim)
        .filter(|adapter| !adapter.is_empty())
    {
        let mut table = toml::map::Map::new();
        table.insert(
            "adapter".to_string(),
            toml::Value::String(adapter.to_string()),
        );
        return Ok(Some(toml::Value::Table(table)));
    }
    let Some(root) = state.and_then(|state| state.project_root.as_ref()) else {
        return Ok(None);
    };
    let path = root.join("io.toml");
    if !path.is_file() {
        return Ok(None);
    }
    let config = crate::config::IoConfig::load(path)
        .map_err(|error| format!("failed to load io.toml for EtherCAT discovery: {error}"))?;
    Ok(config
        .drivers
        .into_iter()
        .find(|driver| canonical_protocol(driver.name.as_str()) == "ethercat")
        .map(|driver| driver.params))
}

fn discover_gpio(
    request: &CommDiscoverRequest,
    warnings: &mut Vec<String>,
) -> Vec<DiscoverCandidate> {
    if request.origin != DiscoverOrigin::Runtime {
        warnings.push(
            "GPIO discovery must run from the runtime host that owns the GPIO hardware."
                .to_string(),
        );
        return Vec::new();
    }
    match crate::io::discover_gpio_lines() {
        Ok(lines) if lines.is_empty() => {
            warnings.push("No GPIO chip lines were exposed by this runtime host.".to_string());
            Vec::new()
        }
        Ok(lines) => lines
            .into_iter()
            .map(|line| DiscoverCandidate {
                id: format!("gpio:{}:{}", sanitize_id(line.chip.as_str()), line.line),
                label: line.label.as_ref().map_or_else(
                    || format!("{} line {}", line.chip, line.line),
                    |label| format!("{label} · line {}", line.line),
                ),
                source: "gpio_sysfs",
                confidence: "observed",
                params: json!({
                    "chip": line.chip,
                    "label": line.label,
                    "line": line.line,
                    "direction": line.direction,
                }),
                warnings: Vec::new(),
            })
            .collect(),
        Err(error) => {
            warnings.push(format!("GPIO discovery did not complete: {error}"));
            Vec::new()
        }
    }
}

fn mqtt_targets(
    scope: &DiscoverScope,
    warnings: &mut Vec<String>,
) -> Result<Option<Vec<SocketAddr>>, String> {
    let Some(host) = scope
        .host
        .as_deref()
        .map(str::trim)
        .filter(|host| !host.is_empty())
    else {
        warnings.push(
            "MQTT discovery needs a broker host; broad network scans are not run for MQTT."
                .to_string(),
        );
        return Ok(None);
    };
    if let Ok(addr) = host.parse::<SocketAddr>() {
        return Ok(Some(vec![addr]));
    }
    if let Ok(mut addrs) = host.to_socket_addrs() {
        if let Some(addr) = addrs.next() {
            return Ok(Some(vec![addr]));
        }
    }
    let mut addrs = Vec::new();
    for port in [MQTT_PORT, MQTTS_PORT] {
        if let Some(addr) = (host, port)
            .to_socket_addrs()
            .map_err(|error| format!("resolve MQTT target '{host}:{port}': {error}"))?
            .next()
        {
            addrs.push(addr);
        }
    }
    if addrs.is_empty() {
        return Err(format!("resolve MQTT target '{host}': no address found"));
    }
    Ok(Some(addrs))
}

#[cfg(feature = "ads-wire")]
fn discover_ads(
    scope: &DiscoverScope,
    warnings: &mut Vec<String>,
) -> Result<Vec<DiscoverCandidate>, String> {
    use crate::ads::onboarding::runtime_address_candidates_from_interfaces;
    use crate::ads::onboarding::{
        directed_broadcast_targets_from_candidates, discover_targets, AdsRsOnboardingWire,
        DiscoveryRequest, DiscoverySource,
    };

    let mut request = DiscoveryRequest {
        target: scope.host.clone(),
        target_ams_net_id: None,
        ams_port: Some(851),
        target_name: None,
        include_broadcast: scope.host.is_none(),
        broadcast_targets: Vec::new(),
        timeout_ms: scope.timeout_ms,
    };
    if let Some(cidr) = scope.cidr.as_deref() {
        request.include_broadcast = true;
        request.broadcast_targets = vec![broadcast_target_for_cidr(cidr)?];
    } else if request.include_broadcast {
        let candidates = runtime_address_candidates_from_interfaces()
            .map_err(|error| format!("enumerate interfaces for ADS broadcast: {error}"))?;
        request.broadcast_targets = directed_broadcast_targets_from_candidates(&candidates);
    }
    if request.include_broadcast && request.broadcast_targets.is_empty() {
        warnings
            .push("No non-loopback IPv4 broadcast target was available for ADS discovery.".into());
    }

    let mut wire = AdsRsOnboardingWire::default();
    let results = discover_targets(&mut wire, &request)
        .map_err(|error| format!("ADS discovery failed: {error}"))?;
    let mut candidates = Vec::new();
    for result in results {
        let source = match result.source {
            DiscoverySource::Manual => "manual",
            DiscoverySource::DirectedIdentify => "ads_identify",
            DiscoverySource::DirectedBroadcast => "ads_broadcast",
        };
        let target = result.target;
        let label = target
            .name
            .as_deref()
            .filter(|name| !name.trim().is_empty())
            .map_or_else(
                || format!("TwinCAT {}", target.ams_net_id),
                |name| format!("{name} · {}", target.ams_net_id),
            );
        candidates.push(DiscoverCandidate {
            id: format!("ads:{}", sanitize_id(target.ams_net_id.as_str())),
            label,
            source,
            confidence: "observed",
            params: json!({
                "ams_net_id": target.ams_net_id,
                "host": target.ip,
                "name": target.name,
                "ams_port": target.ams_port,
                "tc_version": target.tc_version,
            }),
            warnings: Vec::new(),
        });
    }
    Ok(candidates)
}

#[cfg(not(feature = "ads-wire"))]
fn discover_ads(
    _scope: &DiscoverScope,
    warnings: &mut Vec<String>,
) -> Result<Vec<DiscoverCandidate>, String> {
    warnings.push("ADS discovery needs a runtime built with the ads-wire feature.".to_string());
    Ok(Vec::new())
}

fn discover_trust_runtimes(
    request: &CommDiscoverRequest,
    state: Option<&ControlState>,
    warnings: &mut Vec<String>,
) -> Result<Vec<DiscoverCandidate>, String> {
    let entries = if request.origin == DiscoverOrigin::Runtime {
        match state {
            Some(state) => state.discovery.snapshot(),
            None => browse_trust_mdns(timeout(&request.scope), warnings)?,
        }
    } else {
        browse_trust_mdns(timeout(&request.scope), warnings)?
    };
    Ok(entries
        .into_iter()
        .map(|entry| {
            let host = entry
                .addresses
                .first()
                .map(ToString::to_string)
                .unwrap_or_else(|| entry.name.to_string());
            let control_endpoint = entry
                .control
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default();
            DiscoverCandidate {
                id: format!("trust:{}", sanitize_id(entry.id.as_str())),
                label: entry.name.to_string(),
                source: "mdns",
                confidence: "observed",
                params: json!({
                    "control_endpoint": control_endpoint,
                    "name": entry.name.as_str(),
                    "host": host,
                    "web_port": entry.web_port,
                    "host_group": entry.host_group.as_ref().map(ToString::to_string),
                }),
                warnings: if control_endpoint.is_empty() {
                    vec!["Runtime advertised without a control endpoint; adopt may require manual endpoint entry.".to_string()]
                } else {
                    Vec::new()
                },
            }
        })
        .collect())
}

fn browse_trust_mdns(
    timeout: Duration,
    warnings: &mut Vec<String>,
) -> Result<Vec<crate::discovery::DiscoveryEntry>, String> {
    use mdns_sd::{ServiceDaemon, ServiceEvent};

    let daemon =
        ServiceDaemon::new().map_err(|error| format!("mDNS daemon start failed: {error}"))?;
    let receiver = daemon
        .browse(TRUST_MDNS_SERVICE_TYPE)
        .map_err(|error| format!("mDNS browse failed: {error}"))?;
    let started = std::time::Instant::now();
    let mut entries = Vec::new();
    while started.elapsed() < timeout {
        let remaining = timeout.saturating_sub(started.elapsed());
        match receiver.recv_timeout(remaining.min(Duration::from_millis(100))) {
            Ok(ServiceEvent::ServiceResolved(info)) => {
                let entry = mdns_info_to_entry(info.as_ref());
                if !entries
                    .iter()
                    .any(|existing: &crate::discovery::DiscoveryEntry| existing.id == entry.id)
                {
                    entries.push(entry);
                }
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    if entries.is_empty() {
        warnings
            .push("No truST runtimes answered mDNS discovery in the timeout window.".to_string());
    }
    Ok(entries)
}

fn mdns_info_to_entry(info: &mdns_sd::ResolvedService) -> crate::discovery::DiscoveryEntry {
    let id = info
        .get_property_val_str("id")
        .map(str::to_string)
        .unwrap_or_else(|| info.get_fullname().to_string());
    let name = info
        .get_property_val_str("name")
        .map(str::to_string)
        .unwrap_or_else(|| info.get_fullname().to_string());
    crate::discovery::DiscoveryEntry {
        id: smol_str::SmolStr::new(id),
        name: smol_str::SmolStr::new(name),
        addresses: info
            .get_addresses()
            .iter()
            .map(|address| address.to_ip_addr())
            .collect(),
        web_port: info
            .get_property_val_str("web_port")
            .and_then(|value| value.parse::<u16>().ok()),
        web_tls: info
            .get_property_val_str("web_tls")
            .map(|value| matches!(value, "true" | "1" | "yes"))
            .unwrap_or(false),
        mesh_port: info
            .get_property_val_str("mesh_port")
            .and_then(|value| value.parse::<u16>().ok()),
        control: info
            .get_property_val_str("control")
            .map(smol_str::SmolStr::new),
        host_group: info
            .get_property_val_str("host_group")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(smol_str::SmolStr::new),
        last_seen_ns: now_ns(),
    }
}

fn default_origin() -> DiscoverOrigin {
    DiscoverOrigin::ThisHost
}

fn default_passive() -> bool {
    true
}

fn timeout(scope: &DiscoverScope) -> Duration {
    Duration::from_millis(
        scope
            .timeout_ms
            .unwrap_or(DEFAULT_TIMEOUT_MS)
            .min(MAX_TIMEOUT_MS),
    )
}

fn canonical_protocol(protocol: &str) -> String {
    match protocol
        .trim()
        .to_ascii_lowercase()
        .replace('-', "_")
        .as_str()
    {
        "modbus_tcp" => "modbus_tcp".to_string(),
        "opcua_client" | "opc_ua_client" => "opcua_client".to_string(),
        "opcua" | "opc_ua" | "opcua_server" | "opc_ua_server" => "opcua".to_string(),
        "mqtt" | "mqtt_broker" => "mqtt".to_string(),
        "ads" | "ads_client" | "twincat" => "ads".to_string(),
        "discovery" | "trust" | "trust_runtime" | "trust_runtimes" | "mdns" => {
            "discovery".to_string()
        }
        other => other.to_string(),
    }
}

fn modbus_socket_target(host: &str) -> Result<SocketAddr, String> {
    socket_target(host, MODBUS_PORT, "Modbus")
}

fn socket_target(host: &str, default_port: u16, label: &str) -> Result<SocketAddr, String> {
    if let Ok(addr) = host.parse::<SocketAddr>() {
        return Ok(addr);
    }
    if let Ok(mut addrs) = host.to_socket_addrs() {
        if let Some(addr) = addrs.next() {
            return Ok(addr);
        }
    }
    (host, default_port)
        .to_socket_addrs()
        .map_err(|error| format!("resolve {label} target '{host}': {error}"))?
        .next()
        .ok_or_else(|| format!("resolve {label} target '{host}': no address found"))
}

fn known_protocol_without_discovery(protocol: &str) -> bool {
    matches!(
        protocol,
        "ethercat"
            | "gpio"
            | "simulated"
            | "loopback"
            | "openot"
            | "opcua"
            | "mesh"
            | "runtime_cloud"
            | "realtime_t0"
            | "ads_server"
    )
}

fn protocol_title(protocol: &str) -> &'static str {
    match protocol {
        "ads_server" => "ADS server",
        "ethercat" => "EtherCAT",
        "gpio" => "GPIO",
        "loopback" => "Loopback",
        "mesh" => "Mesh",
        "openot" => "OpenOT",
        "opcua" => "OPC UA server",
        "opcua_client" => "OPC UA client",
        "realtime_t0" => "Realtime T0",
        "runtime_cloud" => "Runtime cloud",
        "simulated" => "Simulated I/O",
        _ => "Selected protocol",
    }
}

fn opcua_endpoint_url(scope: &DiscoverScope) -> Result<String, String> {
    let host = scope
        .host
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "OPC UA client discovery needs scope.host".to_string())?;
    if host.starts_with("opc.tcp://") {
        return Ok(host.to_string());
    }
    if host.contains('/') {
        return Ok(format!("opc.tcp://{host}"));
    }
    if host
        .rsplit_once(':')
        .is_some_and(|(_, port)| port.parse::<u16>().is_ok())
    {
        Ok(format!("opc.tcp://{host}"))
    } else {
        Ok(format!("opc.tcp://{host}:{OPCUA_PORT}"))
    }
}

fn default_ipv4_cidr() -> Option<String> {
    let candidates = crate::ads::onboarding::runtime_address_candidates_from_interfaces().ok()?;
    let ip = candidates.into_iter().find_map(|candidate| {
        let ip = candidate.ip.parse::<Ipv4Addr>().ok()?;
        if ip.is_loopback() || ip.is_link_local() || !ip.is_private() {
            return None;
        }
        Some(ip)
    })?;
    let [a, b, c, _] = ip.octets();
    Some(format!("{a}.{b}.{c}.0/24"))
}

fn ipv4_targets_for_cidr(cidr: &str) -> Result<Vec<Ipv4Addr>, String> {
    let (base, prefix) = parse_ipv4_cidr(cidr)?;
    if prefix < 24 {
        return Err("Discovery CIDR must be /24 or tighter for safety.".to_string());
    }
    let host_count = 1u64 << (32 - u64::from(prefix));
    if host_count > 256 {
        return Err("Discovery CIDR is too large; use a /24 or tighter range.".to_string());
    }
    let base_raw = u32::from(base);
    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - u32::from(prefix))
    };
    let network = base_raw & mask;
    let broadcast = network | !mask;
    let mut addrs = Vec::new();
    for raw in network..=broadcast {
        if prefix <= 30 && (raw == network || raw == broadcast) {
            continue;
        }
        addrs.push(Ipv4Addr::from(raw));
    }
    Ok(addrs)
}

fn parse_ipv4_cidr(cidr: &str) -> Result<(Ipv4Addr, u8), String> {
    let (ip, prefix) = cidr
        .split_once('/')
        .ok_or_else(|| format!("invalid CIDR '{cidr}', expected a.b.c.d/24"))?;
    let ip = ip
        .parse::<Ipv4Addr>()
        .map_err(|error| format!("invalid CIDR address '{ip}': {error}"))?;
    let prefix = prefix
        .parse::<u8>()
        .map_err(|error| format!("invalid CIDR prefix '{prefix}': {error}"))?;
    if prefix > 32 {
        return Err(format!("invalid CIDR prefix '{prefix}', expected 0..32"));
    }
    Ok((ip, prefix))
}

#[cfg(feature = "ads-wire")]
fn broadcast_target_for_cidr(cidr: &str) -> Result<String, String> {
    use crate::ads::onboarding::directed_broadcast_target;

    let (ip, prefix) = parse_ipv4_cidr(cidr)?;
    directed_broadcast_target(&ip.to_string(), prefix)
        .ok_or_else(|| format!("could not compute directed broadcast target for {cidr}"))
}

fn sanitize_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

#[cfg(test)]
#[path = "discover_tests.rs"]
mod discover_tests;
