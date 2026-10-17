//! The sources over today's per-protocol searches (V7 slice C1a). Each one turns the search's
//! targets into `comm.discover` requests, runs them where the search runs, and reports every
//! candidate as a result while the search runs. The listening sources listen until the search
//! ends (specification 11, "Search"). C1b moves the searches themselves into this folder.

use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde_json::{json, Value};

use crate::connectors::DiscoveryConfidence;
use crate::discovery::DiscoveryEntry;

use super::super::model::{
    AdsIdentity, DeviceKey, EthercatIdentity, GpioIdentity, Identity, ModbusIdentity, MqttIdentity,
    OpcuaIdentity, ProtocolFind, RuntimeIdentity, SourceId,
};
use super::super::runner::SourceSink;
use super::SourceContext;

/// How often the runtime's own discovery state is read.
const DISCOVERY_READ_PERIOD: Duration = Duration::from_millis(250);
/// The pause between two ADS broadcast rounds.
const ADS_ROUND_PAUSE: Duration = Duration::from_millis(1_000);
/// How many Modbus TCP hosts are probed at the same time.
const MODBUS_PARALLEL_PROBES: usize = 32;
/// A directed MQTT probe waits this long for an answer.
const MQTT_PROBE: Duration = Duration::from_millis(1_000);
/// The adapter of an EtherCAT driver that names none: the simulated bus.
const MOCK_ETHERCAT_ADAPTER: &str = "mock";
/// The label `comm.discover` gives an OPC UA endpoint whose server has no application name.
const UNNAMED_OPCUA_SERVER: &str = "OPC UA server";

/// One host that Modbus TCP probes, and how long the probe waits.
#[derive(Debug, Clone)]
pub(crate) struct ModbusTarget {
    pub(crate) host: String,
    pub(crate) timeout: Duration,
}

/// The answer of one `comm.discover` request.
#[derive(Debug, Deserialize)]
struct Answer {
    candidates: Vec<Candidate>,
    #[serde(default)]
    warnings: Vec<String>,
}

/// One candidate of a `comm.discover` answer.
#[derive(Debug, Deserialize)]
struct Candidate {
    #[serde(default)]
    label: String,
    source: String,
    confidence: String,
    params: Value,
    #[serde(default)]
    warnings: Vec<String>,
}

/// truST runtimes. Inside a runtime it reads the runtime's own mDNS discovery; on the tool's
/// computer it browses mDNS itself. Either way it listens until the search ends.
pub(crate) fn trust_runtimes(context: &SourceContext, sink: &SourceSink) {
    match &context.runtime {
        Some(runtime) => loop {
            for entry in runtime.discovery.snapshot() {
                sink.found(runtime_find(&entry));
            }
            if sink.ended() {
                return;
            }
            sink.wait(DISCOVERY_READ_PERIOD);
        },
        None => {
            let browsed = crate::discovery::browse_runtimes(&|| sink.ended(), &mut |entry| {
                sink.found(runtime_find(&entry));
            });
            if let Err(error) = browsed {
                sink.warn(error);
            }
        }
    }
}

/// ADS. Each known address is identified once. Without one, or with a subnet, broadcast rounds
/// listen until the search ends.
pub(crate) fn ads(context: &SourceContext, sink: &SourceSink) {
    for host in &context.hosts {
        if sink.ended() {
            return;
        }
        let host = host_part(host);
        match discover("ads", json!({ "host": host }), context) {
            Ok(answer) => report(sink, answer, Some(host), ads_find),
            Err(error) => sink.warn(format!("{host}: {error}")),
        }
    }
    if !context.hosts.is_empty() && context.cidr.is_none() {
        return;
    }
    let scope = context
        .cidr
        .as_ref()
        .map_or_else(|| json!({}), |cidr| json!({ "cidr": cidr }));
    while !sink.ended() {
        match discover("ads", scope.clone(), context) {
            Ok(answer) => report(sink, answer, None, ads_find),
            Err(error) => sink.warn(error),
        }
        sink.wait(ADS_ROUND_PAUSE);
    }
}

