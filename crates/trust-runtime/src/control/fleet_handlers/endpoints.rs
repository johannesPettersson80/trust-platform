use super::*;

pub(super) fn service_endpoints(
    runtime_id: &str,
    inputs: &FleetRuntimeInputs<'_>,
) -> Vec<FleetEndpoint> {
    let mut endpoints = Vec::new();
    let state = inputs.state;
    let settings = inputs.settings;
    if settings.web.enabled {
        let bound = state.web_listener_bound.load(Ordering::Relaxed);
        endpoints.push(FleetEndpoint {
            id: endpoint_id(runtime_id, "web"),
            kind: "service".to_string(),
            protocol: "web".to_string(),
            name: "Web / IDE".to_string(),
            address: Some(settings.web.listen.to_string()),
            role: Some("server".to_string()),
            health: if bound {
                "connected"
            } else {
                "configured_policy"
            }
            .to_string(),
            detail: if bound {
                "Web listener successfully bound during runtime startup.".to_string()
            } else {
                "Web is configured, but this control state has no listener-bound evidence."
                    .to_string()
            },
            live: bound.then(|| json!({ "last_seen_ms": now_ms() })),
            params: Some(json!({
                "enabled": settings.web.enabled,
                "listen": settings.web.listen.to_string(),
                "auth": settings.web.auth.to_string(),
                "tls": settings.web.tls,
            })),
            children: Vec::new(),
            owned: true,
            supports_test: false,
            source: Some("self".to_string()),
        });
    }
    if settings.opcua.enabled || !settings.opcua.expose.is_empty() {
        let bound = state.opcua_server_bound.load(Ordering::Relaxed);
        endpoints.push(FleetEndpoint {
            id: endpoint_id(runtime_id, "opcua"),
            kind: "service".to_string(),
            protocol: "opcua".to_string(),
            name: "OPC UA server".to_string(),
            address: Some(settings.opcua.listen.to_string()),
            role: Some("server".to_string()),
            health: if bound {
                "connected"
            } else if settings.opcua.enabled {
                "configured_policy"
            } else {
                "not_configured"
            }
            .to_string(),
            detail: if bound {
                "OPC UA server startup probe succeeded.".to_string()
            } else if settings.opcua.enabled {
                "OPC UA is configured, but no bound server evidence is available.".to_string()
            } else {
                "OPC UA exposure exists but the server is disabled.".to_string()
            },
            live: bound.then(|| json!({ "last_seen_ms": now_ms() })),
            params: Some(json!({
                "enabled": settings.opcua.enabled,
                "listen": settings.opcua.listen.to_string(),
                "endpoint_path": settings.opcua.endpoint_path.to_string(),
                "namespace_uri": settings.opcua.namespace_uri.to_string(),
                "publish_interval_ms": settings.opcua.publish_interval_ms,
                "max_nodes": settings.opcua.max_nodes,
                "expose": settings.opcua.expose.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "security_policy": settings.opcua.security_policy.to_string(),
                "security_mode": settings.opcua.security_mode.to_string(),
                "allow_anonymous": settings.opcua.allow_anonymous,
                "username_set": settings.opcua.username_set,
            })),
            children: Vec::new(),
            owned: true,
            supports_test: false,
            source: Some("self".to_string()),
        });
    }
    if settings.discovery.enabled {
        let peer_count = inputs
            .discovery_entries
            .iter()
            .filter(|entry| entry.name.as_str() != runtime_id)
            .count();
        endpoints.push(FleetEndpoint {
            id: endpoint_id(runtime_id, "discovery"),
            kind: "peer".to_string(),
            protocol: "discovery".to_string(),
            name: "Runtime discovery".to_string(),
            address: Some(settings.discovery.service_name.to_string()),
            role: Some(if settings.discovery.advertise {
                "advertise_browse".to_string()
            } else {
                "browse".to_string()
            }),
            health: if peer_count > 0 {
                "connected"
            } else {
                "configured_policy"
            }
            .to_string(),
            detail: if peer_count > 0 {
                format!("Discovery has observed {peer_count} runtime peer(s).")
            } else {
                "Discovery is configured; no peers have been observed yet.".to_string()
            },
            live: Some(json!({
                "value": { "observed_peers": peer_count },
                "last_seen_ms": newest_discovery_seen_ms(inputs.discovery_entries),
            })),
            params: Some(json!({
                "enabled": settings.discovery.enabled,
                "service_name": settings.discovery.service_name.to_string(),
                "advertise": settings.discovery.advertise,
                "interfaces": settings.discovery.interfaces.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "host_group": settings.discovery.host_group.as_ref().map(ToString::to_string),
            })),
            children: Vec::new(),
            owned: true,
            supports_test: false,
            source: Some("self".to_string()),
        });
    }
    if settings.mesh.enabled || !settings.mesh.connect.is_empty() {
        let mesh_snapshot = inputs
            .mesh_evidence
            .map(|evidence| evidence.liveliness_snapshot());
        let peer_count = mesh_snapshot
            .as_ref()
            .map_or(0, |snapshot| snapshot.peers.len());
        let health = match inputs.mesh_evidence {
            Some(evidence) if evidence.is_ready() => "connected",
            Some(_) => "degraded",
            None => "configured_policy",
        };
        endpoints.push(FleetEndpoint {
            id: endpoint_id(runtime_id, "mesh"),
            kind: "peer".to_string(),
            protocol: "mesh".to_string(),
            name: "Mesh / Zenoh".to_string(),
            address: Some(settings.mesh.listen.to_string()),
            role: Some(settings.mesh.role.as_str().to_string()),
            health: health.to_string(),
            detail: mesh_detail(inputs.mesh_evidence, peer_count),
            live: mesh_snapshot.map(|snapshot| {
                json!({
                    "value": { "peers": snapshot.peers },
                    "last_seen_ms": snapshot.history.iter().map(|event| event.timestamp_ns / 1_000_000).max(),
                })
            }),
            params: Some(json!({
                "enabled": settings.mesh.enabled,
                "role": settings.mesh.role.as_str(),
                "listen": settings.mesh.listen.to_string(),
                "connect": settings.mesh.connect.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "tls": settings.mesh.tls,
                "publish": settings.mesh.publish.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "subscribe": settings.mesh.subscribe.iter().map(|(key, value)| {
                    json!({ "topic": key.to_string(), "target": value.to_string() })
                }).collect::<Vec<_>>(),
                "zenohd_version": settings.mesh.zenohd_version.to_string(),
                "plugin_versions": settings.mesh.plugin_versions.iter().map(|(key, value)| {
                    json!({ "plugin": key.to_string(), "version": value.to_string() })
                }).collect::<Vec<_>>(),
            })),
            children: Vec::new(),
            owned: true,
            supports_test: false,
            source: Some("self".to_string()),
        });
    }
    if runtime_cloud_configured(settings) {
        endpoints.push(FleetEndpoint {
            id: endpoint_id(runtime_id, "runtime_cloud"),
            kind: "peer".to_string(),
            protocol: "runtime_cloud".to_string(),
            name: "Runtime cloud / federation".to_string(),
            address: None,
            role: Some("policy".to_string()),
            health: "configured_policy".to_string(),
            detail: "Federation policy is configured; it is not a live transport by itself."
                .to_string(),
            live: None,
            params: Some(json!({
                "profile": settings.runtime_cloud.profile.as_str(),
                "wan_allow_write": settings.runtime_cloud.wan_allow_write.iter().map(|rule| {
                    json!({
                        "action": rule.action.to_string(),
                        "target": rule.target.to_string(),
                    })
                }).collect::<Vec<_>>(),
                "link_preferences": settings.runtime_cloud.link_preferences.iter().map(|rule| {
                    json!({
                        "source": rule.source.to_string(),
                        "target": rule.target.to_string(),
                        "transport": rule.transport.as_str(),
                    })
                }).collect::<Vec<_>>(),
            })),
            children: Vec::new(),
            owned: true,
            supports_test: false,
            source: Some("self".to_string()),
        });
    }
    if settings.realtime.enabled {
        let (health, detail) = inputs
            .realtime
            .map(realtime_health_and_detail)
            .unwrap_or_else(|| {
                (
                    "configured_policy".to_string(),
                    "Realtime posture is configured, but no runtime posture evidence is available."
                        .to_string(),
                )
            });
        endpoints.push(FleetEndpoint {
            id: endpoint_id(runtime_id, "realtime_t0"),
            kind: "peer".to_string(),
            protocol: "realtime_t0".to_string(),
            name: "Realtime T0".to_string(),
            address: None,
            role: Some("same_host".to_string()),
            health,
            detail,
            live: inputs.realtime.map(|status| {
                json!({
                    "value": {
                        "active": status.active,
                        "scheduler": status.active_scheduler.map(|scheduler| scheduler.as_str()),
                        "priority": status.active_priority,
                    },
                    "last_seen_ms": now_ms(),
                })
            }),
            params: Some(json!({
                "enabled": settings.realtime.enabled,
                "require_preempt_rt_kernel": settings.realtime.require_preempt_rt_kernel,
                "lock_memory": settings.realtime.lock_memory,
                "scheduler": settings.realtime.scheduler.as_str(),
                "priority": settings.realtime.priority,
                "cpu_affinity": settings.realtime.cpu_affinity.clone(),
                "strict": settings.realtime.strict,
            })),
            children: Vec::new(),
            owned: true,
            supports_test: false,
            source: Some("self".to_string()),
        });
    }
    if let Some(config) = inputs
        .ads_client_config
        .filter(|config| !config.connections.is_empty())
    {
        let (health, detail) = ads_client_endpoint_health_and_detail(config, inputs.ads_status);
        let live = ads_client_live(config, inputs.ads_status);
        endpoints.push(FleetEndpoint {
            id: endpoint_id(runtime_id, "ads"),
            kind: "peer".to_string(),
            protocol: "ads".to_string(),
            name: "ADS client".to_string(),
            address: ads_client_local_net_id(config),
            role: Some("client".to_string()),
            health,
            detail,
            live,
            params: Some(ads_client_params(config)),
            children: Vec::new(),
            owned: true,
            supports_test: true,
            source: Some("self".to_string()),
        });
    }
    if let Some(config) = inputs
        .opcua_client_config
        .filter(|config| !config.connections.is_empty())
    {
        let (health, detail) =
            opcua_client_endpoint_health_and_detail(config, inputs.opcua_client_status);
        endpoints.push(FleetEndpoint {
            id: endpoint_id(runtime_id, "opcua_client"),
            kind: "peer".to_string(),
            protocol: "opcua_client".to_string(),
            name: "OPC UA client".to_string(),
            address: opcua_client_primary_endpoint(config),
            role: Some("client".to_string()),
            health,
            detail,
            live: opcua_client_live(config, inputs.opcua_client_status),
            params: Some(opcua_client_params(config)),
            children: Vec::new(),
            owned: true,
            supports_test: true,
            source: Some("self".to_string()),
        });
    }
    if let Some(config) = state
        .ads_server_config
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
        .filter(|config| config.enabled || !config.expose.is_empty())
    {
        let connected_clients = ads_server_connected_clients(state);
        let active = connected_clients.is_some();
        let live = ads_server_live(state, connected_clients, active);
        endpoints.push(FleetEndpoint {
            id: endpoint_id(runtime_id, "ads_server"),
            kind: "service".to_string(),
            protocol: "ads_server".to_string(),
            name: "ADS server".to_string(),
            address: config.listen.as_ref().map(ToString::to_string),
            role: Some("server".to_string()),
            health: if active {
                "connected"
            } else if config.enabled {
                "configured_policy"
            } else {
                "not_configured"
            }
            .to_string(),
            detail: if active {
                "ADS server runtime is active and listening.".to_string()
            } else if config.enabled {
                "ADS server is configured, but no active server runtime handle is available."
                    .to_string()
            } else {
                "ADS server exposure exists but the server is disabled.".to_string()
            },
            live,
            params: Some(ads_server_params(&config)),
            children: Vec::new(),
            owned: true,
            supports_test: false,
            source: Some("self".to_string()),
        });
    }
    endpoints
}

