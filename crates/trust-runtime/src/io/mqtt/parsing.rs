fn parse_broker_endpoint(text: &str) -> Result<BrokerEndpoint, RuntimeError> {
    let trimmed = text.trim();
    let stripped = trimmed
        .strip_prefix("tcp://")
        .or_else(|| trimmed.strip_prefix("mqtt://"))
        .or_else(|| trimmed.strip_prefix("ssl://"))
        .or_else(|| trimmed.strip_prefix("mqtts://"))
        .unwrap_or(trimmed);
    if let Some(rest) = stripped.strip_prefix('[') {
        let (host, port) = rest.split_once("]:").ok_or_else(|| {
            RuntimeError::InvalidConfig(
                format!("io.params.broker '{text}' must be host:port").into(),
            )
        })?;
        return Ok(BrokerEndpoint {
            host: SmolStr::new(host),
            port: parse_port(port, text)?,
        });
    }
    let (host, port) = stripped.rsplit_once(':').ok_or_else(|| {
        RuntimeError::InvalidConfig(format!("io.params.broker '{text}' must be host:port").into())
    })?;
    if host.trim().is_empty() {
        return Err(RuntimeError::InvalidConfig(
            format!("io.params.broker '{text}' has empty host").into(),
        ));
    }
    Ok(BrokerEndpoint {
        host: SmolStr::new(host.trim()),
        port: parse_port(port, text)?,
    })
}

fn parse_port(port: &str, full: &str) -> Result<u16, RuntimeError> {
    let port = port.trim().parse::<u16>().map_err(|err| {
        RuntimeError::InvalidConfig(
            format!("io.params.broker '{full}': invalid port: {err}").into(),
        )
    })?;
    if port == 0 {
        return Err(RuntimeError::InvalidConfig(
            format!("io.params.broker '{full}': port must be > 0").into(),
        ));
    }
    Ok(port)
}

fn is_local_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "::1"
}

fn broker_uses_tls_scheme(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.starts_with("ssl://") || trimmed.starts_with("mqtts://")
}
