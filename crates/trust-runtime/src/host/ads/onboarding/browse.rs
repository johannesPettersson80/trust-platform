//! The live ADS symbol browse of `comm.browse_symbols` (specification 11, "Symbol browsing"): the
//! route check, the symbol upload and the response shape that the control request and the ADS
//! port probe of a search share. The cached (offline) snapshot browse stays with the control
//! request in `control::comm_handlers::browse_symbols`.

use serde::Serialize;
use serde_json::Value;
use trust_ads_core::SymbolDescriptor;

#[cfg(feature = "ads-wire")]
use crate::ads::diagnostics::RoutePlan;
use crate::ads::diagnostics::{CredentialChannelClassification, TargetIdentity};
#[cfg(feature = "ads-wire")]
use crate::ads::onboarding::{
    build_route_plan, build_symbol_import_response, RoutePlanRequest, RoutePlanRole,
    SymbolImportRequest,
};

/// The schema version of a `comm.browse_symbols` answer.
pub(crate) const BROWSE_SYMBOLS_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Serialize)]
struct BrowseSymbolsResponse {
    schema_version: u32,
    protocol: String,
    kind: String,
    tree: Vec<SymbolTreeNode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<CommProtocolError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    route: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ads_import: Option<crate::ads::onboarding::SymbolImportResponse>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<String>,
}

/// One node of a `comm.browse_symbols` tree.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SymbolTreeNode {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) path: String,
    #[serde(rename = "type")]
    pub(crate) type_label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) node_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) data_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) writable: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) children: Vec<SymbolTreeNode>,
}

#[derive(Debug, Clone, Serialize)]
struct CommProtocolError {
    code: String,
    message: String,
}

/// Browses the symbols of a live ADS `target` over the wire, as `comm.browse_symbols` does.
///
/// # Errors
/// The target is invalid, or the response cannot be serialized.
pub(crate) fn browse_ads_live(
    target: TargetIdentity,
    connection_name: Option<String>,
    include_patterns: Vec<String>,
    name_prefix: Option<String>,
    channel: CredentialChannelClassification,
) -> Result<Value, String> {
    let connection_name = connection_name
        .or_else(|| target.name.clone())
        .unwrap_or_else(|| format!("ads_{}", sanitize_id(target.ams_net_id.as_str())));
    browse_live_ads_symbols(
        target,
        connection_name,
        include_patterns,
        name_prefix,
        channel,
    )
}

#[cfg(feature = "ads-wire")]
fn browse_live_ads_symbols(
    target: TargetIdentity,
    connection_name: String,
    include_patterns: Vec<String>,
    name_prefix: Option<String>,
    channel: CredentialChannelClassification,
) -> Result<Value, String> {
    use crate::ads::onboarding::{
        upload_failure_implies_missing_return_route, AdsOnboardingWire, AdsRsOnboardingWire,
    };

    let local = derive_local_identity(&target)?;
    let route_plan = build_route_plan(RoutePlanRequest {
        role: RoutePlanRole::Client,
        route_name: connection_name.clone(),
        target: target.clone(),
        local: local.clone(),
        channel,
    });
    let mut wire = AdsRsOnboardingWire::default();
    if let Err(error) = wire.check_route(&target, &local) {
        if route_check_failure_implies_missing_route(&error) {
            return missing_ads_route_browse_response(error.to_string(), route_plan);
        }
        let code = classify_ads_browse_error(&error);
        return response_tree_error_value(
            "ads".to_string(),
            "symbols".to_string(),
            code,
            format!("ADS port {} route check failed: {error}", target.ams_port),
        );
    }
    let selected_port = target.ams_port;
    let symbols = match wire.upload_symbols(&target) {
        Ok(symbols) => symbols,
        Err(error) if upload_failure_implies_missing_return_route(&error) => {
            return missing_ads_route_browse_response(error.to_string(), route_plan);
        }
        Err(error) => {
            let code = classify_ads_browse_error(&error);
            return response_tree_error_value(
                "ads".to_string(),
                "symbols".to_string(),
                code,
                format!("ADS port {selected_port} symbol browse failed: {error}"),
            );
        }
    };
    if symbols.is_empty() {
        return response_tree_error_value(
            "ads".to_string(),
            "symbols".to_string(),
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
            include_patterns,
            name_prefix,
        },
        symbols,
    );
    let route = serde_json::json!({
        "status": "ok",
        "detail": "ADS route accepted symbol upload.",
        "action": "ads.route_plan",
        "route_plan": route_plan,
    });
    response_value(
        "ads".to_string(),
        "symbols".to_string(),
        &import,
        Some(route),
        Vec::new(),
    )
}