fn ads_server_live(
    state: &ControlState,
    connected_clients: Option<usize>,
    active: bool,
) -> Option<serde_json::Value> {
    let latest_doctor =
        state.ads_doctor_jobs.lock().ok().and_then(|jobs| {
            jobs.latest_completed_report(crate::ads::diagnostics::DoctorRole::Server)
        });
    let evidence = latest_doctor
        .as_ref()
        .and_then(|report| report.evidence.as_ref());
    let external_client_verified =
        evidence.is_some_and(|evidence| evidence.external_client_verified);

    if !active && !external_client_verified {
        return None;
    }

    let proof_status = if latest_doctor
        .as_ref()
        .is_some_and(|report| report.production_ready)
    {
        "production_ready"
    } else if external_client_verified {
        "external_client_verified"
    } else if active {
        "self_test_available"
    } else {
        "not_ready"
    };

    let mut value = json!({
        "connected_clients": connected_clients.unwrap_or(0),
        "proof_status": proof_status,
        "external_client_verified": external_client_verified,
    });
    if let Some(kind) = evidence.and_then(|evidence| evidence.external_client_kind.as_deref()) {
        value["external_client_kind"] = json!(kind);
    }
    if let Some(name) = evidence.and_then(|evidence| evidence.external_client_name.as_deref()) {
        value["external_client_name"] = json!(name);
    }
    if let Some(timestamp_ms) = evidence.and_then(|evidence| evidence.external_client_timestamp_ms)
    {
        value["external_client_timestamp_ms"] = json!(timestamp_ms);
    }

    Some(json!({
        "value": value,
        "last_seen_ms": now_ms(),
    }))
}
