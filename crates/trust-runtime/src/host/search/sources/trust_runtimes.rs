//! truST runtimes (specification 11, "comm.discover" and "Sources"): the runtimes that announce
//! `_trust._plc._tcp.local.` over mDNS. `discover` answers one `comm.discover` request from the
//! runtime's own discovery, or browses mDNS for the request's browse window; `search` reads the
//! runtime's own discovery, or browses mDNS, until the search ends.

use std::net::IpAddr;
use std::time::Duration;

use mdns_sd::RecvTimeoutError;
use serde_json::json;

use crate::connectors::DiscoveryConfidence;
use crate::discovery::{DiscoveryEntry, DiscoveryState};
use crate::runtime_cloud::projection::RuntimePresenceRecord;

use super::super::model::{DeviceKey, Identity, ProtocolFind, RuntimeIdentity, SourceId};
use super::super::runner::SourceSink;
use super::cloud_presence::{presence_warnings, PresenceCache};
use super::mesh_peers;
use super::report::{sanitize_id, CandidateSink, Deadline, DiscoverCandidate};
use super::scope::{timeout, DiscoverScope};
use super::{RuntimeFacts, SourceContext};

const TRUST_MDNS_SERVICE_TYPE: &str = "_trust._plc._tcp.local.";
/// The mDNS browse window of a `comm.discover` request that names no timeout.
const DEFAULT_MDNS_BROWSE_WINDOW_MS: u64 = 2_000;
/// How often a search inside a runtime reads the runtime's own discovery.
const DISCOVERY_READ_PERIOD: Duration = Duration::from_millis(250);
/// The warning of a runtime that announces no control endpoint.
const NO_CONTROL_WARNING: &str =
    "Runtime advertised without a control endpoint; adopt may require manual endpoint entry.";

/// The runtimes in `discovery`, the discovery of the runtime that answers; without it, the
/// runtimes that answer an mDNS browse within the browse window of `scope`.
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
        None => browse_trust_mdns(deadline.bound(mdns_browse_window(scope)), sink)?,
    }
    Ok(())
}

fn browse_trust_mdns(window: Duration, sink: &mut dyn CandidateSink) -> Result<(), String> {
    use mdns_sd::{ServiceDaemon, ServiceEvent};

    let daemon =
        ServiceDaemon::new().map_err(|error| format!("mDNS daemon start failed: {error}"))?;
    let browsed = daemon.browse(TRUST_MDNS_SERVICE_TYPE).map(|receiver| {
        let started = std::time::Instant::now();
        collect_resolved_runtimes(
            window,
            || started.elapsed(),
            |wait| {
                receiver.recv_timeout(wait).map(|event| match event {
                    ServiceEvent::ServiceResolved(info) => Some(mdns_info_to_entry(info.as_ref())),
                    _ => None,
                })
            },
        )
    });
    // The daemon thread keeps its own command sender, so it runs until it is shut down.
    let _ = daemon.shutdown();
    let entries = browsed.map_err(|error| format!("mDNS browse failed: {error}"))?;
    if entries.is_empty() {
        sink.warn("No truST runtimes answered mDNS discovery in the timeout window.".to_string());
    }
    for entry in &entries {
        sink.found(candidate(entry));
    }
    Ok(())
}

// Collects each runtime resolved before the window ends. A receive that times out is only a quiet
// interval: a responder may delay its answer (RFC 6762 §6) and the querier repeats its first query
// after one second (§5.2). Only a closed channel ends the browse early.
fn collect_resolved_runtimes(
    window: Duration,
    elapsed: impl Fn() -> Duration,
    mut receive: impl FnMut(Duration) -> Result<Option<DiscoveryEntry>, RecvTimeoutError>,
) -> Vec<DiscoveryEntry> {
    let mut entries: Vec<DiscoveryEntry> = Vec::new();
    while elapsed() < window {
        let remaining = window.saturating_sub(elapsed());
        match receive(remaining.min(Duration::from_millis(100))) {
            Ok(Some(entry)) => {
                if !entries.iter().any(|existing| existing.id == entry.id) {
                    entries.push(entry);
                }
            }
            Ok(None) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    entries
}

/// How long one `comm.discover` request browses mDNS for truST runtimes (specification 11 §6.5,
/// "Communication discovery scope and symbol browsing"): its timeout, clamped as a probe's is, and
/// 2,000 ms without one.
fn mdns_browse_window(scope: &DiscoverScope) -> Duration {
    match scope.timeout_ms {
        Some(_) => timeout(scope),
        None => Duration::from_millis(DEFAULT_MDNS_BROWSE_WINDOW_MS),
    }
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
        confidence: "confirmed",
        params: json!({
            "control_endpoint": control_endpoint,
            "name": entry.name.as_str(),
            "host": host,
            "web_port": entry.web_port,
            "host_group": entry.host_group.as_ref().map(ToString::to_string),
        }),
        warnings: if control_endpoint.is_empty() {
            vec![NO_CONTROL_WARNING.to_string()]
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

/// truST runtimes in a search. Inside a runtime it reads the runtime's own mDNS discovery, with the
/// runtime cloud's presence of each runtime, and asks the mesh peers for their identity; on the
/// tool's computer it browses mDNS itself. Either way it listens until the search ends.
pub(super) fn search(context: &SourceContext, sink: &SourceSink) {
    match &context.runtime {
        Some(runtime) => std::thread::scope(|scope| {
            // The mesh peers are asked on a thread of their own, so that a slow peer holds up no
            // runtime that discovery finds.
            scope.spawn(|| mesh_peers::search(runtime, sink));
            read_discovery(runtime, sink);
        }),
        None => {
            let browsed = crate::discovery::browse_runtimes(&|| sink.ended(), &mut |entry| {
                sink.found(runtime_find(&entry, None));
            });
            if let Err(error) = browsed {
                sink.warn(error);
            }
        }
    }
}

/// Reads the runtime's own discovery until the search ends. Each runtime is reported with its
/// presence (specification 11, "Runtime cloud presence").
fn read_discovery(runtime: &RuntimeFacts, sink: &SourceSink) {
    let mut presence = PresenceCache::default();
    loop {
        let entries = runtime.discovery.snapshot();
        presence.retain_listed(&entries);
        for entry in &entries {
            // A runtime's search never reports the runtime itself (specification 11, "One
            // search, device identity, and presence").
            if entry.id == runtime.runtime_id {
                continue;
            }
            let record = presence.presence_of(entry, std::time::Instant::now(), now_ns());
            sink.found(runtime_find(entry, Some(&record)));
        }
        if sink.ended() {
            return;
        }
        sink.wait(DISCOVERY_READ_PERIOD);
    }
}

/// The truST runtime find of one discovered runtime. With its `presence`, the find's `last_seen`
/// is the observation time of that presence, and its marks are warnings.
fn runtime_find(entry: &DiscoveryEntry, presence: Option<&RuntimePresenceRecord>) -> ProtocolFind {
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
    let mut warnings = if control.is_none() {
        vec![NO_CONTROL_WARNING.to_string()]
    } else {
        Vec::new()
    };
    let last_seen_ns = match presence {
        Some(record) => {
            warnings.extend(presence_warnings(record));
            record.last_seen_ns
        }
        None => entry.last_seen_ns,
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
        version: None,
        platform: None,
    });
    ProtocolFind::new(
        SourceId::TrustRuntime,
        "mdns",
        DiscoveryConfidence::Confirmed,
        identity,
        params,
        last_seen_ns / 1_000_000,
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
