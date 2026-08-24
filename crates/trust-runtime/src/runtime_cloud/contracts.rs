//! Versioned runtime cloud contracts for T1/T2/T3 planes.

#![allow(missing_docs)]

use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

pub const RUNTIME_CLOUD_API_VERSION: &str = "1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ApiVersion {
    pub major: u16,
    pub minor: u16,
}

impl ApiVersion {
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    pub fn as_text(self) -> String {
        self.to_string()
    }
}

impl fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiVersionParseError {
    Empty,
    InvalidFormat,
    InvalidMajor,
    InvalidMinor,
}

impl fmt::Display for ApiVersionParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "api_version is empty"),
            Self::InvalidFormat => write!(f, "api_version must use '<major>.<minor>' format"),
            Self::InvalidMajor => write!(f, "api_version major is not a valid integer"),
            Self::InvalidMinor => write!(f, "api_version minor is not a valid integer"),
        }
    }
}

impl std::error::Error for ApiVersionParseError {}

impl FromStr for ApiVersion {
    type Err = ApiVersionParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let text = value.trim();
        if text.is_empty() {
            return Err(ApiVersionParseError::Empty);
        }
        let Some((major, minor)) = text.split_once('.') else {
            return Err(ApiVersionParseError::InvalidFormat);
        };
        let major = major
            .parse::<u16>()
            .map_err(|_| ApiVersionParseError::InvalidMajor)?;
        let minor = minor
            .parse::<u16>()
            .map_err(|_| ApiVersionParseError::InvalidMinor)?;
        Ok(Self { major, minor })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractCompatibility {
    Exact,
    AdditiveWithinMajor,
    BreakingMajor,
}

pub fn validate_api_version(value: &str) -> Result<ApiVersion, ApiVersionParseError> {
    value.parse::<ApiVersion>()
}

pub fn evaluate_compatibility(
    producer: &str,
    consumer: &str,
) -> Result<ContractCompatibility, ApiVersionParseError> {
    let producer = validate_api_version(producer)?;
    let consumer = validate_api_version(consumer)?;
    if producer == consumer {
        return Ok(ContractCompatibility::Exact);
    }
    if producer.major == consumer.major {
        return Ok(ContractCompatibility::AdditiveWithinMajor);
    }
    Ok(ContractCompatibility::BreakingMajor)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasonCode {
    NotConfigured,
    ContractViolation,
    SchemaMismatch,
    PermissionDenied,
    Timeout,
    PeerNotAvailable,
    TransportFailure,
    StaleData,
    Conflict,
    AclDeniedCfgWrite,
    TargetUnreachable,
    RevisionConflict,
    LeaseUnavailable,
}

impl ReasonCode {
    pub const fn remediation_hint(self) -> &'static str {
        match self {
            Self::NotConfigured => "Complete runtime cloud setup before retrying the action.",
            Self::ContractViolation => "Update the caller payload to match the API contract.",
            Self::SchemaMismatch => "Refresh schema/catalog and retry with the current revision.",
            Self::PermissionDenied | Self::AclDeniedCfgWrite => {
                "Request an appropriate role for protected operations."
            }
            Self::Timeout | Self::PeerNotAvailable | Self::TargetUnreachable => {
                "Verify target reachability and retry."
            }
            Self::TransportFailure => "Check mesh transport health and TLS/material settings.",
            Self::StaleData => "Refresh data and confirm stale marker is cleared.",
            Self::Conflict | Self::RevisionConflict => {
                "Rebase on latest desired/reported revisions before applying."
            }
            Self::LeaseUnavailable => "Reacquire HA lease/fencing token before retrying.",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityPayload {
    pub api_version: String,
    pub runtime_id: String,
    pub site: String,
    pub catalog_epoch: u64,
    pub build: String,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub api_version: String,
    pub schema_id: String,
    pub schema_version: u32,
    pub schema_hash: String,
    pub encoding: String,
    pub qos: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaFieldLayout {
    pub name: String,
    pub offset: u32,
    pub size: u32,
    pub type_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaLayout {
    pub fields: Vec<SchemaFieldLayout>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaCompatibilityError {
    EmptyFieldName,
    DuplicateField(String),
    InvalidFieldRange {
        field: String,
        offset: u32,
        size: u32,
    },
    OverlappingFields {
        first: String,
        second: String,
    },
    NewFieldNotAppended {
        field: String,
        minimum_offset: u32,
        actual: u32,
    },
    MissingField(String),
    OffsetChanged {
        field: String,
        expected: u32,
        actual: u32,
    },
    SizeChanged {
        field: String,
        expected: u32,
        actual: u32,
    },
    TypeChanged {
        field: String,
        expected: String,
        actual: String,
    },
}

/// Validate forward-additive mesh schema compatibility.
///
/// Existing fields must keep stable offset/size/type; new fields may be appended.
pub fn validate_forward_additive_schema(
    previous: &SchemaLayout,
    candidate: &SchemaLayout,
) -> Result<(), SchemaCompatibilityError> {
    let previous_end = validate_schema_layout_shape(previous)?;
    validate_schema_layout_shape(candidate)?;

    for prior in &previous.fields {
        let Some(next) = candidate
            .fields
            .iter()
            .find(|field| field.name == prior.name)
        else {
            return Err(SchemaCompatibilityError::MissingField(prior.name.clone()));
        };
        if next.offset != prior.offset {
            return Err(SchemaCompatibilityError::OffsetChanged {
                field: prior.name.clone(),
                expected: prior.offset,
                actual: next.offset,
            });
        }
        if next.size != prior.size {
            return Err(SchemaCompatibilityError::SizeChanged {
                field: prior.name.clone(),
                expected: prior.size,
                actual: next.size,
            });
        }
        if next.type_name != prior.type_name {
            return Err(SchemaCompatibilityError::TypeChanged {
                field: prior.name.clone(),
                expected: prior.type_name.clone(),
                actual: next.type_name.clone(),
            });
        }
    }
    for field in &candidate.fields {
        let existed = previous.fields.iter().any(|prior| prior.name == field.name);
        if !existed && field.offset < previous_end {
            return Err(SchemaCompatibilityError::NewFieldNotAppended {
                field: field.name.clone(),
                minimum_offset: previous_end,
                actual: field.offset,
            });
        }
    }
    Ok(())
}

fn validate_schema_layout_shape(layout: &SchemaLayout) -> Result<u32, SchemaCompatibilityError> {
    let mut names = BTreeSet::new();
    let mut ranges = Vec::with_capacity(layout.fields.len());
    for field in &layout.fields {
        if field.name.trim().is_empty() {
            return Err(SchemaCompatibilityError::EmptyFieldName);
        }
        if !names.insert(field.name.as_str()) {
            return Err(SchemaCompatibilityError::DuplicateField(field.name.clone()));
        }
        let Some(end) = field.offset.checked_add(field.size) else {
            return Err(SchemaCompatibilityError::InvalidFieldRange {
                field: field.name.clone(),
                offset: field.offset,
                size: field.size,
            });
        };
        if field.size == 0 {
            return Err(SchemaCompatibilityError::InvalidFieldRange {
                field: field.name.clone(),
                offset: field.offset,
                size: field.size,
            });
        }
        ranges.push((field.offset, end, field.name.as_str()));
    }
    ranges.sort_unstable();
    for adjacent in ranges.windows(2) {
        let (_first_start, first_end, first_name) = adjacent[0];
        let (second_start, _second_end, second_name) = adjacent[1];
        if second_start < first_end {
            return Err(SchemaCompatibilityError::OverlappingFields {
                first: first_name.to_string(),
                second: second_name.to_string(),
            });
        }
    }
    Ok(ranges.iter().map(|(_, end, _)| *end).max().unwrap_or(0))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CatalogEpochCache {
    last_seen: Option<u64>,
}

impl CatalogEpochCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Observe a remote epoch.
    ///
    /// Returns true when cache refresh is required.
    pub fn observe(&mut self, epoch: u64) -> bool {
        match self.last_seen {
            None => {
                self.last_seen = Some(epoch);
                true
            }
            Some(last) if epoch > last => {
                self.last_seen = Some(epoch);
                true
            }
            _ => false,
        }
    }

    #[must_use]
    pub fn last_seen(&self) -> Option<u64> {
        self.last_seen
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigMeta {
    pub api_version: String,
    pub desired_revision: u64,
    pub reported_revision: u64,
    pub desired_etag: String,
    pub reported_etag: String,
    pub last_writer: String,
    pub apply_policy: String,
    pub updated_at_ns: u64,
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
pub struct ConfigStatus {
    pub api_version: String,
    pub state: ConfigState,
    pub applied_revision: u64,
    pub pending_revision: Option<u64>,
    pub required_action: Option<String>,
    pub blocked_reason: Option<ReasonCode>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandEnvelope {
    pub api_version: String,
    pub request_id: String,
    pub group_id: String,
    pub command_seq: u64,
    pub command: String,
    pub target: String,
    pub issued_at_ns: u64,
    pub body: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRecord {
    pub api_version: String,
    pub event_id: String,
    pub timestamp_ns: u64,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub request_id: String,
    pub result: String,
    pub error: Option<ReasonCode>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_version_parsing_and_compatibility_follow_contract_rules() {
        let version = validate_api_version("1.0").expect("parse version");
        assert_eq!(version, ApiVersion::new(1, 0));
        assert_eq!(
            evaluate_compatibility("1.2", "1.0").expect("compatibility"),
            ContractCompatibility::AdditiveWithinMajor
        );
        assert_eq!(
            evaluate_compatibility("2.0", "1.9").expect("compatibility"),
            ContractCompatibility::BreakingMajor
        );
    }

    #[test]
    fn cloud_contract_payloads_round_trip_with_reason_codes() {
        let identity = IdentityPayload {
            api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
            runtime_id: "rt-1".to_string(),
            site: "site-a".to_string(),
            catalog_epoch: 12,
            build: "0.9.15".to_string(),
            capabilities: vec!["cfg-agent".to_string(), "audit".to_string()],
        };
        let encoded = serde_json::to_string(&identity).expect("encode");
        let decoded: IdentityPayload = serde_json::from_str(&encoded).expect("decode");
        assert_eq!(decoded, identity);

        let status = ConfigStatus {
            api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
            state: ConfigState::Blocked,
            applied_revision: 7,
            pending_revision: Some(8),
            required_action: Some("restart_required".to_string()),
            blocked_reason: Some(ReasonCode::AclDeniedCfgWrite),
            errors: vec!["write denied".to_string()],
        };
        let encoded = serde_json::to_string(&status).expect("encode status");
        let decoded: ConfigStatus = serde_json::from_str(&encoded).expect("decode status");
        assert_eq!(decoded, status);

        let audit = AuditRecord {
            api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
            event_id: "evt-1".to_string(),
            timestamp_ns: 42,
            actor: "spiffe://trust/site-a/admin".to_string(),
            action: "cfg_write".to_string(),
            target: "truST/site-a/cfg/desired/rt-1".to_string(),
            request_id: "req-1".to_string(),
            result: "denied".to_string(),
            error: Some(ReasonCode::PermissionDenied),
        };
        let encoded = serde_json::to_string(&audit).expect("encode audit");
        let decoded: AuditRecord = serde_json::from_str(&encoded).expect("decode audit");
        assert_eq!(decoded, audit);

        let catalog = CatalogEntry {
            api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
            schema_id: "io.main".to_string(),
            schema_version: 3,
            schema_hash: "sha256:feedface".to_string(),
            encoding: "twf".to_string(),
            qos: "t0".to_string(),
        };
        let encoded = serde_json::to_string(&catalog).expect("encode catalog");
        let decoded: CatalogEntry = serde_json::from_str(&encoded).expect("decode catalog");
        assert_eq!(decoded.schema_id, "io.main");
        assert_eq!(decoded.schema_version, 3);
        assert_eq!(decoded.schema_hash, "sha256:feedface");
    }

    #[test]
    fn schema_layout_accepts_forward_additive_changes_and_rejects_breaking_changes() {
        let previous = SchemaLayout {
            fields: vec![
                SchemaFieldLayout {
                    name: "a".to_string(),
                    offset: 0,
                    size: 4,
                    type_name: "DINT".to_string(),
                },
                SchemaFieldLayout {
                    name: "b".to_string(),
                    offset: 4,
                    size: 2,
                    type_name: "INT".to_string(),
                },
            ],
        };
        let additive = SchemaLayout {
            fields: vec![
                SchemaFieldLayout {
                    name: "a".to_string(),
                    offset: 0,
                    size: 4,
                    type_name: "DINT".to_string(),
                },
                SchemaFieldLayout {
                    name: "b".to_string(),
                    offset: 4,
                    size: 2,
                    type_name: "INT".to_string(),
                },
                SchemaFieldLayout {
                    name: "c".to_string(),
                    offset: 6,
                    size: 1,
                    type_name: "BYTE".to_string(),
                },
            ],
        };
        validate_forward_additive_schema(&previous, &additive).expect("additive schema must pass");

        let missing = SchemaLayout {
            fields: vec![SchemaFieldLayout {
                name: "a".to_string(),
                offset: 0,
                size: 4,
                type_name: "DINT".to_string(),
            }],
        };
        assert!(matches!(
            validate_forward_additive_schema(&previous, &missing),
            Err(SchemaCompatibilityError::MissingField(_))
        ));

        let offset_changed = SchemaLayout {
            fields: vec![
                SchemaFieldLayout {
                    name: "a".to_string(),
                    offset: 8,
                    size: 4,
                    type_name: "DINT".to_string(),
                },
                SchemaFieldLayout {
                    name: "b".to_string(),
                    offset: 4,
                    size: 2,
                    type_name: "INT".to_string(),
                },
            ],
        };
        assert!(matches!(
            validate_forward_additive_schema(&previous, &offset_changed),
            Err(SchemaCompatibilityError::OffsetChanged { .. })
        ));
    }

    #[test]
    fn schema_layout_rejects_ambiguous_or_non_appended_fields() {
        let previous = SchemaLayout {
            fields: vec![SchemaFieldLayout {
                name: "a".to_string(),
                offset: 0,
                size: 4,
                type_name: "DINT".to_string(),
            }],
        };

        for candidate in [
            SchemaLayout {
                fields: vec![previous.fields[0].clone(), previous.fields[0].clone()],
            },
            SchemaLayout {
                fields: vec![
                    previous.fields[0].clone(),
                    SchemaFieldLayout {
                        name: "over_existing".to_string(),
                        offset: 2,
                        size: 4,
                        type_name: "DINT".to_string(),
                    },
                ],
            },
            SchemaLayout {
                fields: vec![
                    previous.fields[0].clone(),
                    SchemaFieldLayout {
                        name: "new_a".to_string(),
                        offset: 4,
                        size: 4,
                        type_name: "DINT".to_string(),
                    },
                    SchemaFieldLayout {
                        name: "new_b".to_string(),
                        offset: 6,
                        size: 4,
                        type_name: "DINT".to_string(),
                    },
                ],
            },
            SchemaLayout {
                fields: vec![
                    previous.fields[0].clone(),
                    SchemaFieldLayout {
                        name: " ".to_string(),
                        offset: 4,
                        size: 1,
                        type_name: "BYTE".to_string(),
                    },
                ],
            },
            SchemaLayout {
                fields: vec![
                    previous.fields[0].clone(),
                    SchemaFieldLayout {
                        name: "empty".to_string(),
                        offset: 4,
                        size: 0,
                        type_name: "BYTE".to_string(),
                    },
                ],
            },
            SchemaLayout {
                fields: vec![
                    previous.fields[0].clone(),
                    SchemaFieldLayout {
                        name: "overflow".to_string(),
                        offset: u32::MAX,
                        size: 2,
                        type_name: "INT".to_string(),
                    },
                ],
            },
        ] {
            assert!(
                validate_forward_additive_schema(&previous, &candidate).is_err(),
                "ambiguous, overlapping, zero-sized, or overflowing fields must not be compatible: {candidate:?}"
            );
        }
    }

    #[test]
    fn catalog_epoch_cache_requests_refresh_only_on_monotonic_increase() {
        let mut cache = CatalogEpochCache::new();
        assert!(cache.observe(3));
        assert_eq!(cache.last_seen(), Some(3));
        assert!(!cache.observe(3));
        assert!(!cache.observe(2));
        assert!(cache.observe(4));
        assert_eq!(cache.last_seen(), Some(4));
    }
}
