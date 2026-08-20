fn parse_opcua_section(section: Option<OpcUaSection>) -> Result<OpcUaRuntimeConfig, RuntimeError> {
    let opcua_section = section.unwrap_or(OpcUaSection {
        enabled: Some(false),
        listen: Some("0.0.0.0:4840".into()),
        endpoint_path: Some("/".into()),
        namespace_uri: Some("urn:trust:runtime".into()),
        publish_interval_ms: Some(250),
        max_nodes: Some(128),
        expose: Some(Vec::new()),
        security_policy: Some("basic256sha256".into()),
        security_mode: Some("sign_and_encrypt".into()),
        allow_anonymous: Some(false),
        username: None,
        password: None,
    });

    if opcua_section
        .listen
        .as_deref()
        .is_some_and(|listen| listen.trim().is_empty())
    {
        return Err(RuntimeError::InvalidConfig(
            "runtime.opcua.listen must not be empty".into(),
        ));
    }
    if opcua_section
        .endpoint_path
        .as_deref()
        .is_some_and(|path| path.trim().is_empty())
    {
        return Err(RuntimeError::InvalidConfig(
            "runtime.opcua.endpoint_path must not be empty".into(),
        ));
    }
    if opcua_section
        .namespace_uri
        .as_deref()
        .is_some_and(|uri| uri.trim().is_empty())
    {
        return Err(RuntimeError::InvalidConfig(
            "runtime.opcua.namespace_uri must not be empty".into(),
        ));
    }

    let publish_interval_ms = opcua_section.publish_interval_ms.unwrap_or(250);
    if publish_interval_ms == 0 {
        return Err(RuntimeError::InvalidConfig(
            "runtime.opcua.publish_interval_ms must be >= 1".into(),
        ));
    }
    let max_nodes = opcua_section.max_nodes.unwrap_or(128);
    if max_nodes == 0 {
        return Err(RuntimeError::InvalidConfig(
            "runtime.opcua.max_nodes must be >= 1".into(),
        ));
    }

    let expose = opcua_section
        .expose
        .unwrap_or_default()
        .into_iter()
        .map(|value| parse_nonempty_entry(value, "runtime.opcua.expose").map(SmolStr::new))
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    for pattern in &expose {
        Pattern::new(pattern.as_str()).map_err(|err| {
            RuntimeError::InvalidConfig(
                format!("runtime.opcua.expose invalid pattern '{}': {err}", pattern).into(),
            )
        })?;
    }

    let security_policy_raw = opcua_section
        .security_policy
        .as_deref()
        .unwrap_or("basic256sha256");
    let security_mode_raw = opcua_section
        .security_mode
        .as_deref()
        .unwrap_or("sign_and_encrypt");
    let security_policy = OpcUaSecurityPolicy::parse(security_policy_raw).ok_or_else(|| {
        RuntimeError::InvalidConfig(
            format!("invalid runtime.opcua.security_policy '{security_policy_raw}'").into(),
        )
    })?;
    let security_mode = OpcUaMessageSecurityMode::parse(security_mode_raw).ok_or_else(|| {
        RuntimeError::InvalidConfig(
            format!("invalid runtime.opcua.security_mode '{security_mode_raw}'").into(),
        )
    })?;
    let allow_anonymous = opcua_section.allow_anonymous.unwrap_or(false);

    match (security_policy, security_mode) {
        (OpcUaSecurityPolicy::None, OpcUaMessageSecurityMode::None)
        | (OpcUaSecurityPolicy::Basic256Sha256, OpcUaMessageSecurityMode::Sign)
        | (OpcUaSecurityPolicy::Basic256Sha256, OpcUaMessageSecurityMode::SignAndEncrypt)
        | (OpcUaSecurityPolicy::Aes128Sha256RsaOaep, OpcUaMessageSecurityMode::Sign)
        | (
            OpcUaSecurityPolicy::Aes128Sha256RsaOaep,
            OpcUaMessageSecurityMode::SignAndEncrypt,
        ) => {}
        (policy, mode) => {
            return Err(RuntimeError::InvalidConfig(
                format!("unsupported runtime.opcua security profile {policy:?}/{mode:?}").into(),
            ))
        }
    }

    let username = opcua_section
        .username
        .map(|value| parse_nonempty_entry(value, "runtime.opcua.username"))
        .transpose()?
        .map(SmolStr::new);
    let password = opcua_section
        .password
        .map(|value| parse_nonempty_entry(value, "runtime.opcua.password"))
        .transpose()?
        .map(SmolStr::new);
    if username.is_some() ^ password.is_some() {
        return Err(RuntimeError::InvalidConfig(
            "runtime.opcua.username/password must both be set or both be omitted".into(),
        ));
    }

    let enabled = opcua_section.enabled.unwrap_or(false);
    if enabled && !allow_anonymous && username.is_none() {
        return Err(RuntimeError::InvalidConfig(
            "runtime.opcua requires anonymous access or username/password when enabled".into(),
        ));
    }

    let endpoint_path = opcua_section
        .endpoint_path
        .unwrap_or_else(|| "/".to_string())
        .trim()
        .to_string();
    if !endpoint_path.starts_with('/') {
        return Err(RuntimeError::InvalidConfig(
            "runtime.opcua.endpoint_path must start with '/'".into(),
        ));
    }

    Ok(OpcUaRuntimeConfig {
        enabled,
        listen: SmolStr::new(
            opcua_section
                .listen
                .unwrap_or_else(|| "0.0.0.0:4840".to_string())
                .trim(),
        ),
        endpoint_path: SmolStr::new(endpoint_path),
        namespace_uri: SmolStr::new(
            opcua_section
                .namespace_uri
                .unwrap_or_else(|| "urn:trust:runtime".to_string())
                .trim(),
        ),
        publish_interval_ms,
        max_nodes,
        expose,
        security: OpcUaSecurityProfile {
            policy: security_policy,
            mode: security_mode,
            allow_anonymous,
        },
        username,
        password,
    })
}
