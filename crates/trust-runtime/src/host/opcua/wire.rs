#[cfg(feature = "opcua-wire")]
pub fn start_wire_server(
    resource_name: &str,
    config: &OpcUaRuntimeConfig,
    snapshot_provider: Arc<dyn Fn() -> Option<DebugSnapshot> + Send + Sync>,
    runtime_root: Option<&Path>,
) -> Result<Option<OpcUaWireServer>, RuntimeError> {
    if !config.enabled {
        return Ok(None);
    }
    validate_security_profile(&config.security)?;
    let (bind_host, bind_port) = parse_listen(config.listen.as_str())?;
    let endpoint_path = normalize_endpoint_path(config.endpoint_path.as_str())?;
    let connect_host = if bind_host == "0.0.0.0" || bind_host == "::" {
        "127.0.0.1".to_string()
    } else {
        bind_host.clone()
    };
    let endpoint_url = format!("opc.tcp://{connect_host}:{bind_port}{endpoint_path}");
    let pki_root = runtime_root
        .map(|root| root.join("security").join("opcua"))
        .unwrap_or_else(|| std::env::temp_dir().join("trust-runtime-opcua"));
    let server_pki_dir = pki_root.join("server");
    let client_pki_dir = pki_root.join("client");
    std::fs::create_dir_all(&server_pki_dir).map_err(|err| {
        RuntimeError::ControlError(format!("create OPC UA server PKI: {err}").into())
    })?;
    std::fs::create_dir_all(&client_pki_dir).map_err(|err| {
        RuntimeError::ControlError(format!("create OPC UA client PKI: {err}").into())
    })?;

    let exposure_patterns = compile_exposure_patterns(config.expose.as_slice())?;
    let candidates = snapshot_provider()
        .as_ref()
        .map(|snapshot| collect_exposed_nodes(snapshot, exposure_patterns.as_slice(), config.max_nodes))
        .unwrap_or_default();
    let (user_token_ids, user_credentials) = user_tokens(config)?;
    let mut builder = ::opcua::server::prelude::ServerBuilder::new()
        .application_name(format!("truST Runtime {resource_name}"))
        .application_uri(format!("urn:trust:runtime:{resource_name}"))
        .product_uri("urn:trust:runtime")
        .create_sample_keypair(true)
        .pki_dir(server_pki_dir.clone())
        .trust_client_certs()
        .host_and_port(bind_host.clone(), bind_port)
        .discovery_urls(vec![endpoint_url.clone()])
        .endpoint(
            "trust-runtime",
            build_server_endpoint(endpoint_path.as_str(), config.security, &user_token_ids)?,
        );
    if let Some((username, password)) = user_credentials {
        builder = builder.user_token(
            "runtime_user",
            ::opcua::server::prelude::ServerUserToken::user_pass(username, password),
        );
    }
    let mut server = builder.server().ok_or_else(|| {
        RuntimeError::ControlError("failed to create OPC UA server from runtime profile".into())
    })?;

    let address_space = server.address_space();
    let mut node_ids = HashMap::<SmolStr, ::opcua::types::NodeId>::new();
    let mut exposed_nodes = Vec::<OpcUaExposedNode>::new();
    let namespace;
    let folder_id;
    {
        let mut address_space_guard = ::opcua::trace_write_lock!(address_space);
        namespace = address_space_guard
            .register_namespace(config.namespace_uri.as_str())
            .map_err(|_| {
                RuntimeError::ControlError("failed to register OPC UA namespace".into())
            })?;
        folder_id = address_space_guard
            .add_folder(
                "truST",
                "truST",
                &::opcua::types::NodeId::objects_folder_id(),
            )
            .map_err(|_| {
                RuntimeError::ControlError("failed to create OPC UA root folder".into())
            })?;
        publish_new_nodes(
            &mut address_space_guard,
            namespace,
            &folder_id,
            candidates,
            &mut node_ids,
            Some(&mut exposed_nodes),
        )?;
    }

    let node_ids = Arc::new(Mutex::new(node_ids));
    {
        let refresh_space = address_space.clone();
        let refresh_nodes = node_ids.clone();
        let refresh_snapshot = snapshot_provider.clone();
        let refresh_patterns = exposure_patterns;
        let refresh_max_nodes = config.max_nodes;
        let refresh_folder_id = folder_id.clone();
        server.add_polling_action(config.publish_interval_ms, move || {
            let Some(snapshot) = refresh_snapshot() else {
                return;
            };
            let candidates =
                collect_exposed_nodes(&snapshot, refresh_patterns.as_slice(), refresh_max_nodes);
            let now = ::opcua::types::DateTime::now();
            let mut address_space_guard = ::opcua::trace_write_lock!(refresh_space);
            let Ok(mut node_ids_guard) = refresh_nodes.lock() else {
                return;
            };
            for candidate in candidates {
                if let Some(node_id) = node_ids_guard.get(&candidate.name) {
                    address_space_guard.set_variable_value(
                        node_id.clone(),
                        to_wire_variant(&candidate.value),
                        &now,
                        &now,
                    );
                    continue;
                }
                if node_ids_guard.len() >= refresh_max_nodes {
                    break;
                }
                let mut added_nodes = HashMap::<SmolStr, ::opcua::types::NodeId>::new();
                if publish_new_nodes(
                    &mut address_space_guard,
                    namespace,
                    &refresh_folder_id,
                    vec![candidate],
                    &mut added_nodes,
                    None,
                )
                .is_err()
                {
                    continue;
                }
                node_ids_guard.extend(added_nodes);
            }
        });
    }

    let server = Arc::new(::opcua::sync::RwLock::new(server));
    let server_task = server.clone();
    let server_thread = std::thread::Builder::new()
        .name("trust-runtime-opcua".to_string())
        .spawn(move || {
            ::opcua::server::prelude::Server::run_server(server_task);
        })
        .map_err(|err| RuntimeError::ThreadSpawn(format!("OPC UA server thread: {err}").into()))?;

    wait_for_endpoint(connect_host.as_str(), bind_port, StdDuration::from_secs(4))?;
    let wire_server = OpcUaWireServer {
        endpoint_url,
        security: config.security,
        exposed_nodes,
        node_ids,
        client_pki_dir,
        server,
        server_thread: Some(server_thread),
    };

    if let Some(node) = wire_server.exposed_nodes.first() {
        let identity = if config.security.allow_anonymous {
            OpcUaClientIdentity::Anonymous
        } else if let (Some(username), Some(password)) =
            (config.username.as_ref(), config.password.as_ref())
        {
            OpcUaClientIdentity::UserName { username, password }
        } else {
            return Err(RuntimeError::ControlError(
                "OPC UA startup probe requires runtime.opcua.username/password".into(),
            ));
        };
        wire_server.probe_read(node.name.as_str(), identity)?;
    }

    Ok(Some(wire_server))
}

