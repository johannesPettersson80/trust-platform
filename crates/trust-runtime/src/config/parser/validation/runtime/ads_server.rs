fn parse_ads_server_section(
    section: Option<AdsServerSection>,
) -> Result<AdsServerRuntimeConfig, RuntimeError> {
    let section = section.unwrap_or_else(default_ads_server_section);
    let enabled = section.enabled.unwrap_or(false);
    let allow_unpinned_clients = section.allow_unpinned_clients.unwrap_or(false);
    let unsafe_allow_public_bind = section.unsafe_allow_public_bind.unwrap_or(false);
    let insecure_transport = section.insecure_transport.unwrap_or(false);

    if enabled && !insecure_transport {
        return Err(RuntimeError::InvalidConfig(
            "runtime.ads_server.insecure_transport=true is required for plain ADS server".into(),
        ));
    }

    let listen = parse_ads_server_listen(
        section.listen.as_deref(),
        enabled,
        unsafe_allow_public_bind,
    )?;
    let ams_net_id = parse_ads_server_ams_net_id(section.ams_net_id.as_deref(), listen.as_deref())?;
    let ads_port = section
        .ads_port
        .unwrap_or(crate::ads::server::DEFAULT_ADS_SERVER_ADS_PORT);
    if enabled && ads_port == 0 {
        return Err(RuntimeError::InvalidConfig(
            "runtime.ads_server.ads_port must be >= 1".into(),
        ));
    }

    let expose = parse_glob_list(section.expose, "runtime.ads_server.expose")?;
    let writable = parse_glob_list(section.writable, "runtime.ads_server.writable")?;
    validate_writable_subset(&expose, &writable)?;

    let clients = parse_ads_server_clients(
        section.allow_clients,
        section.clients,
        allow_unpinned_clients,
    )?;

    Ok(AdsServerRuntimeConfig {
        enabled,
        listen: listen.map(SmolStr::new),
        ads_port,
        ams_net_id,
        insecure_transport,
        writes_enabled: section.writes_enabled.unwrap_or(false),
        symbol_namespace: SmolStr::new(
            section
                .symbol_namespace
                .as_deref()
                .map(str::trim)
                .unwrap_or(""),
        ),
        allow_unpinned_clients,
        unsafe_allow_public_bind,
        expose,
        writable,
        clients,
        max_symbols: nonzero_usize(
            section.max_symbols,
            crate::ads::server::DEFAULT_ADS_SERVER_MAX_SYMBOLS,
            "runtime.ads_server.max_symbols",
        )?,
        max_clients: nonzero_usize(
            section.max_clients,
            crate::ads::server::DEFAULT_ADS_SERVER_MAX_CLIENTS,
            "runtime.ads_server.max_clients",
        )?,
        max_subscriptions_per_client: nonzero_usize(
            section.max_subscriptions_per_client,
            crate::ads::server::DEFAULT_ADS_SERVER_MAX_SUBSCRIPTIONS_PER_CLIENT,
            "runtime.ads_server.max_subscriptions_per_client",
        )?,
        max_total_subscriptions: nonzero_usize(
            section.max_total_subscriptions,
            crate::ads::server::DEFAULT_ADS_SERVER_MAX_TOTAL_SUBSCRIPTIONS,
            "runtime.ads_server.max_total_subscriptions",
        )?,
        max_frame_bytes: nonzero_usize(
            section.max_frame_bytes,
            crate::ads::server::DEFAULT_ADS_SERVER_MAX_FRAME_BYTES,
            "runtime.ads_server.max_frame_bytes",
        )?,
        max_sumup_items: nonzero_usize(
            section.max_sumup_items,
            crate::ads::server::DEFAULT_ADS_SERVER_MAX_SUMUP_ITEMS,
            "runtime.ads_server.max_sumup_items",
        )?,
        max_write_bytes: nonzero_usize(
            section.max_write_bytes,
            crate::ads::server::DEFAULT_ADS_SERVER_MAX_WRITE_BYTES,
            "runtime.ads_server.max_write_bytes",
        )?,
        max_string_bytes: nonzero_usize(
            section.max_string_bytes,
            crate::ads::server::DEFAULT_ADS_SERVER_MAX_STRING_BYTES,
            "runtime.ads_server.max_string_bytes",
        )?,
        read_timeout_ms: nonzero_u64(
            section.read_timeout_ms,
            crate::ads::server::DEFAULT_ADS_SERVER_READ_TIMEOUT_MS,
            "runtime.ads_server.read_timeout_ms",
        )?,
        idle_timeout_ms: nonzero_u64(
            section.idle_timeout_ms,
            crate::ads::server::DEFAULT_ADS_SERVER_IDLE_TIMEOUT_MS,
            "runtime.ads_server.idle_timeout_ms",
        )?,
        min_notification_cycle_ms: nonzero_u64(
            section.min_notification_cycle_ms,
            crate::ads::server::DEFAULT_ADS_SERVER_MIN_NOTIFICATION_CYCLE_MS,
            "runtime.ads_server.min_notification_cycle_ms",
        )?,
    })
}

