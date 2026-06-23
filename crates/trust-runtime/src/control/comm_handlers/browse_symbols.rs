#[cfg(feature = "ads-wire")]
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use trust_ads_core::{SymbolFlag, SymbolSnapshot};

use crate::ads::diagnostics::{CredentialChannelClassification, TargetIdentity};
#[cfg(feature = "ads-wire")]
use crate::ads::diagnostics::{LocalIdentity, RoutePlan};
#[cfg(feature = "ads-wire")]
use crate::ads::onboarding::upload_failure_implies_missing_return_route;
#[cfg(feature = "ads-wire")]
use crate::ads::onboarding::{build_route_plan, RoutePlanRequest};
use crate::ads::onboarding::{build_symbol_import_response, SymbolImportRequest};
#[cfg(feature = "ads-wire")]
use crate::ads::onboarding::{
    derive_runtime_identity_from_source, resolve_os_source_ip,
    runtime_address_candidates_from_interfaces, IdentityRequest,
};
use crate::bundle_builder::collect_project_source_files;
use crate::harness::CompileSession;

use super::super::{ControlResponse, ControlState};

const BROWSE_SYMBOLS_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
struct BrowseSymbolsRequest {
    protocol: String,
    #[serde(default)]
    target: Option<BrowseTarget>,
    #[serde(default)]
    instance_id: Option<String>,
    #[serde(default = "default_kind")]
    kind: String,
    #[serde(default)]
    connection_name: Option<String>,
    #[serde(default)]
    include_patterns: Vec<String>,
    #[serde(default)]
    name_prefix: Option<String>,
    #[serde(default)]
    snapshot: Option<SymbolSnapshot>,
    #[serde(default)]
    credential_channel: Option<CredentialChannelClassification>,
}

#[derive(Debug, Deserialize)]
struct BrowseTarget {
    #[serde(default)]
    local: bool,
    #[serde(default)]
    endpoint_url: String,
    #[serde(default, alias = "host")]
    ip: String,
    #[serde(default, alias = "ams_net_id")]
    ams_net_id: String,
    #[serde(default)]
    ams_port: Option<u16>,
    #[serde(default)]
    security_policy: Option<String>,
    #[serde(default)]
    security_mode: Option<String>,
    #[serde(default)]
    auth: Option<String>,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    trust_server_certificate: Option<bool>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    tc_version: Option<String>,
}

