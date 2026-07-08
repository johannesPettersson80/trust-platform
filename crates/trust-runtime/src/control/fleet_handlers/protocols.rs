use super::*;

pub(super) fn ads_client_params(config: &AdsClientConfig) -> serde_json::Value {
    json!({
        "connections": config.connections.iter().map(|connection| {
            json!({
                "name": connection.route.name,
                "target_net_id": connection.route.target_net_id.0,
                "host": connection.route.host,
                "ams_port": connection.route.ams_port,
                "local_net_id_set": connection.route.local_net_id.is_some(),
                "transport": match connection.route.security.transport {
                    trust_ads_core::TransportSecurity::Secure => "secure",
                    trust_ads_core::TransportSecurity::Plain => "plain",
                },
                "auto_add_route": connection.route.security.auto_add_route,
                "points": connection.points.iter().map(|point| {
                    let (symbol, address) = ads_point_external_ref(&point.address);
                    json!({
                        "var": point.point_name,
                        "symbol": symbol,
                        "address": address,
                        "type": ads_type_name(&point.data_type),
                        "access": ads_access_name(point.access),
                    })
                }).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
    })
}

pub(super) fn ads_point_external_ref(
    address: &AdsPointAddress,
) -> (Option<String>, Option<String>) {
    match address {
        AdsPointAddress::Symbol(symbol) => (Some(symbol.clone()), None),
        AdsPointAddress::Index {
            index_group,
            index_offset,
            size,
        } => (
            None,
            Some(format!(
                "index {index_group:#x}:{index_offset:#x} · {size} bytes"
            )),
        ),
    }
}

pub(super) fn ads_type_name(data_type: &AdsDataTypeDescriptor) -> String {
    if data_type.source_name.trim().is_empty() {
        format!("{:?}", data_type.iec_type).to_ascii_uppercase()
    } else {
        data_type.source_name.clone()
    }
}

pub(super) fn ads_access_name(access: PointAccess) -> &'static str {
    match access {
        PointAccess::Read => "read",
        PointAccess::Write => "write",
        PointAccess::ReadWrite => "read_write",
    }
}

pub(super) fn opcua_client_params(config: &OpcUaClientConfig) -> serde_json::Value {
    json!({
        "connections": config.connections.iter().map(|connection| {
            json!({
                "name": connection.name.to_string(),
                "endpoint_url": connection.endpoint_url,
                "security_policy": connection.security.policy.as_config_value(),
                "security_mode": connection.security.mode.as_config_value(),
                "auth": match connection.auth {
                    crate::opcua::OpcUaClientAuthConfig::Anonymous => "anonymous",
                    crate::opcua::OpcUaClientAuthConfig::UserName { .. } => "username",
                },
                "username_set": matches!(
                    connection.auth,
                    crate::opcua::OpcUaClientAuthConfig::UserName { .. }
                ),
                "trust_server_certificate": connection.trust_server_certificate,
                "poll_interval_ms": connection.poll_interval_ms,
                "timeout_ms": connection.timeout_ms,
                "points": connection.points.iter().map(|point| {
                    json!({
                        "var": point.var.to_string(),
                        "node_id": point.node_id,
                        "type": point.data_type.as_config_value(),
                        "access": point.access.as_config_value(),
                        "writable": point.access.can_write(),
                    })
                }).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
    })
}

pub(super) fn ads_server_params(
    config: &crate::ads::server::AdsServerRuntimeConfig,
) -> serde_json::Value {
    json!({
        "enabled": config.enabled,
        "listen": config.listen.as_ref().map(ToString::to_string),
        "ads_port": config.ads_port,
        "ams_net_id": config.ams_net_id.as_ref().map(|net_id| net_id.0.clone()),
        "insecure_transport": config.insecure_transport,
        "writes_enabled": config.writes_enabled,
        "symbol_namespace": config.symbol_namespace.to_string(),
        "allow_unpinned_clients": config.allow_unpinned_clients,
        "unsafe_allow_public_bind": config.unsafe_allow_public_bind,
        "expose": config.expose.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "writable": config.writable.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "clients_summary": config.clients.iter().map(ads_server_client_summary).collect::<Vec<_>>(),
        "max_symbols": config.max_symbols,
        "max_clients": config.max_clients,
        "max_subscriptions_per_client": config.max_subscriptions_per_client,
        "max_total_subscriptions": config.max_total_subscriptions,
        "max_frame_bytes": config.max_frame_bytes,
        "max_sumup_items": config.max_sumup_items,
        "max_write_bytes": config.max_write_bytes,
        "max_string_bytes": config.max_string_bytes,
        "read_timeout_ms": config.read_timeout_ms,
        "idle_timeout_ms": config.idle_timeout_ms,
        "min_notification_cycle_ms": config.min_notification_cycle_ms,
    })
}

pub(super) fn ads_server_client_summary(
    client: &crate::ads::server::AdsServerClientConfig,
) -> String {
    let net_id = client.ams_net_id.0.as_str();
    match &client.source {
        crate::ads::server::AdsServerSourcePin::Ip(ip) => {
            format!("{net_id} (from {ip})")
        }
        crate::ads::server::AdsServerSourcePin::Cidr(cidr) => {
            format!("{net_id} (from {cidr})")
        }
        crate::ads::server::AdsServerSourcePin::Unpinned => {
            format!("{net_id} (unpinned lab client)")
        }
    }
}

pub(super) fn mesh_detail(
    evidence: Option<&crate::mesh::MeshTopologyEvidence>,
    peer_count: usize,
) -> String {
    match evidence {
        Some(evidence) if evidence.is_ready() => {
            format!("Mesh session is ready; {peer_count} live peer(s) reported by liveliness.")
        }
        Some(evidence) => evidence
            .degraded_reason()
            .map(|reason| format!("Mesh session is not ready: {reason}"))
            .unwrap_or_else(|| "Mesh session is not ready.".to_string()),
        None => "Mesh is configured, but no live mesh service evidence is available.".to_string(),
    }
}

pub(super) fn configured_mesh_link_status(
    target: &str,
    evidence: Option<&crate::mesh::MeshTopologyEvidence>,
) -> String {
    let Some(evidence) = evidence else {
        return "configured_policy".to_string();
    };
    let snapshot = evidence.liveliness_snapshot();
    if snapshot
        .peers
        .iter()
        .any(|peer| peer == target || target.contains(peer.as_str()))
    {
        "connected".to_string()
    } else if evidence.is_ready() {
        "degraded".to_string()
    } else {
        "configured_policy".to_string()
    }
}

pub(super) fn realtime_health_and_detail(status: &LinuxRtRuntimeStatus) -> (String, String) {
    if !status.errors.is_empty() {
        return (
            "error".to_string(),
            format!(
                "Realtime posture has {} error(s): {}",
                status.errors.len(),
                status
                    .errors
                    .iter()
                    .map(|value| value.as_str())
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
        );
    }
    if !status.active || !status.warnings.is_empty() {
        return (
            "degraded".to_string(),
            format!(
                "Realtime posture is not fully active; {} warning(s).",
                status.warnings.len()
            ),
        );
    }
    (
        "connected".to_string(),
        "Realtime posture evidence is active.".to_string(),
    )
}

pub(super) fn ads_server_connected_clients(state: &ControlState) -> Option<usize> {
    #[cfg(feature = "ads-server")]
    {
        state
            .ads_server_runtime
            .lock()
            .ok()
            .and_then(|runtime| runtime.as_ref().map(|runtime| runtime.connected_clients()))
    }
    #[cfg(not(feature = "ads-server"))]
    {
        let _ = state;
        None
    }
}

pub(super) fn ads_status_report(state: &ControlState) -> Option<AdsStatusReport> {
    let (tx, rx) = mpsc::channel();
    state
        .resource
        .send_command(ResourceCommand::AdsStatus { respond_to: tx })
        .ok()?;
    rx.recv_timeout(ADS_STATUS_TIMEOUT).ok()
}

pub(super) fn opcua_client_status_report(state: &ControlState) -> Option<OpcUaClientStatusReport> {
    let (tx, rx) = mpsc::channel();
    state
        .resource
        .send_command(ResourceCommand::OpcUaClientStatus { respond_to: tx })
        .ok()?;
    rx.recv_timeout(OPCUA_CLIENT_STATUS_TIMEOUT).ok()
}

pub(super) fn ads_client_endpoint_health_and_detail(
    config: &AdsClientConfig,
    status: Option<&AdsStatusReport>,
) -> (String, String) {
    let Some(status) = status else {
        return (
            "configured_policy".to_string(),
            format!(
                "{} ADS connection(s) configured; no live ADS client status has been reported yet.",
                config.connections.len()
            ),
        );
    };
    let connection_statuses = config
        .connections
        .iter()
        .filter_map(|connection| ads_status_for_connection(Some(status), connection))
        .collect::<Vec<_>>();
    if connection_statuses.is_empty() {
        return (
            "configured_policy".to_string(),
            format!(
                "{} ADS connection(s) configured; live status has no matching connection yet.",
                config.connections.len()
            ),
        );
    }
    if connection_statuses
        .iter()
        .any(|connection| connection.state == AdsConnectionStatusState::Faulted)
    {
        return (
            "error".to_string(),
            "One or more ADS client connections are faulted.".to_string(),
        );
    }
    if connection_statuses.iter().any(|connection| {
        !matches!(connection.state, AdsConnectionStatusState::Connected)
            || connection.degraded_points > 0
    }) {
        return (
            "degraded".to_string(),
            "One or more ADS client connections are reconnecting, stale, or degraded.".to_string(),
        );
    }
    (
        "connected".to_string(),
        format!(
            "{} ADS client connection(s) are connected.",
            connection_statuses.len()
        ),
    )
}

pub(super) fn ads_client_live(
    config: &AdsClientConfig,
    status: Option<&AdsStatusReport>,
) -> Option<serde_json::Value> {
    let status = status?;
    let connections = config
        .connections
        .iter()
        .map(|connection| {
            let live = ads_status_for_connection(Some(status), connection);
            json!({
                "name": connection.route.name.as_str(),
                "target": connection.route.target_net_id.0.as_str(),
                "state": live.map(|item| item.state),
                "point_count": live.map(|item| item.point_count),
                "degraded_points": live.map(|item| item.degraded_points),
                "last_good_value_ms": live.and_then(|item| item.last_good_value_ms),
            })
        })
        .collect::<Vec<_>>();
    let connected = status
        .connections
        .iter()
        .filter(|connection| connection.state == AdsConnectionStatusState::Connected)
        .count();
    Some(json!({
        "value": {
            "connected": connected,
            "total": config.connections.len(),
            "connections": connections,
        },
        "last_seen_ms": status
            .connections
            .iter()
            .filter_map(|connection| connection.last_good_value_ms)
            .max()
            .unwrap_or_else(now_ms),
    }))
}

pub(super) fn opcua_client_endpoint_health_and_detail(
    config: &OpcUaClientConfig,
    status: Option<&OpcUaClientStatusReport>,
) -> (String, String) {
    let Some(status) = status else {
        return (
            "configured_policy".to_string(),
            format!(
                "{} OPC UA client connection(s) configured; no live OPC UA client status has been reported yet.",
                config.connections.len()
            ),
        );
    };
    let connection_statuses = config
        .connections
        .iter()
        .filter_map(|connection| opcua_client_status_for_connection(Some(status), connection))
        .collect::<Vec<_>>();
    if connection_statuses.is_empty() {
        return (
            "configured_policy".to_string(),
            format!(
                "{} OPC UA client connection(s) configured; live status has no matching connection yet.",
                config.connections.len()
            ),
        );
    }
    if connection_statuses
        .iter()
        .any(|connection| connection.state == OpcUaClientConnectionState::Faulted)
    {
        return (
            "error".to_string(),
            "One or more OPC UA client connections are faulted.".to_string(),
        );
    }
    if connection_statuses.iter().any(|connection| {
        connection.state != OpcUaClientConnectionState::Connected || connection.degraded_points > 0
    }) {
        return (
            "degraded".to_string(),
            "One or more OPC UA client connections are reconnecting, stale, or degraded."
                .to_string(),
        );
    }
    (
        "connected".to_string(),
        format!(
            "{} OPC UA client connection(s) have live values.",
            connection_statuses.len()
        ),
    )
}

pub(super) fn opcua_client_live(
    config: &OpcUaClientConfig,
    status: Option<&OpcUaClientStatusReport>,
) -> Option<serde_json::Value> {
    let status = status?;
    let connections = config
        .connections
        .iter()
        .map(|connection| {
            let live = opcua_client_status_for_connection(Some(status), connection);
            json!({
                "name": connection.name.as_str(),
                "endpoint_url": connection.endpoint_url.as_str(),
                "state": live.map(|item| opcua_client_state_label(item.state)),
                "point_count": live.map(|item| item.point_count),
                "degraded_points": live.map(|item| item.degraded_points),
                "last_seen_ms": live.and_then(|item| item.last_seen_ms),
                "points": live.map(|item| {
                    item.points.iter().map(|point| {
                        json!({
                            "var": point.var.as_str(),
                            "node_id": point.node_id.as_str(),
                            "state": opcua_client_state_label(point.state),
                            "last_seen_ms": point.last_seen_ms,
                            "value": point.value.as_ref().map(|value| format!("{value:?}")),
                            "detail": point.detail.as_str(),
                        })
                    }).collect::<Vec<_>>()
                }),
            })
        })
        .collect::<Vec<_>>();
    let connected = status
        .connections
        .iter()
        .filter(|connection| connection.state == OpcUaClientConnectionState::Connected)
        .count();
    Some(json!({
        "value": {
            "connected": connected,
            "total": config.connections.len(),
            "connections": connections,
        },
        "last_seen_ms": status
            .connections
            .iter()
            .filter_map(|connection| connection.last_seen_ms)
            .max()
            .unwrap_or_else(now_ms),
    }))
}

pub(super) fn opcua_client_primary_endpoint(config: &OpcUaClientConfig) -> Option<String> {
    config
        .connections
        .first()
        .map(|connection| connection.endpoint_url.clone())
}

pub(super) fn ads_client_local_net_id(config: &AdsClientConfig) -> Option<String> {
    config
        .connections
        .iter()
        .find_map(|connection| connection.route.local_net_id.as_ref())
        .map(|net_id| net_id.0.clone())
}

pub(super) fn ads_status_for_connection<'a>(
    status: Option<&'a AdsStatusReport>,
    connection: &AdsConnectionConfig,
) -> Option<&'a AdsConnectionStatus> {
    let status = status?;
    let route = &connection.route;
    status.connections.iter().find(|item| {
        item.name == route.name
            || item.target.as_ref().is_some_and(|target| {
                target.ams_net_id == route.target_net_id.0 && target.ams_port == route.ams_port
            })
    })
}

pub(super) fn opcua_client_status_for_connection<'a>(
    status: Option<&'a OpcUaClientStatusReport>,
    connection: &crate::opcua::OpcUaClientConnectionConfig,
) -> Option<&'a crate::opcua::OpcUaClientConnectionStatus> {
    let status = status?;
    status
        .connections
        .iter()
        .find(|item| item.name == connection.name || item.endpoint_url == connection.endpoint_url)
}

pub(super) fn opcua_client_connection_health(
    status: &crate::opcua::OpcUaClientConnectionStatus,
) -> String {
    match status.state {
        OpcUaClientConnectionState::Connected if status.degraded_points == 0 => "connected",
        OpcUaClientConnectionState::Faulted => "error",
        OpcUaClientConnectionState::Connected
        | OpcUaClientConnectionState::Connecting
        | OpcUaClientConnectionState::Reconnecting
        | OpcUaClientConnectionState::Stale => "degraded",
        OpcUaClientConnectionState::Configured | OpcUaClientConnectionState::Disabled => {
            "configured_policy"
        }
    }
    .to_string()
}

pub(super) fn opcua_client_state_label(state: OpcUaClientConnectionState) -> &'static str {
    match state {
        OpcUaClientConnectionState::Disabled => "disabled",
        OpcUaClientConnectionState::Configured => "configured_policy",
        OpcUaClientConnectionState::Connecting => "connecting",
        OpcUaClientConnectionState::Connected => "connected",
        OpcUaClientConnectionState::Reconnecting => "reconnecting",
        OpcUaClientConnectionState::Stale => "stale",
        OpcUaClientConnectionState::Faulted => "error",
    }
}

pub(super) fn ads_connection_status_health(status: &AdsConnectionStatus) -> String {
    match status.state {
        AdsConnectionStatusState::Connected if status.degraded_points == 0 => "connected",
        AdsConnectionStatusState::Faulted => "error",
        AdsConnectionStatusState::Connected
        | AdsConnectionStatusState::Reconnecting
        | AdsConnectionStatusState::NotReady
        | AdsConnectionStatusState::Stale => "degraded",
        AdsConnectionStatusState::Disabled | AdsConnectionStatusState::Unknown => {
            "configured_policy"
        }
    }
    .to_string()
}

pub(super) fn ads_route_secure(connection: &AdsConnectionConfig) -> bool {
    !matches!(
        connection.route.security.transport,
        trust_ads_core::TransportSecurity::Plain
    )
}
