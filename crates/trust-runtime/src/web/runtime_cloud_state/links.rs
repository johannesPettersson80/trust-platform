use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use super::*;
use crate::runtime_cloud::link_policy;
use crate::runtime_cloud::projection::RuntimeCloudUiState;

pub(in crate::web) fn runtime_cloud_links_state_path(
    bundle_root: Option<&PathBuf>,
) -> Option<PathBuf> {
    let root = bundle_root?;
    Some(
        root.join(".trust")
            .join("runtime-cloud")
            .join("link-transport-state.json"),
    )
}

pub(in crate::web) fn runtime_cloud_links_load_state(
    path: Option<&Path>,
) -> Result<RuntimeCloudLinkTransportState, RuntimeError> {
    let Some(path) = path else {
        return Ok(RuntimeCloudLinkTransportState::default());
    };
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(RuntimeCloudLinkTransportState::default());
        }
        Err(err) => {
            return Err(RuntimeError::ControlError(
                format!(
                    "load runtime-cloud link transport state '{}': {err}",
                    path.display()
                )
                .into(),
            ));
        }
    };
    serde_json::from_str::<RuntimeCloudLinkTransportState>(&text).map_err(|err| {
        RuntimeError::ControlError(
            format!(
                "corrupt persisted runtime-cloud link transport state '{}': {err}",
                path.display()
            )
            .into(),
        )
    })
}

pub(in crate::web) fn runtime_cloud_links_store_state(
    path: Option<&Path>,
    state: &RuntimeCloudLinkTransportState,
) {
    let Some(path) = path else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(state) {
        let _ = std::fs::write(path, text);
    }
}

#[cfg(test)]
pub(in crate::web) fn runtime_cloud_link_transport_for(
    state: &Mutex<RuntimeCloudLinkTransportState>,
    source: &str,
    target: &str,
) -> Option<RuntimeCloudLinkTransport> {
    let guard = state.lock().ok()?;
    link_policy::runtime_cloud_link_transport_for(&guard, source, target)
}

pub(in crate::web) fn runtime_cloud_link_set_transport(
    state: &Mutex<RuntimeCloudLinkTransportState>,
    source: &str,
    target: &str,
    transport: RuntimeCloudLinkTransport,
    actor: &str,
    persist_path: Option<&Path>,
) -> Result<RuntimeCloudLinkTransportPreference, ReasonCode> {
    let mut guard = state.lock().map_err(|_| ReasonCode::TransportFailure)?;
    let preference = link_policy::runtime_cloud_set_link_transport(
        &mut guard,
        source,
        target,
        transport,
        actor,
        now_ns(),
    )?;
    runtime_cloud_links_store_state(persist_path, &guard);
    Ok(preference)
}

pub(in crate::web) fn runtime_cloud_seed_link_transport_preferences(
    state: &mut RuntimeCloudLinkTransportState,
    preferences: &[crate::config::RuntimeCloudLinkPreferenceRule],
    actor: &str,
) -> bool {
    link_policy::runtime_cloud_seed_link_transport_preferences(state, preferences, actor, now_ns())
}

pub(in crate::web) fn runtime_cloud_apply_link_transport_preferences(
    ui_state: &mut RuntimeCloudUiState,
    state: &Mutex<RuntimeCloudLinkTransportState>,
    discovery: Option<&DiscoveryState>,
) {
    let Ok(guard) = state.lock() else {
        return;
    };
    link_policy::runtime_cloud_apply_link_transport_preferences(
        ui_state,
        &guard,
        |source, target| {
            discovery
                .map(|discovery_state| {
                    runtime_cloud_link_is_same_host(discovery_state, source, target)
                })
                .unwrap_or(true)
        },
    );
}

pub(in crate::web) fn runtime_cloud_link_is_same_host(
    discovery: &DiscoveryState,
    source: &str,
    target: &str,
) -> bool {
    let source = source.trim();
    let target = target.trim();
    if source.is_empty() || target.is_empty() {
        return false;
    }
    if source == target {
        return true;
    }
    let host_groups_by_runtime = runtime_cloud_discovery_host_groups_by_runtime(discovery);
    if let Some(match_by_host_group) =
        runtime_cloud_host_group_match(source, target, &host_groups_by_runtime)
    {
        return match_by_host_group;
    }
    let addresses_by_runtime = runtime_cloud_discovery_addresses_by_runtime(discovery);
    let source_addresses = addresses_by_runtime
        .get(source)
        .cloned()
        .unwrap_or_default();
    let target_addresses = addresses_by_runtime
        .get(target)
        .cloned()
        .unwrap_or_default();
    link_policy::runtime_cloud_addresses_share_host(
        source_addresses.as_slice(),
        target_addresses.as_slice(),
    )
}