#[derive(Debug, Serialize)]
struct BrowseSymbolsResponse {
    schema_version: u32,
    protocol: String,
    kind: String,
    tree: Vec<SymbolTreeNode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    route: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ads_import: Option<crate::ads::onboarding::SymbolImportResponse>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct SymbolTreeNode {
    id: String,
    name: String,
    path: String,
    #[serde(rename = "type")]
    data_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    writable: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    children: Vec<SymbolTreeNode>,
}

pub(super) fn handle_comm_browse_symbols(
    id: u64,
    params: Option<Value>,
    state: &ControlState,
) -> ControlResponse {
    let Some(params) = params else {
        return ControlResponse::error(id, "missing comm.browse_symbols params".into());
    };
    match browse_symbols_value(params, Some(state), state.project_root.as_deref()) {
        Ok(value) => ControlResponse::ok(id, value),
        Err(error) => ControlResponse::error(id, error),
    }
}

pub(super) fn browse_symbols_value(
    params: Value,
    state: Option<&ControlState>,
    project_root: Option<&Path>,
) -> Result<Value, String> {
    let mut request: BrowseSymbolsRequest = serde_json::from_value(params)
        .map_err(|error| format!("invalid comm.browse_symbols payload: {error}"))?;
    request.protocol = canonical_protocol(request.protocol.as_str());
    if is_local_symbol_picker(&request) {
        return browse_local_project_symbols(request, project_root);
    }
    if request.protocol == "ethercat" && request.kind == "channels" {
        return browse_ethercat_channels(request, project_root);
    }
    if request.protocol == "ads" && request.kind != "symbols" {
        return Err(format!(
            "ADS comm.browse_symbols supports kind='symbols', got '{}'",
            request.kind
        ));
    }
    if request.protocol == "ads" {
        return browse_ads_symbols(request, state);
    }
    if request.protocol == "opcua_client" && request.kind == "nodes" {
        return browse_opcua_client_nodes(request);
    }
    Err(format!(
        "comm.browse_symbols does not support protocol '{}' with kind '{}'",
        request.protocol, request.kind
    ))
}

fn browse_opcua_client_nodes(mut request: BrowseSymbolsRequest) -> Result<Value, String> {
    let Some(target) = request.target.take() else {
        return Err("OPC UA client browse requires target endpoint settings".to_string());
    };
    let endpoint_url = target.opcua_endpoint_url()?;
    let security = target.opcua_security_profile()?;
    let auth = target.opcua_auth()?;
    let trust_server_certificate = target.trust_server_certificate.unwrap_or(false);
    let nodes = crate::opcua::browse_opcua_client_nodes(
        endpoint_url.as_str(),
        security,
        auth,
        trust_server_certificate,
        4,
        512,
    )
    .map_err(|error| format!("OPC UA node browse failed: {error}"))?;
    let tree = nodes.into_iter().map(opcua_node_to_symbol).collect();
    response_tree_value(request.protocol, request.kind, tree, Vec::new())
}

fn browse_ads_symbols(
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
    use crate::ads::onboarding::{AdsOnboardingWire, AdsRsOnboardingWire};

    let local = derive_local_identity(&target)?;
    let route_plan = build_route_plan(RoutePlanRequest {
        role: crate::ads::onboarding::RoutePlanRole::Client,
        route_name: connection_name.clone(),
        target: target.clone(),
        local: local.clone(),
        channel,
    });
    let mut wire = AdsRsOnboardingWire::default();
    if let Err(error) = wire.check_route(&target, &local) {
        return missing_ads_route_browse_response(error.to_string(), route_plan);
    }
    let symbols = match wire.upload_symbols(&target) {
        Ok(symbols) => symbols,
        Err(error) if upload_failure_implies_missing_return_route(&error) => {
            return missing_ads_route_browse_response(error.to_string(), route_plan);
        }
        Err(error) => return Err(format!("ADS symbol upload failed: {error}")),
    };
    let import = build_symbol_import_response(
        &SymbolImportRequest {
            connection_name,
            symbols: Vec::new(),
            include_patterns: request.include_patterns,
            name_prefix: request.name_prefix,
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

#[cfg(feature = "ads-wire")]
fn missing_ads_route_browse_response(
    detail: String,
    route_plan: RoutePlan,
) -> Result<Value, String> {
    let response = BrowseSymbolsResponse {
        schema_version: BROWSE_SYMBOLS_SCHEMA_VERSION,
        protocol: "ads".to_string(),
        kind: "symbols".to_string(),
        tree: Vec::new(),
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

fn response_value(
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
        route,
        ads_import: Some(import.clone()),
        warnings,
    })
    .map_err(|error| format!("comm.browse_symbols serialization failed: {error}"))
}

fn response_tree_value(
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
        route: None,
        ads_import: None,
        warnings,
    })
    .map_err(|error| format!("comm.browse_symbols serialization failed: {error}"))
}

fn is_local_symbol_picker(request: &BrowseSymbolsRequest) -> bool {
    request.kind == "symbols"
        && matches!(
            request.protocol.as_str(),
            "opcua_server" | "ads_server" | "openot"
        )
        && match request.target.as_ref() {
            Some(target) => target.local || target.ip.trim().is_empty(),
            None => true,
        }
}

fn browse_local_project_symbols(
    request: BrowseSymbolsRequest,
    project_root: Option<&Path>,
) -> Result<Value, String> {
    let Some(project_root) = project_root else {
        return Err(format!(
            "{} local symbol browsing needs --project or a runtime control state with project_root",
            request.protocol
        ));
    };
    let sources = collect_project_source_files(project_root, None)
        .map_err(|error| format!("failed to collect project sources: {error}"))?;
    let runtime = CompileSession::from_sources(sources)
        .build_runtime()
        .map_err(|error| format!("failed to compile project globals: {error}"))?;
    let mut children = runtime
        .globals()
        .iter()
        .map(|(name, meta)| SymbolTreeNode {
            id: format!("local:symbol:{}", sanitize_id(name.as_str())),
            name: name.to_string(),
            path: format!("global.{name}"),
            data_type: runtime
                .registry()
                .type_name(meta.type_id)
                .map(|name| name.to_string())
                .unwrap_or_else(|| "UNKNOWN".to_string()),
            size: None,
            writable: None,
            children: Vec::new(),
        })
        .collect::<Vec<_>>();
    children.sort_by(|left, right| left.name.cmp(&right.name));
    response_tree_value(
        request.protocol,
        request.kind,
        vec![SymbolTreeNode {
            id: "local:group:global".to_string(),
            name: "global".to_string(),
            path: "global".to_string(),
            data_type: "group".to_string(),
            size: None,
            writable: None,
            children,
        }],
        Vec::new(),
    )
}

fn browse_ethercat_channels(
    request: BrowseSymbolsRequest,
    project_root: Option<&Path>,
) -> Result<Value, String> {
    let Some(project_root) = project_root else {
        return Err(
            "EtherCAT channel browsing needs --project or a runtime control state with project_root"
                .to_string(),
        );
    };
    let drivers = load_project_io_drivers(project_root)?;
    let selected = select_ethercat_driver(&drivers, request.instance_id.as_deref())?;
    let modules = crate::io::configured_ethercat_modules(&selected.params)
        .map_err(|error| format!("failed to parse EtherCAT modules: {error}"))?;
    let tree = modules
        .iter()
        .map(ethercat_module_channel_node)
        .collect::<Vec<_>>();
    response_tree_value(request.protocol, request.kind, tree, Vec::new())
}

fn load_project_io_drivers(
    project_root: &Path,
) -> Result<Vec<crate::config::IoDriverConfig>, String> {
    let path = project_root.join("io.toml");
    if !path.is_file() {
        return Ok(Vec::new());
    }
    crate::config::IoConfig::load(path)
        .map(|config| config.drivers)
        .map_err(|error| format!("failed to load io.toml: {error}"))
}

fn select_ethercat_driver<'a>(
    drivers: &'a [crate::config::IoDriverConfig],
    instance_id: Option<&str>,
) -> Result<&'a crate::config::IoDriverConfig, String> {
    let ethercat = drivers
        .iter()
        .enumerate()
        .filter(|(_, driver)| canonical_protocol(driver.name.as_str()) == "ethercat")
        .collect::<Vec<_>>();
    if ethercat.is_empty() {
        return Err("project has no configured EtherCAT driver in io.toml".to_string());
    }
    if let Some(instance_id) = instance_id {
        return ethercat
            .into_iter()
            .find(|(index, _)| {
                instance_id == format!("ethercat:{index}")
                    || instance_id == format!("endpoint:ethercat:{index}")
                    || instance_id.ends_with(format!(":ethercat:{index}").as_str())
                    || (*index == 0 && instance_id.ends_with(":ethercat"))
            })
            .map(|(_, driver)| driver)
            .ok_or_else(|| format!("no EtherCAT driver matches instance_id '{instance_id}'"));
    }
    Ok(ethercat[0].1)
}

