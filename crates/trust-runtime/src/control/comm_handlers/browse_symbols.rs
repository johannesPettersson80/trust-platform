use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use trust_ads_core::{SymbolFlag, SymbolSnapshot};

use crate::ads::diagnostics::CredentialChannelClassification;
#[cfg(all(test, feature = "ads-wire"))]
use crate::ads::diagnostics::{LocalIdentity, TargetIdentity};
#[cfg(all(test, feature = "ads-wire"))]
use crate::ads::onboarding::{
    build_route_plan, OnboardingWireError, OnboardingWireErrorKind, RoutePlanRequest,
};
use crate::bundle_builder::collect_project_source_files;
use crate::harness::CompileSession;

use super::super::{ControlResponse, ControlState};

mod ads;

use ads::browse_ads_symbols;
#[cfg(all(test, feature = "ads-wire"))]
use ads::{
    classify_ads_browse_error, missing_ads_route_browse_response, reciprocal_route_context,
    route_check_failure_implies_missing_route, route_check_failure_requires_recovery,
    upload_failure_requires_route_recovery,
};

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
    #[serde(default, alias = "ams_net_id", alias = "target_net_id")]
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
    error: Option<CommProtocolError>,
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
    type_label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    node_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    writable: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    children: Vec<SymbolTreeNode>,
}

#[derive(Debug, Clone, Serialize)]
struct CommProtocolError {
    code: String,
    message: String,
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
    let nodes = match crate::opcua::browse_opcua_client_nodes(
        endpoint_url.as_str(),
        security,
        auth,
        trust_server_certificate,
        4,
        512,
    ) {
        Ok(nodes) => nodes,
        Err(error) => {
            return response_tree_error_value(
                request.protocol,
                request.kind,
                crate::opcua::classify_opcua_client_browse_error(&error).as_str(),
                format!("OPC UA node browse failed: {error}"),
            )
        }
    };
    let tree = nodes.into_iter().map(opcua_node_to_symbol).collect();
    response_tree_value(request.protocol, request.kind, tree, Vec::new())
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
        error: None,
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
        error: None,
        route: None,
        ads_import: None,
        warnings,
    })
    .map_err(|error| format!("comm.browse_symbols serialization failed: {error}"))
}

fn response_tree_error_value(
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
            type_label: runtime
                .registry()
                .type_name(meta.type_id)
                .map(|name| name.to_string())
                .unwrap_or_else(|| "UNKNOWN".to_string()),
            node_id: None,
            data_type: None,
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
            type_label: "group".to_string(),
            node_id: None,
            data_type: None,
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
                type_label: "BOOL".to_string(),
                node_id: None,
                data_type: None,
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
        type_label: "field_slave".to_string(),
        node_id: None,
        data_type: None,
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
            type_label: symbol.data_type.source_name.clone(),
            node_id: None,
            data_type: None,
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
}

fn opcua_node_to_symbol(node: crate::opcua::OpcUaBrowseNode) -> SymbolTreeNode {
    SymbolTreeNode {
        id: format!("opcua:node:{}", sanitize_id(node.id.as_str())),
        name: node.name,
        path: node.path,
        type_label: node.data_type_id,
        node_id: Some(node.id),
        data_type: Some(node.data_type),
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
mod tests;