#[cfg(feature = "opcua-wire")]
fn publish_new_nodes(
    address_space: &mut ::opcua::server::address_space::AddressSpace,
    namespace: u16,
    folder_id: &::opcua::types::NodeId,
    candidates: Vec<ExposedNodeCandidate>,
    node_ids: &mut HashMap<SmolStr, ::opcua::types::NodeId>,
    mut exposed_nodes: Option<&mut Vec<OpcUaExposedNode>>,
) -> Result<(), RuntimeError> {
    let mut variables = Vec::new();
    let mut inserted = Vec::new();
    for node in candidates {
        if node_ids.contains_key(&node.name) {
            continue;
        }
        let browse_name = node.name.to_string();
        let node_id = ::opcua::types::NodeId::new(namespace, browse_name.clone());
        let mut variable = ::opcua::server::prelude::Variable::new(
            &node_id,
            browse_name.as_str(),
            browse_name.as_str(),
            to_wire_variant(&node.value),
        );
        variable.set_writable(false);
        variables.push(variable);
        inserted.push((node.name, node.data_type, node_id));
    }
    if variables.is_empty() {
        return Ok(());
    }
    let added = address_space.add_variables(variables, folder_id);
    if added.iter().any(|inserted| !*inserted) {
        return Err(RuntimeError::ControlError(
            "failed to publish OPC UA variables".into(),
        ));
    }
    for (name, data_type, node_id) in inserted {
        if let Some(nodes) = exposed_nodes.as_deref_mut() {
            nodes.push(OpcUaExposedNode {
                name: name.clone(),
                node_id: node_id.to_string(),
                data_type,
            });
        }
        node_ids.insert(name, node_id);
    }
    Ok(())
}