fn default_ads_server_section() -> AdsServerSection {
    AdsServerSection {
        enabled: Some(false),
        listen: None,
        ads_port: Some(crate::ads::server::DEFAULT_ADS_SERVER_ADS_PORT),
        ams_net_id: None,
        insecure_transport: Some(false),
        writes_enabled: Some(false),
        symbol_namespace: Some(String::new()),
        allow_unpinned_clients: Some(false),
        unsafe_allow_public_bind: Some(false),
        max_symbols: Some(crate::ads::server::DEFAULT_ADS_SERVER_MAX_SYMBOLS),
        max_clients: Some(crate::ads::server::DEFAULT_ADS_SERVER_MAX_CLIENTS),
        max_subscriptions_per_client: Some(
            crate::ads::server::DEFAULT_ADS_SERVER_MAX_SUBSCRIPTIONS_PER_CLIENT,
        ),
        max_total_subscriptions: Some(crate::ads::server::DEFAULT_ADS_SERVER_MAX_TOTAL_SUBSCRIPTIONS),
        max_frame_bytes: Some(crate::ads::server::DEFAULT_ADS_SERVER_MAX_FRAME_BYTES),
        max_sumup_items: Some(crate::ads::server::DEFAULT_ADS_SERVER_MAX_SUMUP_ITEMS),
        max_write_bytes: Some(crate::ads::server::DEFAULT_ADS_SERVER_MAX_WRITE_BYTES),
        max_string_bytes: Some(crate::ads::server::DEFAULT_ADS_SERVER_MAX_STRING_BYTES),
        read_timeout_ms: Some(crate::ads::server::DEFAULT_ADS_SERVER_READ_TIMEOUT_MS),
        idle_timeout_ms: Some(crate::ads::server::DEFAULT_ADS_SERVER_IDLE_TIMEOUT_MS),
        min_notification_cycle_ms: Some(
            crate::ads::server::DEFAULT_ADS_SERVER_MIN_NOTIFICATION_CYCLE_MS,
        ),
        expose: Some(Vec::new()),
        writable: Some(Vec::new()),
        allow_clients: Some(Vec::new()),
        clients: Some(Vec::new()),
    }
}

