//! Read-only ADS values projected from scan-owned runtime storage.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use trust_ads_core::{PointAccess, PointQuality, QualityState};

use crate::memory::VariableStorage;
use crate::value::{format_user_value, Value};

use super::{AdsBinding, AdsPointAddress};

/// Schema version for the ADS live-values snapshot contract.
pub const ADS_LIVE_VALUES_SCHEMA_VERSION: u32 = 1;

/// One read-only snapshot of all active ADS bindings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdsLiveValuesSnapshot {
    /// Contract schema version.
    pub schema_version: u32,
    /// Runtime scan that owns the projected values.
    pub scan: u64,
    /// Active ADS binding values in configured connection/point order.
    pub entries: Vec<AdsLiveValueEntry>,
}

impl AdsLiveValuesSnapshot {
    /// Creates a snapshot for one runtime scan.
    #[must_use]
    pub fn new(scan: u64, entries: Vec<AdsLiveValueEntry>) -> Self {
        Self {
            schema_version: ADS_LIVE_VALUES_SCHEMA_VERSION,
            scan,
            entries,
        }
    }
}

/// Read-only value and metadata for one active ADS binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdsLiveValueEntry {
    /// Configured ADS connection name.
    pub connection: String,
    /// Generated or user-selected local variable name.
    pub name: String,
    /// Remote ADS symbol name or deterministic index address.
    pub remote_symbol: String,
    /// IEC-formatted value captured from runtime storage.
    pub value: String,
    /// Imported ADS source type.
    pub value_type: String,
    /// Configured access direction (`read`, `write`, or `read_write`).
    pub access: String,
    /// Communication quality owned by the ADS connection cache.
    pub quality: AdsLiveValueQuality,
}

/// Stable quality projection for the ADS live-values contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdsLiveValueQuality {
    /// Current point quality state.
    pub state: QualityState,
    /// Last state change or good update time in Unix milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_update_ms: Option<u64>,
    /// Human-readable stale/error detail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl From<&PointQuality> for AdsLiveValueQuality {
    fn from(quality: &PointQuality) -> Self {
        Self {
            state: quality.state,
            last_update_ms: quality.last_update_ms,
            detail: quality.detail.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct AdsAppliedLiveValues {
    points: BTreeMap<String, AdsAppliedLiveValue>,
}

#[derive(Debug, Clone)]
struct AdsAppliedLiveValue {
    value: Option<Value>,
    quality: PointQuality,
}

impl AdsAppliedLiveValues {
    pub(super) fn new(bindings: &[AdsBinding]) -> Self {
        let points = bindings
            .iter()
            .map(|binding| {
                (
                    binding.point.point_name.clone(),
                    AdsAppliedLiveValue {
                        value: None,
                        quality: PointQuality::stale("waiting for first ADS scan"),
                    },
                )
            })
            .collect();
        Self { points }
    }

    pub(super) fn initialize(&mut self, bindings: &[AdsBinding], storage: &VariableStorage) {
        for binding in bindings {
            let value = storage.read_by_ref(binding.reference.clone()).cloned();
            if let Some(point) = self.points.get_mut(binding.point.point_name.as_str()) {
                point.value = value;
            }
        }
    }

    pub(super) fn commit_quality(&mut self, point_name: &str, quality: PointQuality) {
        if let Some(point) = self.points.get_mut(point_name) {
            point.quality = quality;
        }
    }

    pub(super) fn commit_value(&mut self, point_name: &str, value: Value, quality: PointQuality) {
        if let Some(point) = self.points.get_mut(point_name) {
            point.value = Some(value);
            point.quality = quality;
        }
    }

    pub(super) fn quality(&self, point_name: &str) -> Option<PointQuality> {
        self.points
            .get(point_name)
            .map(|point| point.quality.clone())
    }

    pub(super) fn entries(
        &self,
        connection: &str,
        bindings: &[AdsBinding],
    ) -> Vec<AdsLiveValueEntry> {
        bindings
            .iter()
            .map(|binding| {
                let applied = self.points.get(binding.point.point_name.as_str());
                AdsLiveValueEntry {
                    connection: connection.to_string(),
                    name: binding.point.point_name.clone(),
                    remote_symbol: remote_address(&binding.point.address),
                    value: applied
                        .and_then(|point| point.value.as_ref())
                        .map(format_user_value)
                        .unwrap_or_else(|| "Unavailable".to_string()),
                    value_type: binding.point.data_type.source_name.clone(),
                    access: access_name(binding.point.access).to_string(),
                    quality: applied
                        .map(|point| AdsLiveValueQuality::from(&point.quality))
                        .unwrap_or_else(missing_quality),
                }
            })
            .collect()
    }
}

fn missing_quality() -> AdsLiveValueQuality {
    AdsLiveValueQuality {
        state: QualityState::Error,
        last_update_ms: None,
        detail: Some("ADS quality is unavailable".to_string()),
    }
}

fn remote_address(address: &AdsPointAddress) -> String {
    match address {
        AdsPointAddress::Symbol(symbol) => symbol.clone(),
        AdsPointAddress::Index {
            index_group,
            index_offset,
            size,
        } => format!("index group=0x{index_group:08X} offset=0x{index_offset:08X} size={size}"),
    }
}

fn access_name(access: PointAccess) -> &'static str {
    match access {
        PointAccess::Read => "read",
        PointAccess::Write => "write",
        PointAccess::ReadWrite => "read_write",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_serializes_the_camel_case_read_only_contract() {
        let snapshot = AdsLiveValuesSnapshot::new(
            12,
            vec![AdsLiveValueEntry {
                connection: "line1".to_string(),
                name: "line1_temp".to_string(),
                remote_symbol: "MAIN.Temperature".to_string(),
                value: "42.5".to_string(),
                value_type: "REAL".to_string(),
                access: "read".to_string(),
                quality: AdsLiveValueQuality {
                    state: QualityState::Good,
                    last_update_ms: Some(10),
                    detail: None,
                },
            }],
        );

        let json = serde_json::to_value(&snapshot).expect("serialize ADS live values");
        assert_eq!(json["schemaVersion"], 1);
        assert_eq!(json["scan"], 12);
        assert_eq!(json["entries"][0]["remoteSymbol"], "MAIN.Temperature");
        assert_eq!(json["entries"][0]["valueType"], "REAL");
        assert_eq!(json["entries"][0]["quality"]["state"], "good");
        assert_eq!(json["entries"][0]["quality"]["lastUpdateMs"], 10);
        assert!(json["entries"][0]["quality"].get("detail").is_none());
    }
}
