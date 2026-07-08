use serde::Serialize;
use sha2::{Digest, Sha256};
use trust_ads_core::SymbolSnapshot;

use super::*;

pub fn build_production_evidence(
    input: ProductionEvidenceInput<'_>,
) -> Result<ProductionEvidence, ProductionEvidenceError> {
    Ok(ProductionEvidence {
        doctor_timestamp_ms: input.doctor_timestamp_ms,
        doctor_schema_version: ADS_DIAGNOSTICS_SCHEMA_VERSION,
        runtime_identity_hash: hash_json(input.runtime_identity)?,
        target_identity_hash: Some(hash_json(input.target_identity)?),
        allowed_clients_hash: None,
        ads_config_hash: sha256_evidence_hash(input.ads_toml.as_bytes()),
        symbol_snapshot_hash: hash_symbol_snapshots(input.symbol_snapshots)?,
        generated_st_hash: input
            .generated_st
            .map(|source| sha256_evidence_hash(source.as_bytes())),
        deployed_ads_config_hash: input
            .deployed_ads_toml
            .map(|source| sha256_evidence_hash(source.as_bytes())),
        runtime_ads_status_hash: input.runtime_ads_status.map(hash_json).transpose()?,
        external_client_verified: false,
        external_client_kind: None,
        external_client_name: None,
        external_client_timestamp_ms: None,
        discoverable: false,
        freshness: EvidenceFreshness {
            stale_after_ms: input.stale_after_ms,
            expires_at_ms: input.expires_at_ms,
            runtime_clock_warning: input.runtime_clock_warning.map(ToString::to_string),
        },
    })
}

pub fn build_server_production_evidence<T: Serialize + ?Sized>(
    input: ServerProductionEvidenceInput<'_, T>,
) -> Result<ProductionEvidence, ProductionEvidenceError> {
    Ok(ProductionEvidence {
        doctor_timestamp_ms: input.doctor_timestamp_ms,
        doctor_schema_version: ADS_DIAGNOSTICS_SCHEMA_VERSION,
        runtime_identity_hash: hash_json(input.runtime_identity)?,
        target_identity_hash: None,
        allowed_clients_hash: Some(hash_json(input.allowed_clients)?),
        ads_config_hash: sha256_evidence_hash(input.ads_server_config.as_bytes()),
        symbol_snapshot_hash: hash_symbol_snapshots(std::slice::from_ref(input.symbol_snapshot))?,
        generated_st_hash: None,
        deployed_ads_config_hash: input
            .deployed_ads_server_config
            .map(|source| sha256_evidence_hash(source.as_bytes())),
        runtime_ads_status_hash: input.runtime_ads_status.map(hash_json).transpose()?,
        external_client_verified: input.external_client_verified,
        external_client_kind: input.external_client_kind.map(ToString::to_string),
        external_client_name: input.external_client_name.map(ToString::to_string),
        external_client_timestamp_ms: input.external_client_timestamp_ms,
        discoverable: input.discoverable,
        freshness: EvidenceFreshness {
            stale_after_ms: input.stale_after_ms,
            expires_at_ms: input.expires_at_ms,
            runtime_clock_warning: input.runtime_clock_warning.map(ToString::to_string),
        },
    })
}

pub fn evaluate_production_readiness(
    evidence: Option<&ProductionEvidence>,
    runtime_status: Option<&AdsStatusReport>,
    now_ms: u64,
) -> Result<ProductionReadinessReport, ProductionEvidenceError> {
    let Some(evidence) = evidence else {
        return Ok(production_readiness_report(
            ProductionReadinessState::NotReady,
            vec![ProductionReadinessReason::MissingEvidence],
        ));
    };
    let Some(status) = runtime_status else {
        return Ok(production_readiness_report(
            ProductionReadinessState::NeedsRecheck,
            vec![ProductionReadinessReason::MissingRuntimeStatus],
        ));
    };

    let mut reasons = Vec::new();
    if evidence
        .freshness
        .expires_at_ms
        .is_some_and(|expires_at| now_ms > expires_at)
    {
        reasons.push(ProductionReadinessReason::EvidenceExpired);
    }
    match (
        evidence.deployed_ads_config_hash.as_deref(),
        status.deployed_ads_config_hash.as_deref(),
    ) {
        (None, _) | (_, None) => reasons.push(ProductionReadinessReason::DeployedAdsConfigMissing),
        (Some(expected), Some(actual)) if expected != actual => {
            reasons.push(ProductionReadinessReason::DeployedAdsConfigMismatch);
        }
        _ => {}
    }
    if let Some(expected_status_hash) = evidence.runtime_ads_status_hash.as_deref() {
        let actual_status_hash = hash_json(status)?;
        if actual_status_hash != expected_status_hash {
            reasons.push(ProductionReadinessReason::RuntimeAdsStatusChanged);
        }
    }
    match status.overall {
        AdsStatusOverall::Faulted => reasons.push(ProductionReadinessReason::RuntimeAdsFaulted),
        AdsStatusOverall::Degraded
        | AdsStatusOverall::NotReady
        | AdsStatusOverall::Disabled
        | AdsStatusOverall::Unknown => reasons.push(ProductionReadinessReason::RuntimeAdsDegraded),
        AdsStatusOverall::Healthy => {}
    }

    let state = if reasons.is_empty() {
        ProductionReadinessState::Ready
    } else {
        ProductionReadinessState::NeedsRecheck
    };
    Ok(production_readiness_report(state, reasons))
}

fn production_readiness_report(
    state: ProductionReadinessState,
    reasons: Vec<ProductionReadinessReason>,
) -> ProductionReadinessReport {
    let summary = match state {
        ProductionReadinessState::Ready => {
            "ADS production-ready evidence matches the deployed runtime.".to_string()
        }
        ProductionReadinessState::NeedsRecheck => {
            format!(
                "ADS production-ready evidence needs recheck: {} reason(s).",
                reasons.len()
            )
        }
        ProductionReadinessState::NotReady => {
            "ADS production-ready evidence is not available.".to_string()
        }
    };
    ProductionReadinessReport {
        state,
        reasons,
        summary,
    }
}

pub fn sha256_evidence_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

fn hash_json<T: Serialize + ?Sized>(value: &T) -> Result<String, ProductionEvidenceError> {
    serde_json::to_vec(value)
        .map(|bytes| sha256_evidence_hash(&bytes))
        .map_err(ProductionEvidenceError::Serialize)
}

fn hash_symbol_snapshots(snapshots: &[SymbolSnapshot]) -> Result<String, ProductionEvidenceError> {
    let mut sorted = snapshots.to_vec();
    sorted.sort_by(|left, right| left.route_name.cmp(&right.route_name));
    let mut hasher = Sha256::new();
    for snapshot in &sorted {
        let json = snapshot
            .to_deterministic_json()
            .map_err(ProductionEvidenceError::Serialize)?;
        hasher.update(json.as_bytes());
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}
