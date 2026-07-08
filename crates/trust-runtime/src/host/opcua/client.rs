#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpcUaClientEndpointInfo {
    pub endpoint_url: String,
    pub application_name: Option<String>,
    pub security_policy: OpcUaSecurityPolicy,
    pub security_mode: OpcUaMessageSecurityMode,
    pub anonymous_supported: bool,
    pub username_supported: bool,
}

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
    let trusted_root = opcua_client_pki_dir().join("trusted");
    let mut certs = Vec::new();
    collect_certificate_files(&trusted_root, &mut certs)?;
    certs.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(certs)
}

pub fn clear_trusted_opcua_client_server_certificates() -> Result<usize, RuntimeError> {
    let certs = list_trusted_opcua_client_server_certificates()?;
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

fn promote_rejected_opcua_client_server_certificates() -> Result<usize, RuntimeError> {
    let pki_dir = opcua_client_pki_dir();
    let rejected_root = pki_dir.join("rejected");
    let mut certs = Vec::new();
    collect_certificate_files(rejected_root.as_path(), &mut certs)?;
    if certs.is_empty() {
        return Ok(0);
    }
    let trusted_root = pki_dir.join("trusted");
    std::fs::create_dir_all(trusted_root.as_path()).map_err(|err| {
        RuntimeError::ControlError(
            format!(
                "failed to create OPC UA trusted certificate directory {}: {err}",
                trusted_root.display()
            )
            .into(),
        )
    })?;
    let mut promoted = 0usize;
    for cert in certs {
        let target = trusted_root.join(cert.file_name.as_str());
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
) -> Result<Vec<OpcUaClientEndpointInfo>, RuntimeError> {
    let client = build_opcua_client(
        "truST OPC UA discovery",
        "urn:trust:runtime:opcua:discover",
        false,
    )?;
    let endpoints = client
        .get_server_endpoints_from_url(endpoint_url)
        .map_err(opcua_status_error)?;
    Ok(endpoints
        .into_iter()
        .filter_map(endpoint_info_from_wire)
        .collect::<Vec<_>>())
}

#[cfg(not(feature = "opcua-wire"))]
pub fn discover_opcua_client_endpoints(
    _endpoint_url: &str,
) -> Result<Vec<OpcUaClientEndpointInfo>, RuntimeError> {
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
) -> Result<Vec<OpcUaBrowseNode>, RuntimeError> {
    let session = connect_opcua_client_session(
        endpoint_url,
        security,
        &auth,
        trust_server_certificate,
        "truST OPC UA browse",
        "urn:trust:runtime:opcua:browse",
    )?;
    let tree = {
        let session = session.read();
        browse_children(
            &session,
            ::opcua::types::NodeId::objects_folder_id(),
            "",
            0,
            max_depth.max(1),
            &mut 0,
            max_nodes.max(1),
        )?
    };
    session.read().disconnect();
    Ok(tree)
}

#[cfg(not(feature = "opcua-wire"))]
pub fn browse_opcua_client_nodes(
    _endpoint_url: &str,
    _security: OpcUaSecurityProfile,
    _auth: OpcUaClientAuthConfig,
    _trust_server_certificate: bool,
    _max_depth: usize,
    _max_nodes: usize,
) -> Result<Vec<OpcUaBrowseNode>, RuntimeError> {
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
fn endpoint_info_from_wire(
    endpoint: ::opcua::types::EndpointDescription,
) -> Option<OpcUaClientEndpointInfo> {
    let security_policy = security_policy_from_uri(endpoint.security_policy_uri.as_ref())?;
    let security_mode = security_mode_from_wire(endpoint.security_mode)?;
    let (anonymous_supported, username_supported) =
        endpoint
            .user_identity_tokens
            .as_ref()
            .map_or((false, false), |tokens| {
                let anonymous = tokens.iter().any(|token| {
                    matches!(token.token_type, ::opcua::types::UserTokenType::Anonymous)
                });
                let username = tokens.iter().any(|token| {
                    matches!(token.token_type, ::opcua::types::UserTokenType::UserName)
                });
                (anonymous, username)
            });
    Some(OpcUaClientEndpointInfo {
        endpoint_url: endpoint.endpoint_url.to_string(),
        application_name: localized_text(endpoint.server.application_name),
        security_policy,
        security_mode,
        anonymous_supported,
        username_supported,
    })
}

#[cfg(feature = "opcua-wire")]
fn browse_children(
    session: &::opcua::client::prelude::Session,
    node_id: ::opcua::types::NodeId,
    parent_path: &str,
    depth: usize,
    max_depth: usize,
    count: &mut usize,
    max_nodes: usize,
) -> Result<Vec<OpcUaBrowseNode>, RuntimeError> {
    if depth >= max_depth || *count >= max_nodes {
        return Ok(Vec::new());
    }
    let Some(results) = session
        .browse(&[::opcua::types::BrowseDescription {
            node_id,
            browse_direction: ::opcua::types::BrowseDirection::Forward,
            reference_type_id: ::opcua::types::ReferenceTypeId::HierarchicalReferences.into(),
            include_subtypes: true,
            node_class_mask: (::opcua::types::NodeClassMask::OBJECT
                | ::opcua::types::NodeClassMask::VARIABLE)
                .bits(),
            result_mask: ::opcua::types::BrowseDescriptionResultMask::all().bits(),
        }])
        .map_err(opcua_status_error)?
    else {
        return Ok(Vec::new());
    };
    let Some(result) = results.first() else {
        return Ok(Vec::new());
    };
    let references = result.references.clone().unwrap_or_default();
    let mut nodes = Vec::new();
    for reference in references {
        if *count >= max_nodes {
            break;
        }
        let node_id = reference.node_id.node_id.clone();
        if node_id.is_null() {
            continue;
        }
        let name = localized_text(reference.display_name)
            .unwrap_or_else(|| reference.browse_name.name.to_string());
        let path = if parent_path.is_empty() {
            name.clone()
        } else {
            format!("{parent_path}/{name}")
        };
        let is_variable = reference.node_class == ::opcua::types::NodeClass::Variable;
        let (data_type_id, data_type, writable) = if is_variable {
            variable_metadata(session, node_id.clone())
        } else {
            ("Object".to_string(), "object".to_string(), false)
        };
        *count += 1;
        let children = browse_children(
            session,
            node_id.clone(),
            path.as_str(),
            depth + 1,
            max_depth,
            count,
            max_nodes,
        )?;
        nodes.push(OpcUaBrowseNode {
            id: node_id.to_string(),
            name,
            path,
            data_type_id,
            data_type,
            writable,
            children,
        });
    }
    Ok(nodes)
}

#[cfg(feature = "opcua-wire")]
fn variable_metadata(
    session: &::opcua::client::prelude::Session,
    node_id: ::opcua::types::NodeId,
) -> (String, String, bool) {
    let values = session
        .read(
            &[
                ::opcua::types::ReadValueId {
                    node_id: node_id.clone(),
                    attribute_id: ::opcua::types::AttributeId::DataType as u32,
                    index_range: ::opcua::types::UAString::null(),
                    data_encoding: ::opcua::types::QualifiedName::null(),
                },
                ::opcua::types::ReadValueId {
                    node_id,
                    attribute_id: ::opcua::types::AttributeId::UserAccessLevel as u32,
                    index_range: ::opcua::types::UAString::null(),
                    data_encoding: ::opcua::types::QualifiedName::null(),
                },
            ],
            ::opcua::types::TimestampsToReturn::Neither,
            0.0,
        )
        .unwrap_or_default();
    let data_type_id = values
        .first()
        .and_then(|value| value.value.as_ref())
        .map(|value| value.to_string())
        .unwrap_or_else(|| "Value".to_string());
    let data_type = opcua_data_type_for_apply(data_type_id.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| "unknown".to_string());
    let writable = values
        .get(1)
        .and_then(|value| value.value.as_ref())
        .and_then(|value| match value {
            ::opcua::types::Variant::Byte(bits) => Some(bits & 0b10 != 0),
            ::opcua::types::Variant::UInt16(bits) => Some(bits & 0b10 != 0),
            ::opcua::types::Variant::UInt32(bits) => Some(bits & 0b10 != 0),
            _ => None,
        })
        .unwrap_or(false);
    (data_type_id, data_type, writable)
}

#[cfg(feature = "opcua-wire")]
fn opcua_data_type_for_apply(data_type_id: &str) -> Option<&'static str> {
    match data_type_id {
        "i=1" | "ns=0;i=1" => Some("bool"),
        "i=2" | "ns=0;i=2" | "i=4" | "ns=0;i=4" => Some("int16"),
        "i=3" | "ns=0;i=3" | "i=5" | "ns=0;i=5" => Some("uint16"),
        "i=6" | "ns=0;i=6" => Some("int32"),
        "i=7" | "ns=0;i=7" => Some("uint32"),
        "i=8" | "ns=0;i=8" => Some("int64"),
        "i=9" | "ns=0;i=9" => Some("uint64"),
        "i=10" | "ns=0;i=10" => Some("float"),
        "i=11" | "ns=0;i=11" => Some("double"),
        "i=12" | "ns=0;i=12" => Some("string"),
        _ => None,
    }
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
fn security_policy_from_uri(uri: &str) -> Option<OpcUaSecurityPolicy> {
    if uri.ends_with("#None") {
        Some(OpcUaSecurityPolicy::None)
    } else if uri.ends_with("#Basic256Sha256") {
        Some(OpcUaSecurityPolicy::Basic256Sha256)
    } else if uri.ends_with("#Aes128_Sha256_RsaOaep") {
        Some(OpcUaSecurityPolicy::Aes128Sha256RsaOaep)
    } else {
        None
    }
}

#[cfg(feature = "opcua-wire")]
fn security_mode_from_wire(
    mode: ::opcua::types::MessageSecurityMode,
) -> Option<OpcUaMessageSecurityMode> {
    match mode {
        ::opcua::types::MessageSecurityMode::None => Some(OpcUaMessageSecurityMode::None),
        ::opcua::types::MessageSecurityMode::Sign => Some(OpcUaMessageSecurityMode::Sign),
        ::opcua::types::MessageSecurityMode::SignAndEncrypt => {
            Some(OpcUaMessageSecurityMode::SignAndEncrypt)
        }
        _ => None,
    }
}

#[cfg(feature = "opcua-wire")]
fn parse_node_id(text: &str) -> Result<::opcua::types::NodeId, RuntimeError> {
    text.parse::<::opcua::types::NodeId>()
        .map_err(|_| RuntimeError::ControlError(format!("invalid OPC UA node id '{text}'").into()))
}

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
        (OpcUaDataType::Float, OpcUaVariant::Float(value)) => Ok(Value::Real(value)),
        (OpcUaDataType::Double, OpcUaVariant::Double(value)) => Ok(Value::LReal(value)),
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
    if !root.exists() {
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
        let path = entry.path();
        if path.is_dir() {
            collect_certificate_files(path.as_path(), certs)?;
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
