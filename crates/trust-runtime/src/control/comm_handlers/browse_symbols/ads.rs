#[cfg(feature = "ads-wire")]
use std::net::{SocketAddr, ToSocketAddrs};

use serde_json::Value;

use crate::ads::diagnostics::{CredentialChannelClassification, TargetIdentity};
#[cfg(feature = "ads-wire")]
use crate::ads::diagnostics::{LocalIdentity, RoutePlan};
#[cfg(feature = "ads-wire")]
use crate::ads::onboarding::{build_route_plan, RoutePlanRequest};
use crate::ads::onboarding::{build_symbol_import_response, SymbolImportRequest};
#[cfg(feature = "ads-wire")]
use crate::ads::onboarding::{
    derive_host_ads_identity, runtime_address_candidates_from_interfaces, IdentityRequest,
};
#[cfg(feature = "ads-wire")]
use crate::ads::onboarding::{
    upload_failure_implies_missing_return_route, OnboardingWireError, OnboardingWireErrorKind,
};

#[cfg(feature = "ads-wire")]
use super::{response_tree_error_value, BrowseSymbolsResponse, BROWSE_SYMBOLS_SCHEMA_VERSION};
use super::{response_value, sanitize_id, BrowseSymbolsRequest, BrowseTarget, ControlState};

pub(super) fn browse_ads_symbols(
    mut request: BrowseSymbolsRequest,
    _state: Option<&ControlState>,
) -> Result<Value, String> {
    if let Some(mut snapshot) = request.snapshot {
        snapshot.canonicalize();
        let connection_name = request
            .connection_name
            .clone()
            .unwrap_or_else(|| snapshot.route_name.clone());
        let import = build_symbol_import_response(
            &SymbolImportRequest {
                connection_name,
                symbols: Vec::new(),
                include_patterns: request.include_patterns,
                name_prefix: request.name_prefix,
            },
            snapshot.symbols.clone(),
        );
        return response_value(request.protocol, request.kind, &import, None, Vec::new());
    }

    if request.instance_id.is_some() && request.target.is_none() {
        return Err(
            "ADS comm.browse_symbols by instance_id needs the UI to pass target params for now"
                .to_string(),
        );
    }
    let Some(target) = request.target.take() else {
        return Err("ADS comm.browse_symbols requires target or cached snapshot".to_string());
    };
    let target = target.into_identity()?;
    let connection_name = request
        .connection_name
        .clone()
        .or_else(|| target.name.clone())
        .unwrap_or_else(|| format!("ads_{}", sanitize_id(target.ams_net_id.as_str())));
    let channel = request
        .credential_channel
        .unwrap_or(CredentialChannelClassification::TrustedSameHost);
    browse_live_ads_symbols(target, connection_name, request, channel)
}