/// Modbus TCP: every target, many at the same time.
pub(crate) fn modbus_tcp(context: &SourceContext, targets: &[ModbusTarget], sink: &SourceSink) {
    let next = AtomicUsize::new(0);
    let probe_targets = || {
        while !sink.ended() {
            let Some(target) = targets.get(next.fetch_add(1, Ordering::Relaxed)) else {
                return;
            };
            let timeout_ms = millis(target.timeout.min(sink.remaining())).max(1);
            let scope = json!({ "host": target.host, "timeout_ms": timeout_ms });
            match discover("modbus_tcp", scope, context) {
                Ok(answer) => report(sink, answer, Some(target.host.as_str()), modbus_find),
                Err(error) => sink.warn(format!("{}: {error}", target.host)),
            }
        }
    };
    std::thread::scope(|scope| {
        for _ in 1..MODBUS_PARALLEL_PROBES.min(targets.len()) {
            let spawned = std::thread::Builder::new()
                .name("search-modbus".to_string())
                .spawn_scoped(scope, probe_targets);
            if spawned.is_err() {
                break;
            }
        }
        probe_targets();
    });
}

/// MQTT: each known address.
pub(crate) fn mqtt(context: &SourceContext, sink: &SourceSink) {
    for host in &context.hosts {
        if sink.ended() {
            return;
        }
        let timeout_ms = millis(MQTT_PROBE.min(sink.remaining())).max(1);
        match discover(
            "mqtt",
            json!({ "host": host, "timeout_ms": timeout_ms }),
            context,
        ) {
            Ok(answer) => report(sink, answer, Some(host.as_str()), mqtt_find),
            Err(error) => sink.warn(format!("{host}: {error}")),
        }
    }
}

/// OPC UA: the endpoints of each known address.
pub(crate) fn opcua(context: &SourceContext, sink: &SourceSink) {
    for host in &context.hosts {
        if sink.ended() {
            return;
        }
        match discover("opcua_client", json!({ "host": host }), context) {
            Ok(answer) => report(sink, answer, Some(host.as_str()), |candidate| {
                opcua_find(candidate, host)
            }),
            Err(error) => sink.warn(format!("{host}: {error}")),
        }
    }
}

/// EtherCAT: each requested adapter, or the adapter of the project's EtherCAT driver.
pub(crate) fn ethercat(context: &SourceContext, sink: &SourceSink) {
    for adapter in &context.ethercat_adapters {
        if sink.ended() {
            return;
        }
        let scope = adapter
            .as_ref()
            .map_or_else(|| json!({}), |adapter| json!({ "adapter": adapter }));
        match discover("ethercat", scope, context) {
            Ok(answer) => report(sink, answer, adapter.as_deref(), ethercat_find),
            Err(error) => sink.warn(error),
        }
    }
}

/// GPIO: the lines of the runtime's computer.
pub(crate) fn gpio(context: &SourceContext, sink: &SourceSink) {
    match discover("gpio", json!({}), context) {
        Ok(answer) => report(sink, answer, None, gpio_find),
        Err(error) => sink.warn(error),
    }
}

/// Runs one `comm.discover` request where the search runs.
fn discover(protocol: &str, scope: Value, context: &SourceContext) -> Result<Answer, String> {
    let origin = if context.runtime.is_some() {
        "runtime"
    } else {
        "this_host"
    };
    let params = json!({ "protocol": protocol, "scope": scope, "origin": origin, "passive": true });
    let project_root = context
        .runtime
        .as_ref()
        .and_then(|runtime| runtime.project_root.as_deref());
    let answer = crate::control::comm_discover_port(params, project_root)?;
    serde_json::from_value(answer)
        .map_err(|error| format!("unreadable {protocol} discovery answer: {error}"))
}

/// Reports every candidate as a result and every warning, naming `target` in the warnings.
fn report(
    sink: &SourceSink,
    answer: Answer,
    target: Option<&str>,
    to_find: impl Fn(Candidate) -> ProtocolFind,
) {
    for warning in answer.warnings {
        sink.warn(match target {
            Some(target) => format!("{target}: {warning}"),
            None => warning,
        });
    }
    for candidate in answer.candidates {
        sink.found(to_find(candidate));
    }
}

/// The confidence of a `comm.discover` candidate. Its word `observed` names an ADS identify reply,
/// an OPC UA GetEndpoints answer, an EtherCAT read, a GPIO listing or an mDNS answer, each of
/// which is `confirmed` (specification 11, "Confidence"); any word outside the vocabulary is no
/// evidence.
pub(crate) fn confidence(word: &str) -> DiscoveryConfidence {
    match word {
        "confirmed" | "observed" => DiscoveryConfidence::Confirmed,
        "likely" => DiscoveryConfidence::Likely,
        "port_reachable" => DiscoveryConfidence::PortReachable,
        _ => DiscoveryConfidence::Unavailable,
    }
}

