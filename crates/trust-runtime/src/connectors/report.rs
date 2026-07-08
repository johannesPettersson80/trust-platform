//! Helpers for assembling connector status reports.

use super::contract::{
    ConnectorHealth, ConnectorKind, ConnectorPointCounts, ConnectorPointStatus, ConnectorProtocol,
    ConnectorState, ConnectorStatusReport, DiscoveryConfidence, ReconnectPolicy,
    CONNECTOR_STATUS_SCHEMA_VERSION,
};

/// Builder for a single connector status report.
#[derive(Debug, Clone)]
pub struct ConnectorStatusBuilder {
    connector_id: String,
    display_name: Option<String>,
    protocol: ConnectorProtocol,
    kind: ConnectorKind,
    endpoint: Option<String>,
    state: ConnectorState,
    health: ConnectorHealth,
    confidence: DiscoveryConfidence,
    reconnect_policy: Option<ReconnectPolicy>,
    last_error: Option<String>,
    last_transition_ms: Option<u64>,
    freshness_ms: Option<u64>,
    point_counts: Option<ConnectorPointCounts>,
    points: Vec<ConnectorPointStatus>,
}

impl ConnectorStatusBuilder {
    /// Create a builder.
    #[must_use]
    pub fn new(
        connector_id: impl Into<String>,
        protocol: ConnectorProtocol,
        kind: ConnectorKind,
        state: ConnectorState,
        health: ConnectorHealth,
    ) -> Self {
        Self {
            connector_id: connector_id.into(),
            display_name: None,
            protocol,
            kind,
            endpoint: None,
            state,
            health,
            confidence: DiscoveryConfidence::Unavailable,
            reconnect_policy: None,
            last_error: None,
            last_transition_ms: None,
            freshness_ms: None,
            point_counts: None,
            points: Vec::new(),
        }
    }

    /// Set display name.
    #[must_use]
    pub fn display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = Some(display_name.into());
        self
    }

    /// Set endpoint.
    #[must_use]
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// Set discovery confidence.
    #[must_use]
    pub fn confidence(mut self, confidence: DiscoveryConfidence) -> Self {
        self.confidence = confidence;
        self
    }

    /// Set reconnect policy.
    #[must_use]
    pub fn reconnect_policy(mut self, reconnect_policy: ReconnectPolicy) -> Self {
        self.reconnect_policy = Some(reconnect_policy);
        self
    }

    /// Set last error.
    #[must_use]
    pub fn last_error(mut self, last_error: impl Into<String>) -> Self {
        self.last_error = Some(last_error.into());
        self
    }

    /// Set last transition timestamp.
    #[must_use]
    pub fn last_transition_ms(mut self, last_transition_ms: u64) -> Self {
        self.last_transition_ms = Some(last_transition_ms);
        self
    }

    /// Set freshness age.
    #[must_use]
    pub fn freshness_ms(mut self, freshness_ms: u64) -> Self {
        self.freshness_ms = Some(freshness_ms);
        self
    }

    /// Set aggregate point counts when per-point detail is intentionally absent.
    #[must_use]
    pub fn point_counts(mut self, point_counts: ConnectorPointCounts) -> Self {
        self.point_counts = Some(point_counts);
        self
    }

    /// Set detailed point statuses.
    #[must_use]
    pub fn points(mut self, points: Vec<ConnectorPointStatus>) -> Self {
        self.points = points;
        self
    }

    /// Build the report.
    #[must_use]
    pub fn build(self) -> ConnectorStatusReport {
        let point_counts = self
            .point_counts
            .unwrap_or_else(|| point_counts(self.points.as_slice()));
        ConnectorStatusReport {
            schema_version: CONNECTOR_STATUS_SCHEMA_VERSION,
            connector_id: self.connector_id,
            display_name: self.display_name,
            protocol: self.protocol,
            kind: self.kind,
            endpoint: self.endpoint,
            state: self.state,
            health: self.health,
            confidence: self.confidence,
            reconnect_policy: self.reconnect_policy,
            last_error: self.last_error,
            last_transition_ms: self.last_transition_ms,
            freshness_ms: self.freshness_ms,
            point_counts,
            points: self.points,
        }
    }
}

fn point_counts(points: &[ConnectorPointStatus]) -> ConnectorPointCounts {
    let mut counts = ConnectorPointCounts {
        total: points.len(),
        ..ConnectorPointCounts::default()
    };
    for point in points {
        match point.quality {
            super::contract::PointQuality::Good => counts.good += 1,
            super::contract::PointQuality::Unsupported
            | super::contract::PointQuality::Unavailable => counts.unavailable += 1,
            super::contract::PointQuality::Stale
            | super::contract::PointQuality::Bad
            | super::contract::PointQuality::WritePending
            | super::contract::PointQuality::WriteFailed => counts.degraded += 1,
        }
    }
    counts
}
