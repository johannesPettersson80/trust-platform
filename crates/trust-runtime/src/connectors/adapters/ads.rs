//! ADS status adapter home.

use crate::ads::diagnostics::{
    AdsConnectionStatus, AdsConnectionStatusState, AdsStatusOverall, AdsStatusReport, DoctorRole,
    TargetIdentity,
};
use crate::ads::onboarding::ActiveAdsDeviceSnapshot;
use crate::ads::AdsConnectionState;

use super::super::contract::{
    ConnectorHealth, ConnectorKind, ConnectorPointCounts, ConnectorPointMetadata,
    ConnectorPointStatus, ConnectorProtocol, ConnectorState, ConnectorStatusReport,
    DiscoveryConfidence, PointDirection, PointQuality, ReconnectPolicy,
};
use super::super::mapping::{
    ads_connection_state_status, ads_connection_status_state, ConnectorStatusProjection,
};
use super::super::report::ConnectorStatusBuilder;
use trust_ads_core::{
    PointQuality as AdsPointQuality, PointStatus as AdsPointStatus, QualityState,
};

/// Project low-level ADS connection state.
#[must_use]
pub fn project_ads_connection_state(state: AdsConnectionState) -> ConnectorStatusProjection {
    ads_connection_state_status(state)
}

/// Project ADS report connection state.
#[must_use]
pub fn project_ads_report_state(
    state: AdsConnectionStatusState,
    degraded_points: usize,
) -> ConnectorStatusProjection {
    ads_connection_status_state(state, degraded_points)
}

/// Project an ADS client or server status report into connector reports.
#[must_use]
pub fn project_ads_status_report(report: &AdsStatusReport) -> Vec<ConnectorStatusReport> {
    project_ads_status_report_with_default_endpoint(report, None)
}

/// Project an ADS status report with a default endpoint for target-less reports.
#[must_use]
pub fn project_ads_status_report_with_default_endpoint(
    report: &AdsStatusReport,
    default_endpoint: Option<&str>,
) -> Vec<ConnectorStatusReport> {
    let kind = connector_kind(report.role);
    if report.connections.is_empty() {
        return vec![empty_report_connector(report, kind, default_endpoint)];
    }
    report
        .connections
        .iter()
        .map(|connection| project_ads_connection(report.role, kind, connection, default_endpoint))
        .collect()
}

/// Project a live ADS device snapshot, including per-point quality.
#[must_use]
pub fn project_active_ads_device_snapshot(
    snapshot: &ActiveAdsDeviceSnapshot,
) -> ConnectorStatusReport {
    let points = project_ads_point_statuses(snapshot.point_statuses.as_slice());
    let degraded_points = points
        .iter()
        .filter(|point| point.quality != PointQuality::Good)
        .count();
    let projection = project_ads_report_state(snapshot.state, degraded_points);
    let mut builder = ConnectorStatusBuilder::new(
        format!(
            "ads:client:{}",
            sanitize_id(snapshot.target.ams_net_id.as_str())
        ),
        ConnectorProtocol::Ads,
        ConnectorKind::SupervisoryClient,
        projection.state,
        projection.health,
    )
    .display_name(snapshot.connection_name.as_str())
    .endpoint(target_endpoint(&snapshot.target))
    .confidence(DiscoveryConfidence::Confirmed)
    .reconnect_policy(ReconnectPolicy::FixedDelay)
    .points(points);
    if matches!(
        projection.health,
        ConnectorHealth::Degraded | ConnectorHealth::Faulted
    ) {
        builder = builder.last_error("ADS point quality is degraded");
    }
    builder.build()
}

/// Project ADS point statuses into connector point statuses.
#[must_use]
pub fn project_ads_point_statuses(statuses: &[AdsPointStatus]) -> Vec<ConnectorPointStatus> {
    statuses
        .iter()
        .map(|status| ConnectorPointStatus {
            metadata: ConnectorPointMetadata {
                name: status.point_name.clone(),
                source: None,
                data_type: None,
                direction: PointDirection::ReadWrite,
            },
            quality: project_ads_point_quality(&status.quality),
            last_update_ms: status.quality.last_update_ms,
            detail: status.quality.detail.clone(),
        })
        .collect()
}

fn project_ads_connection(
    role: DoctorRole,
    kind: ConnectorKind,
    connection: &AdsConnectionStatus,
    default_endpoint: Option<&str>,
) -> ConnectorStatusReport {
    let projection = project_ads_report_state(connection.state, connection.degraded_points);
    let mut builder = ConnectorStatusBuilder::new(
        connector_id(role, connection),
        ConnectorProtocol::Ads,
        kind,
        projection.state,
        projection.health,
    )
    .display_name(connection.name.as_str())
    .confidence(confidence_for_connection(connection))
    .point_counts(point_counts(connection));

    if role == DoctorRole::Client {
        builder = builder.reconnect_policy(ReconnectPolicy::FixedDelay);
    }
    if let Some(endpoint) = connection
        .target
        .as_ref()
        .map(target_endpoint)
        .or_else(|| default_endpoint.map(ToString::to_string))
    {
        builder = builder.endpoint(endpoint);
    }
    if matches!(
        projection.health,
        ConnectorHealth::Degraded | ConnectorHealth::Faulted
    ) {
        builder = builder.last_error(connection.summary.as_str());
    }
    builder.build()
}