pub(in crate::web) fn runtime_cloud_compute_host_groups(
    discovery: Option<&DiscoveryState>,
    nodes: &[crate::runtime_cloud::projection::FleetNode],
) -> Vec<Vec<String>> {
    if nodes.is_empty() {
        return Vec::new();
    }
    let Some(discovery) = discovery else {
        let mut groups = nodes
            .iter()
            .map(|node| vec![node.runtime_id.clone()])
            .collect::<Vec<_>>();
        groups.sort_by(|a, b| a[0].cmp(&b[0]));
        return groups;
    };

    link_policy::runtime_cloud_compute_host_groups(nodes, |source, target| {
        runtime_cloud_link_is_same_host(discovery, source, target)
    })
}

pub(in crate::web) fn runtime_cloud_topology_feature_flags(
    profile: crate::config::RuntimeCloudProfile,
) -> BTreeMap<String, bool> {
    link_policy::runtime_cloud_topology_feature_flags(profile)
}

fn runtime_cloud_discovery_addresses_by_runtime(
    discovery: &DiscoveryState,
) -> BTreeMap<String, Vec<IpAddr>> {
    let mut by_runtime = BTreeMap::<String, Vec<IpAddr>>::new();
    for entry in discovery.snapshot() {
        by_runtime
            .entry(entry.name.to_string())
            .or_default()
            .extend(entry.addresses.iter().copied());
    }
    for addresses in by_runtime.values_mut() {
        addresses.sort_unstable();
        addresses.dedup();
    }
    by_runtime
}

fn runtime_cloud_discovery_host_groups_by_runtime(
    discovery: &DiscoveryState,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut by_runtime = BTreeMap::<String, BTreeSet<String>>::new();
    for entry in discovery.snapshot() {
        let Some(host_group) = entry.host_group.as_deref() else {
            continue;
        };
        let host_group = host_group.trim();
        if host_group.is_empty() {
            continue;
        }
        by_runtime
            .entry(entry.name.to_string())
            .or_default()
            .insert(host_group.to_ascii_lowercase());
    }
    by_runtime
}

