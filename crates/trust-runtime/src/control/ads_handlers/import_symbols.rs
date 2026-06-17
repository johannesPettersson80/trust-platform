use serde::Deserialize;
use trust_ads_core::SymbolSnapshot;

use crate::ads::diagnostics::TargetIdentity;
use crate::ads::onboarding::{build_symbol_import_response, SymbolImportRequest};

use super::super::{ControlResponse, ControlState};

pub(in crate::control) fn handle_ads_import_symbols(
    id: u64,
    params: Option<serde_json::Value>,
    _state: &ControlState,
) -> ControlResponse {
    let params: ImportSymbolsControlParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(error) => return ControlResponse::error(id, format!("invalid params: {error}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };

    let request = SymbolImportRequest {
        connection_name: params.connection_name.clone(),
        include_patterns: params.include_patterns,
        name_prefix: params.name_prefix,
    };
    let symbols = if let Some(mut snapshot) = params.snapshot {
        snapshot.canonicalize();
        if snapshot.route_name != request.connection_name {
            return ControlResponse::error(
                id,
                format!(
                    "snapshot route '{}' does not match import connection '{}'",
                    snapshot.route_name, request.connection_name
                ),
            );
        }
        snapshot.symbols
    } else {
        let Some(target) = params.target else {
            return ControlResponse::error(
                id,
                "ads.import_symbols requires either a cached snapshot or a live target".to_string(),
            );
        };
        match upload_live_symbols(&target) {
            Ok(symbols) => symbols,
            Err(error) => return ControlResponse::error(id, error),
        }
    };

    let response = build_symbol_import_response(&request, symbols);
    match serde_json::to_value(response) {
        Ok(value) => ControlResponse::ok(id, value),
        Err(error) => ControlResponse::error(
            id,
            format!("ADS import-symbols serialization failed: {error}"),
        ),
    }
}

#[cfg(feature = "ads-wire")]
fn upload_live_symbols(
    target: &TargetIdentity,
) -> Result<Vec<trust_ads_core::SymbolDescriptor>, String> {
    let mut wire = crate::ads::onboarding::AdsRsOnboardingWire::default();
    crate::ads::onboarding::AdsOnboardingWire::upload_symbols(&mut wire, target)
        .map_err(|error| error.to_string())
}

#[cfg(not(feature = "ads-wire"))]
fn upload_live_symbols(
    _target: &TargetIdentity,
) -> Result<Vec<trust_ads_core::SymbolDescriptor>, String> {
    Err("ADS live symbol import needs an ads-wire build or a cached snapshot".to_string())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportSymbolsControlParams {
    connection_name: String,
    #[serde(default)]
    include_patterns: Vec<String>,
    #[serde(default)]
    name_prefix: Option<String>,
    #[serde(default)]
    target: Option<TargetIdentity>,
    #[serde(default)]
    snapshot: Option<SymbolSnapshot>,
}
