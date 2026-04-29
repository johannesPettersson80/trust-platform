#[derive(Debug, Clone)]
struct BrokerEndpoint {
    host: SmolStr,
    port: u16,
}

#[derive(Debug, Clone)]
struct MqttIoConfig {
    endpoint: BrokerEndpoint,
    client_id: SmolStr,
    topic_in: SmolStr,
    topic_out: SmolStr,
    username: Option<SmolStr>,
    password: Option<SmolStr>,
    reconnect: StdDuration,
    keep_alive: StdDuration,
    tls: Option<MqttTlsConfig>,
}

#[derive(Debug, Clone)]
struct MqttTlsConfig {
    ca: Vec<u8>,
    client_auth: Option<(Vec<u8>, Vec<u8>)>,
    alpn: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct MqttToml {
    broker: String,
    client_id: Option<String>,
    topic_in: Option<String>,
    topic_out: Option<String>,
    username: Option<String>,
    password: Option<String>,
    reconnect_ms: Option<u64>,
    keep_alive_s: Option<u64>,
    tls: Option<bool>,
    tls_ca_path: Option<String>,
    tls_client_cert_path: Option<String>,
    tls_client_key_path: Option<String>,
    tls_alpn: Option<Vec<String>>,
    allow_insecure_remote: Option<bool>,
}

impl MqttIoConfig {
    fn from_params(value: &toml::Value) -> Result<Self, RuntimeError> {
        let params: MqttToml = value
            .clone()
            .try_into()
            .map_err(|err| RuntimeError::InvalidConfig(format!("io.params: {err}").into()))?;
        let broker_implies_tls = broker_uses_tls_scheme(&params.broker);
        let endpoint = parse_broker_endpoint(&params.broker)?;
        let tls_enabled = params.tls.unwrap_or(broker_implies_tls);
        if broker_implies_tls && params.tls == Some(false) {
            return Err(RuntimeError::InvalidConfig(
                "mqtt broker uses a TLS scheme but io.params.tls=false".into(),
            ));
        }
        let tls = parse_tls_config(&params, tls_enabled)?;
        let allow_insecure_remote = params.allow_insecure_remote.unwrap_or(false);
        if tls.is_none() && !allow_insecure_remote && !is_local_host(endpoint.host.as_str()) {
            return Err(RuntimeError::InvalidConfig(
                format!(
                    "mqtt insecure remote broker '{}' requires allow_insecure_remote=true",
                    endpoint.host
                )
                .into(),
            ));
        }
        let username = params.username.map(SmolStr::new);
        let password = params.password.map(SmolStr::new);
        if username.is_some() ^ password.is_some() {
            return Err(RuntimeError::InvalidConfig(
                "mqtt username/password must be set together".into(),
            ));
        }
        let client_id = params
            .client_id
            .map(SmolStr::new)
            .unwrap_or_else(|| SmolStr::new(format!("trust-runtime-{}", std::process::id())));
        let topic_in = params
            .topic_in
            .map(SmolStr::new)
            .unwrap_or_else(|| SmolStr::new("trust/io/in"));
        let topic_out = params
            .topic_out
            .map(SmolStr::new)
            .unwrap_or_else(|| SmolStr::new("trust/io/out"));
        let reconnect = StdDuration::from_millis(params.reconnect_ms.unwrap_or(500).max(1));
        let keep_alive_s = params.keep_alive_s.unwrap_or(5).max(1);
        if keep_alive_s > u16::MAX.into() {
            return Err(RuntimeError::InvalidConfig(
                "mqtt keep_alive_s must be <= 65535".into(),
            ));
        }

        Ok(Self {
            endpoint,
            client_id,
            topic_in,
            topic_out,
            username,
            password,
            reconnect,
            keep_alive: StdDuration::from_secs(keep_alive_s),
            tls,
        })
    }
}

fn parse_tls_config(
    params: &MqttToml,
    tls_enabled: bool,
) -> Result<Option<MqttTlsConfig>, RuntimeError> {
    let tls_fields_present = params.tls_ca_path.is_some()
        || params.tls_client_cert_path.is_some()
        || params.tls_client_key_path.is_some()
        || params.tls_alpn.is_some();
    if !tls_enabled {
        if tls_fields_present {
            return Err(RuntimeError::InvalidConfig(
                "mqtt tls_* parameters require io.params.tls=true".into(),
            ));
        }
        return Ok(None);
    }

    let ca_path = params.tls_ca_path.as_deref().ok_or_else(|| {
        RuntimeError::InvalidConfig("mqtt tls=true requires tls_ca_path".into())
    })?;
    let ca = read_tls_file(ca_path, "tls_ca_path")?;
    let client_auth = match (
        params.tls_client_cert_path.as_deref(),
        params.tls_client_key_path.as_deref(),
    ) {
        (Some(cert_path), Some(key_path)) => Some((
            read_tls_file(cert_path, "tls_client_cert_path")?,
            read_tls_file(key_path, "tls_client_key_path")?,
        )),
        (None, None) => None,
        _ => {
            return Err(RuntimeError::InvalidConfig(
                "mqtt mTLS requires tls_client_cert_path and tls_client_key_path together".into(),
            ));
        }
    };
    let alpn = params
        .tls_alpn
        .as_ref()
        .map(|protocols| {
            protocols
                .iter()
                .map(|protocol| {
                    let protocol = protocol.trim();
                    if protocol.is_empty() {
                        return Err(RuntimeError::InvalidConfig(
                            "mqtt tls_alpn entries must not be empty".into(),
                        ));
                    }
                    Ok(protocol.to_owned())
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;

    Ok(Some(MqttTlsConfig {
        ca,
        client_auth,
        alpn,
    }))
}

fn read_tls_file(path: &str, field: &str) -> Result<Vec<u8>, RuntimeError> {
    let path = path.trim();
    if path.is_empty() {
        return Err(RuntimeError::InvalidConfig(
            format!("mqtt {field} must not be empty").into(),
        ));
    }
    fs::read(path).map_err(|err| {
        RuntimeError::InvalidConfig(format!("mqtt {field} '{path}' cannot be read: {err}").into())
    })
}