#[cfg(feature = "ads-wire")]
fn browse_live_ads_symbols(
    target: TargetIdentity,
    connection_name: String,
    request: BrowseSymbolsRequest,
    channel: CredentialChannelClassification,
) -> Result<Value, String> {
    use crate::ads::onboarding::{AdsOnboardingWire, AdsRouteRequirement, AdsRsOnboardingWire};

    let mut wire = AdsRsOnboardingWire::default();
    let route_requirement = wire.route_requirement(&target.ip);
    let reciprocal_route = reciprocal_route_context(
        &target,
        connection_name.as_str(),
        channel,
        route_requirement,
    )?;
    if let Some((local, route_plan)) = reciprocal_route.as_ref() {
        if let Err(error) = wire.check_route(&target, local) {
            if route_check_failure_requires_recovery(route_requirement, &error) {
                return missing_ads_route_browse_response(error.to_string(), route_plan.clone());
            }
            let code = classify_ads_browse_error(&error);
            return response_tree_error_value(
                request.protocol,
                request.kind,
                code,
                format!("ADS port {} route check failed: {error}", target.ams_port),
            );
        }
    }
    let selected_port = target.ams_port;
    let symbols = match wire.upload_symbols(&target) {
        Ok(symbols) => symbols,
        Err(error) if upload_failure_requires_route_recovery(route_requirement, &error) => {
            let route_plan = reciprocal_route
                .as_ref()
                .map(|(_, route_plan)| route_plan.clone())
                .expect("reciprocal route recovery has a route plan");
            return missing_ads_route_browse_response(error.to_string(), route_plan);
        }
        Err(error) => {
            let code = classify_ads_browse_error(&error);
            return response_tree_error_value(
                request.protocol,
                request.kind,
                code,
                format!("ADS port {selected_port} symbol browse failed: {error}"),
            );
        }
    };
    if symbols.is_empty() {
        return response_tree_error_value(
            request.protocol,
            request.kind,
            "empty_symbol_table",
            format!(
                "ADS port {selected_port} returned an empty symbol table or no compatible symbols"
            ),
        );
    }
    let import = build_symbol_import_response(
        &SymbolImportRequest {
            connection_name,
            symbols: Vec::new(),
            include_patterns: request.include_patterns,
            name_prefix: request.name_prefix,
        },
        symbols,
    );
    let route = match route_requirement {
        AdsRouteRequirement::ReciprocalRouteRequired => serde_json::json!({
            "status": "ok",
            "detail": "ADS route accepted symbol upload.",
            "action": "ads.route_plan",
            "route_plan": reciprocal_route
                .as_ref()
                .map(|(_, route_plan)| route_plan)
                .expect("reciprocal ADS browse has a route plan"),
        }),
        AdsRouteRequirement::NativeLocalRouter => serde_json::json!({
            "status": "not_required",
            "detail": "This computer uses the native Windows ADS router; no self-route is required."
        }),
    };
    response_value(
        "ads".to_string(),
        "symbols".to_string(),
        &import,
        Some(route),
        Vec::new(),
    )
}

#[cfg(feature = "ads-wire")]
pub(super) fn reciprocal_route_context(
    target: &TargetIdentity,
    connection_name: &str,
    channel: CredentialChannelClassification,
    route_requirement: crate::ads::onboarding::AdsRouteRequirement,
) -> Result<Option<(LocalIdentity, RoutePlan)>, String> {
    if route_requirement == crate::ads::onboarding::AdsRouteRequirement::NativeLocalRouter {
        return Ok(None);
    }
    let local = derive_local_identity(target)?;
    let route_plan = build_route_plan(RoutePlanRequest {
        role: crate::ads::onboarding::RoutePlanRole::Client,
        route_name: connection_name.to_string(),
        target: target.clone(),
        local: local.clone(),
        channel,
    });
    Ok(Some((local, route_plan)))
}

#[cfg(feature = "ads-wire")]
pub(super) fn classify_ads_browse_error(error: &OnboardingWireError) -> &'static str {
    if matches!(error.kind, OnboardingWireErrorKind::WrongPlcPort)
        || matches!(
            error.transport_failure,
            Some(
                crate::ads::AdsTransportFailureKind::ConnectionRefused
                    | crate::ads::AdsTransportFailureKind::HostUnreachable
                    | crate::ads::AdsTransportFailureKind::NetworkUnreachable
            )
        )
        || error.ads_error.as_ref().is_some_and(|error| {
            matches!(
                error.code,
                0x006 | 0x00D | 0x012 | 0x018 | 0x507 | 0x509 | 0x50D | 0x748
            )
        })
    {
        "ads_port_unavailable"
    } else if matches!(error.kind, OnboardingWireErrorKind::UnsupportedOperation)
        || error
            .ads_error
            .as_ref()
            .is_some_and(|error| matches!(error.code, 0x008 | 0x00B | 0x701 | 0x702))
    {
        "symbol_upload_unsupported"
    } else if error
        .ads_error
        .as_ref()
        .is_some_and(|error| error.code == 0x753)
    {
        "empty_symbol_table"
    } else {
        "symbol_upload_failed"
    }
}

#[cfg(feature = "ads-wire")]
pub(super) fn route_check_failure_implies_missing_route(error: &OnboardingWireError) -> bool {
    matches!(error.kind, OnboardingWireErrorKind::RouteMissing)
        && classify_ads_browse_error(error) == "symbol_upload_failed"
}

