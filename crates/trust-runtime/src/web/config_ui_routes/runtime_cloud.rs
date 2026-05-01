use super::*;

pub(super) fn host_groups_from_workspace(workspace: &WorkspaceModel) -> Vec<Vec<String>> {
    let mut grouped = BTreeMap::<String, Vec<String>>::new();
    for runtime in &workspace.runtimes {
        let group_key = runtime
            .runtime
            .discovery
            .host_group
            .as_ref()
            .map(|value| value.to_string())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("host-{}", runtime.runtime_id));
        grouped
            .entry(group_key)
            .or_default()
            .push(runtime.runtime_id.clone());
    }
    grouped
        .into_values()
        .map(|mut ids| {
            ids.sort();
            ids
        })
        .collect()
}

pub(super) fn config_mode_runtime_cloud_state(workspace: &WorkspaceModel) -> RuntimeCloudUiState {
    let now = now_ns();
    let connected_via = workspace
        .runtimes
        .first()
        .map(|runtime| runtime.runtime_id.clone())
        .unwrap_or_else(|| "runtime-1".to_string());
    let acting_on = workspace
        .runtimes
        .iter()
        .map(|runtime| runtime.runtime_id.clone())
        .collect::<Vec<_>>();
    let peers = workspace
        .runtimes
        .iter()
        .filter(|runtime| runtime.runtime_id != connected_via)
        .map(|runtime| RuntimePresenceRecord {
            runtime_id: runtime.runtime_id.clone(),
            site: "local".to_string(),
            display_name: runtime.runtime_id.clone(),
            mesh_reachable: true,
            last_seen_ns: now,
            stale: false,
            partitioned: false,
        })
        .collect::<Vec<_>>();

    let mut state = project_runtime_cloud_state(
        UiContext {
            connected_via: connected_via.clone(),
            acting_on,
            site_scope: vec!["local".to_string()],
            identity: "config://local-engineering".to_string(),
            role: "engineer".to_string(),
            mode: UiMode::Edit,
        },
        connected_via.as_str(),
        "local",
        now,
        &peers,
    );
    state.topology.host_groups = host_groups_from_workspace(workspace);
    state.feature_flags = runtime_cloud_topology_feature_flags(RuntimeCloudProfile::Dev);
    state.feature_flags.insert("edit_mode".to_string(), true);

    let mut existing = BTreeSet::<(String, String)>::new();
    for edge in &state.topology.edges {
        existing.insert((edge.source.clone(), edge.target.clone()));
    }

    let known = workspace
        .runtimes
        .iter()
        .map(|runtime| runtime.runtime_id.clone())
        .collect::<BTreeSet<_>>();

    for runtime in &workspace.runtimes {
        for preference in &runtime.runtime.runtime_cloud_link_preferences {
            let source = preference.source.to_string();
            let target = preference.target.to_string();
            if source == target || !known.contains(&source) || !known.contains(&target) {
                continue;
            }
            let channel = match preference.transport {
                crate::config::RuntimeCloudPreferredTransport::Realtime => ChannelType::T0HardRt,
                crate::config::RuntimeCloudPreferredTransport::Zenoh => ChannelType::MeshT2Ops,
                crate::config::RuntimeCloudPreferredTransport::Mesh => ChannelType::MeshT1Fast,
                crate::config::RuntimeCloudPreferredTransport::Discovery => ChannelType::MeshT3Diag,
                crate::config::RuntimeCloudPreferredTransport::Mqtt
                | crate::config::RuntimeCloudPreferredTransport::ModbusTcp
                | crate::config::RuntimeCloudPreferredTransport::OpcUa
                | crate::config::RuntimeCloudPreferredTransport::Web => {
                    ChannelType::FederationBridge
                }
            };
            if let Some(edge) = state
                .topology
                .edges
                .iter_mut()
                .find(|edge| edge.source == source && edge.target == target)
            {
                edge.channel_type = channel;
                continue;
            }
            if existing.insert((source.clone(), target.clone())) {
                state.topology.edges.push(FleetEdge {
                    source,
                    target,
                    channel_type: channel,
                    state: ChannelState::Healthy,
                    latency_ms_p95: Some(2.0),
                    loss_pct: Some(0.0),
                    stale: false,
                    last_ok_ns: now,
                });
            }
        }
    }

    apply_config_mode_offline_projection(&mut state);
    apply_config_mode_live_overlay(&mut state);
    state
}

fn apply_config_mode_offline_projection(state: &mut RuntimeCloudUiState) {
    // Config UI can run without live runtimes. Render topology as planned/offline by default.
    for node in &mut state.topology.nodes {
        node.lifecycle_state = crate::runtime_cloud::projection::LifecycleState::Offline;
        node.health_state = crate::runtime_cloud::projection::HealthState::Degraded;
        node.config_state = crate::runtime_cloud::projection::ConfigState::Pending;
        node.last_seen_ns = 0;
    }
    for edge in &mut state.topology.edges {
        edge.state = ChannelState::Failed;
        edge.stale = true;
        edge.latency_ms_p95 = None;
        edge.loss_pct = None;
        edge.last_ok_ns = 0;
    }
    state.timeline.clear();
}

fn apply_config_mode_live_overlay(state: &mut RuntimeCloudUiState) {
    let remote = config_ui_live_runtime_cloud_overlay();
    let Some(remote) = remote else {
        return;
    };

    let remote_nodes = remote
        .topology
        .nodes
        .into_iter()
        .map(|node| (node.runtime_id.clone(), node))
        .collect::<BTreeMap<_, _>>();
    let mut overlay_count = 0usize;
    for node in &mut state.topology.nodes {
        if let Some(remote_node) = remote_nodes.get(node.runtime_id.as_str()) {
            node.lifecycle_state = remote_node.lifecycle_state;
            node.health_state = remote_node.health_state;
            node.config_state = remote_node.config_state;
            node.last_seen_ns = remote_node.last_seen_ns;
            overlay_count += 1;
        }
    }

    for edge in &mut state.topology.edges {
        if let Some(remote_edge) = remote
            .topology
            .edges
            .iter()
            .find(|candidate| {
                candidate.source == edge.source
                    && candidate.target == edge.target
                    && candidate.channel_type == edge.channel_type
            })
            .or_else(|| {
                remote.topology.edges.iter().find(|candidate| {
                    candidate.source == edge.source && candidate.target == edge.target
                })
            })
        {
            edge.state = remote_edge.state;
            edge.latency_ms_p95 = remote_edge.latency_ms_p95;
            edge.loss_pct = remote_edge.loss_pct;
            edge.stale = remote_edge.stale;
            edge.last_ok_ns = remote_edge.last_ok_ns;
        }
    }
    if overlay_count > 0 {
        state
            .feature_flags
            .insert("config_live_overlay".to_string(), true);
    }
}

pub(super) fn config_mode_runtime_cloud_config_snapshot(
    workspace: &WorkspaceModel,
) -> RuntimeCloudConfigSnapshot {
    let runtime_id = workspace
        .runtimes
        .first()
        .map(|runtime| runtime.runtime_id.clone())
        .unwrap_or_else(|| "runtime-1".to_string());
    let state = runtime_cloud_config_initial_state();
    RuntimeCloudConfigSnapshot {
        api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
        runtime_id,
        desired: state.desired,
        reported: state.reported,
        meta: state.meta,
        status: state.status,
    }
}