fn parse_ads_server_listen(
    listen: Option<&str>,
    enabled: bool,
    unsafe_allow_public_bind: bool,
) -> Result<Option<String>, RuntimeError> {
    let listen = listen.map(str::trim).filter(|value| !value.is_empty());
    if enabled && listen.is_none() {
        return Err(RuntimeError::InvalidConfig(
            "runtime.ads_server.enabled=true requires explicit listen IP".into(),
        ));
    }
    let Some(listen) = listen else {
        return Ok(None);
    };
    let address = listen.parse::<std::net::IpAddr>().map_err(|_| {
        RuntimeError::InvalidConfig(
            format!("runtime.ads_server.listen must be an IP address, got '{listen}'").into(),
        )
    })?;
    if address.is_unspecified() {
        return Err(RuntimeError::InvalidConfig(
            "runtime.ads_server.listen must not be 0.0.0.0 or ::".into(),
        ));
    }
    let classification = crate::ads::onboarding::classify_local_address(listen, None);
    if matches!(
        classification,
        crate::ads::diagnostics::LocalNetworkClassification::Public
            | crate::ads::diagnostics::LocalNetworkClassification::NatSuspect
    ) && !unsafe_allow_public_bind
    {
        return Err(RuntimeError::InvalidConfig(
            "runtime.ads_server.listen is public/NAT-suspect; set unsafe_allow_public_bind=true to start anyway".into(),
        ));
    }
    Ok(Some(listen.to_string()))
}

fn parse_ads_server_ams_net_id(
    configured: Option<&str>,
    listen: Option<&str>,
) -> Result<Option<trust_ads_core::AmsNetId>, RuntimeError> {
    let net_id = match configured.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => value.to_string(),
        None => {
            let Some(listen) = listen else {
                return Ok(None);
            };
            crate::ads::onboarding::derive_default_ams_net_id(listen).ok_or_else(|| {
                RuntimeError::InvalidConfig(
                    "runtime.ads_server.ams_net_id is required when listen is not IPv4".into(),
                )
            })?
        }
    };
    validate_ams_net_id(&net_id, "runtime.ads_server.ams_net_id")?;
    Ok(Some(trust_ads_core::AmsNetId::new(net_id)))
}

fn parse_glob_list(
    values: Option<Vec<String>>,
    field: &str,
) -> Result<Vec<SmolStr>, RuntimeError> {
    let values = values
        .unwrap_or_default()
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    for value in &values {
        Pattern::new(value).map_err(|err| {
            RuntimeError::InvalidConfig(format!("{field} invalid pattern '{value}': {err}").into())
        })?;
    }
    Ok(values.into_iter().map(SmolStr::new).collect())
}

fn validate_writable_subset(expose: &[SmolStr], writable: &[SmolStr]) -> Result<(), RuntimeError> {
    let expose_patterns = expose
        .iter()
        .map(|pattern| Pattern::new(pattern.as_str()).map(|compiled| (pattern, compiled)))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| RuntimeError::InvalidConfig(format!("{err}").into()))?;
    for writable_pattern in writable {
        let covered = expose_patterns.iter().any(|(raw, pattern)| {
            raw.as_str() == writable_pattern.as_str() || pattern.matches(writable_pattern.as_str())
        });
        if !covered {
            return Err(RuntimeError::InvalidConfig(
                format!(
                    "runtime.ads_server.writable entry '{}' is not covered by runtime.ads_server.expose",
                    writable_pattern
                )
                .into(),
            ));
        }
    }
    Ok(())
}

fn parse_ads_server_clients(
    bare: Option<Vec<String>>,
    structured: Option<Vec<AdsServerClientSection>>,
    allow_unpinned_clients: bool,
) -> Result<Vec<AdsServerClientConfig>, RuntimeError> {
    let mut clients = Vec::new();
    let bare = bare.unwrap_or_default();
    if !bare.is_empty() && !allow_unpinned_clients {
        return Err(RuntimeError::InvalidConfig(
            "runtime.ads_server.allow_clients requires allow_unpinned_clients=true".into(),
        ));
    }
    for value in bare {
        let net_id = value.trim();
        validate_ams_net_id(net_id, "runtime.ads_server.allow_clients")?;
        clients.push(AdsServerClientConfig {
            ams_net_id: trust_ads_core::AmsNetId::new(net_id),
            source: AdsServerSourcePin::Unpinned,
        });
    }

    for client in structured.unwrap_or_default() {
        let net_id = client.ams_net_id.trim().to_string();
        validate_ams_net_id(&net_id, "runtime.ads_server.clients[].ams_net_id")?;
        let source = parse_ads_server_client_source(client, allow_unpinned_clients)?;
        clients.push(AdsServerClientConfig {
            ams_net_id: trust_ads_core::AmsNetId::new(net_id),
            source,
        });
    }
    Ok(clients)
}