/// The host of a known address without its port: `10.0.0.9:502` is `10.0.0.9`, `[fe80::1]:502`
/// is `fe80::1`, and a bare IPv6 address stays whole.
pub(crate) fn host_part(authority: &str) -> &str {
    if let Some(bracketed) = authority.strip_prefix('[') {
        return bracketed.split(']').next().unwrap_or(bracketed);
    }
    match authority.split_once(':') {
        Some((host, port)) if !port.contains(':') => host,
        _ => authority,
    }
}

/// The device address of a host, an IP address or a socket address: the IP address, or the host
/// name in lower case.
fn address_key(authority: &str) -> Option<DeviceKey> {
    let authority = authority.trim();
    if let Ok(socket) = authority.parse::<SocketAddr>() {
        return Some(DeviceKey::Address(socket.ip().to_string()));
    }
    let host = host_part(authority);
    if host.is_empty() {
        return None;
    }
    let address = host
        .parse::<IpAddr>()
        .map_or_else(|_| host.to_ascii_lowercase(), |ip| ip.to_string());
    Some(DeviceKey::Address(address))
}

fn runtime_find(entry: &DiscoveryEntry) -> ProtocolFind {
    let control = entry
        .control
        .as_ref()
        .map(ToString::to_string)
        .filter(|control| !control.is_empty());
    let host_group = entry.host_group.as_ref().map(ToString::to_string);
    let host = entry
        .addresses
        .first()
        .map_or_else(|| entry.name.to_string(), ToString::to_string);
    let params = json!({
        "control_endpoint": control.clone().unwrap_or_default(),
        "name": entry.name.as_str(),
        "host": host,
        "web_port": entry.web_port,
        "host_group": host_group,
    });
    let warnings = if control.is_none() {
        vec!["Runtime advertised without a control endpoint; adopt may require manual endpoint entry.".to_string()]
    } else {
        Vec::new()
    };
    let mut keys: Vec<DeviceKey> = entry
        .addresses
        .iter()
        .map(|address| DeviceKey::Address(address.to_string()))
        .collect();
    keys.push(DeviceKey::RuntimeId(entry.id.to_string()));
    let identity = Identity::TrustRuntime(RuntimeIdentity {
        name: entry.name.to_string(),
        id: entry.id.to_string(),
        control,
        web: web_address(entry),
        host_group,
        mesh_port: entry.mesh_port,
    });
    ProtocolFind::new(
        SourceId::TrustRuntime,
        "mdns",
        DiscoveryConfidence::Confirmed,
        identity,
        params,
        entry.last_seen_ns / 1_000_000,
    )
    .with_warnings(warnings)
    .keyed(entry.id.to_string(), keys)
}

/// The web address a runtime announces: its web port at its first IPv4 address, or its first
/// address.
fn web_address(entry: &DiscoveryEntry) -> Option<String> {
    let port = entry.web_port?;
    let scheme = if entry.web_tls { "https" } else { "http" };
    let address = entry
        .addresses
        .iter()
        .find(|address| address.is_ipv4())
        .or_else(|| entry.addresses.first())?;
    Some(match address {
        IpAddr::V4(address) => format!("{scheme}://{address}:{port}"),
        IpAddr::V6(address) => format!("{scheme}://[{address}]:{port}"),
    })
}

fn ads_find(candidate: Candidate) -> ProtocolFind {
    let ams_net_id = text(&candidate.params, "ams_net_id").unwrap_or_default();
    let mut keys = vec![DeviceKey::AmsNetId(ams_net_id.clone())];
    keys.extend(text(&candidate.params, "host").and_then(|host| address_key(&host)));
    let identity = Identity::Ads(AdsIdentity {
        ams_net_id: ams_net_id.clone(),
        computer_name: text(&candidate.params, "name"),
        twincat_version: text(&candidate.params, "tc_version"),
    });
    find(SourceId::Ads, candidate, identity).keyed(ams_net_id, keys)
}