#[cfg(feature = "ads-wire")]
pub(super) fn route_check_failure_requires_recovery(
    requirement: crate::ads::onboarding::AdsRouteRequirement,
    error: &OnboardingWireError,
) -> bool {
    requirement == crate::ads::onboarding::AdsRouteRequirement::ReciprocalRouteRequired
        && route_check_failure_implies_missing_route(error)
}

#[cfg(feature = "ads-wire")]
pub(super) fn upload_failure_requires_route_recovery(
    requirement: crate::ads::onboarding::AdsRouteRequirement,
    error: &OnboardingWireError,
) -> bool {
    requirement == crate::ads::onboarding::AdsRouteRequirement::ReciprocalRouteRequired
        && upload_failure_implies_missing_return_route(error)
}

#[cfg(feature = "ads-wire")]
pub(super) fn missing_ads_route_browse_response(
    detail: String,
    route_plan: RoutePlan,
) -> Result<Value, String> {
    let response = BrowseSymbolsResponse {
        schema_version: BROWSE_SYMBOLS_SCHEMA_VERSION,
        protocol: "ads".to_string(),
        kind: "symbols".to_string(),
        tree: Vec::new(),
        error: None,
        route: Some(ads_route_missing_payload(detail, route_plan)),
        ads_import: None,
        warnings: vec![
            "ADS route is not ready; create or fix the route before browsing symbols.".to_string(),
        ],
    };
    serde_json::to_value(response)
        .map_err(|error| format!("comm.browse_symbols serialization failed: {error}"))
}

#[cfg(feature = "ads-wire")]
fn ads_route_missing_payload(detail: String, route_plan: RoutePlan) -> Value {
    serde_json::json!({
        "status": "missing",
        "detail": detail,
        "action": "ads.route_plan",
        "route_plan": route_plan,
    })
}

#[cfg(not(feature = "ads-wire"))]
fn browse_live_ads_symbols(
    _target: TargetIdentity,
    _connection_name: String,
    _request: BrowseSymbolsRequest,
    _channel: CredentialChannelClassification,
) -> Result<Value, String> {
    Err("ADS live symbol browsing needs a runtime built with the ads-wire feature; pass a cached snapshot for offline browsing".to_string())
}

#[cfg(feature = "ads-wire")]
fn derive_local_identity(target: &TargetIdentity) -> Result<LocalIdentity, String> {
    let resolved_ip = resolve_target_ip(target.ip.as_str())?;
    let candidates = runtime_address_candidates_from_interfaces()
        .map_err(|error| format!("enumerate local interfaces for ADS route check: {error}"))?;
    derive_host_ads_identity(
        &IdentityRequest {
            target_ip: resolved_ip,
            local_net_id_override: None,
        },
        candidates,
    )
    .map_err(|error| error.to_string())
}

#[cfg(feature = "ads-wire")]
fn resolve_target_ip(host: &str) -> Result<String, String> {
    if host.parse::<std::net::IpAddr>().is_ok() {
        return Ok(host.to_string());
    }
    (host, 48898)
        .to_socket_addrs()
        .map_err(|error| format!("resolve ADS target '{host}': {error}"))?
        .map(|addr: SocketAddr| addr.ip().to_string())
        .next()
        .ok_or_else(|| format!("resolve ADS target '{host}': no address found"))
}

impl BrowseTarget {
    pub(super) fn into_identity(self) -> Result<TargetIdentity, String> {
        let host = self.ip.trim();
        if host.is_empty() {
            return Err("ADS browse target needs host/ip".to_string());
        }
        let ams_net_id = self.ams_net_id.trim();
        if ams_net_id.is_empty() {
            return Err("ADS browse target needs ams_net_id".to_string());
        }
        let ams_port = self.ams_port.unwrap_or(851);
        if ams_port == 0 {
            return Err("ADS browse target ams_port must be between 1 and 65535".to_string());
        }
        Ok(TargetIdentity {
            name: self.name,
            ip: host.to_string(),
            ams_net_id: ams_net_id.to_string(),
            ams_port,
            tc_version: self.tc_version,
        })
    }
}