fn empty_report_connector(
    report: &AdsStatusReport,
    kind: ConnectorKind,
    default_endpoint: Option<&str>,
) -> ConnectorStatusReport {
    let projection = overall_projection(report.overall);
    let role = role_label(report.role);
    let mut builder = ConnectorStatusBuilder::new(
        format!("ads:{role}"),
        ConnectorProtocol::Ads,
        kind,
        projection.state,
        projection.health,
    )
    .display_name(format!("ADS {role}"))
    .confidence(DiscoveryConfidence::Unavailable)
    .point_counts(ConnectorPointCounts::default());
    if report.role == DoctorRole::Client {
        builder = builder.reconnect_policy(ReconnectPolicy::FixedDelay);
    }
    if let Some(endpoint) = default_endpoint {
        builder = builder.endpoint(endpoint);
    }
    if matches!(
        projection.health,
        ConnectorHealth::Degraded | ConnectorHealth::Faulted
    ) {
        builder = builder.last_error(report.summary.as_str());
    }
    builder.build()
}

fn connector_kind(role: DoctorRole) -> ConnectorKind {
    match role {
        DoctorRole::Client => ConnectorKind::SupervisoryClient,
        DoctorRole::Server => ConnectorKind::SupervisoryServer,
    }
}

fn overall_projection(overall: AdsStatusOverall) -> ConnectorStatusProjection {
    match overall {
        AdsStatusOverall::Healthy => {
            ConnectorStatusProjection::new(ConnectorState::Ready, ConnectorHealth::Ok)
        }
        AdsStatusOverall::Degraded => {
            ConnectorStatusProjection::new(ConnectorState::Degraded, ConnectorHealth::Degraded)
        }
        AdsStatusOverall::NotReady => {
            ConnectorStatusProjection::new(ConnectorState::NotReady, ConnectorHealth::Unknown)
        }
        AdsStatusOverall::Faulted => {
            ConnectorStatusProjection::new(ConnectorState::Faulted, ConnectorHealth::Faulted)
        }
        AdsStatusOverall::Disabled => {
            ConnectorStatusProjection::new(ConnectorState::Disabled, ConnectorHealth::Unknown)
        }
        AdsStatusOverall::Unknown => {
            ConnectorStatusProjection::new(ConnectorState::NotReady, ConnectorHealth::Unknown)
        }
    }
}

fn connector_id(role: DoctorRole, connection: &AdsConnectionStatus) -> String {
    let key = connection
        .target
        .as_ref()
        .map(|target| target.ams_net_id.as_str())
        .unwrap_or(connection.name.as_str());
    format!("ads:{}:{}", role_label(role), sanitize_id(key))
}

fn role_label(role: DoctorRole) -> &'static str {
    match role {
        DoctorRole::Client => "client",
        DoctorRole::Server => "server",
    }
}

fn confidence_for_connection(connection: &AdsConnectionStatus) -> DiscoveryConfidence {
    if connection.target.is_some() {
        DiscoveryConfidence::Confirmed
    } else {
        DiscoveryConfidence::Unavailable
    }
}

fn target_endpoint(target: &TargetIdentity) -> String {
    format!("{}:{}@{}", target.ams_net_id, target.ams_port, target.ip)
}

fn point_counts(connection: &AdsConnectionStatus) -> ConnectorPointCounts {
    ConnectorPointCounts {
        total: connection.point_count,
        good: connection
            .point_count
            .saturating_sub(connection.degraded_points),
        degraded: connection.degraded_points,
        unavailable: 0,
    }
}

fn project_ads_point_quality(quality: &AdsPointQuality) -> PointQuality {
    match quality.state {
        QualityState::Good => PointQuality::Good,
        QualityState::Stale if is_write_pending(quality.detail.as_deref()) => {
            PointQuality::WritePending
        }
        QualityState::Stale => PointQuality::Stale,
        QualityState::Error if is_write_failure(quality.detail.as_deref()) => {
            PointQuality::WriteFailed
        }
        QualityState::Error => PointQuality::Bad,
    }
}

fn is_write_pending(detail: Option<&str>) -> bool {
    detail.is_some_and(|detail| detail.to_ascii_lowercase().contains("write pending"))
}

fn is_write_failure(detail: Option<&str>) -> bool {
    detail.is_some_and(|detail| {
        let detail = detail.to_ascii_lowercase();
        detail.contains("write") || detail.contains("output") || detail.contains("writable")
    })
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