#[cfg(not(feature = "ads-wire"))]
fn browse_live_ads_symbols(
    _target: TargetIdentity,
    _connection_name: String,
    _include_patterns: Vec<String>,
    _name_prefix: Option<String>,
    _channel: CredentialChannelClassification,
) -> Result<Value, String> {
    Err("ADS live symbol browsing needs a runtime built with the ads-wire feature; pass a cached snapshot for offline browsing".to_string())
}

#[cfg(feature = "ads-wire")]
pub(crate) fn classify_ads_browse_error(
    error: &crate::ads::onboarding::OnboardingWireError,
) -> &'static str {
    use crate::ads::onboarding::OnboardingWireErrorKind;
    let detail = error.detail.to_ascii_lowercase();
    if matches!(error.kind, OnboardingWireErrorKind::WrongPlcPort)
        || [
            "connection refused",
            "host unreachable",
            "network unreachable",
            "target port",
            "wrong plc port",
            "invalid ams port",
            "port disabled",
            "port not connected",
            "ads port not opened",
            "port not registered",
            "port is invalid",
            "port removed",
        ]
        .iter()
        .any(|needle| detail.contains(needle))
    {
        "ads_port_unavailable"
    } else if matches!(error.kind, OnboardingWireErrorKind::UnsupportedOperation)
        || [
            "not supported",
            "unsupported",
            "invalid index group",
            "service is not available",
            "unknown command id",
            "unknown ams command",
        ]
        .iter()
        .any(|needle| detail.contains(needle))
    {
        "symbol_upload_unsupported"
    } else if detail.contains("no more symbols in cache") {
        "empty_symbol_table"
    } else {
        "symbol_upload_failed"
    }
}

#[cfg(feature = "ads-wire")]
pub(crate) fn route_check_failure_implies_missing_route(
    error: &crate::ads::onboarding::OnboardingWireError,
) -> bool {
    matches!(
        error.kind,
        crate::ads::onboarding::OnboardingWireErrorKind::RouteMissing
    ) && classify_ads_browse_error(error) == "symbol_upload_failed"
}