fn ethercat_module_channel_node(module: &crate::io::EthercatModuleInfo) -> SymbolTreeNode {
    let module_path = format!("ethercat.slot{}", module.slot);
    let children = (0..module.channels)
        .map(|channel| {
            let path = format!("{module_path}.channel{channel}");
            SymbolTreeNode {
                id: format!("ethercat:channel:slot{}:{}", module.slot, channel),
                name: format!("Channel {channel}"),
                path,
                data_type: "BOOL".to_string(),
                size: Some(1),
                writable: None,
                children: Vec::new(),
            }
        })
        .collect();
    SymbolTreeNode {
        id: format!("ethercat:module:slot{}", module.slot),
        name: format!("{} (slot {})", module.model, module.slot),
        path: module_path,
        data_type: "field_slave".to_string(),
        size: Some(u32::from(module.channels)),
        writable: None,
        children,
    }
}

fn symbol_tree(symbols: &[trust_ads_core::SymbolDescriptor]) -> Vec<SymbolTreeNode> {
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
    symbol: &trust_ads_core::SymbolDescriptor,
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
            data_type: symbol.data_type.source_name.clone(),
            size: Some(symbol.byte_size),
            writable: Some(symbol.flags.contains(&SymbolFlag::Write)),
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
                data_type: "group".to_string(),
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
fn derive_local_identity(target: &TargetIdentity) -> Result<LocalIdentity, String> {
    let resolved_ip = resolve_target_ip(target.ip.as_str())?;
    let candidates = runtime_address_candidates_from_interfaces()
        .map_err(|error| format!("enumerate local interfaces for ADS route check: {error}"))?;
    let source_ip = resolve_os_source_ip(resolved_ip.as_str())
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

fn canonical_protocol(protocol: &str) -> String {
    match protocol
        .trim()
        .to_ascii_lowercase()
        .replace('-', "_")
        .as_str()
    {
        "ads" | "ads_client" | "twincat" => "ads".to_string(),
        "ads_server" | "ads-server" => "ads_server".to_string(),
        "opcua" | "opc_ua" | "opcua_server" | "opc_ua_server" | "opcua-server" => {
            "opcua_server".to_string()
        }
        "opcua_client" | "opc_ua_client" | "opcua-client" => "opcua_client".to_string(),
        "openot" | "open_ot" => "openot".to_string(),
        "ethercat" | "ether_cat" | "ecat" => "ethercat".to_string(),
        other => other.to_string(),
    }
}

fn default_kind() -> String {
    "symbols".to_string()
}

impl BrowseTarget {
    fn opcua_endpoint_url(&self) -> Result<String, String> {
        let endpoint_url = self.endpoint_url.trim();
        if !endpoint_url.is_empty() {
            return Ok(endpoint_url.to_string());
        }
        let host = self.ip.trim();
        if host.is_empty() {
            return Err("OPC UA browse target needs endpoint_url or host".to_string());
        }
        if host.starts_with("opc.tcp://") {
            Ok(host.to_string())
        } else if host.contains('/')
            || host
                .rsplit_once(':')
                .is_some_and(|(_, port)| port.parse::<u16>().is_ok())
        {
            Ok(format!("opc.tcp://{host}"))
        } else {
            Ok(format!("opc.tcp://{host}:4840"))
        }
    }

    fn opcua_security_profile(&self) -> Result<crate::opcua::OpcUaSecurityProfile, String> {
        let policy_raw = self.security_policy.as_deref().unwrap_or("none");
        let mode_raw = self.security_mode.as_deref().unwrap_or("none");
        let policy = crate::opcua::OpcUaSecurityPolicy::parse(policy_raw)
            .ok_or_else(|| format!("invalid OPC UA security_policy '{policy_raw}'"))?;
        let mode = crate::opcua::OpcUaMessageSecurityMode::parse(mode_raw)
            .ok_or_else(|| format!("invalid OPC UA security_mode '{mode_raw}'"))?;
        Ok(crate::opcua::OpcUaSecurityProfile {
            policy,
            mode,
            allow_anonymous: self.auth.as_deref().unwrap_or("anonymous") == "anonymous",
        })
    }

    fn opcua_auth(&self) -> Result<crate::opcua::OpcUaClientAuthConfig, String> {
        match self
            .auth
            .as_deref()
            .unwrap_or("anonymous")
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "anonymous" => Ok(crate::opcua::OpcUaClientAuthConfig::Anonymous),
            "username" | "user_name" | "user" => {
                let username = self
                    .username
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| "OPC UA username auth needs username".to_string())?;
                let password = self
                    .password
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| "OPC UA username auth needs password".to_string())?;
                Ok(crate::opcua::OpcUaClientAuthConfig::UserName {
                    username: smol_str::SmolStr::new(username),
                    password: smol_str::SmolStr::new(password),
                })
            }
            other => Err(format!(
                "OPC UA auth must be anonymous or username, got '{other}'"
            )),
        }
    }

    fn into_identity(self) -> Result<TargetIdentity, String> {
        let host = self.ip.trim();
        if host.is_empty() {
            return Err("ADS browse target needs host/ip".to_string());
        }
        let ams_net_id = self.ams_net_id.trim();
        if ams_net_id.is_empty() {
            return Err("ADS browse target needs ams_net_id".to_string());
        }
        Ok(TargetIdentity {
            name: self.name,
            ip: host.to_string(),
            ams_net_id: ams_net_id.to_string(),
            ams_port: self.ams_port.unwrap_or(851),
            tc_version: self.tc_version,
        })
    }
}

