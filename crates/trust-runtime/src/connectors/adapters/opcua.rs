//! OPC UA status adapter home.

use crate::opcua::{
    OpcUaClientConnectionState, OpcUaClientConnectionStatus, OpcUaClientPointAccess,
    OpcUaClientPointStatus, OpcUaClientStatusReport,
};

use super::super::contract::{
    ConnectorHealth, ConnectorKind, ConnectorPointCounts, ConnectorPointMetadata,
    ConnectorPointStatus, ConnectorProtocol, ConnectorState, ConnectorStatusReport,
    DiscoveryConfidence, PointDirection, PointQuality, ReconnectPolicy,
};

use super::super::mapping::{
    opcua_client_status, opcua_server_snapshot_status, ConnectorStatusProjection,
    OpcUaServerSnapshotState,
};
use super::super::report::ConnectorStatusBuilder;

/// Project OPC UA client state.
#[must_use]
pub fn project_opcua_client_state(
    state: OpcUaClientConnectionState,
    degraded_points: usize,
) -> ConnectorStatusProjection {
    opcua_client_status(state, degraded_points)
}

/// Project an OPC UA client runtime status report into connector reports.
#[must_use]
pub fn project_opcua_client_status_report(
    report: &OpcUaClientStatusReport,
) -> Vec<ConnectorStatusReport> {
    if report.connections.is_empty() {
        return vec![empty_client_report(report.enabled)];
    }
    report
        .connections
        .iter()
        .map(project_opcua_client_connection)
        .collect()
}

/// Project OPC UA server snapshot availability.
#[must_use]
pub fn project_opcua_server_snapshot(state: OpcUaServerSnapshotState) -> ConnectorStatusProjection {
    opcua_server_snapshot_status(state)
}

fn project_opcua_client_connection(
    connection: &OpcUaClientConnectionStatus,
) -> ConnectorStatusReport {
    let points = project_opcua_client_point_statuses(connection.points.as_slice());
    let projection = project_opcua_client_state(connection.state, connection.degraded_points);
    let mut builder = ConnectorStatusBuilder::new(
        format!("opcua:client:{}", sanitize_id(connection.name.as_str())),
        ConnectorProtocol::Opcua,
        ConnectorKind::SupervisoryClient,
        projection.state,
        projection.health,
    )
    .display_name(connection.name.as_str())
    .endpoint(connection.endpoint_url.as_str())
    .confidence(confidence_for_connection(connection))
    .reconnect_policy(ReconnectPolicy::FixedDelay);

    if points.is_empty() {
        builder = builder.point_counts(ConnectorPointCounts {
            total: connection.point_count,
            good: connection
                .point_count
                .saturating_sub(connection.degraded_points),
            degraded: connection.degraded_points,
            unavailable: 0,
        });
    } else {
        builder = builder.points(points);
    }

    if matches!(
        projection.health,
        ConnectorHealth::Degraded | ConnectorHealth::Faulted
    ) {
        builder = builder.last_error(connection.detail.as_str());
    }

    builder.build()
}

fn empty_client_report(enabled: bool) -> ConnectorStatusReport {
    let (state, health) = if enabled {
        (ConnectorState::Configured, ConnectorHealth::Unknown)
    } else {
        (ConnectorState::Disabled, ConnectorHealth::Unknown)
    };
    ConnectorStatusBuilder::new(
        "opcua:client",
        ConnectorProtocol::Opcua,
        ConnectorKind::SupervisoryClient,
        state,
        health,
    )
    .display_name("OPC UA client")
    .confidence(DiscoveryConfidence::Unavailable)
    .point_counts(ConnectorPointCounts::default())
    .build()
}

/// Project OPC UA client point statuses into connector point statuses.
#[must_use]
pub fn project_opcua_client_point_statuses(
    statuses: &[OpcUaClientPointStatus],
) -> Vec<ConnectorPointStatus> {
    statuses
        .iter()
        .map(|status| ConnectorPointStatus {
            metadata: ConnectorPointMetadata {
                name: status.var.to_string(),
                source: Some(status.node_id.clone()),
                data_type: Some(status.data_type.as_config_value().to_string()),
                direction: point_direction(status.access),
            },
            quality: point_quality(status),
            last_update_ms: status.last_seen_ms,
            detail: Some(status.detail.clone()).filter(|detail| !detail.is_empty()),
        })
        .collect()
}

fn confidence_for_connection(connection: &OpcUaClientConnectionStatus) -> DiscoveryConfidence {
    match connection.state {
        OpcUaClientConnectionState::Connected | OpcUaClientConnectionState::Stale => {
            DiscoveryConfidence::Confirmed
        }
        OpcUaClientConnectionState::Configured
        | OpcUaClientConnectionState::Connecting
        | OpcUaClientConnectionState::Reconnecting
        | OpcUaClientConnectionState::Faulted => DiscoveryConfidence::Likely,
        OpcUaClientConnectionState::Disabled => DiscoveryConfidence::Unavailable,
    }
}

fn point_direction(access: OpcUaClientPointAccess) -> PointDirection {
    match access {
        OpcUaClientPointAccess::Read => PointDirection::Read,
        OpcUaClientPointAccess::Write => PointDirection::Write,
        OpcUaClientPointAccess::ReadWrite => PointDirection::ReadWrite,
    }
}

fn point_quality(status: &OpcUaClientPointStatus) -> PointQuality {
    match status.state {
        OpcUaClientConnectionState::Connected => PointQuality::Good,
        OpcUaClientConnectionState::Stale | OpcUaClientConnectionState::Reconnecting
            if status.last_seen_ms.is_some() =>
        {
            PointQuality::Stale
        }
        OpcUaClientConnectionState::Faulted => PointQuality::Bad,
        OpcUaClientConnectionState::Configured
        | OpcUaClientConnectionState::Connecting
        | OpcUaClientConnectionState::Reconnecting
        | OpcUaClientConnectionState::Disabled
        | OpcUaClientConnectionState::Stale => PointQuality::Unavailable,
    }
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
