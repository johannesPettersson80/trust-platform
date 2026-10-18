#[cfg(feature = "opcua-wire")]
mod client_browse;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpcUaBrowseNode {
    pub id: String,
    pub name: String,
    pub path: String,
    pub data_type_id: String,
    pub data_type: String,
    pub writable: bool,
    pub children: Vec<OpcUaBrowseNode>,
}

/// The browsed OPC UA tree and any depth or node bound that cut it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpcUaBrowseResult {
    /// The tree under the OPC UA Objects folder.
    pub nodes: Vec<OpcUaBrowseNode>,
    /// Warnings naming a bound that limited the tree; empty when the whole tree was read.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpcUaClientErrorCode {
    CertUntrusted,
    AuthRequired,
    EndpointUnreachable,
    BrowseDenied,
    UnsupportedSecurityProfile,
}

impl OpcUaClientErrorCode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CertUntrusted => "cert_untrusted",
            Self::AuthRequired => "auth_required",
            Self::EndpointUnreachable => "endpoint_unreachable",
            Self::BrowseDenied => "browse_denied",
            Self::UnsupportedSecurityProfile => "unsupported_security_profile",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpcUaClientConnectionState {
    Disabled,
    Configured,
    Connecting,
    Connected,
    Reconnecting,
    Stale,
    Faulted,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpcUaClientPointStatus {
    pub var: SmolStr,
    pub node_id: String,
    pub data_type: OpcUaDataType,
    pub access: OpcUaClientPointAccess,
    pub state: OpcUaClientConnectionState,
    pub last_seen_ms: Option<u64>,
    pub value: Option<Value>,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpcUaClientConnectionStatus {
    pub name: SmolStr,
    pub endpoint_url: String,
    pub state: OpcUaClientConnectionState,
    pub point_count: usize,
    pub degraded_points: usize,
    pub last_seen_ms: Option<u64>,
    pub detail: String,
    pub points: Vec<OpcUaClientPointStatus>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpcUaClientStatusReport {
    pub enabled: bool,
    pub deployed_config_hash: Option<String>,
    pub connections: Vec<OpcUaClientConnectionStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpcUaTrustedCertificate {
    pub path: std::path::PathBuf,
    pub file_name: String,
}

#[must_use]
pub fn opcua_client_pki_dir() -> std::path::PathBuf {
    std::env::var_os("TRUST_RUNTIME_OPCUA_CLIENT_PKI_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("trust-runtime-opcua-client"))
}

pub fn list_trusted_opcua_client_server_certificates(
) -> Result<Vec<OpcUaTrustedCertificate>, RuntimeError> {
    list_trusted_certificates_in(&opcua_client_pki_dir())
}

fn list_trusted_certificates_in(
    pki_dir: &Path,
) -> Result<Vec<OpcUaTrustedCertificate>, RuntimeError> {
    if !certificate_directory_is_safe(pki_dir)? {
        return Ok(Vec::new());
    }
    let trusted_root = pki_dir.join("trusted");
    let mut certs = Vec::new();
    collect_certificate_files(&trusted_root, &mut certs)?;
    certs.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(certs)
}

pub fn clear_trusted_opcua_client_server_certificates() -> Result<usize, RuntimeError> {
    clear_trusted_certificates_in(&opcua_client_pki_dir())
}

fn clear_trusted_certificates_in(pki_dir: &Path) -> Result<usize, RuntimeError> {
    let certs = list_trusted_certificates_in(pki_dir)?;
    let mut cleared = 0usize;
    for cert in certs {
        if cert.path.is_file() {
            std::fs::remove_file(&cert.path).map_err(|err| {
                RuntimeError::ControlError(
                    format!(
                        "failed to remove trusted OPC UA certificate {}: {err}",
                        cert.path.display()
                    )
                    .into(),
                )
            })?;
            cleared += 1;
        }
    }
    Ok(cleared)
}

#[cfg(feature = "opcua-wire")]
fn promote_rejected_opcua_client_server_certificates() -> Result<usize, RuntimeError> {
    promote_rejected_certificates_in(&opcua_client_pki_dir())
}

#[cfg(any(feature = "opcua-wire", test))]
fn promote_rejected_certificates_in(pki_dir: &Path) -> Result<usize, RuntimeError> {
    if !certificate_directory_is_safe(pki_dir)? {
        return Ok(0);
    }
    let rejected_root = pki_dir.join("rejected");
    let mut certs = Vec::new();
    collect_certificate_files(rejected_root.as_path(), &mut certs)?;
    if certs.is_empty() {
        return Ok(0);
    }
    let trusted_root = pki_dir.join("trusted");
    ensure_certificate_directory(trusted_root.as_path())?;
    let mut promoted = 0usize;
    for cert in certs {
        let target = trusted_root.join(cert.file_name.as_str());
        if std::fs::symlink_metadata(target.as_path())
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err(RuntimeError::ControlError(
                format!(
                    "refusing to promote OPC UA certificate through symbolic-link target {}",
                    target.display()
                )
                .into(),
            ));
        }
        std::fs::copy(cert.path.as_path(), target.as_path()).map_err(|err| {
            RuntimeError::ControlError(
                format!(
                    "failed to trust OPC UA certificate {}: {err}",
                    cert.path.display()
                )
                .into(),
            )
        })?;
        std::fs::remove_file(cert.path.as_path()).map_err(|err| {
            RuntimeError::ControlError(
                format!(
                    "failed to remove rejected OPC UA certificate {} after trust: {err}",
                    cert.path.display()
                )
                .into(),
            )
        })?;
        promoted += 1;
    }
    Ok(promoted)
}

#[must_use]
pub fn classify_opcua_client_error(error: &RuntimeError) -> OpcUaClientErrorCode {
    classify_opcua_client_error_message(error.to_string().as_str())
}

#[must_use]
pub fn classify_opcua_client_browse_error(error: &RuntimeError) -> OpcUaClientErrorCode {
    let message = error.to_string();
    if message.to_ascii_lowercase().contains("baduseraccessdenied")
        || message.to_ascii_lowercase().contains("badnotreadable")
        || message.to_ascii_lowercase().contains("badnodeidunknown")
    {
        return OpcUaClientErrorCode::BrowseDenied;
    }
    classify_opcua_client_error_message(message.as_str())
}

#[cfg(feature = "opcua-wire")]
pub fn discover_opcua_client_endpoints(
    endpoint_url: &str,
    timeout: std::time::Duration,
) -> Result<Vec<OpcUaClientEndpointInfo>, RuntimeError> {
    // The GetEndpoints call runs on its own thread, so a server that never answers cannot hold a
    // search past its deadline; on timeout the thread is left to finish or die on its own.
    let url = endpoint_url.to_string();
    let (sender, receiver) = std::sync::mpsc::channel();
    let handle = std::thread::Builder::new()
        .name("opcua-discover".to_string())
        .spawn(move || {
            let _ = sender.send(discover_opcua_client_endpoints_blocking(url.as_str()));
        })
        .map_err(|err| {
            RuntimeError::ControlError(format!("start OPC UA discovery: {err}").into())
        })?;
    match receiver.recv_timeout(timeout) {
        Ok(result) => {
            let _ = handle.join();
            result
        }
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(RuntimeError::ControlError(
            format!(
                "OPC UA discovery of {endpoint_url} did not answer within {} ms",
                timeout.as_millis()
            )
            .into(),
        )),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(RuntimeError::ControlError(
            "OPC UA discovery ended without an answer".into(),
        )),
    }
}

#[cfg(feature = "opcua-wire")]
fn discover_opcua_client_endpoints_blocking(
    endpoint_url: &str,
) -> Result<Vec<OpcUaClientEndpointInfo>, RuntimeError> {
    let client = build_opcua_client(
        "truST OPC UA discovery",
        "urn:trust:runtime:opcua:discover",
        false,
    )?;
    let endpoints = client
        .get_server_endpoints_from_url(endpoint_url)
        .map_err(opcua_status_error)?;
    Ok(endpoints.into_iter().map(endpoint_info_from_wire).collect())
}

#[cfg(not(feature = "opcua-wire"))]
pub fn discover_opcua_client_endpoints(
    _endpoint_url: &str,
    _timeout: std::time::Duration,
) -> Result<Vec<OpcUaClientEndpointInfo>, RuntimeError> {
    Err(opcua_wire_feature_error())
}

/// One server that a discovery server listed in its `FindServers` answer (specification 11,
/// "Sources"): what it calls itself and the discovery URLs through which it can be asked for its
/// endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpcUaFoundServer {
    /// The server's application name, when it announces one.
    pub(crate) application_name: Option<String>,
    /// The discovery URLs it announces, in order.
    pub(crate) discovery_urls: Vec<String>,
}

/// Asks the discovery server at `discovery_url` for the servers it lists, through `FindServers`.
#[cfg(feature = "opcua-wire")]
pub(crate) fn discover_opcua_discovery_servers(
    discovery_url: &str,
    timeout: std::time::Duration,
) -> Result<Vec<OpcUaFoundServer>, RuntimeError> {
    // The FindServers call runs on its own thread, so a discovery server that never answers cannot
    // hold a search past its deadline; on timeout the thread is left to finish or die on its own.
    let url = discovery_url.to_string();
    let (sender, receiver) = std::sync::mpsc::channel();
    let handle = std::thread::Builder::new()
        .name("opcua-find-servers".to_string())
        .spawn(move || {
            let _ = sender.send(find_servers_blocking(url.as_str()));
        })
        .map_err(|err| {
            RuntimeError::ControlError(format!("start OPC UA FindServers: {err}").into())
        })?;
    match receiver.recv_timeout(timeout) {
        Ok(result) => {
            let _ = handle.join();
            result
        }
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(RuntimeError::ControlError(
            format!(
                "OPC UA FindServers at {discovery_url} did not answer within {} ms",
                timeout.as_millis()
            )
            .into(),
        )),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(RuntimeError::ControlError(
            "OPC UA FindServers ended without an answer".into(),
        )),
    }
}

#[cfg(feature = "opcua-wire")]
fn find_servers_blocking(discovery_url: &str) -> Result<Vec<OpcUaFoundServer>, RuntimeError> {
    let mut client = build_opcua_client(
        "truST OPC UA discovery",
        "urn:trust:runtime:opcua:discover",
        false,
    )?;
    let servers = client
        .find_servers(discovery_url)
        .map_err(opcua_status_error)?;
    Ok(servers.into_iter().map(found_server_from_wire).collect())
}

/// One server of a `FindServers` answer, as truST keeps it.
#[cfg(feature = "opcua-wire")]
fn found_server_from_wire(server: ::opcua::types::ApplicationDescription) -> OpcUaFoundServer {
    OpcUaFoundServer {
        application_name: localized_text(server.application_name),
        discovery_urls: server
            .discovery_urls
            .unwrap_or_default()
            .into_iter()
            .map(|url| url.as_ref().trim().to_string())
            .filter(|url| !url.is_empty())
            .collect(),
    }
}

#[cfg(not(feature = "opcua-wire"))]
pub(crate) fn discover_opcua_discovery_servers(
    _discovery_url: &str,
    _timeout: std::time::Duration,
) -> Result<Vec<OpcUaFoundServer>, RuntimeError> {
    Err(opcua_wire_feature_error())
}

#[cfg(feature = "opcua-wire")]
pub fn test_opcua_client_endpoint(
    endpoint_url: &str,
    security: OpcUaSecurityProfile,
    auth: OpcUaClientAuthConfig,
    trust_server_certificate: bool,
) -> Result<(), RuntimeError> {
    let session = connect_opcua_client_session(
        endpoint_url,
        security,
        &auth,
        trust_server_certificate,
        "truST OPC UA test",
        "urn:trust:runtime:opcua:test",
    )?;
    session.read().disconnect();
    Ok(())
}

#[cfg(not(feature = "opcua-wire"))]
pub fn test_opcua_client_endpoint(
    _endpoint_url: &str,
    _security: OpcUaSecurityProfile,
    _auth: OpcUaClientAuthConfig,
    _trust_server_certificate: bool,
) -> Result<(), RuntimeError> {
    Err(opcua_wire_feature_error())
}

#[cfg(feature = "opcua-wire")]
pub fn browse_opcua_client_nodes(
    endpoint_url: &str,
    security: OpcUaSecurityProfile,
    auth: OpcUaClientAuthConfig,
    trust_server_certificate: bool,
    max_depth: usize,
    max_nodes: usize,
) -> Result<OpcUaBrowseResult, RuntimeError> {
    let session = connect_opcua_client_session(
        endpoint_url,
        security,
        &auth,
        trust_server_certificate,
        "truST OPC UA browse",
        "urn:trust:runtime:opcua:browse",
    )?;
    let result = {
        let session = session.read();
        let source = client_browse::SessionBrowseSource { session: &session };
        client_browse::browse_tree(&source, max_depth.max(1), max_nodes.max(1))?
    };
    session.read().disconnect();
    Ok(result)
}

#[cfg(not(feature = "opcua-wire"))]
pub fn browse_opcua_client_nodes(
    _endpoint_url: &str,
    _security: OpcUaSecurityProfile,
    _auth: OpcUaClientAuthConfig,
    _trust_server_certificate: bool,
    _max_depth: usize,
    _max_nodes: usize,
) -> Result<OpcUaBrowseResult, RuntimeError> {
    Err(opcua_wire_feature_error())
}

#[cfg(feature = "opcua-wire")]
pub fn read_opcua_client_point_values(
    connection: &OpcUaClientConnectionConfig,
    points: &[OpcUaClientPointConfig],
) -> Result<Vec<(SmolStr, Value)>, RuntimeError> {
    let session = connect_opcua_client_session(
        connection.endpoint_url.as_str(),
        connection.security,
        &connection.auth,
        connection.trust_server_certificate,
        "truST OPC UA client",
        "urn:trust:runtime:opcua:client",
    )?;
    let node_ids = points
        .iter()
        .map(|point| parse_node_id(point.node_id.as_str()))
        .collect::<Result<Vec<_>, _>>()?;
    let values = {
        let session = session.read();
        let read_values = node_ids
            .iter()
            .cloned()
            .map(::opcua::types::ReadValueId::from)
            .collect::<Vec<_>>();
        session
            .read(
                read_values.as_slice(),
                ::opcua::types::TimestampsToReturn::Both,
                0.0,
            )
            .map_err(opcua_status_error)?
    };
    session.read().disconnect();
    validate_opcua_result_cardinality("read", points.len(), values.len())?;

    let mut mapped = Vec::with_capacity(points.len());
    for (point, data_value) in points.iter().zip(values) {
        let status = data_value
            .status
            .unwrap_or(::opcua::types::StatusCode::Good);
        if !status.is_good() {
            return Err(RuntimeError::ControlError(
                format!("OPC UA node '{}' read returned {status}", point.node_id).into(),
            ));
        }
        let variant = data_value.value.ok_or_else(|| {
            RuntimeError::ControlError(
                format!("OPC UA node '{}' read returned no value", point.node_id).into(),
            )
        })?;
        let variant = from_wire_variant(&variant).ok_or_else(|| {
            RuntimeError::ControlError(
                format!(
                    "OPC UA node '{}' returned unsupported value {variant:?}",
                    point.node_id
                )
                .into(),
            )
        })?;
        mapped.push((
            point.var.clone(),
            value_from_opcua_variant(point.data_type, variant)?,
        ));
    }
    Ok(mapped)
}

#[cfg(not(feature = "opcua-wire"))]
pub fn read_opcua_client_point_values(
    _connection: &OpcUaClientConnectionConfig,
    _points: &[OpcUaClientPointConfig],
) -> Result<Vec<(SmolStr, Value)>, RuntimeError> {
    Err(opcua_wire_feature_error())
}

#[cfg(feature = "opcua-wire")]
pub fn write_opcua_client_point_values(
    connection: &OpcUaClientConnectionConfig,
    values: &[(OpcUaClientPointConfig, Value)],
) -> Result<(), RuntimeError> {
    if values.is_empty() {
        return Ok(());
    }
    let session = connect_opcua_client_session(
        connection.endpoint_url.as_str(),
        connection.security,
        &connection.auth,
        connection.trust_server_certificate,
        "truST OPC UA client",
        "urn:trust:runtime:opcua:client",
    )?;
    let write_values = values
        .iter()
        .map(|(point, value)| {
            let mapped = map_iec_value(value).ok_or_else(|| {
                RuntimeError::ControlError(
                    format!(
                        "OPC UA point '{}' has unsupported value {value:?}",
                        point.var
                    )
                    .into(),
                )
            })?;
            if mapped.data_type != point.data_type {
                return Err(RuntimeError::ControlError(
                    format!(
                        "OPC UA point '{}' expected {}, got {}",
                        point.var,
                        point.data_type.as_config_value(),
                        mapped.data_type.as_config_value()
                    )
                    .into(),
                ));
            }
            Ok(::opcua::types::WriteValue {
                node_id: parse_node_id(point.node_id.as_str())?,
                attribute_id: ::opcua::types::AttributeId::Value as u32,
                index_range: ::opcua::types::UAString::null(),
                value: ::opcua::types::DataValue {
                    value: Some(to_wire_variant(&mapped.value)),
                    status: Some(::opcua::types::StatusCode::Good),
                    source_timestamp: Some(::opcua::types::DateTime::now()),
                    ..Default::default()
                },
            })
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    let statuses = {
        let session = session.read();
        session
            .write(write_values.as_slice())
            .map_err(opcua_status_error)?
    };
    session.read().disconnect();
    validate_opcua_result_cardinality("write", values.len(), statuses.len())?;
    for (point, status) in values.iter().map(|(point, _)| point).zip(statuses) {
        if !status.is_good() {
            return Err(RuntimeError::ControlError(
                format!("OPC UA node '{}' write returned {status}", point.node_id).into(),
            ));
        }
    }
    Ok(())
}

#[cfg(not(feature = "opcua-wire"))]
pub fn write_opcua_client_point_values(
    _connection: &OpcUaClientConnectionConfig,
    _values: &[(OpcUaClientPointConfig, Value)],
) -> Result<(), RuntimeError> {
    Err(opcua_wire_feature_error())
}

#[cfg(any(feature = "opcua-wire", test))]
fn validate_opcua_result_cardinality(
    operation: &str,
    expected: usize,
    actual: usize,
) -> Result<(), RuntimeError> {
    if actual == expected {
        return Ok(());
    }
    Err(RuntimeError::ControlError(
        format!("OPC UA {operation} returned {actual} result(s) for {expected} request(s)").into(),
    ))
}

#[cfg(feature = "opcua-wire")]
fn build_opcua_client(
    application_name: &str,
    application_uri: &str,
    trust_server_certificate: bool,
) -> Result<::opcua::client::prelude::Client, RuntimeError> {
    let pki_dir = opcua_client_pki_dir();
    std::fs::create_dir_all(&pki_dir).map_err(|err| {
        RuntimeError::ControlError(format!("create OPC UA client PKI: {err}").into())
    })?;
    ::opcua::client::prelude::ClientBuilder::new()
        .application_name(application_name)
        .application_uri(application_uri)
        .product_uri("urn:trust:runtime")
        .pki_dir(pki_dir)
        .create_sample_keypair(true)
        .trust_server_certs(trust_server_certificate)
        .verify_server_certs(!trust_server_certificate)
        .session_retry_limit(1)
        .client()
        .ok_or_else(|| RuntimeError::ControlError("failed to build OPC UA client".into()))
}

#[cfg(feature = "opcua-wire")]
fn connect_opcua_client_session(
    endpoint_url: &str,
    security: OpcUaSecurityProfile,
    auth: &OpcUaClientAuthConfig,
    trust_server_certificate: bool,
    application_name: &str,
    application_uri: &str,
) -> Result<Arc<::opcua::sync::RwLock<::opcua::client::prelude::Session>>, RuntimeError> {
    if trust_server_certificate {
        // The opcua crate treats an already-rejected certificate as stronger than the
        // auto-trust flag. The canvas "Trust certificate" action is explicit user consent,
        // so promote rejected certificates before retrying the secure connection.
        promote_rejected_opcua_client_server_certificates()?;
    }
    let mut client =
        build_opcua_client(application_name, application_uri, trust_server_certificate)?;
    let endpoints = client
        .get_server_endpoints_from_url(endpoint_url)
        .map_err(opcua_status_error)?;
    let endpoint = ::opcua::client::prelude::Client::find_matching_endpoint(
        endpoints.as_slice(),
        endpoint_url,
        to_wire_security_policy(security.policy),
        to_wire_security_mode(security.mode),
    )
    .ok_or_else(|| {
        RuntimeError::ControlError(
            format!(
                "no matching OPC UA endpoint for {} / {}",
                security.policy.as_config_value(),
                security.mode.as_config_value()
            )
            .into(),
        )
    })?;
    let token = match auth {
        OpcUaClientAuthConfig::Anonymous => ::opcua::client::prelude::IdentityToken::Anonymous,
        OpcUaClientAuthConfig::UserName { username, password } => {
            ::opcua::client::prelude::IdentityToken::UserName(
                username.to_string(),
                password.to_string(),
            )
        }
    };
    client
        .connect_to_endpoint(endpoint, token)
        .map_err(opcua_status_error)
}

#[cfg(feature = "opcua-wire")]
fn localized_text(value: ::opcua::types::LocalizedText) -> Option<String> {
    let text = value.to_string();
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

#[cfg(feature = "opcua-wire")]
fn parse_node_id(text: &str) -> Result<::opcua::types::NodeId, RuntimeError> {
    text.parse::<::opcua::types::NodeId>()
        .map_err(|_| RuntimeError::ControlError(format!("invalid OPC UA node id '{text}'").into()))
}

#[cfg(feature = "opcua-wire")]
fn value_from_opcua_variant(
    data_type: OpcUaDataType,
    variant: OpcUaVariant,
) -> Result<Value, RuntimeError> {
    match (data_type, variant) {
        (OpcUaDataType::Boolean, OpcUaVariant::Boolean(value)) => Ok(Value::Bool(value)),
        (OpcUaDataType::Int16, OpcUaVariant::Int16(value)) => Ok(Value::Int(value)),
        (OpcUaDataType::Int32, OpcUaVariant::Int32(value)) => Ok(Value::DInt(value)),
        (OpcUaDataType::Int64, OpcUaVariant::Int64(value)) => Ok(Value::LInt(value)),
        (OpcUaDataType::UInt16, OpcUaVariant::UInt16(value)) => Ok(Value::UInt(value)),
        (OpcUaDataType::UInt32, OpcUaVariant::UInt32(value)) => Ok(Value::UDInt(value)),
        (OpcUaDataType::UInt64, OpcUaVariant::UInt64(value)) => Ok(Value::ULInt(value)),
        (OpcUaDataType::Float, OpcUaVariant::Float(value)) if value.is_finite() => {
            Ok(Value::Real(value))
        }
        (OpcUaDataType::Double, OpcUaVariant::Double(value)) if value.is_finite() => {
            Ok(Value::LReal(value))
        }
        (OpcUaDataType::Float, actual @ OpcUaVariant::Float(_))
        | (OpcUaDataType::Double, actual @ OpcUaVariant::Double(_)) => {
            Err(RuntimeError::ControlError(
                format!("OPC UA input contains a non-finite floating-point value: {actual:?}")
                    .into(),
            ))
        }
        (OpcUaDataType::String, OpcUaVariant::String(value)) => Ok(Value::String(value.into())),
        (expected, actual) => Err(RuntimeError::ControlError(
            format!(
                "OPC UA value type mismatch: expected {}, got {actual:?}",
                expected.as_config_value()
            )
            .into(),
        )),
    }
}

fn collect_certificate_files(
    root: &std::path::Path,
    certs: &mut Vec<OpcUaTrustedCertificate>,
) -> Result<(), RuntimeError> {
    if !certificate_directory_is_safe(root)? {
        return Ok(());
    }
    let entries = std::fs::read_dir(root).map_err(|err| {
        RuntimeError::ControlError(
            format!(
                "failed to read OPC UA trusted certificate directory {}: {err}",
                root.display()
            )
            .into(),
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|err| {
            RuntimeError::ControlError(
                format!("failed to read OPC UA trusted certificate entry: {err}").into(),
            )
        })?;
        let file_type = entry.file_type().map_err(|err| {
            RuntimeError::ControlError(
                format!(
                    "failed to inspect OPC UA certificate entry {}: {err}",
                    entry.path().display()
                )
                .into(),
            )
        })?;
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            collect_certificate_files(path.as_path(), certs)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
            continue;
        };
        if !matches!(
            extension.to_ascii_lowercase().as_str(),
            "der" | "pem" | "crt"
        ) {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string();
        certs.push(OpcUaTrustedCertificate { path, file_name });
    }
    Ok(())
}

fn certificate_directory_is_safe(root: &std::path::Path) -> Result<bool, RuntimeError> {
    match std::fs::symlink_metadata(root) {
        Ok(metadata) => Ok(metadata.is_dir() && !metadata.file_type().is_symlink()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(RuntimeError::ControlError(
            format!(
                "failed to inspect OPC UA certificate directory {}: {err}",
                root.display()
            )
            .into(),
        )),
    }
}

#[cfg(any(feature = "opcua-wire", test))]
fn ensure_certificate_directory(root: &std::path::Path) -> Result<(), RuntimeError> {
    match std::fs::symlink_metadata(root) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(RuntimeError::ControlError(
            format!(
                "OPC UA certificate directory is not a regular directory: {}",
                root.display()
            )
            .into(),
        )),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => std::fs::create_dir(root)
            .map_err(|err| {
                RuntimeError::ControlError(
                    format!(
                        "failed to create OPC UA trusted certificate directory {}: {err}",
                        root.display()
                    )
                    .into(),
                )
            }),
        Err(err) => Err(RuntimeError::ControlError(
            format!(
                "failed to inspect OPC UA certificate directory {}: {err}",
                root.display()
            )
            .into(),
        )),
    }
}

fn classify_opcua_client_error_message(message: &str) -> OpcUaClientErrorCode {
    let lower = message.to_ascii_lowercase();
    if lower.contains("badcertificateuntrusted")
        || lower.contains("badcertificaterevoked")
        || lower.contains("badcertificatetimeinvalid")
        || lower.contains("badcertificate")
        || lower.contains("certificate") && (lower.contains("untrusted") || lower.contains("trust"))
    {
        return OpcUaClientErrorCode::CertUntrusted;
    }
    if lower.contains("badidentitytoken")
        || lower.contains("baduseraccessdenied")
        || lower.contains("badusersignatureinvalid")
        || lower.contains("badsecuritychecksfailed")
        || lower.contains("badsecuritypolicyrejected")
        || lower.contains("username")
        || lower.contains("password")
        || lower.contains("auth")
    {
        return OpcUaClientErrorCode::AuthRequired;
    }
    if lower.contains("no matching opc ua endpoint")
        || lower.contains("unsupported security")
        || lower.contains("unsupported opc ua client security")
    {
        return OpcUaClientErrorCode::UnsupportedSecurityProfile;
    }
    OpcUaClientErrorCode::EndpointUnreachable
}
