//! Runtime cloud UI projection contracts.

#![allow(missing_docs)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::contracts::{ReasonCode, RUNTIME_CLOUD_API_VERSION};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiMode {
    View,
    Edit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiContext {
    pub connected_via: String,
    pub acting_on: Vec<String>,
    pub site_scope: Vec<String>,
    pub identity: String,
    pub role: String,
    pub mode: UiMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FleetRole {
    Active,
    Standby,
    Member,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    Online,
    Stale,
    Partitioned,
    Offline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthState {
    Healthy,
    Degraded,
    Faulted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustState {
    Trusted,
    Untrusted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigState {
    InSync,
    Pending,
    Blocked,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetNode {
    pub runtime_id: String,
    pub site: String,
    pub display_name: String,
    pub role: FleetRole,
    pub lifecycle_state: LifecycleState,
    pub health_state: HealthState,
    pub trust_state: TrustState,
    pub config_state: ConfigState,
    pub last_seen_ns: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelType {
    T0HardRt,
    MeshT1Fast,
    MeshT2Ops,
    MeshT3Diag,
    HaReplication,
    FederationBridge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelState {
    Healthy,
    Degraded,
    Stale,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetEdge {
    pub source: String,
    pub target: String,
    pub channel_type: ChannelType,
    pub state: ChannelState,
    pub latency_ms_p95: Option<f64>,
    pub loss_pct: Option<f64>,
    pub stale: bool,
    pub last_ok_ns: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionEnvelope {
    pub request_id: String,
    pub connected_via: String,
    pub target_runtimes: Vec<String>,
    pub actor: String,
    pub action_type: String,
    pub dry_run: bool,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionPreflightResult {
    pub request_id: String,
    pub allowed: bool,
    pub denial_code: Option<ReasonCode>,
    pub denial_reason: Option<String>,
    pub affected_targets: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimelineCategory {
    Audit,
    Config,
    Rollout,
    Communication,
    HaRole,
    Security,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimelineSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetTimelineEvent {
    pub event_id: String,
    pub timestamp_ns: u64,
    pub category: TimelineCategory,
    pub runtime_id: String,
    pub request_id: Option<String>,
    pub summary: String,
    pub severity: TimelineSeverity,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetTopology {
    pub nodes: Vec<FleetNode>,
    pub edges: Vec<FleetEdge>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub host_groups: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeCloudUiState {
    pub api_version: String,
    pub context: UiContext,
    pub topology: FleetTopology,
    pub timeline: Vec<FleetTimelineEvent>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub feature_flags: BTreeMap<String, bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePresenceRecord {
    pub runtime_id: String,
    pub site: String,
    pub display_name: String,
    pub mesh_reachable: bool,
    pub last_seen_ns: u64,
    pub stale: bool,
    pub partitioned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePeerObservation {
    // Transport-plane snapshot passed into cloud-plane projection.
    pub runtime_id: String,
    pub site: String,
    pub display_name: String,
    pub mesh_reachable: bool,
    pub last_seen_ns: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresenceThresholds {
    pub stale_timeout_ns: u64,
    pub partition_timeout_ns: u64,
}

pub fn presence_record_from_observation(
    observation: &RuntimePeerObservation,
    now_ns: u64,
    thresholds: PresenceThresholds,
) -> RuntimePresenceRecord {
    let age_ns = now_ns.saturating_sub(observation.last_seen_ns);
    let stale = !observation.mesh_reachable || age_ns >= thresholds.stale_timeout_ns;
    let partitioned = !observation.mesh_reachable && age_ns >= thresholds.partition_timeout_ns;
    RuntimePresenceRecord {
        runtime_id: observation.runtime_id.clone(),
        site: observation.site.clone(),
        display_name: observation.display_name.clone(),
        mesh_reachable: observation.mesh_reachable,
        last_seen_ns: observation.last_seen_ns,
        stale,
        partitioned,
    }
}

pub fn project_runtime_cloud_state(
    context: UiContext,
    local_runtime_id: &str,
    local_site: &str,
    now_ns: u64,
    peers: &[RuntimePresenceRecord],
) -> RuntimeCloudUiState {
    let mut nodes = BTreeMap::<String, FleetNode>::new();
    let mut edges = Vec::new();
    let mut timeline = Vec::new();
    nodes.insert(
        local_runtime_id.to_string(),
        FleetNode {
            runtime_id: local_runtime_id.to_string(),
            site: local_site.to_string(),
            display_name: local_runtime_id.to_string(),
            role: FleetRole::Active,
            lifecycle_state: LifecycleState::Online,
            health_state: HealthState::Healthy,
            trust_state: TrustState::Trusted,
            config_state: ConfigState::InSync,
            last_seen_ns: now_ns,
        },
    );

    for peer in peers {
        if peer.runtime_id.as_str() == local_runtime_id {
            continue;
        }
        let lifecycle_state = if !peer.mesh_reachable && peer.last_seen_ns == 0 {
            LifecycleState::Offline
        } else if peer.partitioned {
            LifecycleState::Partitioned
        } else if peer.stale {
            LifecycleState::Stale
        } else {
            LifecycleState::Online
        };
        let health_state = if peer.stale || peer.partitioned {
            HealthState::Degraded
        } else {
            HealthState::Healthy
        };
        let config_state = if peer.stale || peer.partitioned {
            ConfigState::Pending
        } else {
            ConfigState::InSync
        };
        nodes.insert(
            peer.runtime_id.to_string(),
            FleetNode {
                runtime_id: peer.runtime_id.clone(),
                site: peer.site.clone(),
                display_name: peer.display_name.clone(),
                role: FleetRole::Member,
                lifecycle_state,
                health_state,
                trust_state: TrustState::Trusted,
                config_state,
                last_seen_ns: peer.last_seen_ns,
            },
        );
        let state = if lifecycle_state == LifecycleState::Offline || peer.partitioned {
            ChannelState::Failed
        } else if peer.stale {
            ChannelState::Stale
        } else if !peer.mesh_reachable {
            ChannelState::Degraded
        } else {
            ChannelState::Healthy
        };
        let stale = peer.stale || peer.partitioned || lifecycle_state == LifecycleState::Offline;
        edges.push(FleetEdge {
            source: local_runtime_id.to_string(),
            target: peer.runtime_id.clone(),
            channel_type: ChannelType::MeshT2Ops,
            state,
            latency_ms_p95: if stale { None } else { Some(12.0) },
            loss_pct: if stale { None } else { Some(0.0) },
            stale,
            last_ok_ns: peer.last_seen_ns,
        });
        if stale {
            timeline.push(FleetTimelineEvent {
                event_id: format!("evt-link-state-{}", peer.runtime_id),
                timestamp_ns: now_ns,
                category: TimelineCategory::Communication,
                runtime_id: peer.runtime_id.clone(),
                request_id: None,
                summary: if lifecycle_state == LifecycleState::Offline {
                    format!("{} is offline (no discovery heartbeat)", peer.runtime_id)
                } else {
                    format!("{} marked stale by discovery timeout", peer.runtime_id)
                },
                severity: TimelineSeverity::Warning,
            });
        }
    }

    RuntimeCloudUiState {
        api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
        context,
        topology: FleetTopology {
            nodes: nodes.into_values().collect(),
            edges,
            host_groups: Vec::new(),
        },
        timeline,
        feature_flags: BTreeMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_marks_stale_peers_and_creates_warning_timeline() {
        let context = UiContext {
            connected_via: "runtime-a".to_string(),
            acting_on: vec!["runtime-b".to_string()],
            site_scope: vec!["site-a".to_string()],
            identity: "spiffe://trust/site-a/operator".to_string(),
            role: "operator".to_string(),
            mode: UiMode::View,
        };
        let peers = vec![RuntimePresenceRecord {
            runtime_id: "runtime-b".to_string(),
            site: "site-a".to_string(),
            display_name: "Mixer-02".to_string(),
            mesh_reachable: true,
            last_seen_ns: 77,
            stale: true,
            partitioned: false,
        }];

        let state = project_runtime_cloud_state(context, "runtime-a", "site-a", 88, &peers);
        assert_eq!(state.api_version, RUNTIME_CLOUD_API_VERSION);
        assert_eq!(state.topology.nodes.len(), 2);
        assert_eq!(state.topology.edges.len(), 1);
        assert_eq!(state.topology.edges[0].state, ChannelState::Stale);
        assert_eq!(state.timeline.len(), 1);
        assert_eq!(state.timeline[0].severity, TimelineSeverity::Warning);
    }

    #[test]
    fn runtime_cloud_projection_contract_reports_topology_edges_and_warnings() {
        let context = UiContext {
            connected_via: "runtime-a".to_string(),
            acting_on: vec![
                "runtime-a".to_string(),
                "runtime-b".to_string(),
                "runtime-c".to_string(),
                "runtime-d".to_string(),
                "runtime-z".to_string(),
            ],
            site_scope: vec!["site-a".to_string(), "site-b".to_string()],
            identity: "spiffe://trust/site-a/operator".to_string(),
            role: "engineer".to_string(),
            mode: UiMode::Edit,
        };
        let peers = vec![
            RuntimePresenceRecord {
                runtime_id: "runtime-b".to_string(),
                site: "site-a".to_string(),
                display_name: "Mixer-02".to_string(),
                mesh_reachable: true,
                last_seen_ns: 950,
                stale: false,
                partitioned: false,
            },
            RuntimePresenceRecord {
                runtime_id: "runtime-c".to_string(),
                site: "site-a".to_string(),
                display_name: "Mixer-03".to_string(),
                mesh_reachable: true,
                last_seen_ns: 800,
                stale: true,
                partitioned: false,
            },
            RuntimePresenceRecord {
                runtime_id: "runtime-d".to_string(),
                site: "site-b".to_string(),
                display_name: "Mixer-04".to_string(),
                mesh_reachable: false,
                last_seen_ns: 700,
                stale: true,
                partitioned: true,
            },
            RuntimePresenceRecord {
                runtime_id: "runtime-z".to_string(),
                site: "site-b".to_string(),
                display_name: "Offline-Z".to_string(),
                mesh_reachable: false,
                last_seen_ns: 0,
                stale: true,
                partitioned: false,
            },
        ];

        let state = project_runtime_cloud_state(context, "runtime-a", "site-a", 1_000, &peers);

        assert_eq!(state.api_version, RUNTIME_CLOUD_API_VERSION);
        assert_eq!(state.context.connected_via, "runtime-a");
        assert_eq!(state.context.role, "engineer");
        assert_eq!(state.context.mode, UiMode::Edit);
        assert_eq!(state.topology.nodes.len(), 5);
        assert_eq!(state.topology.edges.len(), 4);
        assert_eq!(
            state
                .topology
                .nodes
                .iter()
                .map(|node| node.runtime_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "runtime-a",
                "runtime-b",
                "runtime-c",
                "runtime-d",
                "runtime-z"
            ]
        );

        let local = state
            .topology
            .nodes
            .iter()
            .find(|node| node.runtime_id == "runtime-a")
            .expect("local node");
        assert_eq!(local.role, FleetRole::Active);
        assert_eq!(local.lifecycle_state, LifecycleState::Online);
        assert_eq!(local.health_state, HealthState::Healthy);
        assert_eq!(local.trust_state, TrustState::Trusted);
        assert_eq!(local.config_state, ConfigState::InSync);
        assert_eq!(local.last_seen_ns, 1_000);

        let healthy = state
            .topology
            .nodes
            .iter()
            .find(|node| node.runtime_id == "runtime-b")
            .expect("healthy peer");
        assert_eq!(healthy.role, FleetRole::Member);
        assert_eq!(healthy.lifecycle_state, LifecycleState::Online);
        assert_eq!(healthy.health_state, HealthState::Healthy);
        assert_eq!(healthy.config_state, ConfigState::InSync);

        let stale = state
            .topology
            .nodes
            .iter()
            .find(|node| node.runtime_id == "runtime-c")
            .expect("stale peer");
        assert_eq!(stale.lifecycle_state, LifecycleState::Stale);
        assert_eq!(stale.health_state, HealthState::Degraded);
        assert_eq!(stale.config_state, ConfigState::Pending);

        let partitioned = state
            .topology
            .nodes
            .iter()
            .find(|node| node.runtime_id == "runtime-d")
            .expect("partitioned peer");
        assert_eq!(partitioned.lifecycle_state, LifecycleState::Partitioned);
        assert_eq!(partitioned.health_state, HealthState::Degraded);
        assert_eq!(partitioned.config_state, ConfigState::Pending);

        let offline = state
            .topology
            .nodes
            .iter()
            .find(|node| node.runtime_id == "runtime-z")
            .expect("offline peer");
        assert_eq!(offline.lifecycle_state, LifecycleState::Offline);
        assert_eq!(offline.health_state, HealthState::Degraded);
        assert_eq!(offline.config_state, ConfigState::Pending);

        let edge_to_b = state
            .topology
            .edges
            .iter()
            .find(|edge| edge.target == "runtime-b")
            .expect("edge to runtime-b");
        assert_eq!(edge_to_b.channel_type, ChannelType::MeshT2Ops);
        assert_eq!(edge_to_b.state, ChannelState::Healthy);
        assert!(!edge_to_b.stale);
        assert_eq!(edge_to_b.latency_ms_p95, Some(12.0));
        assert_eq!(edge_to_b.loss_pct, Some(0.0));

        let edge_to_c = state
            .topology
            .edges
            .iter()
            .find(|edge| edge.target == "runtime-c")
            .expect("edge to runtime-c");
        assert_eq!(edge_to_c.state, ChannelState::Stale);
        assert!(edge_to_c.stale);
        assert_eq!(edge_to_c.latency_ms_p95, None);
        assert_eq!(edge_to_c.loss_pct, None);

        let edge_to_d = state
            .topology
            .edges
            .iter()
            .find(|edge| edge.target == "runtime-d")
            .expect("edge to runtime-d");
        assert_eq!(edge_to_d.state, ChannelState::Failed);
        assert!(edge_to_d.stale);

        let edge_to_z = state
            .topology
            .edges
            .iter()
            .find(|edge| edge.target == "runtime-z")
            .expect("edge to runtime-z");
        assert_eq!(edge_to_z.state, ChannelState::Failed);
        assert!(edge_to_z.stale);
        assert_eq!(edge_to_z.last_ok_ns, 0);

        assert_eq!(state.timeline.len(), 3);
        assert!(state.timeline.iter().all(|event| {
            event.category == TimelineCategory::Communication
                && event.severity == TimelineSeverity::Warning
                && event.timestamp_ns == 1_000
        }));
        assert!(state
            .timeline
            .iter()
            .any(|event| event.summary == "runtime-z is offline (no discovery heartbeat)"));
    }

    #[test]
    fn projection_marks_unseen_unreachable_peer_offline() {
        let context = UiContext {
            connected_via: "runtime-a".to_string(),
            acting_on: vec!["runtime-z".to_string()],
            site_scope: vec!["site-a".to_string()],
            identity: "spiffe://trust/site-a/operator".to_string(),
            role: "operator".to_string(),
            mode: UiMode::View,
        };
        let peers = vec![RuntimePresenceRecord {
            runtime_id: "runtime-z".to_string(),
            site: "site-a".to_string(),
            display_name: "Offline-Z".to_string(),
            mesh_reachable: false,
            last_seen_ns: 0,
            stale: true,
            partitioned: false,
        }];

        let state = project_runtime_cloud_state(context, "runtime-a", "site-a", 88, &peers);
        let peer = state
            .topology
            .nodes
            .iter()
            .find(|node| node.runtime_id == "runtime-z")
            .expect("offline peer node");
        assert_eq!(peer.lifecycle_state, LifecycleState::Offline);
        assert_eq!(state.topology.edges[0].state, ChannelState::Failed);
        assert_eq!(state.timeline.len(), 1);
    }

    #[test]
    fn presence_projection_transitions_stale_before_partitioned() {
        let observation = RuntimePeerObservation {
            runtime_id: "runtime-b".to_string(),
            site: "site-a".to_string(),
            display_name: "Mixer-02".to_string(),
            mesh_reachable: false,
            last_seen_ns: 100,
        };
        let thresholds = PresenceThresholds {
            stale_timeout_ns: 10,
            partition_timeout_ns: 50,
        };

        let stale = presence_record_from_observation(&observation, 120, thresholds);
        assert!(stale.stale);
        assert!(!stale.partitioned);

        let partitioned = presence_record_from_observation(&observation, 200, thresholds);
        assert!(partitioned.stale);
        assert!(partitioned.partitioned);
    }

    #[test]
    fn presence_projection_contract_does_not_stale_future_heartbeat() {
        let observation = RuntimePeerObservation {
            runtime_id: "runtime-b".to_string(),
            site: "site-a".to_string(),
            display_name: "Mixer-02".to_string(),
            mesh_reachable: true,
            last_seen_ns: 200,
        };
        let thresholds = PresenceThresholds {
            stale_timeout_ns: 10,
            partition_timeout_ns: 50,
        };

        let record = presence_record_from_observation(&observation, 100, thresholds);

        assert!(!record.stale);
        assert!(!record.partitioned);
        assert_eq!(record.last_seen_ns, 200);
    }

    #[test]
    fn host_groups_omitted_when_empty() {
        let topology = FleetTopology {
            nodes: vec![],
            edges: vec![],
            host_groups: vec![],
        };
        let json = serde_json::to_string(&topology).unwrap();
        assert!(!json.contains("host_groups"));
    }

    #[test]
    fn host_groups_roundtrips_when_present() {
        let topology = FleetTopology {
            nodes: vec![],
            edges: vec![],
            host_groups: vec![vec!["a".into(), "b".into()]],
        };
        let json = serde_json::to_string(&topology).unwrap();
        assert!(json.contains("host_groups"));
        let parsed: FleetTopology = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed.host_groups,
            vec![vec!["a".to_string(), "b".to_string()]]
        );
    }

    #[test]
    fn feature_flags_omitted_when_empty() {
        let state = RuntimeCloudUiState {
            api_version: "1.0".into(),
            context: UiContext {
                connected_via: "r".into(),
                acting_on: vec![],
                site_scope: vec![],
                identity: "local://e".into(),
                role: "engineer".into(),
                mode: UiMode::Edit,
            },
            topology: FleetTopology {
                nodes: vec![],
                edges: vec![],
                host_groups: vec![],
            },
            timeline: vec![],
            feature_flags: BTreeMap::new(),
        };
        let json = serde_json::to_string(&state).unwrap();
        assert!(!json.contains("feature_flags"));
    }

    #[test]
    fn feature_flags_roundtrips_when_present() {
        let mut flags = BTreeMap::new();
        flags.insert("host_containers".to_string(), true);
        flags.insert("device_discovery".to_string(), false);
        let state = RuntimeCloudUiState {
            api_version: "1.0".into(),
            context: UiContext {
                connected_via: "r".into(),
                acting_on: vec![],
                site_scope: vec![],
                identity: "local://e".into(),
                role: "engineer".into(),
                mode: UiMode::Edit,
            },
            topology: FleetTopology {
                nodes: vec![],
                edges: vec![],
                host_groups: vec![],
            },
            timeline: vec![],
            feature_flags: flags.clone(),
        };
        let json = serde_json::to_string(&state).unwrap();
        let parsed: RuntimeCloudUiState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.feature_flags, flags);
    }

    #[test]
    fn missing_host_groups_deserializes_to_empty() {
        let json = r#"{"nodes":[],"edges":[]}"#;
        let topology: FleetTopology = serde_json::from_str(json).unwrap();
        assert!(topology.host_groups.is_empty());
    }
}
