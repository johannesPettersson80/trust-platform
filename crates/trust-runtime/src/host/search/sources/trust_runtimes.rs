//! truST runtimes (specification 11, "comm.discover" and "Sources"): the runtimes that announce
//! `_trust._plc._tcp.local.` over mDNS. `discover` answers one `comm.discover` request from the
//! runtime's own discovery, or browses mDNS for the request's timeout; `search` reads the
//! runtime's own discovery, or browses mDNS, until the search ends.

use std::net::IpAddr;
use std::time::Duration;

use serde_json::json;

use crate::connectors::DiscoveryConfidence;
use crate::discovery::{DiscoveryEntry, DiscoveryState};

use super::super::model::{DeviceKey, Identity, ProtocolFind, RuntimeIdentity, SourceId};
use super::super::runner::SourceSink;
use super::report::{sanitize_id, CandidateSink, Deadline, DiscoverCandidate};
use super::scope::{timeout, DiscoverScope};
use super::SourceContext;

const TRUST_MDNS_SERVICE_TYPE: &str = "_trust._plc._tcp.local.";
/// How often a search inside a runtime reads the runtime's own discovery.
const DISCOVERY_READ_PERIOD: Duration = Duration::from_millis(250);

/// The runtimes in `discovery`, the discovery of the runtime that answers; without it, the
/// runtimes that answer an mDNS browse within the timeout of `scope`.
pub(crate) fn discover(
    scope: &DiscoverScope,
    discovery: Option<&DiscoveryState>,
    deadline: Deadline,
    sink: &mut dyn CandidateSink,
) -> Result<(), String> {
    if deadline.passed() {
        return Ok(());
    }
    match discovery {
        Some(discovery) => {
            for entry in discovery.snapshot() {
                sink.found(candidate(&entry));
            }
        }
        None => browse_trust_mdns(deadline.bound(timeout(scope)), sink)?,
    }
    Ok(())
}

fn browse_trust_mdns(timeout: Duration, sink: &mut dyn CandidateSink) -> Result<(), String> {
    use mdns_sd::{ServiceDaemon, ServiceEvent};

    let daemon =
        ServiceDaemon::new().map_err(|error| format!("mDNS daemon start failed: {error}"))?;
    let receiver = daemon
        .browse(TRUST_MDNS_SERVICE_TYPE)
        .map_err(|error| format!("mDNS browse failed: {error}"))?;
    let started = std::time::Instant::now();
    let mut seen = Vec::new();
    while started.elapsed() < timeout {
        let remaining = timeout.saturating_sub(started.elapsed());
        match receiver.recv_timeout(remaining.min(Duration::from_millis(100))) {
            Ok(ServiceEvent::ServiceResolved(info)) => {
                let entry = mdns_info_to_entry(info.as_ref());
                if !seen.contains(&entry.id) {
                    seen.push(entry.id.clone());
                    sink.found(candidate(&entry));
                }
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    if seen.is_empty() {
        sink.warn("No truST runtimes answered mDNS discovery in the timeout window.".to_string());
    }
    Ok(())
}

fn mdns_info_to_entry(info: &mdns_sd::ResolvedService) -> DiscoveryEntry {
    let id = info
        .get_property_val_str("id")
        .map(str::to_string)
        .unwrap_or_else(|| info.get_fullname().to_string());
    let name = info
        .get_property_val_str("name")
        .map(str::to_string)
        .unwrap_or_else(|| info.get_fullname().to_string());
    DiscoveryEntry {
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

/// The `comm.discover` candidate of one runtime.
fn candidate(entry: &DiscoveryEntry) -> DiscoverCandidate {
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
}

fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

/// truST runtimes in a search. Inside a runtime it reads the runtime's own mDNS discovery; on the
/// tool's computer it browses mDNS itself. Either way it listens until the search ends.
pub(super) fn search(context: &SourceContext, sink: &SourceSink) {
    match &context.runtime {
        Some(runtime) => loop {
            for entry in runtime.discovery.snapshot() {
                // A runtime's search never reports the runtime itself (specification 11, "One
                // search, device identity, and presence").
                if entry.id == runtime.runtime_id {
                    continue;
                }
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

#[cfg(test)]
#[path = "trust_runtimes_tests.rs"]
mod trust_runtimes_tests;