#[cfg(not(feature = "opcua-wire"))]
pub fn start_wire_server(
    _resource_name: &str,
    config: &OpcUaRuntimeConfig,
    _snapshot_provider: Arc<dyn Fn() -> Option<DebugSnapshot> + Send + Sync>,
    _runtime_root: Option<&Path>,
) -> Result<Option<OpcUaWireServer>, RuntimeError> {
    if !config.enabled {
        return Ok(None);
    }
    Err(opcua_wire_feature_error())
}

#[cfg(feature = "opcua-wire")]
#[derive(Debug, Clone)]
struct ExposedNodeCandidate {
    name: SmolStr,
    data_type: OpcUaDataType,
    value: OpcUaVariant,
}

#[cfg(feature = "opcua-wire")]
fn collect_exposed_nodes(
    snapshot: &DebugSnapshot,
    patterns: &[Pattern],
    max_nodes: usize,
) -> Vec<ExposedNodeCandidate> {
    let mut nodes = Vec::new();
    for (name, value) in snapshot.storage.globals() {
        if !patterns.is_empty()
            && !patterns
                .iter()
                .any(|pattern| pattern.matches(name.as_str()))
        {
            continue;
        }
        let Some(mapped) = map_iec_value(value) else {
            continue;
        };
        nodes.push(ExposedNodeCandidate {
            name: name.clone(),
            data_type: mapped.data_type,
            value: mapped.value,
        });
        if nodes.len() >= max_nodes {
            break;
        }
    }
    nodes
}

#[cfg(feature = "opcua-wire")]
fn compile_exposure_patterns(patterns: &[SmolStr]) -> Result<Vec<Pattern>, RuntimeError> {
    patterns
        .iter()
        .map(|pattern| {
            Pattern::new(pattern.as_str()).map_err(|err| {
                RuntimeError::InvalidConfig(
                    format!("runtime.opcua.expose invalid pattern '{}': {err}", pattern).into(),
                )
            })
        })
        .collect()
}

#[cfg(feature = "opcua-wire")]
type OpcUaUserTokens = (Vec<String>, Option<(String, String)>);

#[cfg(feature = "opcua-wire")]
fn user_tokens(config: &OpcUaRuntimeConfig) -> Result<OpcUaUserTokens, RuntimeError> {
    let mut user_token_ids = Vec::<String>::new();
    if config.security.allow_anonymous {
        user_token_ids.push(::opcua::server::prelude::ANONYMOUS_USER_TOKEN_ID.to_string());
    }
    let credentials = match (config.username.as_ref(), config.password.as_ref()) {
        (Some(username), Some(password)) => {
            user_token_ids.push("runtime_user".to_string());
            Some((username.to_string(), password.to_string()))
        }
        (None, None) => None,
        _ => {
            return Err(RuntimeError::InvalidConfig(
                "runtime.opcua.username/password must both be set or both be omitted".into(),
            ))
        }
    };
    if user_token_ids.is_empty() {
        return Err(RuntimeError::InvalidConfig(
            "runtime.opcua must allow anonymous or configure username/password".into(),
        ));
    }
    Ok((user_token_ids, credentials))
}

