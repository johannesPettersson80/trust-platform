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
    node_ids: HashMap<SmolStr, ::opcua::types::NodeId>,
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
        self.node_ids.get(node_name).cloned().ok_or_else(|| {
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
