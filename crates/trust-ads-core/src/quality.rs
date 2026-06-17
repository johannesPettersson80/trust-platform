use serde::{Deserialize, Serialize};

/// ADS point quality state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityState {
    /// Last update was successful.
    Good,
    /// No current good value is available.
    Stale,
    /// The last update failed.
    Error,
}

/// Communication quality for one point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PointQuality {
    /// Current quality state.
    pub state: QualityState,
    /// Milliseconds since Unix epoch for the last state change or good update.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_update_ms: Option<u64>,
    /// Human-readable detail for stale/error states.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl PointQuality {
    /// Creates a stale quality record.
    #[must_use]
    pub fn stale(detail: impl Into<String>) -> Self {
        Self {
            state: QualityState::Stale,
            last_update_ms: None,
            detail: Some(detail.into()),
        }
    }

    /// Creates a stale quality record with the last known update timestamp.
    #[must_use]
    pub fn stale_at(last_update_ms: u64, detail: impl Into<String>) -> Self {
        Self {
            state: QualityState::Stale,
            last_update_ms: Some(last_update_ms),
            detail: Some(detail.into()),
        }
    }

    /// Creates a good quality record.
    #[must_use]
    pub fn good(last_update_ms: u64) -> Self {
        Self {
            state: QualityState::Good,
            last_update_ms: Some(last_update_ms),
            detail: None,
        }
    }

    /// Creates an error quality record.
    #[must_use]
    pub fn error(last_update_ms: u64, detail: impl Into<String>) -> Self {
        Self {
            state: QualityState::Error,
            last_update_ms: Some(last_update_ms),
            detail: Some(detail.into()),
        }
    }

    /// Marks this quality record good.
    pub fn mark_good(&mut self, last_update_ms: u64) {
        self.state = QualityState::Good;
        self.last_update_ms = Some(last_update_ms);
        self.detail = None;
    }

    /// Marks this quality record stale.
    pub fn mark_stale(&mut self, detail: impl Into<String>) {
        self.state = QualityState::Stale;
        self.detail = Some(detail.into());
    }

    /// Marks this quality record failed.
    pub fn mark_error(&mut self, last_update_ms: u64, detail: impl Into<String>) {
        self.state = QualityState::Error;
        self.last_update_ms = Some(last_update_ms);
        self.detail = Some(detail.into());
    }
}

/// Status for a named ADS point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PointStatus {
    /// Point name from the ADS import model.
    pub point_name: String,
    /// Current quality.
    pub quality: PointQuality,
}

impl PointStatus {
    /// Creates a stale point status for cold start.
    #[must_use]
    pub fn cold_start(point_name: impl Into<String>) -> Self {
        Self {
            point_name: point_name.into(),
            quality: PointQuality::stale("waiting for first ADS update"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_transitions_clear_and_preserve_fields() {
        let mut quality = PointQuality::stale("cold start");
        assert_eq!(quality.state, QualityState::Stale);
        assert_eq!(quality.last_update_ms, None);

        quality.mark_good(10);
        assert_eq!(quality, PointQuality::good(10));

        quality.mark_error(20, "read failed");
        assert_eq!(quality.state, QualityState::Error);
        assert_eq!(quality.last_update_ms, Some(20));
        assert_eq!(quality.detail.as_deref(), Some("read failed"));

        quality.mark_stale("reconnecting");
        assert_eq!(quality.state, QualityState::Stale);
        assert_eq!(quality.last_update_ms, Some(20));
        assert_eq!(quality.detail.as_deref(), Some("reconnecting"));
    }

    #[test]
    fn cold_start_status_is_stale() {
        let status = PointStatus::cold_start("line1_temp");

        assert_eq!(status.point_name, "line1_temp");
        assert_eq!(status.quality.state, QualityState::Stale);
    }

    #[test]
    fn stale_at_preserves_last_update_timestamp() {
        let quality = PointQuality::stale_at(42, "snapshot too old");

        assert_eq!(quality.state, QualityState::Stale);
        assert_eq!(quality.last_update_ms, Some(42));
        assert_eq!(quality.detail.as_deref(), Some("snapshot too old"));
    }
}