fn opcua_node_to_symbol(node: crate::opcua::OpcUaBrowseNode) -> SymbolTreeNode {
    SymbolTreeNode {
        id: format!("opcua:node:{}", sanitize_id(node.id.as_str())),
        name: node.name,
        path: node.path,
        data_type: node.data_type,
        size: None,
        writable: Some(node.writable),
        children: node
            .children
            .into_iter()
            .map(opcua_node_to_symbol)
            .collect(),
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;
    use trust_ads_core::{AdsDataTypeDescriptor, IecDataType, SymbolDescriptor};

    use super::*;

    #[test]
    fn ads_cached_snapshot_returns_tree_and_existing_import_shape() {
        let mut writable = SymbolDescriptor::new(
            "GVL.Setpoint",
            AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
            0x4020,
            4,
            4,
        );
        writable.flags.insert(SymbolFlag::Read);
        writable.flags.insert(SymbolFlag::Write);
        let snapshot = SymbolSnapshot::new(
            "line1",
            vec![
                writable,
                SymbolDescriptor::new(
                    "MAIN.Temperature",
                    AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
                    0x4020,
                    8,
                    4,
                )
                .with_flag(SymbolFlag::Read),
            ],
        );

        let value = browse_symbols_value(
            json!({
                "protocol": "ads",
                "kind": "symbols",
                "connection_name": "line1",
                "snapshot": snapshot,
                "include_patterns": ["setpoint"]
            }),
            None,
            None,
        )
        .expect("browse symbols");
        let tree = value.get("tree").and_then(Value::as_array).expect("tree");
        assert_eq!(tree.len(), 2);
        let gvl = tree
            .iter()
            .find(|node| node.get("name").and_then(Value::as_str) == Some("GVL"))
            .expect("GVL group");
        let setpoint = gvl
            .get("children")
            .and_then(Value::as_array)
            .and_then(|children| children.first())
            .expect("setpoint child");
        assert_eq!(
            setpoint.get("writable").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(setpoint.get("type").and_then(Value::as_str), Some("REAL"));
        let candidates = value
            .get("ads_import")
            .and_then(|import| import.get("candidates"))
            .and_then(Value::as_array)
            .expect("ads import candidates");
        assert_eq!(candidates.len(), 2);
        assert_eq!(
            candidates[0].get("access").and_then(Value::as_str),
            Some("read")
        );
        assert!(
            serde_json::to_string(&value)
                .expect("json")
                .contains("ads_import"),
            "browse response must expose the existing ADS import shape"
        );
    }

    #[cfg(feature = "ads-wire")]
    #[test]
    fn ads_symbol_upload_timeout_returns_route_missing_response() {
        let route_plan = build_route_plan(RoutePlanRequest {
            role: crate::ads::onboarding::RoutePlanRole::Client,
            route_name: "line1".to_string(),
            target: TargetIdentity {
                name: Some("TwinCAT".to_string()),
                ip: "192.168.77.11".to_string(),
                ams_net_id: "100.67.6.217.1.1".to_string(),
                ams_port: 851,
                tc_version: Some("3.1.4026".to_string()),
            },
            local: LocalIdentity {
                host_name: Some("trust-pi".to_string()),
                chosen_ip: "192.168.77.10".to_string(),
                ams_net_id: "192.168.77.10.1.1".to_string(),
                nic: Some("eth0".to_string()),
                candidates: Vec::new(),
                classification: crate::ads::diagnostics::LocalNetworkClassification::Lan,
            },
            channel: CredentialChannelClassification::TrustedSameHost,
        });

        let value = missing_ads_route_browse_response(
            "NoSymbols: upload ADS symbol table: receiving reply (route set?): timed out"
                .to_string(),
            route_plan,
        )
        .expect("route missing browse response");

        assert_eq!(
            value.pointer("/route/status").and_then(Value::as_str),
            Some("missing")
        );
        assert_eq!(
            value.pointer("/route/action").and_then(Value::as_str),
            Some("ads.route_plan")
        );
        assert_eq!(
            value
                .pointer("/tree")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(0)
        );
        assert!(value
            .pointer("/route/detail")
            .and_then(Value::as_str)
            .is_some_and(|detail| detail.contains("timed out")));
        assert!(
            value
                .pointer("/route/route_plan/local/ams_net_id")
                .is_some(),
            "route-missing response must include the route plan the UI uses"
        );
    }

    #[test]
    fn local_project_symbol_picker_returns_declared_globals() {
        let root = temp_dir("browse-local-globals");
        write_file(
            &root.join("src/main.st"),
            r#"
VAR_GLOBAL
    Setpoint : REAL;
    PumpRunning : BOOL;
END_VAR

PROGRAM Main
END_PROGRAM
"#,
        );

        let value = browse_symbols_value(
            json!({
                "protocol": "opcua_server",
                "kind": "symbols",
                "target": { "local": true }
            }),
            None,
            Some(root.as_path()),
        )
        .expect("local project symbols");

        assert_eq!(
            value.pointer("/protocol").and_then(Value::as_str),
            Some("opcua_server")
        );
        let children = value
            .pointer("/tree/0/children")
            .and_then(Value::as_array)
            .expect("global children");
        assert!(children.iter().any(|node| {
            node.get("path").and_then(Value::as_str) == Some("global.Setpoint")
                && node.get("type").and_then(Value::as_str) == Some("REAL")
        }));
        assert!(children.iter().any(|node| {
            node.get("path").and_then(Value::as_str) == Some("global.PumpRunning")
                && node.get("type").and_then(Value::as_str) == Some("BOOL")
        }));
    }

    #[test]
    fn ethercat_channel_browse_returns_configured_module_channels() {
        let root = temp_dir("browse-ethercat-channels");
        write_file(
            &root.join("io.toml"),
            r#"
[io]
safe_state = []

[[io.drivers]]
name = "ethercat"
params = { adapter = "mock", modules = [{ model = "EK1100", slot = 0, channels = 1 }, { model = "EL1008", slot = 1, channels = 8 }] }
"#,
        );

        let value = browse_symbols_value(
            json!({
                "protocol": "ethercat",
                "kind": "channels"
            }),
            None,
            Some(root.as_path()),
        )
        .expect("ethercat channels");

        let modules = value
            .pointer("/tree")
            .and_then(Value::as_array)
            .expect("module tree");
        let input = modules
            .iter()
            .find(|node| node.get("name").and_then(Value::as_str) == Some("EL1008 (slot 1)"))
            .expect("EL1008 module");
        assert_eq!(
            input
                .get("children")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(8)
        );
        assert_eq!(
            input.get("type").and_then(Value::as_str),
            Some("field_slave")
        );
    }

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("trust-browse-symbols-{name}-{stamp}"));
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    fn write_file(path: &std::path::Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dir");
        }
        fs::write(path, content).expect("write file");
    }
}