#[cfg(feature = "opcua-wire")]
fn build_server_endpoint(
    endpoint_path: &str,
    security: OpcUaSecurityProfile,
    user_token_ids: &[String],
) -> Result<::opcua::server::prelude::ServerEndpoint, RuntimeError> {
    let endpoint = match (security.policy, security.mode) {
        (OpcUaSecurityPolicy::None, OpcUaMessageSecurityMode::None) => {
            ::opcua::server::prelude::ServerEndpoint::new_none(endpoint_path, user_token_ids)
        }
        (OpcUaSecurityPolicy::Basic256Sha256, OpcUaMessageSecurityMode::Sign) => {
            ::opcua::server::prelude::ServerEndpoint::new_basic256sha256_sign(
                endpoint_path,
                user_token_ids,
            )
        }
        (OpcUaSecurityPolicy::Basic256Sha256, OpcUaMessageSecurityMode::SignAndEncrypt) => {
            ::opcua::server::prelude::ServerEndpoint::new_basic256sha256_sign_encrypt(
                endpoint_path,
                user_token_ids,
            )
        }
        (OpcUaSecurityPolicy::Aes128Sha256RsaOaep, OpcUaMessageSecurityMode::Sign) => {
            ::opcua::server::prelude::ServerEndpoint::new_aes128_sha256_rsaoaep_sign(
                endpoint_path,
                user_token_ids,
            )
        }
        (OpcUaSecurityPolicy::Aes128Sha256RsaOaep, OpcUaMessageSecurityMode::SignAndEncrypt) => {
            ::opcua::server::prelude::ServerEndpoint::new_aes128_sha256_rsaoaep_sign_encrypt(
                endpoint_path,
                user_token_ids,
            )
        }
        (policy, mode) => {
            return Err(RuntimeError::InvalidConfig(
                format!("unsupported OPC UA security profile {policy:?}/{mode:?}").into(),
            ))
        }
    };
    Ok(endpoint)
}

#[cfg(any(feature = "opcua-wire", test))]
fn validate_security_profile(profile: &OpcUaSecurityProfile) -> Result<(), RuntimeError> {
    match (profile.policy, profile.mode) {
        (OpcUaSecurityPolicy::None, OpcUaMessageSecurityMode::None)
        | (OpcUaSecurityPolicy::Basic256Sha256, OpcUaMessageSecurityMode::Sign)
        | (OpcUaSecurityPolicy::Basic256Sha256, OpcUaMessageSecurityMode::SignAndEncrypt)
        | (OpcUaSecurityPolicy::Aes128Sha256RsaOaep, OpcUaMessageSecurityMode::Sign)
        | (OpcUaSecurityPolicy::Aes128Sha256RsaOaep, OpcUaMessageSecurityMode::SignAndEncrypt) => {
            Ok(())
        }
        (policy, mode) => Err(RuntimeError::InvalidConfig(
            format!("invalid runtime.opcua security profile {policy:?}/{mode:?}").into(),
        )),
    }
}

#[cfg(feature = "opcua-wire")]
fn normalize_endpoint_path(path: &str) -> Result<String, RuntimeError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Ok("/".to_string());
    }
    if trimmed.starts_with('/') {
        return Ok(trimmed.to_string());
    }
    Err(RuntimeError::InvalidConfig(
        "runtime.opcua.endpoint_path must start with '/'".into(),
    ))
}

#[cfg(feature = "opcua-wire")]
fn parse_listen(listen: &str) -> Result<(String, u16), RuntimeError> {
    let trimmed = listen.trim();
    if trimmed.is_empty() {
        return Err(RuntimeError::InvalidConfig(
            "runtime.opcua.listen must not be empty".into(),
        ));
    }
    if let Ok(socket) = trimmed.parse::<std::net::SocketAddr>() {
        return Ok((socket.ip().to_string(), socket.port()));
    }
    let (host, port) = trimmed.rsplit_once(':').ok_or_else(|| {
        RuntimeError::InvalidConfig("runtime.opcua.listen must be host:port".into())
    })?;
    let host = host.trim().trim_start_matches('[').trim_end_matches(']');
    if host.is_empty() {
        return Err(RuntimeError::InvalidConfig(
            "runtime.opcua.listen host must not be empty".into(),
        ));
    }
    let port = port.parse::<u16>().map_err(|err| {
        RuntimeError::InvalidConfig(format!("invalid OPC UA listen port: {err}").into())
    })?;
    Ok((host.to_string(), port))
}