fn parse_ads_server_client_source(
    client: AdsServerClientSection,
    allow_unpinned_clients: bool,
) -> Result<AdsServerSourcePin, RuntimeError> {
    let source_ip = client
        .source_ip
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let source_cidr = client
        .source_cidr
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    match (source_ip, source_cidr) {
        (Some(_), Some(_)) => Err(RuntimeError::InvalidConfig(
            "runtime.ads_server.clients[] must set only one of source_ip or source_cidr".into(),
        )),
        (Some(ip), None) => {
            ip.parse::<std::net::IpAddr>().map_err(|_| {
                RuntimeError::InvalidConfig(
                    format!("runtime.ads_server.clients[].source_ip '{ip}' is invalid").into(),
                )
            })?;
            Ok(AdsServerSourcePin::Ip(SmolStr::new(ip)))
        }
        (None, Some(cidr)) => {
            validate_cidr(cidr, "runtime.ads_server.clients[].source_cidr")?;
            Ok(AdsServerSourcePin::Cidr(SmolStr::new(cidr)))
        }
        (None, None) if allow_unpinned_clients => Ok(AdsServerSourcePin::Unpinned),
        (None, None) => Err(RuntimeError::InvalidConfig(
            "runtime.ads_server.clients[] requires source_ip or source_cidr".into(),
        )),
    }
}

fn validate_ams_net_id(value: &str, field: &str) -> Result<(), RuntimeError> {
    let parts = value.split('.').collect::<Vec<_>>();
    if parts.len() != 6 || parts.iter().any(|part| part.parse::<u8>().is_err()) {
        return Err(RuntimeError::InvalidConfig(
            format!("{field} must be a six-byte AMS Net ID like 192.168.10.20.1.1").into(),
        ));
    }
    Ok(())
}

fn validate_cidr(value: &str, field: &str) -> Result<(), RuntimeError> {
    let Some((ip, prefix)) = value.split_once('/') else {
        return Err(RuntimeError::InvalidConfig(
            format!("{field} must be CIDR notation like 192.168.10.0/24").into(),
        ));
    };
    let ip = ip.parse::<std::net::IpAddr>().map_err(|_| {
        RuntimeError::InvalidConfig(format!("{field} has invalid IP address '{ip}'").into())
    })?;
    let prefix = prefix.parse::<u8>().map_err(|_| {
        RuntimeError::InvalidConfig(format!("{field} has invalid prefix '{prefix}'").into())
    })?;
    let max_prefix = if ip.is_ipv4() { 32 } else { 128 };
    if prefix > max_prefix {
        return Err(RuntimeError::InvalidConfig(
            format!("{field} prefix must be <= {max_prefix}").into(),
        ));
    }
    Ok(())
}

fn nonzero_usize(
    value: Option<usize>,
    default: usize,
    field: &str,
) -> Result<usize, RuntimeError> {
    let value = value.unwrap_or(default);
    if value == 0 {
        return Err(RuntimeError::InvalidConfig(
            format!("{field} must be >= 1").into(),
        ));
    }
    Ok(value)
}

fn nonzero_u64(value: Option<u64>, default: u64, field: &str) -> Result<u64, RuntimeError> {
    let value = value.unwrap_or(default);
    if value == 0 {
        return Err(RuntimeError::InvalidConfig(
            format!("{field} must be >= 1").into(),
        ));
    }
    Ok(value)
}
