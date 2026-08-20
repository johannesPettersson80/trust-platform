#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpcUaSecurityPolicy {
    None,
    Basic256Sha256,
    Aes128Sha256RsaOaep,
}

impl OpcUaSecurityPolicy {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let normalized = text.trim().to_ascii_lowercase().replace(['-', '_'], "");
        match normalized.as_str() {
            "none" => Some(Self::None),
            "basic256sha256" => Some(Self::Basic256Sha256),
            "aes128sha256rsaoaep" => Some(Self::Aes128Sha256RsaOaep),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_config_value(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Basic256Sha256 => "basic256sha256",
            Self::Aes128Sha256RsaOaep => "aes128sha256rsaoaep",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpcUaMessageSecurityMode {
    None,
    Sign,
    SignAndEncrypt,
}

impl OpcUaMessageSecurityMode {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let normalized = text.trim().to_ascii_lowercase().replace(['-', '_'], "");
        match normalized.as_str() {
            "none" => Some(Self::None),
            "sign" => Some(Self::Sign),
            "signandencrypt" => Some(Self::SignAndEncrypt),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_config_value(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Sign => "sign",
            Self::SignAndEncrypt => "sign_and_encrypt",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpcUaSecurityProfile {
    pub policy: OpcUaSecurityPolicy,
    pub mode: OpcUaMessageSecurityMode,
    pub allow_anonymous: bool,
}

impl Default for OpcUaSecurityProfile {
    fn default() -> Self {
        Self {
            policy: OpcUaSecurityPolicy::Basic256Sha256,
            mode: OpcUaMessageSecurityMode::SignAndEncrypt,
            allow_anonymous: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OpcUaRuntimeConfig {
    pub enabled: bool,
    pub listen: SmolStr,
    pub endpoint_path: SmolStr,
    pub namespace_uri: SmolStr,
    pub publish_interval_ms: u64,
    pub max_nodes: usize,
    pub expose: Vec<SmolStr>,
    pub security: OpcUaSecurityProfile,
    pub username: Option<SmolStr>,
    pub password: Option<SmolStr>,
}

impl Default for OpcUaRuntimeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            listen: SmolStr::new("0.0.0.0:4840"),
            endpoint_path: SmolStr::new("/"),
            namespace_uri: SmolStr::new("urn:trust:runtime"),
            publish_interval_ms: 250,
            max_nodes: 128,
            expose: Vec::new(),
            security: OpcUaSecurityProfile::default(),
            username: None,
            password: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpcUaClientConfig {
    pub connections: Vec<OpcUaClientConnectionConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpcUaClientConnectionConfig {
    pub name: SmolStr,
    pub endpoint_url: String,
    pub security: OpcUaSecurityProfile,
    pub auth: OpcUaClientAuthConfig,
    pub trust_server_certificate: bool,
    pub poll_interval_ms: u64,
    pub timeout_ms: u64,
    pub points: Vec<OpcUaClientPointConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpcUaClientAuthConfig {
    Anonymous,
    UserName { username: SmolStr, password: SmolStr },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpcUaClientPointAccess {
    Read,
    Write,
    ReadWrite,
}

impl OpcUaClientPointAccess {
    #[must_use]
    pub fn can_read(self) -> bool {
        matches!(self, Self::Read | Self::ReadWrite)
    }

    #[must_use]
    pub fn can_write(self) -> bool {
        matches!(self, Self::Write | Self::ReadWrite)
    }

    #[must_use]
    pub fn as_config_value(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::ReadWrite => "read_write",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpcUaClientPointConfig {
    pub var: SmolStr,
    pub node_id: String,
    pub data_type: OpcUaDataType,
    pub access: OpcUaClientPointAccess,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct OpcUaClientToml {
    connections: Vec<OpcUaClientConnectionSection>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct OpcUaClientConnectionSection {
    name: String,
    endpoint_url: String,
    security_policy: Option<String>,
    security_mode: Option<String>,
    auth: Option<String>,
    username: Option<String>,
    password: Option<String>,
    trust_server_certificate: Option<bool>,
    poll_interval_ms: Option<u64>,
    timeout_ms: Option<u64>,
    points: Vec<OpcUaClientPointSection>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct OpcUaClientPointSection {
    var: String,
    node_id: String,
    #[serde(rename = "type")]
    type_name: String,
    access: Option<String>,
    writable: Option<bool>,
}

pub fn parse_opcua_client_toml(text: &str) -> Result<OpcUaClientConfig, RuntimeError> {
    let raw: OpcUaClientToml = toml::from_str(text).map_err(|err| {
        RuntimeError::InvalidConfig(format!("opcua_client.toml: {err}").into())
    })?;
    raw.into_config()
}

impl OpcUaClientToml {
    fn into_config(self) -> Result<OpcUaClientConfig, RuntimeError> {
        if self.connections.is_empty() {
            return invalid_opcua_client("opcua_client.toml requires at least one [[connections]] entry");
        }
        let mut connection_names = std::collections::BTreeSet::new();
        let mut point_names = std::collections::BTreeSet::new();
        let connections = self
            .connections
            .into_iter()
            .map(|connection| connection.into_config(&mut connection_names, &mut point_names))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(OpcUaClientConfig { connections })
    }
}

impl OpcUaClientConnectionSection {
    fn into_config(
        self,
        connection_names: &mut std::collections::BTreeSet<String>,
        point_names: &mut std::collections::BTreeSet<String>,
    ) -> Result<OpcUaClientConnectionConfig, RuntimeError> {
        let name = non_empty_opcua_client("connections.name", self.name)?;
        if !connection_names.insert(name.clone()) {
            return invalid_opcua_client(format!(
                "OPC UA connection name '{name}' is declared more than once"
            ));
        }
        let endpoint_url = non_empty_opcua_client("connections.endpoint_url", self.endpoint_url)?;
        if !valid_opc_tcp_endpoint(endpoint_url.as_str()) {
            return invalid_opcua_client(format!(
                "OPC UA connection '{name}' endpoint_url must use opc.tcp:// with a non-empty authority"
            ));
        }
        if self.points.is_empty() {
            return invalid_opcua_client(format!(
                "OPC UA connection '{name}' requires at least one point"
            ));
        }
        let policy_raw = self.security_policy.as_deref().unwrap_or("none");
        let mode_raw = self.security_mode.as_deref().unwrap_or("none");
        let policy = OpcUaSecurityPolicy::parse(policy_raw).ok_or_else(|| {
            RuntimeError::InvalidConfig(
                format!("invalid OPC UA client security_policy '{policy_raw}'").into(),
            )
        })?;
        let mode = OpcUaMessageSecurityMode::parse(mode_raw).ok_or_else(|| {
            RuntimeError::InvalidConfig(
                format!("invalid OPC UA client security_mode '{mode_raw}'").into(),
            )
        })?;
        validate_client_security_profile(policy, mode)?;
        let auth = parse_client_auth(self.auth.as_deref(), self.username, self.password, &name)?;
        let poll_interval_ms = self.poll_interval_ms.unwrap_or(250);
        if poll_interval_ms < 10 {
            return invalid_opcua_client(format!(
                "OPC UA connection '{name}' poll_interval_ms must be >= 10"
            ));
        }
        let timeout_ms = self.timeout_ms.unwrap_or(2_000);
        if timeout_ms == 0 || timeout_ms > 60_000 {
            return invalid_opcua_client(format!(
                "OPC UA connection '{name}' timeout_ms must be between 1 and 60000"
            ));
        }
        let points = self
            .points
            .into_iter()
            .map(|point| point.into_config(&name, point_names))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(OpcUaClientConnectionConfig {
            name: SmolStr::new(name),
            endpoint_url,
            security: OpcUaSecurityProfile {
                policy,
                mode,
                allow_anonymous: matches!(auth, OpcUaClientAuthConfig::Anonymous),
            },
            auth,
            trust_server_certificate: self.trust_server_certificate.unwrap_or(false),
            poll_interval_ms,
            timeout_ms,
            points,
        })
    }
}

impl OpcUaClientPointSection {
    fn into_config(
        self,
        connection_name: &str,
        point_names: &mut std::collections::BTreeSet<String>,
    ) -> Result<OpcUaClientPointConfig, RuntimeError> {
        let var = non_empty_opcua_client("connections.points.var", self.var)?;
        if !point_names.insert(var.clone()) {
            return invalid_opcua_client(format!(
                "OPC UA point '{var}' is bound more than once; one binding per declared variable is required"
            ));
        }
        let node_id = non_empty_opcua_client("connections.points.node_id", self.node_id)?;
        let data_type = parse_opcua_client_data_type(self.type_name.as_str())?;
        let access = parse_opcua_client_access(self.access.as_deref(), self.writable)?;
        if matches!(access, OpcUaClientPointAccess::Write) {
            return invalid_opcua_client(format!(
                "OPC UA point '{var}' in connection '{connection_name}' cannot be write-only; runtime variables need read evidence before green status"
            ));
        }
        Ok(OpcUaClientPointConfig {
            var: SmolStr::new(var),
            node_id,
            data_type,
            access,
        })
    }
}

fn parse_client_auth(
    auth: Option<&str>,
    username: Option<String>,
    password: Option<String>,
    connection_name: &str,
) -> Result<OpcUaClientAuthConfig, RuntimeError> {
    let auth = auth.unwrap_or("anonymous").trim().to_ascii_lowercase();
    match auth.as_str() {
        "anonymous" => {
            if username.is_some() || password.is_some() {
                return invalid_opcua_client(format!(
                    "OPC UA connection '{connection_name}' uses anonymous auth but also sets username/password"
                ));
            }
            Ok(OpcUaClientAuthConfig::Anonymous)
        }
        "username" | "user_name" | "user" => {
            let username = username
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    RuntimeError::InvalidConfig(
                        format!("OPC UA connection '{connection_name}' username is required")
                            .into(),
                    )
                })?;
            let password = password
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    RuntimeError::InvalidConfig(
                        format!("OPC UA connection '{connection_name}' password is required")
                            .into(),
                    )
                })?;
            Ok(OpcUaClientAuthConfig::UserName {
                username: SmolStr::new(username),
                password: SmolStr::new(password),
            })
        }
        other => invalid_opcua_client(format!(
            "OPC UA connection '{connection_name}' auth must be anonymous or username, got '{other}'"
        )),
    }
}

fn validate_client_security_profile(
    policy: OpcUaSecurityPolicy,
    mode: OpcUaMessageSecurityMode,
) -> Result<(), RuntimeError> {
    match (policy, mode) {
        (OpcUaSecurityPolicy::None, OpcUaMessageSecurityMode::None)
        | (OpcUaSecurityPolicy::Basic256Sha256, OpcUaMessageSecurityMode::Sign)
        | (OpcUaSecurityPolicy::Basic256Sha256, OpcUaMessageSecurityMode::SignAndEncrypt)
        | (OpcUaSecurityPolicy::Aes128Sha256RsaOaep, OpcUaMessageSecurityMode::Sign)
        | (
            OpcUaSecurityPolicy::Aes128Sha256RsaOaep,
            OpcUaMessageSecurityMode::SignAndEncrypt,
        ) => Ok(()),
        (policy, mode) => invalid_opcua_client(format!(
            "unsupported OPC UA client security profile {policy:?}/{mode:?}"
        )),
    }
}

fn parse_opcua_client_data_type(value: &str) -> Result<OpcUaDataType, RuntimeError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "bool" | "boolean" => Ok(OpcUaDataType::Boolean),
        "int" | "int16" => Ok(OpcUaDataType::Int16),
        "dint" | "int32" => Ok(OpcUaDataType::Int32),
        "lint" | "int64" => Ok(OpcUaDataType::Int64),
        "uint" | "uint16" => Ok(OpcUaDataType::UInt16),
        "udint" | "uint32" => Ok(OpcUaDataType::UInt32),
        "ulint" | "uint64" => Ok(OpcUaDataType::UInt64),
        "real" | "float" | "float32" => Ok(OpcUaDataType::Float),
        "lreal" | "double" | "float64" => Ok(OpcUaDataType::Double),
        "string" => Ok(OpcUaDataType::String),
        other => invalid_opcua_client(format!(
            "unsupported OPC UA client point type '{other}'"
        )),
    }
}

fn parse_opcua_client_access(
    access: Option<&str>,
    writable: Option<bool>,
) -> Result<OpcUaClientPointAccess, RuntimeError> {
    if access.is_some() && writable.is_some() {
        return invalid_opcua_client(
            "OPC UA client point must not set both access and legacy writable",
        );
    }
    if let Some(writable) = writable {
        return Ok(if writable {
            OpcUaClientPointAccess::ReadWrite
        } else {
            OpcUaClientPointAccess::Read
        });
    }
    match access.unwrap_or("read").trim().to_ascii_lowercase().as_str() {
        "read" => Ok(OpcUaClientPointAccess::Read),
        "write" => Ok(OpcUaClientPointAccess::Write),
        "read_write" | "readwrite" | "read-write" => Ok(OpcUaClientPointAccess::ReadWrite),
        other => invalid_opcua_client(format!(
            "OPC UA client point access must be read or read_write, got '{other}'"
        )),
    }
}

fn valid_opc_tcp_endpoint(value: &str) -> bool {
    value
        .strip_prefix("opc.tcp://")
        .map(|remainder| {
            let authority_end = remainder
                .find(['/', '?', '#'])
                .unwrap_or(remainder.len());
            &remainder[..authority_end]
        })
        .is_some_and(|authority| {
            !authority.is_empty() && !authority.chars().any(char::is_whitespace)
        })
}

fn non_empty_opcua_client(field: &str, value: String) -> Result<String, RuntimeError> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return invalid_opcua_client(format!("{field} must not be empty"));
    }
    Ok(value)
}

fn invalid_opcua_client<T>(message: impl Into<String>) -> Result<T, RuntimeError> {
    Err(RuntimeError::InvalidConfig(message.into().into()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpcUaDataType {
    Boolean,
    Int16,
    Int32,
    Int64,
    UInt16,
    UInt32,
    UInt64,
    Float,
    Double,
    String,
}

impl OpcUaDataType {
    #[must_use]
    pub fn as_config_value(self) -> &'static str {
        match self {
            Self::Boolean => "BOOL",
            Self::Int16 => "INT",
            Self::Int32 => "DINT",
            Self::Int64 => "LINT",
            Self::UInt16 => "UINT",
            Self::UInt32 => "UDINT",
            Self::UInt64 => "ULINT",
            Self::Float => "REAL",
            Self::Double => "LREAL",
            Self::String => "STRING",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum OpcUaVariant {
    Boolean(bool),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    UInt16(u16),
    UInt32(u32),
    UInt64(u64),
    Float(f32),
    Double(f64),
    String(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpcUaValue {
    pub data_type: OpcUaDataType,
    pub value: OpcUaVariant,
}

#[derive(Debug, Clone)]
pub struct OpcUaExposedNode {
    pub name: SmolStr,
    pub node_id: String,
    pub data_type: OpcUaDataType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpcUaClientIdentity<'a> {
    Anonymous,
    UserName {
        username: &'a str,
        password: &'a str,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct OpcUaClientOptions {
    pub trust_server_certificate: bool,
}

impl Default for OpcUaClientOptions {
    fn default() -> Self {
        Self {
            trust_server_certificate: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpcUaLoadReport {
    pub iterations: usize,
    pub browse_ok: usize,
    pub read_ok: usize,
    pub write_ok: usize,
    pub elapsed_ms: u128,
}

pub struct OpcUaWireServer {
    endpoint_url: String,
    security: OpcUaSecurityProfile,
    exposed_nodes: Vec<OpcUaExposedNode>,
    #[cfg(feature = "opcua-wire")]
    node_ids: Arc<Mutex<HashMap<SmolStr, ::opcua::types::NodeId>>>,
    #[cfg(feature = "opcua-wire")]
    client_pki_dir: PathBuf,
    #[cfg(feature = "opcua-wire")]
    server: Arc<::opcua::sync::RwLock<::opcua::server::prelude::Server>>,
    #[cfg(feature = "opcua-wire")]
    server_thread: Option<std::thread::JoinHandle<()>>,
}

impl std::fmt::Debug for OpcUaWireServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpcUaWireServer")
            .field("endpoint_url", &self.endpoint_url)
            .field("security", &self.security)
            .field("exposed_nodes", &self.exposed_nodes)
            .finish()
    }
}

impl Drop for OpcUaWireServer {
    fn drop(&mut self) {
        self.stop();
    }
}

impl OpcUaWireServer {
    #[must_use]
    pub fn endpoint_url(&self) -> &str {
        self.endpoint_url.as_str()
    }

    #[must_use]
    pub fn security_profile(&self) -> OpcUaSecurityProfile {
        self.security
    }

    #[must_use]
    pub fn exposed_nodes(&self) -> &[OpcUaExposedNode] {
        self.exposed_nodes.as_slice()
    }

    #[cfg(feature = "opcua-wire")]
    pub fn stop(&mut self) {
        if let Some(join) = self.server_thread.take() {
            self.server.write().abort();
            let _ = join.join();
        }
    }

    #[cfg(not(feature = "opcua-wire"))]
    pub fn stop(&mut self) {}

    #[cfg(feature = "opcua-wire")]
    pub fn probe_read(
        &self,
        node_name: &str,
        identity: OpcUaClientIdentity<'_>,
    ) -> Result<OpcUaVariant, RuntimeError> {
        self.probe_read_with_options(node_name, identity, OpcUaClientOptions::default())
    }

    #[cfg(not(feature = "opcua-wire"))]
    pub fn probe_read(
        &self,
        _node_name: &str,
        _identity: OpcUaClientIdentity<'_>,
    ) -> Result<OpcUaVariant, RuntimeError> {
        Err(opcua_wire_feature_error())
    }

    #[cfg(feature = "opcua-wire")]
    pub fn probe_read_with_options(
        &self,
        node_name: &str,
        identity: OpcUaClientIdentity<'_>,
        options: OpcUaClientOptions,
    ) -> Result<OpcUaVariant, RuntimeError> {
        let node_id = self.node_id(node_name)?;
        let session = self.connect_session(identity, options)?;
        let value = {
            let session_guard = session.read();
            let values = session_guard
                .read(
                    &[::opcua::types::ReadValueId::from(node_id)],
                    ::opcua::types::TimestampsToReturn::Both,
                    0.0,
                )
                .map_err(opcua_status_error)?;
            values
                .into_iter()
                .next()
                .and_then(|item| item.value)
                .ok_or_else(|| RuntimeError::ControlError("OPC UA read returned no value".into()))?
        };
        session.read().disconnect();
        from_wire_variant(&value).ok_or_else(|| {
            RuntimeError::ControlError(format!("unsupported OPC UA variant: {value:?}").into())
        })
    }

    #[cfg(not(feature = "opcua-wire"))]
    pub fn probe_read_with_options(
        &self,
        _node_name: &str,
        _identity: OpcUaClientIdentity<'_>,
        _options: OpcUaClientOptions,
    ) -> Result<OpcUaVariant, RuntimeError> {
        Err(opcua_wire_feature_error())
    }

    #[cfg(feature = "opcua-wire")]
    pub fn run_load_fixture(
        &self,
        node_name: &str,
        iterations: usize,
        identity: OpcUaClientIdentity<'_>,
        options: OpcUaClientOptions,
    ) -> Result<OpcUaLoadReport, RuntimeError> {
        let node_id = self.node_id(node_name)?;
        let session = self.connect_session(identity, options)?;
        let start = Instant::now();
        let mut browse_ok = 0usize;
        let mut read_ok = 0usize;
        let mut write_ok = 0usize;

        {
            let session_guard = session.read();
            for _ in 0..iterations {
                let browse = session_guard
                    .browse(&[::opcua::types::BrowseDescription {
                        node_id: ::opcua::types::NodeId::objects_folder_id(),
                        browse_direction: ::opcua::types::BrowseDirection::Forward,
                        reference_type_id: ::opcua::types::ReferenceTypeId::References.into(),
                        include_subtypes: true,
                        node_class_mask: ::opcua::types::NodeClassMask::all().bits(),
                        result_mask: ::opcua::types::BrowseDescriptionResultMask::all().bits(),
                    }])
                    .map_err(opcua_status_error)?;
                if browse.is_some() {
                    browse_ok += 1;
                }

                let values = session_guard
                    .read(
                        &[::opcua::types::ReadValueId::from(node_id.clone())],
                        ::opcua::types::TimestampsToReturn::Both,
                        0.0,
                    )
                    .map_err(opcua_status_error)?;
                let Some(value) = values.first().and_then(|item| item.value.clone()) else {
                    continue;
                };
                read_ok += 1;

                let write_result = session_guard
                    .write(&[::opcua::types::WriteValue {
                        node_id: node_id.clone(),
                        attribute_id: ::opcua::types::AttributeId::Value as u32,
                        index_range: ::opcua::types::UAString::null(),
                        value: ::opcua::types::DataValue {
                            value: Some(value),
                            status: Some(::opcua::types::StatusCode::Good),
                            source_timestamp: Some(::opcua::types::DateTime::now()),
                            ..Default::default()
                        },
                    }])
                    .map_err(opcua_status_error)?;
                if write_result
                    .first()
                    .is_some_and(::opcua::types::StatusCode::is_good)
                {
                    write_ok += 1;
                }
            }
        }

        session.read().disconnect();
        Ok(OpcUaLoadReport {
            iterations,
            browse_ok,
            read_ok,
            write_ok,
            elapsed_ms: start.elapsed().as_millis(),
        })
    }

    #[cfg(not(feature = "opcua-wire"))]
    pub fn run_load_fixture(
        &self,
        _node_name: &str,
        _iterations: usize,
        _identity: OpcUaClientIdentity<'_>,
        _options: OpcUaClientOptions,
    ) -> Result<OpcUaLoadReport, RuntimeError> {
        Err(opcua_wire_feature_error())
    }

    #[cfg(feature = "opcua-wire")]
    fn connect_session(
        &self,
        identity: OpcUaClientIdentity<'_>,
        options: OpcUaClientOptions,
    ) -> Result<Arc<::opcua::sync::RwLock<::opcua::client::prelude::Session>>, RuntimeError> {
        let client_pki_dir = if options.trust_server_certificate {
            self.client_pki_dir.clone()
        } else {
            self.client_pki_dir.join("strict")
        };
        std::fs::create_dir_all(&client_pki_dir).map_err(|err| {
            RuntimeError::ControlError(format!("create OPC UA client PKI: {err}").into())
        })?;

        let mut client = ::opcua::client::prelude::ClientBuilder::new()
            .application_name("truST OPC UA probe")
            .application_uri("urn:trust:runtime:opcua:probe")
            .product_uri("urn:trust:runtime")
            .pki_dir(client_pki_dir)
            .create_sample_keypair(true)
            .trust_server_certs(options.trust_server_certificate)
            .verify_server_certs(!options.trust_server_certificate)
            .session_retry_limit(1)
            .client()
            .ok_or_else(|| RuntimeError::ControlError("failed to build OPC UA client".into()))?;

        let security_policy = to_wire_security_policy(self.security.policy);
        let security_mode = to_wire_security_mode(self.security.mode);
        let endpoints = client
            .get_server_endpoints_from_url(self.endpoint_url.as_str())
            .map_err(opcua_status_error)?;
        let endpoint = ::opcua::client::prelude::Client::find_matching_endpoint(
            endpoints.as_slice(),
            self.endpoint_url.as_str(),
            security_policy,
            security_mode,
        )
        .ok_or_else(|| {
            RuntimeError::ControlError(
                format!(
                    "no matching OPC UA endpoint for {} / {:?}",
                    security_policy.to_uri(),
                    security_mode
                )
                .into(),
            )
        })?;
        let token = match identity {
            OpcUaClientIdentity::Anonymous => ::opcua::client::prelude::IdentityToken::Anonymous,
            OpcUaClientIdentity::UserName { username, password } => {
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
    fn node_id(&self, node_name: &str) -> Result<::opcua::types::NodeId, RuntimeError> {
        let node_ids = self.node_ids.lock().map_err(|_| {
            RuntimeError::ControlError("OPC UA node registry unavailable".into())
        })?;
        node_ids.get(node_name).cloned().ok_or_else(|| {
            RuntimeError::ControlError(format!("unknown OPC UA node '{node_name}'").into())
        })
    }
}

#[cfg(not(feature = "opcua-wire"))]
fn opcua_wire_feature_error() -> RuntimeError {
    RuntimeError::ControlError(
        "OPC UA wire support is disabled in this build (enable feature 'opcua-wire')".into(),
    )
}
