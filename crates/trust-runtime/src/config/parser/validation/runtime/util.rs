fn parse_optional_path(
    field: &str,
    value: Option<String>,
) -> Result<Option<PathBuf>, RuntimeError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(RuntimeError::InvalidConfig(
            format!("{field} must not be empty").into(),
        ));
    }
    Ok(Some(PathBuf::from(trimmed)))
}

fn parse_nonempty_entry(value: String, field: &str) -> Result<String, RuntimeError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(RuntimeError::InvalidConfig(
            format!("{field} must not be empty").into(),
        ));
    }
    Ok(trimmed.to_string())
}

fn listen_is_remote(listen: &str) -> bool {
    if let Ok(addr) = listen.parse::<std::net::SocketAddr>() {
        return !addr.ip().is_loopback();
    }
    let host = listen
        .rsplit_once(':')
        .map_or(listen, |(host, _)| host)
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']');
    !host.eq_ignore_ascii_case("localhost") && host != "127.0.0.1" && host != "::1"
}

const MAX_RUNTIME_DURATION_MILLIS: u64 = (i64::MAX / 1_000_000) as u64;

fn parse_runtime_duration_millis(
    value: u64,
    minimum: u64,
    field: &str,
) -> Result<Duration, RuntimeError> {
    if value < minimum {
        return Err(RuntimeError::InvalidConfig(
            format!("{field} must be >= {minimum}").into(),
        ));
    }
    if value > MAX_RUNTIME_DURATION_MILLIS {
        return Err(RuntimeError::InvalidConfig(
            format!("{field} must be <= {MAX_RUNTIME_DURATION_MILLIS}").into(),
        ));
    }
    Ok(Duration::from_millis(value as i64))
}