#[cfg(feature = "ads-wire")]
pub(crate) fn missing_ads_route_browse_response(
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

pub(crate) fn response_value(
    protocol: String,
    kind: String,
    import: &crate::ads::onboarding::SymbolImportResponse,
    route: Option<Value>,
    warnings: Vec<String>,
) -> Result<Value, String> {
    serde_json::to_value(BrowseSymbolsResponse {
        schema_version: BROWSE_SYMBOLS_SCHEMA_VERSION,
        protocol,
        kind,
        tree: symbol_tree(import.snapshot.symbols.as_slice()),
        error: None,
        route,
        ads_import: Some(import.clone()),
        warnings,
    })
    .map_err(|error| format!("comm.browse_symbols serialization failed: {error}"))
}

pub(crate) fn response_tree_value(
    protocol: String,
    kind: String,
    tree: Vec<SymbolTreeNode>,
    warnings: Vec<String>,
) -> Result<Value, String> {
    serde_json::to_value(BrowseSymbolsResponse {
        schema_version: BROWSE_SYMBOLS_SCHEMA_VERSION,
        protocol,
        kind,
        tree,
        error: None,
        route: None,
        ads_import: None,
        warnings,
    })
    .map_err(|error| format!("comm.browse_symbols serialization failed: {error}"))
}

pub(crate) fn response_tree_error_value(
    protocol: String,
    kind: String,
    code: &str,
    message: String,
) -> Result<Value, String> {
    serde_json::to_value(BrowseSymbolsResponse {
        schema_version: BROWSE_SYMBOLS_SCHEMA_VERSION,
        protocol,
        kind,
        tree: Vec::new(),
        error: Some(CommProtocolError {
            code: code.to_string(),
            message,
        }),
        route: None,
        ads_import: None,
        warnings: Vec::new(),
    })
    .map_err(|error| format!("comm.browse_symbols serialization failed: {error}"))
}

pub(crate) fn symbol_tree(symbols: &[SymbolDescriptor]) -> Vec<SymbolTreeNode> {
    let mut roots: Vec<SymbolTreeNode> = Vec::new();
    for symbol in symbols {
        insert_symbol(
            &mut roots,
            &symbol.name.split('.').collect::<Vec<_>>(),
            symbol,
            "",
        );
    }
    roots
}

fn insert_symbol(
    siblings: &mut Vec<SymbolTreeNode>,
    parts: &[&str],
    symbol: &SymbolDescriptor,
    prefix: &str,
) {
    let Some((head, rest)) = parts.split_first() else {
        return;
    };
    let path = if prefix.is_empty() {
        (*head).to_string()
    } else {
        format!("{prefix}.{head}")
    };
    if rest.is_empty() {
        siblings.push(SymbolTreeNode {
            id: format!("ads:symbol:{}", sanitize_id(symbol.name.as_str())),
            name: (*head).to_string(),
            path: symbol.name.clone(),
            type_label: symbol.data_type.source_name.clone(),
            node_id: None,
            data_type: None,
            size: Some(symbol.byte_size),
            writable: Some(symbol.flags.contains(&trust_ads_core::SymbolFlag::Write)),
            children: Vec::new(),
        });
        siblings.sort_by(|left, right| left.name.cmp(&right.name));
        return;
    }
    let index = match siblings
        .iter()
        .position(|node| node.name == *head && node.size.is_none())
    {
        Some(index) => index,
        None => {
            siblings.push(SymbolTreeNode {
                id: format!("ads:group:{}", sanitize_id(path.as_str())),
                name: (*head).to_string(),
                path: path.clone(),
                type_label: "group".to_string(),
                node_id: None,
                data_type: None,
                size: None,
                writable: None,
                children: Vec::new(),
            });
            siblings.sort_by(|left, right| left.name.cmp(&right.name));
            siblings
                .iter()
                .position(|node| node.name == *head && node.size.is_none())
                .expect("inserted group is present")
        }
    };
    insert_symbol(&mut siblings[index].children, rest, symbol, path.as_str());
}

#[cfg(feature = "ads-wire")]
fn derive_local_identity(
    target: &TargetIdentity,
) -> Result<crate::ads::diagnostics::LocalIdentity, String> {
    use std::net::{SocketAddr, ToSocketAddrs};

    use crate::ads::onboarding::{derive_runtime_identity_from_source, IdentityRequest};

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

    let resolved_ip = resolve_target_ip(target.ip.as_str())?;
    let candidates = crate::ads::onboarding::runtime_address_candidates_from_interfaces()
        .map_err(|error| format!("enumerate local interfaces for ADS route check: {error}"))?;
    let source_ip = crate::ads::onboarding::resolve_os_source_ip(resolved_ip.as_str())
        .map_err(|error| format!("resolve local ADS source IP: {error}"))?;
    let nic = candidates
        .iter()
        .find(|candidate| candidate.ip == source_ip)
        .and_then(|candidate| candidate.nic.clone());
    derive_runtime_identity_from_source(
        &IdentityRequest {
            target_ip: resolved_ip,
            local_net_id_override: None,
        },
        source_ip,
        None,
        nic,
        candidates,
    )
    .map_err(|error| error.to_string())
}

/// `value` with every character except ASCII letters, digits, `-`, `_` and `.` replaced by `_`.
pub(crate) fn sanitize_id(value: &str) -> String {
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
