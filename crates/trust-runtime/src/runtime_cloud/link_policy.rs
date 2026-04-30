//! Runtime-cloud link transport preference and topology projection policy.

#![allow(missing_docs)]

use std::collections::{BTreeMap, HashSet};
use std::net::IpAddr;

use serde::{Deserialize, Serialize};

use crate::config::{
    RuntimeCloudLinkPreferenceRule, RuntimeCloudPreferredTransport, RuntimeCloudProfile,
};
use crate::runtime_cloud::contracts::ReasonCode;
use crate::runtime_cloud::projection::{ChannelType, FleetNode, RuntimeCloudUiState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RuntimeCloudLinkTransport {
    Realtime,
    Zenoh,
    Mesh,
    Mqtt,
    #[serde(rename = "modbus-tcp")]
    ModbusTcp,
    #[serde(rename = "opcua")]
    OpcUa,
    Discovery,
    Web,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RuntimeCloudLinkTransportPreference {
    pub(crate) source: String,
    pub(crate) target: String,
    pub(crate) transport: RuntimeCloudLinkTransport,
    pub(crate) actor: String,
    pub(crate) updated_at_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct RuntimeCloudLinkTransportState {
    pub(crate) links: BTreeMap<String, RuntimeCloudLinkTransportPreference>,
}

pub(crate) fn runtime_cloud_link_transport_for(
    state: &RuntimeCloudLinkTransportState,
    source: &str,
    target: &str,
) -> Option<RuntimeCloudLinkTransport> {
    let key = runtime_cloud_link_key(source, target)?;
    state.links.get(key.as_str()).map(|entry| entry.transport)
}

pub(crate) fn runtime_cloud_set_link_transport(
    state: &mut RuntimeCloudLinkTransportState,
    source: &str,
    target: &str,
    transport: RuntimeCloudLinkTransport,
    actor: &str,
    updated_at_ns: u64,
) -> Result<RuntimeCloudLinkTransportPreference, ReasonCode> {
    if actor.trim().is_empty() {
        return Err(ReasonCode::ContractViolation);
    }
    let Some(key) = runtime_cloud_link_key(source, target) else {
        return Err(ReasonCode::ContractViolation);
    };
    let preference = RuntimeCloudLinkTransportPreference {
        source: source.trim().to_string(),
        target: target.trim().to_string(),
        transport,
        actor: actor.trim().to_string(),
        updated_at_ns,
    };
    state.links.insert(key, preference.clone());
    Ok(preference)
}

pub(crate) fn runtime_cloud_seed_link_transport_preferences(
    state: &mut RuntimeCloudLinkTransportState,
    preferences: &[RuntimeCloudLinkPreferenceRule],
    actor: &str,
    updated_at_ns: u64,
) -> bool {
    let actor = actor.trim();
    if actor.is_empty() {
        return false;
    }
    let mut changed = false;
    let mut configured_keys = HashSet::<String>::new();

    for rule in preferences {
        let Some(key) = runtime_cloud_link_key(rule.source.as_str(), rule.target.as_str()) else {
            continue;
        };
        configured_keys.insert(key.clone());
        let transport = runtime_cloud_config_transport(rule.transport);
        let source = rule.source.trim().to_string();
        let target = rule.target.trim().to_string();

        let should_update = match state.links.get(key.as_str()) {
            Some(existing) => {
                existing.source != source
                    || existing.target != target
                    || existing.transport != transport
                    || existing.actor != actor
            }
            None => true,
        };
        if !should_update {
            continue;
        }
        state.links.insert(
            key,
            RuntimeCloudLinkTransportPreference {
                source,
                target,
                transport,
                actor: actor.to_string(),
                updated_at_ns,
            },
        );
        changed = true;
    }

    let stale_keys = state
        .links
        .iter()
        .filter_map(|(key, value)| {
            if value.actor == actor && !configured_keys.contains(key) {
                Some(key.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    for key in stale_keys {
        state.links.remove(key.as_str());
        changed = true;
    }

    changed
}

pub(crate) fn runtime_cloud_apply_link_transport_preferences(
    ui_state: &mut RuntimeCloudUiState,
    state: &RuntimeCloudLinkTransportState,
    same_host: impl Fn(&str, &str) -> bool,
) {
    let mut realtime_overlays = Vec::new();
    for edge in &mut ui_state.topology.edges {
        let Some(transport) =
            runtime_cloud_link_transport_for(state, edge.source.as_str(), edge.target.as_str())
        else {
            continue;
        };

        if transport == RuntimeCloudLinkTransport::Realtime {
            if !same_host(edge.source.as_str(), edge.target.as_str()) {
                continue;
            }
            let mut realtime = edge.clone();
            realtime.channel_type = ChannelType::T0HardRt;
            // Mesh packet-loss metrics are not meaningful for local SHM realtime lanes.
            realtime.loss_pct = None;
            realtime.latency_ms_p95 = None;
            realtime_overlays.push(realtime);
            continue;
        }

        edge.channel_type = runtime_cloud_link_channel_type(transport);
        if edge.channel_type != ChannelType::MeshT2Ops {
            edge.loss_pct = None;
            edge.latency_ms_p95 = None;
        }
    }
    ui_state.topology.edges.extend(realtime_overlays);
}

pub(crate) fn runtime_cloud_compute_host_groups(
    nodes: &[FleetNode],
    same_host: impl Fn(&str, &str) -> bool,
) -> Vec<Vec<String>> {
    if nodes.is_empty() {
        return Vec::new();
    }
    let mut ids: Vec<&str> = nodes.iter().map(|n| n.runtime_id.as_str()).collect();
    ids.sort();

    if ids.len() == 1 {
        return vec![vec![ids[0].to_string()]];
    }

    let mut parent: Vec<usize> = (0..ids.len()).collect();
    fn find(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            if same_host(ids[i], ids[j]) {
                let ri = find(&mut parent, i);
                let rj = find(&mut parent, j);
                if ri != rj {
                    parent[ri] = rj;
                }
            }
        }
    }
    let mut groups_map = BTreeMap::<usize, Vec<String>>::new();
    for (i, id) in ids.iter().enumerate() {
        let root = find(&mut parent, i);
        groups_map.entry(root).or_default().push((*id).to_string());
    }
    let mut groups: Vec<Vec<String>> = groups_map.into_values().collect();
    for group in &mut groups {
        group.sort();
    }
    groups.sort_by(|a, b| a[0].cmp(&b[0]));
    groups
}

pub(crate) fn runtime_cloud_topology_feature_flags(
    profile: RuntimeCloudProfile,
) -> BTreeMap<String, bool> {
    let mut flags = BTreeMap::new();
    match profile {
        RuntimeCloudProfile::Dev => {
            flags.insert("host_containers".to_string(), true);
            flags.insert("device_discovery".to_string(), true);
            flags.insert("edit_mode".to_string(), true);
            flags.insert("module_slots".to_string(), true);
        }
        RuntimeCloudProfile::Plant | RuntimeCloudProfile::Wan => {
            flags.insert("host_containers".to_string(), true);
        }
    }
    flags
}

pub(crate) fn runtime_cloud_addresses_share_host(source: &[IpAddr], target: &[IpAddr]) -> bool {
    if target.iter().any(IpAddr::is_loopback) {
        return true;
    }
    if source.is_empty() || target.is_empty() {
        return false;
    }
    let source_set = source.iter().copied().collect::<HashSet<_>>();
    target.iter().any(|address| source_set.contains(address))
}

fn runtime_cloud_link_key(source: &str, target: &str) -> Option<String> {
    let source = source.trim();
    let target = target.trim();
    if source.is_empty() || target.is_empty() {
        return None;
    }
    Some(format!("{source}->{target}"))
}

fn runtime_cloud_config_transport(
    transport: RuntimeCloudPreferredTransport,
) -> RuntimeCloudLinkTransport {
    match transport {
        RuntimeCloudPreferredTransport::Realtime => RuntimeCloudLinkTransport::Realtime,
        RuntimeCloudPreferredTransport::Zenoh => RuntimeCloudLinkTransport::Zenoh,
        RuntimeCloudPreferredTransport::Mesh => RuntimeCloudLinkTransport::Mesh,
        RuntimeCloudPreferredTransport::Mqtt => RuntimeCloudLinkTransport::Mqtt,
        RuntimeCloudPreferredTransport::ModbusTcp => RuntimeCloudLinkTransport::ModbusTcp,
        RuntimeCloudPreferredTransport::OpcUa => RuntimeCloudLinkTransport::OpcUa,
        RuntimeCloudPreferredTransport::Discovery => RuntimeCloudLinkTransport::Discovery,
        RuntimeCloudPreferredTransport::Web => RuntimeCloudLinkTransport::Web,
    }
}

fn runtime_cloud_link_channel_type(transport: RuntimeCloudLinkTransport) -> ChannelType {
    match transport {
        RuntimeCloudLinkTransport::Realtime => ChannelType::T0HardRt,
        RuntimeCloudLinkTransport::Zenoh => ChannelType::MeshT2Ops,
        RuntimeCloudLinkTransport::Mesh => ChannelType::MeshT1Fast,
        RuntimeCloudLinkTransport::Discovery => ChannelType::MeshT3Diag,
        RuntimeCloudLinkTransport::Mqtt
        | RuntimeCloudLinkTransport::ModbusTcp
        | RuntimeCloudLinkTransport::OpcUa
        | RuntimeCloudLinkTransport::Web => ChannelType::FederationBridge,
    }
}