#[cfg(feature = "opcua-wire")]
fn wait_for_endpoint(host: &str, port: u16, timeout: StdDuration) -> Result<(), RuntimeError> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if std::net::TcpStream::connect((host, port)).is_ok() {
            return Ok(());
        }
        std::thread::sleep(StdDuration::from_millis(40));
    }
    Err(RuntimeError::ControlError(
        format!("OPC UA endpoint not reachable at {host}:{port}").into(),
    ))
}

#[cfg(feature = "opcua-wire")]
fn to_wire_security_policy(policy: OpcUaSecurityPolicy) -> ::opcua::crypto::SecurityPolicy {
    match policy {
        OpcUaSecurityPolicy::None => ::opcua::crypto::SecurityPolicy::None,
        OpcUaSecurityPolicy::Basic256Sha256 => ::opcua::crypto::SecurityPolicy::Basic256Sha256,
        OpcUaSecurityPolicy::Aes128Sha256RsaOaep => {
            ::opcua::crypto::SecurityPolicy::Aes128Sha256RsaOaep
        }
    }
}

#[cfg(feature = "opcua-wire")]
fn to_wire_security_mode(mode: OpcUaMessageSecurityMode) -> ::opcua::types::MessageSecurityMode {
    match mode {
        OpcUaMessageSecurityMode::None => ::opcua::types::MessageSecurityMode::None,
        OpcUaMessageSecurityMode::Sign => ::opcua::types::MessageSecurityMode::Sign,
        OpcUaMessageSecurityMode::SignAndEncrypt => {
            ::opcua::types::MessageSecurityMode::SignAndEncrypt
        }
    }
}

#[cfg(feature = "opcua-wire")]
fn to_wire_variant(value: &OpcUaVariant) -> ::opcua::types::Variant {
    match value {
        OpcUaVariant::Boolean(value) => ::opcua::types::Variant::Boolean(*value),
        OpcUaVariant::Int16(value) => ::opcua::types::Variant::Int16(*value),
        OpcUaVariant::Int32(value) => ::opcua::types::Variant::Int32(*value),
        OpcUaVariant::Int64(value) => ::opcua::types::Variant::Int64(*value),
        OpcUaVariant::UInt16(value) => ::opcua::types::Variant::UInt16(*value),
        OpcUaVariant::UInt32(value) => ::opcua::types::Variant::UInt32(*value),
        OpcUaVariant::UInt64(value) => ::opcua::types::Variant::UInt64(*value),
        OpcUaVariant::Float(value) => ::opcua::types::Variant::Float(*value),
        OpcUaVariant::Double(value) => ::opcua::types::Variant::Double(*value),
        OpcUaVariant::String(value) => ::opcua::types::Variant::String(value.as_str().into()),
    }
}

#[cfg(feature = "opcua-wire")]
fn from_wire_variant(value: &::opcua::types::Variant) -> Option<OpcUaVariant> {
    match value {
        ::opcua::types::Variant::Boolean(value) => Some(OpcUaVariant::Boolean(*value)),
        ::opcua::types::Variant::Int16(value) => Some(OpcUaVariant::Int16(*value)),
        ::opcua::types::Variant::Int32(value) => Some(OpcUaVariant::Int32(*value)),
        ::opcua::types::Variant::Int64(value) => Some(OpcUaVariant::Int64(*value)),
        ::opcua::types::Variant::UInt16(value) => Some(OpcUaVariant::UInt16(*value)),
        ::opcua::types::Variant::UInt32(value) => Some(OpcUaVariant::UInt32(*value)),
        ::opcua::types::Variant::UInt64(value) => Some(OpcUaVariant::UInt64(*value)),
        ::opcua::types::Variant::Float(value) => Some(OpcUaVariant::Float(*value)),
        ::opcua::types::Variant::Double(value) => Some(OpcUaVariant::Double(*value)),
        ::opcua::types::Variant::String(value) => Some(OpcUaVariant::String(value.to_string())),
        _ => None,
    }
}

#[cfg(feature = "opcua-wire")]
fn opcua_status_error(status: ::opcua::types::StatusCode) -> RuntimeError {
    RuntimeError::ControlError(format!("OPC UA status: {status}").into())
}