fn modbus_find(candidate: Candidate) -> ProtocolFind {
    let address = text(&candidate.params, "address").unwrap_or_default();
    let unit_id = candidate
        .params
        .get("unit_id")
        .and_then(Value::as_u64)
        .and_then(|unit_id| u8::try_from(unit_id).ok())
        .unwrap_or(1);
    let keys = address_key(&address).into_iter().collect();
    let identity = Identity::Modbus(ModbusIdentity { unit_id });
    find(SourceId::ModbusTcp, candidate, identity).keyed(format!("{address}/{unit_id}"), keys)
}

fn mqtt_find(candidate: Candidate) -> ProtocolFind {
    let broker = text(&candidate.params, "broker").unwrap_or_default();
    let tls = candidate
        .params
        .get("tls")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let auth_required = candidate
        .params
        .get("auth_required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    // A rejected login proves that one is needed; an accepted session proves that none is.
    let login_required = if auth_required {
        Some(true)
    } else if confidence(&candidate.confidence) == DiscoveryConfidence::Confirmed {
        Some(false)
    } else {
        None
    };
    let keys = address_key(&broker).into_iter().collect();
    let identity = Identity::Mqtt(MqttIdentity {
        tls,
        login_required,
    });
    find(SourceId::Mqtt, candidate, identity).keyed(broker, keys)
}

/// One endpoint that the server at `host` announced; the device is the probed host, because an
/// endpoint URL may name the server by another name.
fn opcua_find(candidate: Candidate, host: &str) -> ProtocolFind {
    let endpoint_url = text(&candidate.params, "endpoint_url").unwrap_or_default();
    let security_policy = text(&candidate.params, "security_policy").unwrap_or_default();
    let security_mode = text(&candidate.params, "security_mode").unwrap_or_default();
    let application_name = Some(candidate.label.clone())
        .filter(|label| !label.is_empty() && label != UNNAMED_OPCUA_SERVER);
    let key = format!("{endpoint_url} {security_policy} {security_mode}");
    let keys = address_key(host).into_iter().collect();
    let identity = Identity::Opcua(OpcuaIdentity {
        endpoint_url,
        security_policy,
        security_mode,
        application_name,
    });
    find(SourceId::Opcua, candidate, identity).keyed(key, keys)
}

fn ethercat_find(candidate: Candidate) -> ProtocolFind {
    let adapter =
        text(&candidate.params, "adapter").unwrap_or_else(|| MOCK_ETHERCAT_ADAPTER.to_string());
    let slot = number(&candidate.params, "slot");
    let identity = Identity::Ethercat(EthercatIdentity {
        adapter: adapter.clone(),
        slot,
        model: text(&candidate.params, "model").unwrap_or_default(),
        channels: number(&candidate.params, "channels"),
    });
    let simulated = adapter.eq_ignore_ascii_case(MOCK_ETHERCAT_ADAPTER);
    find(SourceId::Ethercat, candidate, identity)
        .simulated(simulated)
        .keyed(
            format!("{adapter}:{slot}"),
            vec![DeviceKey::Local(format!("ethercat:{adapter}:{slot}"))],
        )
}

fn gpio_find(candidate: Candidate) -> ProtocolFind {
    let chip = text(&candidate.params, "chip").unwrap_or_default();
    let line = number(&candidate.params, "line");
    let identity = Identity::Gpio(GpioIdentity {
        chip: chip.clone(),
        chip_label: text(&candidate.params, "label"),
        line,
        direction: text(&candidate.params, "direction"),
    });
    find(SourceId::Gpio, candidate, identity).keyed(
        format!("{chip}:{line}"),
        vec![DeviceKey::Local(format!("gpio:{chip}"))],
    )
}

fn find(protocol: SourceId, candidate: Candidate, identity: Identity) -> ProtocolFind {
    ProtocolFind::new(
        protocol,
        candidate.source,
        confidence(&candidate.confidence),
        identity,
        candidate.params,
        now_ms(),
    )
    .with_warnings(candidate.warnings)
}

/// A non-empty text parameter.
fn text(params: &Value, key: &str) -> Option<String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// A whole-number parameter, zero when it is absent or out of range.
fn number<T: TryFrom<u64> + Default>(params: &Value, key: &str) -> T {
    params
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| T::try_from(value).ok())
        .unwrap_or_default()
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, millis)
}