fn runtime_cloud_host_group_match(
    source_runtime: &str,
    target_runtime: &str,
    host_groups_by_runtime: &BTreeMap<String, BTreeSet<String>>,
) -> Option<bool> {
    let source_groups = host_groups_by_runtime.get(source_runtime)?;
    let target_groups = host_groups_by_runtime.get(target_runtime)?;
    if source_groups.is_empty() || target_groups.is_empty() {
        return None;
    }
    Some(
        source_groups
            .iter()
            .any(|group| target_groups.contains(group)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    use smol_str::SmolStr;

    use crate::discovery::{DiscoveryEntry, DiscoveryState};
    use crate::runtime_cloud::projection::{
        project_runtime_cloud_state, ChannelType, PresenceThresholds, RuntimePeerObservation,
        UiContext, UiMode,
    };

    #[test]
    fn link_transport_preference_roundtrips_from_state() {
        let state = Mutex::new(RuntimeCloudLinkTransportState::default());
        let preference = runtime_cloud_link_set_transport(
            &state,
            "runtime-a",
            "runtime-b",
            RuntimeCloudLinkTransport::Realtime,
            "local://engineer",
            None,
        )
        .expect("set preference");
        assert_eq!(preference.transport, RuntimeCloudLinkTransport::Realtime);
        assert_eq!(
            runtime_cloud_link_transport_for(&state, "runtime-a", "runtime-b"),
            Some(RuntimeCloudLinkTransport::Realtime)
        );
    }

    #[test]
    fn seed_link_transport_preferences_applies_and_removes_toml_actor_entries() {
        use crate::config::{RuntimeCloudLinkPreferenceRule, RuntimeCloudPreferredTransport};

        let mut state = RuntimeCloudLinkTransportState::default();
        state.links.insert(
            "runtime-a->runtime-z".to_string(),
            RuntimeCloudLinkTransportPreference {
                source: "runtime-a".to_string(),
                target: "runtime-z".to_string(),
                transport: RuntimeCloudLinkTransport::Zenoh,
                actor: "runtime.toml".to_string(),
                updated_at_ns: 1,
            },
        );
        state.links.insert(
            "runtime-a->runtime-q".to_string(),
            RuntimeCloudLinkTransportPreference {
                source: "runtime-a".to_string(),
                target: "runtime-q".to_string(),
                transport: RuntimeCloudLinkTransport::Zenoh,
                actor: "local://engineer".to_string(),
                updated_at_ns: 2,
            },
        );
        let rules = vec![RuntimeCloudLinkPreferenceRule {
            source: SmolStr::new("runtime-a"),
            target: SmolStr::new("runtime-b"),
            transport: RuntimeCloudPreferredTransport::Realtime,
        }];

        let changed =
            runtime_cloud_seed_link_transport_preferences(&mut state, &rules, "runtime.toml");
        assert!(changed);
        assert!(state.links.contains_key("runtime-a->runtime-b"));
        assert!(!state.links.contains_key("runtime-a->runtime-z"));
        assert!(state.links.contains_key("runtime-a->runtime-q"));
    }

    #[test]
    fn apply_preferences_adds_t0_overlay_edge_without_removing_mesh() {
        let context = UiContext {
            connected_via: "runtime-a".to_string(),
            acting_on: vec!["runtime-b".to_string()],
            site_scope: vec!["default-site".to_string()],
            identity: "local://engineer".to_string(),
            role: "engineer".to_string(),
            mode: UiMode::Edit,
        };
        let peers = vec![presence_record_from_observation(
            &RuntimePeerObservation {
                runtime_id: "runtime-b".to_string(),
                site: "default-site".to_string(),
                display_name: "runtime-b".to_string(),
                mesh_reachable: true,
                last_seen_ns: 100,
            },
            120,
            PresenceThresholds {
                stale_timeout_ns: 1_000_000,
                partition_timeout_ns: 2_000_000,
            },
        )];
        let mut projected =
            project_runtime_cloud_state(context, "runtime-a", "default-site", 120, &peers);
        assert_eq!(
            projected.topology.edges[0].channel_type,
            ChannelType::MeshT2Ops
        );
        let state = Mutex::new(RuntimeCloudLinkTransportState::default());
        runtime_cloud_link_set_transport(
            &state,
            "runtime-a",
            "runtime-b",
            RuntimeCloudLinkTransport::Realtime,
            "local://engineer",
            None,
        )
        .expect("set preference");
        runtime_cloud_apply_link_transport_preferences(&mut projected, &state, None);
        assert_eq!(projected.topology.edges.len(), 2);
        assert_eq!(
            projected.topology.edges[0].channel_type,
            ChannelType::MeshT2Ops
        );
        assert_eq!(
            projected.topology.edges[1].channel_type,
            ChannelType::T0HardRt
        );
    }

    #[test]
    fn apply_preferences_overrides_channel_for_extended_transports() {
        let context = UiContext {
            connected_via: "runtime-a".to_string(),
            acting_on: vec!["runtime-b".to_string()],
            site_scope: vec!["default-site".to_string()],
            identity: "local://engineer".to_string(),
            role: "engineer".to_string(),
            mode: UiMode::Edit,
        };
        let peers = vec![presence_record_from_observation(
            &RuntimePeerObservation {
                runtime_id: "runtime-b".to_string(),
                site: "default-site".to_string(),
                display_name: "runtime-b".to_string(),
                mesh_reachable: true,
                last_seen_ns: 100,
            },
            120,
            PresenceThresholds {
                stale_timeout_ns: 1_000_000,
                partition_timeout_ns: 2_000_000,
            },
        )];
        let mut projected =
            project_runtime_cloud_state(context, "runtime-a", "default-site", 120, &peers);
        assert_eq!(projected.topology.edges.len(), 1);
        assert_eq!(
            projected.topology.edges[0].channel_type,
            ChannelType::MeshT2Ops
        );

        let state = Mutex::new(RuntimeCloudLinkTransportState::default());
        runtime_cloud_link_set_transport(
            &state,
            "runtime-a",
            "runtime-b",
            RuntimeCloudLinkTransport::Mqtt,
            "local://engineer",
            None,
        )
        .expect("set preference");
        runtime_cloud_apply_link_transport_preferences(&mut projected, &state, None);
        assert_eq!(projected.topology.edges.len(), 1);
        assert_eq!(
            projected.topology.edges[0].channel_type,
            ChannelType::FederationBridge
        );
    }

    #[test]
    fn same_host_check_uses_discovery_address_overlap() {
        let discovery = DiscoveryState::new();
        discovery.replace_entries(vec![
            DiscoveryEntry {
                id: SmolStr::new("runtime-a-111"),
                name: SmolStr::new("runtime-a"),
                addresses: vec![
                    IpAddr::V4(Ipv4Addr::new(10, 0, 0, 10)),
                    IpAddr::V6(Ipv6Addr::LOCALHOST),
                ],
                web_port: Some(8080),
                web_tls: false,
                mesh_port: Some(5200),
                control: None,
                host_group: None,
                last_seen_ns: 1,
            },
            DiscoveryEntry {
                id: SmolStr::new("runtime-b-222"),
                name: SmolStr::new("runtime-b"),
                addresses: vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 10))],
                web_port: Some(8081),
                web_tls: false,
                mesh_port: Some(5201),
                control: None,
                host_group: None,
                last_seen_ns: 1,
            },
            DiscoveryEntry {
                id: SmolStr::new("runtime-c-333"),
                name: SmolStr::new("runtime-c"),
                addresses: vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 20))],
                web_port: Some(8082),
                web_tls: false,
                mesh_port: Some(5202),
                control: None,
                host_group: None,
                last_seen_ns: 1,
            },
        ]);
        assert!(runtime_cloud_link_is_same_host(
            &discovery,
            "runtime-a",
            "runtime-b"
        ));
        assert!(!runtime_cloud_link_is_same_host(
            &discovery,
            "runtime-a",
            "runtime-c"
        ));
    }

    #[test]
    fn same_host_check_prefers_host_group_when_present() {
        let discovery = DiscoveryState::new();
        discovery.replace_entries(vec![
            DiscoveryEntry {
                id: SmolStr::new("runtime-a-111"),
                name: SmolStr::new("runtime-a"),
                addresses: vec![IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))],
                web_port: Some(8080),
                web_tls: false,
                mesh_port: Some(5200),
                control: None,
                host_group: Some(SmolStr::new("host-a")),
                last_seen_ns: 1,
            },
            DiscoveryEntry {
                id: SmolStr::new("runtime-b-222"),
                name: SmolStr::new("runtime-b"),
                addresses: vec![IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))],
                web_port: Some(8081),
                web_tls: false,
                mesh_port: Some(5201),
                control: None,
                host_group: Some(SmolStr::new("host-b")),
                last_seen_ns: 1,
            },
        ]);
        assert!(!runtime_cloud_link_is_same_host(
            &discovery,
            "runtime-a",
            "runtime-b"
        ));
    }

    #[test]
    fn compute_host_groups_two_same_host() {
        use crate::runtime_cloud::projection::{
            ConfigState, FleetNode, FleetRole, HealthState, LifecycleState, TrustState,
        };
        let discovery = DiscoveryState::new();
        discovery.replace_entries(vec![
            DiscoveryEntry {
                id: SmolStr::new("a-1"),
                name: SmolStr::new("runtime-a"),
                addresses: vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 10))],
                web_port: Some(8080),
                web_tls: false,
                mesh_port: Some(5200),
                control: None,
                host_group: None,
                last_seen_ns: 1,
            },
            DiscoveryEntry {
                id: SmolStr::new("b-2"),
                name: SmolStr::new("runtime-b"),
                addresses: vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 10))],
                web_port: Some(8081),
                web_tls: false,
                mesh_port: Some(5201),
                control: None,
                host_group: None,
                last_seen_ns: 1,
            },
        ]);
        let make_node = |id: &str| FleetNode {
            runtime_id: id.to_string(),
            site: "s".to_string(),
            display_name: id.to_string(),
            role: FleetRole::Member,
            lifecycle_state: LifecycleState::Online,
            health_state: HealthState::Healthy,
            trust_state: TrustState::Trusted,
            config_state: ConfigState::InSync,
            last_seen_ns: 1,
        };
        let nodes = vec![make_node("runtime-a"), make_node("runtime-b")];
        let groups = runtime_cloud_compute_host_groups(Some(&discovery), &nodes);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0], vec!["runtime-a", "runtime-b"]);
    }

    #[test]
    fn compute_host_groups_three_mixed() {
        use crate::runtime_cloud::projection::{
            ConfigState, FleetNode, FleetRole, HealthState, LifecycleState, TrustState,
        };
        let discovery = DiscoveryState::new();
        discovery.replace_entries(vec![
            DiscoveryEntry {
                id: SmolStr::new("a-1"),
                name: SmolStr::new("runtime-a"),
                addresses: vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 10))],
                web_port: Some(8080),
                web_tls: false,
                mesh_port: Some(5200),
                control: None,
                host_group: None,
                last_seen_ns: 1,
            },
            DiscoveryEntry {
                id: SmolStr::new("b-2"),
                name: SmolStr::new("runtime-b"),
                addresses: vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 10))],
                web_port: Some(8081),
                web_tls: false,
                mesh_port: Some(5201),
                control: None,
                host_group: None,
                last_seen_ns: 1,
            },
            DiscoveryEntry {
                id: SmolStr::new("c-3"),
                name: SmolStr::new("runtime-c"),
                addresses: vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 20))],
                web_port: Some(8082),
                web_tls: false,
                mesh_port: Some(5202),
                control: None,
                host_group: None,
                last_seen_ns: 1,
            },
        ]);
        let make_node = |id: &str| FleetNode {
            runtime_id: id.to_string(),
            site: "s".to_string(),
            display_name: id.to_string(),
            role: FleetRole::Member,
            lifecycle_state: LifecycleState::Online,
            health_state: HealthState::Healthy,
            trust_state: TrustState::Trusted,
            config_state: ConfigState::InSync,
            last_seen_ns: 1,
        };
        let nodes = vec![
            make_node("runtime-a"),
            make_node("runtime-b"),
            make_node("runtime-c"),
        ];
        let groups = runtime_cloud_compute_host_groups(Some(&discovery), &nodes);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], vec!["runtime-a", "runtime-b"]);
        assert_eq!(groups[1], vec!["runtime-c"]);
    }

    #[test]
    fn compute_host_groups_empty_discovery() {
        use crate::runtime_cloud::projection::{
            ConfigState, FleetNode, FleetRole, HealthState, LifecycleState, TrustState,
        };
        let make_node = |id: &str| FleetNode {
            runtime_id: id.to_string(),
            site: "s".to_string(),
            display_name: id.to_string(),
            role: FleetRole::Member,
            lifecycle_state: LifecycleState::Online,
            health_state: HealthState::Healthy,
            trust_state: TrustState::Trusted,
            config_state: ConfigState::InSync,
            last_seen_ns: 1,
        };
        let nodes = vec![make_node("runtime-a"), make_node("runtime-b")];
        let groups = runtime_cloud_compute_host_groups(None, &nodes);
        assert_eq!(groups, vec![vec!["runtime-a"], vec!["runtime-b"]]);
    }

    #[test]
    fn compute_host_groups_deterministic_ordering() {
        use crate::runtime_cloud::projection::{
            ConfigState, FleetNode, FleetRole, HealthState, LifecycleState, TrustState,
        };
        let discovery = DiscoveryState::new();
        discovery.replace_entries(vec![
            DiscoveryEntry {
                id: SmolStr::new("z-1"),
                name: SmolStr::new("runtime-z"),
                addresses: vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 10))],
                web_port: Some(8080),
                web_tls: false,
                mesh_port: Some(5200),
                control: None,
                host_group: None,
                last_seen_ns: 1,
            },
            DiscoveryEntry {
                id: SmolStr::new("a-2"),
                name: SmolStr::new("runtime-a"),
                addresses: vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 10))],
                web_port: Some(8081),
                web_tls: false,
                mesh_port: Some(5201),
                control: None,
                host_group: None,
                last_seen_ns: 1,
            },
        ]);
        let make_node = |id: &str| FleetNode {
            runtime_id: id.to_string(),
            site: "s".to_string(),
            display_name: id.to_string(),
            role: FleetRole::Member,
            lifecycle_state: LifecycleState::Online,
            health_state: HealthState::Healthy,
            trust_state: TrustState::Trusted,
            config_state: ConfigState::InSync,
            last_seen_ns: 1,
        };
        // Reverse order input to test sorting
        let nodes = vec![make_node("runtime-z"), make_node("runtime-a")];
        let groups = runtime_cloud_compute_host_groups(Some(&discovery), &nodes);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0], vec!["runtime-a", "runtime-z"]);
    }

    #[test]
    fn topology_feature_flags_dev_all_enabled() {
        use crate::config::RuntimeCloudProfile;
        let flags = runtime_cloud_topology_feature_flags(RuntimeCloudProfile::Dev);
        assert_eq!(flags.get("host_containers"), Some(&true));
        assert_eq!(flags.get("device_discovery"), Some(&true));
        assert_eq!(flags.get("edit_mode"), Some(&true));
        assert_eq!(flags.get("module_slots"), Some(&true));
    }

    #[test]
    fn topology_feature_flags_plant_only_host_containers() {
        use crate::config::RuntimeCloudProfile;
        let flags = runtime_cloud_topology_feature_flags(RuntimeCloudProfile::Plant);
        assert_eq!(flags.get("host_containers"), Some(&true));
        assert_eq!(flags.get("device_discovery"), None);
        assert_eq!(flags.get("edit_mode"), None);
    }
}
